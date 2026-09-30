use std::path::{Path, PathBuf};
use std::time::SystemTime;

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
    if !req.enough_space {
        return Err(format!(
            "Insufficient storage. Required: {}. Available: {}. Please select another drive.",
            crate::drives::format_bytes(req.required_bytes),
            crate::drives::format_bytes(req.available_bytes)
        ));
    }

    let staging = staging_path_for(drive_letter);
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

    update_progress(state, "Copying runtime", 30);
    copy_tree(&state.usb_root.join("runtime"), &runtime_dst)?;
    created_paths.push(path_str(&runtime_dst));

    let settings = state.settings.lock().clone();
    if settings.portable_mode {
        update_progress(state, "Copying model", 55);
        copy_tree(&state.usb_root.join("model"), &model_dst)?;
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
    };
    let manifest_path = staging.join("manifest.json");
    std::fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;

    update_progress(state, "Starting Ollama", 80);
    let model_dir = if model_dst.exists() {
        model_dst
    } else {
        state.usb_root.join("model")
    };

    let base_url = crate::ollama::start_ollama(state, &runtime_dst, &model_dir, &staging)?;

    update_progress(state, "Verifying model", 92);
    crate::ollama::wait_for_api(&base_url, 60)?;
    let _ = crate::ollama::list_models(&base_url);

    let mut rt = state.runtime.lock();
    rt.status = LoadStatus::Running;
    rt.target_drive = Some(drive_letter.to_string());
    rt.staging_path = Some(path_str(&staging));
    rt.progress_message = "Ready.".into();
    rt.progress_percent = 100;
    rt.error = None;
    rt.ollama_base_url = Some(base_url);
    rt.model_name = settings.model_name.clone();
    rt.using_gpu = Some(crate::drives::detect_hardware().gpu_acceleration_available);

    logging::info(&state.usb_root, "Model loaded successfully");
    Ok(rt.clone())
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

fn copy_tree(src: &Path, dst: &Path) -> Result<(), String> {
    if !src.exists() {
        return Err(format!("Source missing: {}", path_str(src)));
    }
    if dst.exists() {
        std::fs::remove_dir_all(dst).map_err(|e| e.to_string())?;
    }
    std::fs::create_dir_all(dst.parent().unwrap_or(dst)).map_err(|e| e.to_string())?;
    copy_recursive(src, dst)
}

fn copy_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    if src.is_file() {
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::copy(src, dst).map_err(|e| e.to_string())?;
        return Ok(());
    }
    std::fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let target = dst.join(entry.file_name());
        copy_recursive(&entry.path(), &target)?;
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
