use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::logging;
use crate::state::{path_str, AppState, LoadStatus, Manifest, RuntimeInfo};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceRequirement {
    pub required_bytes: u64,
    pub runtime_bytes: u64,
    pub model_bytes: u64,
    pub other_bytes: u64,
    pub available_bytes: u64,
    pub enough_space: bool,
    pub missing_runtime: bool,
    pub missing_model: bool,
}

pub fn staging_path_for(drive_letter: &str) -> PathBuf {
    let letter = drive_letter.trim_end_matches('\\').trim_end_matches('/');
    PathBuf::from(format!(r"{letter}\PortableLLM"))
}

pub fn dir_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .sum()
}

pub fn calculate_requirements(state: &AppState, drive_letter: &str) -> SpaceRequirement {
    let runtime_src = state.usb_root.join("runtime");
    let model_src = state.usb_root.join("model");
    let runtime_bytes = dir_size(&runtime_src);
    let model_bytes = dir_size(&model_src);
    let other_bytes = 64 * 1024 * 1024; // headroom for data/temp/manifest
    let settings = state.settings.lock();
    let required = if settings.portable_mode {
        runtime_bytes + model_bytes + other_bytes
    } else {
        runtime_bytes + other_bytes
    };

    let available = crate::drives::list_drives()
        .ok()
        .and_then(|drives| {
            drives
                .into_iter()
                .find(|d| d.letter.eq_ignore_ascii_case(drive_letter))
                .map(|d| d.free_bytes)
        })
        .unwrap_or(0);

    SpaceRequirement {
        required_bytes: required,
        runtime_bytes,
        model_bytes,
        other_bytes,
        available_bytes: available,
        enough_space: available >= required && required > 0,
        missing_runtime: runtime_bytes == 0,
        missing_model: model_bytes == 0,
    }
}

fn update_progress(state: &AppState, message: &str, percent: u8) {
    let mut rt = state.runtime.lock();
    rt.progress_message = message.to_string();
    rt.progress_percent = percent;
    rt.status = LoadStatus::Loading;
}

pub fn load_model(state: &AppState, drive_letter: &str) -> Result<RuntimeInfo, String> {
    logging::info(
        &state.usb_root,
        format!("Load model requested for drive {drive_letter}"),
    );

    {
        let rt = state.runtime.lock();
        if rt.status == LoadStatus::Running || rt.status == LoadStatus::Loading {
            return Err("PortableLLM is already loaded or loading.".into());
        }
    }

    let req = calculate_requirements(state, drive_letter);
    if req.missing_runtime {
        return Err(
            "Ollama runtime is missing from the USB (portable/runtime). Place a portable Ollama build there."
                .into(),
        );
    }
    if req.missing_model {
        return Err(
            "Model files are missing from the USB (portable/model). Place an Ollama models directory there."
                .into(),
        );
    }

    let staging = staging_path_for(drive_letter);
    let fingerprint = setup_fingerprint(state);
    let existing = read_manifest(&staging.join("manifest.json"));
    let same_setup = existing
        .as_ref()
        .map(|m| !fingerprint.is_empty() && m.setup_fingerprint == fingerprint)
        .unwrap_or(false);

    if same_setup {
        logging::info(
            &state.usb_root,
            format!("Reusing matching install at {}", path_str(&staging)),
        );
        update_progress(
            state,
            "Existing install matches this USB. Starting Ollama",
            80,
        );
        return activate_staged(state, drive_letter, &staging);
    }

    if !req.enough_space {
        return Err(format!(
            "Insufficient storage. Required: {}. Available: {}. Please select another drive.",
            crate::drives::format_bytes(req.required_bytes),
            crate::drives::format_bytes(req.available_bytes)
        ));
    }

    if existing.is_some() {
        logging::info(
            &state.usb_root,
            format!(
                "Replacing host install that does not match this USB at {}",
                path_str(&staging)
            ),
        );
    }

    update_progress(state, "Checking storage", 5);

    if staging.exists() {
        // Reuse only if it belongs to us; otherwise refuse to clobber.
        let manifest_path = staging.join("manifest.json");
        if !manifest_path.exists() {
            return Err(format!(
                "Directory already exists and is not a PortableLLM install: {}",
                path_str(&staging)
            ));
        }
    }

    std::fs::create_dir_all(&staging).map_err(|e| format!("Failed to create staging dir: {e}"))?;
    update_progress(state, "Creating runtime directories", 15);

    let runtime_dst = staging.join("runtime");
    let model_dst = staging.join("model");
    let data_dst = staging.join("data");
    let temp_dst = staging.join("temp");
    std::fs::create_dir_all(&data_dst).ok();
    std::fs::create_dir_all(&temp_dst).ok();
    std::fs::create_dir_all(staging.join("knowledge-cache")).ok();

    let mut created_paths = vec![
        path_str(&staging),
        path_str(&runtime_dst),
        path_str(&model_dst),
        path_str(&data_dst),
        path_str(&temp_dst),
    ];

    copy_tree_with_progress(
        state,
        &state.usb_root.join("runtime"),
        &runtime_dst,
        "Copying runtime",
        15,
        35,
    )?;
    created_paths.push(path_str(&runtime_dst));

    let settings = state.settings.lock().clone();
    if settings.portable_mode {
        copy_tree_with_progress(
            state,
            &state.usb_root.join("model"),
            &model_dst,
            "Copying model",
            35,
            80,
        )?;
        created_paths.push(path_str(&model_dst));
    } else {
        // USB-required mode: point to USB model directory via junction/symlink if possible,
        // otherwise keep model on USB path and configure Ollama accordingly.
        update_progress(state, "Linking model from USB", 55);
        let usb_model = state.usb_root.join("model");
        if let Err(e) = create_dir_link(&usb_model, &model_dst) {
            logging::warn(
                &state.usb_root,
                format!("Could not link model dir ({e}); will use USB path directly"),
            );
        }
    }

    update_progress(state, "Writing manifest", 70);
    let source_id = volume_id_for_path(&state.usb_root).unwrap_or_else(|| "unknown".into());
    let manifest = Manifest {
        version: "0.1.0".into(),
        source_path: path_str(&state.usb_root),
        source_id,
        target_drive: drive_letter.to_string(),
        staging_path: path_str(&staging),
        runtime_version: "portable-ollama".into(),
        model_identifier: settings.model_name.clone(),
        created_at: DateTime::<Utc>::from(SystemTime::now()).to_rfc3339(),
        portable_mode: settings.portable_mode,
        created_paths,
        setup_fingerprint: fingerprint,
    };
    let manifest_path = staging.join("manifest.json");
    std::fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;

    update_progress(state, "Starting Ollama", 80);
    activate_staged(state, drive_letter, &staging)
}

fn activate_staged(
    state: &AppState,
    drive_letter: &str,
    staging: &Path,
) -> Result<RuntimeInfo, String> {
    let settings = state.settings.lock().clone();
    let runtime_dst = staging.join("runtime");
    let model_dst = staging.join("model");
    let model_dir = if model_dst.exists() {
        model_dst
    } else {
        state.usb_root.join("model")
    };

    let base_url = crate::ollama::start_ollama(state, &runtime_dst, &model_dir, staging)?;

    update_progress(state, "Verifying model", 92);
    crate::ollama::wait_for_api(&base_url, 60)?;
    let _ = crate::ollama::list_models(&base_url);

    let mut rt = state.runtime.lock();
    rt.status = LoadStatus::Running;
    rt.target_drive = Some(drive_letter.to_string());
    rt.staging_path = Some(path_str(staging));
    rt.progress_message = "Ready.".into();
    rt.progress_percent = 100;
    rt.error = None;
    rt.resume_note = None;
    rt.ollama_base_url = Some(base_url);
    rt.model_name = settings.model_name.clone();
    rt.using_gpu = Some(crate::drives::detect_hardware().gpu_acceleration_available);

    logging::info(&state.usb_root, "Model loaded successfully");
    Ok(rt.clone())
}

pub fn resume_existing(state: &AppState) -> Result<RuntimeInfo, String> {
    {
        let status = state.runtime.lock().status.clone();
        if status == LoadStatus::Running || status == LoadStatus::Loading {
            return Ok(state.runtime.lock().clone());
        }
    }

    update_progress(state, "Looking for an existing PortableLLM folder", 15);
    let fingerprint = setup_fingerprint(state);
    let drives = crate::drives::list_drives().unwrap_or_default();
    let mut matches: Vec<(String, PathBuf)> = Vec::new();
    let mut mismatches: Vec<String> = Vec::new();

    for drive in drives {
        if drive.drive_type != "Fixed" || drive.is_removable || !drive.is_ready {
            continue;
        }
        let staging = staging_path_for(&drive.letter);
        let Some(manifest) = read_manifest(&staging.join("manifest.json")) else {
            continue;
        };
        if !fingerprint.is_empty() && manifest.setup_fingerprint == fingerprint {
            matches.push((drive.letter, staging));
        } else {
            mismatches.push(drive.letter);
        }
    }

    if matches.is_empty() {
        let mut rt = state.runtime.lock();
        if rt.status == LoadStatus::Loading {
            rt.status = LoadStatus::NotLoaded;
            rt.progress_message.clear();
            rt.progress_percent = 0;
        }
        rt.resume_note = if mismatches.is_empty() {
            None
        } else {
            Some(format!(
                "Found PortableLLM on {} but it does not match this USB. Load Model will replace it.",
                mismatches.join(", ")
            ))
        };
        return Ok(rt.clone());
    }

    let preferred = state.settings.lock().preferred_drive.clone();
    let (letter, staging) = matches
        .iter()
        .find(|(letter, _)| preferred.as_ref() == Some(letter))
        .or_else(|| matches.first())
        .map(|(letter, staging)| (letter.clone(), staging.clone()))
        .ok_or_else(|| "No matching install".to_string())?;

    logging::info(
        &state.usb_root,
        format!("Resuming matching install on {letter}"),
    );
    update_progress(
        state,
        "Existing install matches this USB. Starting Ollama",
        80,
    );
    activate_staged(state, &letter, &staging)
}

pub fn setup_fingerprint(state: &AppState) -> String {
    let settings = state.settings.lock().clone();
    let runtime_sig = file_signature(&state.usb_root.join("runtime"));
    let model_sig = if settings.portable_mode {
        file_signature(&state.usb_root.join("model"))
    } else {
        "usb-model".into()
    };
    format!(
        "{}|{}|{}|{}",
        settings.model_name, settings.portable_mode, runtime_sig, model_sig
    )
}

fn file_signature(root: &Path) -> String {
    if !root.exists() {
        return "missing".into();
    }
    let mut rows = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let rel = entry
            .path()
            .strip_prefix(root)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .replace('\\', "/");
        let len = entry.metadata().map(|m| m.len()).unwrap_or(0);
        rows.push(format!("{rel}:{len}"));
    }
    rows.sort();
    let mut hash: u64 = 1469598103934665603;
    for row in &rows {
        for byte in row.as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(1099511628211);
        }
        hash ^= 0xff;
    }
    format!("{hash:x}:{}", rows.len())
}

fn read_manifest(path: &Path) -> Option<Manifest> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn remove_model(state: &AppState) -> Result<RuntimeInfo, String> {
    logging::info(&state.usb_root, "Remove model requested");

    let staging = {
        let mut rt = state.runtime.lock();
        if rt.status == LoadStatus::NotLoaded && rt.staging_path.is_none() {
            return Ok(rt.clone());
        }
        rt.status = LoadStatus::Removing;
        rt.progress_message = "Stopping Ollama...".into();
        rt.progress_percent = 20;
        rt.staging_path.clone()
    };

    crate::ollama::stop_ollama(state)?;

    if let Some(path) = staging {
        let path = PathBuf::from(&path);
        {
            let mut rt = state.runtime.lock();
            rt.progress_message = "Removing host installation...".into();
            rt.progress_percent = 60;
        }
        if path.exists() {
            // Safety: only delete directories named PortableLLM with our manifest.
            let manifest = path.join("manifest.json");
            if !manifest.exists() {
                return Err(format!(
                    "Refusing to delete {}: no PortableLLM manifest found.",
                    path_str(&path)
                ));
            }
            remove_dir_retry(&path, 8)?;
        }
    }

    let mut rt = state.runtime.lock();
    *rt = RuntimeInfo {
        model_name: state.settings.lock().model_name.clone(),
        ..RuntimeInfo::default()
    };
    logging::info(&state.usb_root, "Host installation removed");
    Ok(rt.clone())
}

struct CopyProgress<'a> {
    state: &'a AppState,
    label: &'a str,
    total: u64,
    copied: u64,
    percent_start: u8,
    percent_end: u8,
    last_update: Instant,
}

impl CopyProgress<'_> {
    fn bump(&mut self, n: u64) {
        self.copied = self.copied.saturating_add(n);
        let done = self.total == 0 || self.copied >= self.total;
        if !done && self.last_update.elapsed() < Duration::from_millis(250) {
            return;
        }
        self.last_update = Instant::now();
        let span = self.percent_end.saturating_sub(self.percent_start) as f64;
        let ratio = if self.total == 0 {
            1.0
        } else {
            (self.copied as f64 / self.total as f64).min(1.0)
        };
        let percent = (self.percent_start as f64 + span * ratio).round() as u8;
        let message = format!(
            "{} {} / {}",
            self.label,
            crate::drives::format_bytes(self.copied.min(self.total.max(self.copied))),
            crate::drives::format_bytes(self.total)
        );
        update_progress(self.state, &message, percent.min(self.percent_end));
    }
}

fn copy_tree_with_progress(
    state: &AppState,
    src: &Path,
    dst: &Path,
    label: &str,
    percent_start: u8,
    percent_end: u8,
) -> Result<(), String> {
    if !src.exists() {
        return Err(format!("Source missing: {}", path_str(src)));
    }
    if dst.exists() {
        std::fs::remove_dir_all(dst).map_err(|e| e.to_string())?;
    }
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let total = dir_size(src);
    update_progress(
        state,
        &format!("{label} {}", crate::drives::format_bytes(total)),
        percent_start,
    );
    let mut progress = CopyProgress {
        state,
        label,
        total,
        copied: 0,
        percent_start,
        percent_end,
        last_update: Instant::now() - Duration::from_secs(1),
    };
    copy_recursive(src, dst, &mut progress)?;
    progress.bump(0);
    Ok(())
}

fn copy_recursive(src: &Path, dst: &Path, progress: &mut CopyProgress<'_>) -> Result<(), String> {
    if src.is_file() {
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        copy_file_chunked(src, dst, progress)?;
        return Ok(());
    }
    std::fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let target = dst.join(entry.file_name());
        copy_recursive(&entry.path(), &target, progress)?;
    }
    Ok(())
}

fn copy_file_chunked(src: &Path, dst: &Path, progress: &mut CopyProgress<'_>) -> Result<(), String> {
    let mut input = std::fs::File::open(src).map_err(|e| format!("{}: {e}", path_str(src)))?;
    let mut output = std::fs::File::create(dst).map_err(|e| format!("{}: {e}", path_str(dst)))?;
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let n = input.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        output.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        progress.bump(n as u64);
    }
    Ok(())
}

fn create_dir_link(src: &Path, dst: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use std::process::Command;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        if dst.exists() {
            let _ = std::fs::remove_dir_all(dst);
            let _ = std::fs::remove_file(dst);
        }
        let status = Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                &path_str(dst),
                &path_str(src),
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err("mklink failed".into())
        }
    }
    #[cfg(not(windows))]
    {
        std::os::unix::fs::symlink(src, dst).map_err(|e| e.to_string())
    }
}

fn remove_dir_retry(path: &Path, attempts: u32) -> Result<(), String> {
    let mut last = String::new();
    for i in 0..attempts {
        match std::fs::remove_dir_all(path) {
            Ok(()) => return Ok(()),
            Err(e) => {
                last = e.to_string();
                std::thread::sleep(std::time::Duration::from_millis(250 * (i as u64 + 1)));
            }
        }
    }
    Err(format!(
        "Could not remove {}. A process may still be locking files. Details: {last}",
        path_str(path)
    ))
}

fn volume_id_for_path(path: &Path) -> Option<String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use std::process::Command;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let root = path
            .components()
            .next()
            .map(|c| c.as_os_str().to_string_lossy().to_string())?;
        let output = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!(
                    "(Get-Volume -DriveLetter '{}').UniqueId",
                    root.trim_end_matches(':').trim_end_matches('\\')
                ),
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()?;
        let id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if id.is_empty() {
            None
        } else {
            Some(id)
        }
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        None
    }
}
