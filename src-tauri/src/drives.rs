use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveInfo {
    pub letter: String,
    pub name: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub drive_type: String,
    pub media_type: String,
    pub is_ready: bool,
    pub is_removable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareInfo {
    pub cpu: String,
    pub ram_bytes: u64,
    pub gpu: String,
    pub vram_bytes: Option<u64>,
    pub gpu_acceleration_available: bool,
    pub note: String,
}

pub fn list_drives() -> Result<Vec<DriveInfo>, String> {
    #[cfg(windows)]
    {
        list_drives_windows()
    }
    #[cfg(not(windows))]
    {
        Ok(Vec::new())
    }
}

#[cfg(windows)]
fn list_drives_windows() -> Result<Vec<DriveInfo>, String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let script = r#"
$ErrorActionPreference = 'SilentlyContinue'
$media = @{}
Get-CimInstance Win32_DiskDrive | ForEach-Object {
  $disk = $_
  $mediaType = if ($disk.MediaType) { $disk.MediaType } else { 'Unknown' }
  $bus = if ($disk.InterfaceType) { $disk.InterfaceType } else { '' }
  Get-CimInstance -Query "ASSOCIATORS OF {Win32_DiskDrive.DeviceID='$($disk.DeviceID -replace '\\','\\')'} WHERE AssocClass=Win32_DiskDriveToDiskPartition" | ForEach-Object {
    Get-CimInstance -Query "ASSOCIATORS OF {Win32_DiskPartition.DeviceID='$($_.DeviceID)'} WHERE AssocClass=Win32_LogicalDiskToPartition" | ForEach-Object {
      $media[$_.DeviceID] = @{ MediaType = $mediaType; Bus = $bus }
    }
  }
}
Get-CimInstance Win32_LogicalDisk | ForEach-Object {
  $letter = $_.DeviceID
  $m = $media[$letter]
  $rawMedia = if ($m) { $m.MediaType } else { 'Unknown' }
  $bus = if ($m) { $m.Bus } else { '' }
  $normalized = 'Unknown'
  if ($rawMedia -match 'SSD|Solid' -or $bus -match 'NVMe|SCSI') { $normalized = 'SSD' }
  elseif ($rawMedia -match 'HDD|Fixed hard disk|Rotational') { $normalized = 'HDD' }
  elseif ($bus -eq 'USB') { $normalized = 'Removable' }
  [PSCustomObject]@{
    letter = $letter
    name = $_.VolumeName
    totalBytes = [uint64]($_.Size)
    freeBytes = [uint64]($_.FreeSpace)
    driveType = switch ([int]$_.DriveType) {
      2 { 'Removable' }
      3 { 'Fixed' }
      4 { 'Network' }
      5 { 'CD-ROM' }
      default { 'Unknown' }
    }
    mediaType = $normalized
    isReady = ($null -ne $_.Size)
    isRemovable = ([int]$_.DriveType -eq 2)
  }
} | ConvertTo-Json -Compress -Depth 4
"#;

    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Failed to query drives: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "Drive detection failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        return Ok(Vec::new());
    }

    // PowerShell may return a single object or an array.
    if stdout.starts_with('[') {
        serde_json::from_str(&stdout).map_err(|e| format!("Parse drives JSON: {e}"))
    } else {
        let one: DriveInfo =
            serde_json::from_str(&stdout).map_err(|e| format!("Parse drive JSON: {e}"))?;
        Ok(vec![one])
    }
}

pub fn detect_hardware() -> HardwareInfo {
    let mut sys = sysinfo::System::new();
    sys.refresh_cpu_all();
    sys.refresh_memory();

    let cpu = sys
        .cpus()
        .first()
        .map(|c| c.brand().trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Unknown CPU".into());

    let ram_bytes = sys.total_memory();

    let (gpu, vram_bytes, gpu_ok) = detect_gpu();

    let note = if gpu_ok {
        "GPU acceleration may be available depending on Ollama/CUDA support.".into()
    } else {
        "CPU inference available. GPU acceleration unavailable.".into()
    };

    HardwareInfo {
        cpu,
        ram_bytes,
        gpu,
        vram_bytes,
        gpu_acceleration_available: gpu_ok,
        note,
    }
}

fn detect_gpu() -> (String, Option<u64>, bool) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use std::process::Command;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let script = r#"
$ErrorActionPreference = 'SilentlyContinue'
$gpus = Get-CimInstance Win32_VideoController | Where-Object { $_.Name -and $_.Name -notmatch 'Microsoft Basic' }
if (-not $gpus) {
  [PSCustomObject]@{ name = 'None detected'; adapterRam = $null; accel = $false } | ConvertTo-Json -Compress
} else {
  $g = @($gpus)[0]
  $ram = if ($g.AdapterRAM -and $g.AdapterRAM -gt 0 -and $g.AdapterRAM -lt 4294967295) { [uint64]$g.AdapterRAM } else { $null }
  $accel = $g.Name -match 'NVIDIA|AMD|Radeon|GeForce|Quadro|Intel Arc'
  [PSCustomObject]@{ name = $g.Name; adapterRam = $ram; accel = [bool]$accel } | ConvertTo-Json -Compress
}
"#;

        if let Ok(output) = Command::new("powershell")
            .args(["-NoProfile", "-Command", script])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
        {
            if output.status.success() {
                #[derive(Deserialize)]
                struct GpuJson {
                    name: String,
                    #[serde(rename = "adapterRam")]
                    adapter_ram: Option<u64>,
                    accel: bool,
                }
                if let Ok(parsed) =
                    serde_json::from_str::<GpuJson>(&String::from_utf8_lossy(&output.stdout))
                {
                    return (parsed.name, parsed.adapter_ram, parsed.accel);
                }
            }
        }
    }

    ("Unknown".into(), None, false)
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    const TB: f64 = GB * 1024.0;
    let b = bytes as f64;
    if b >= TB {
        format!("{:.1} TB", b / TB)
    } else if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.0} MB", b / MB)
    } else if b >= KB {
        format!("{:.0} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}
