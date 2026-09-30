use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde::Deserialize;

use crate::logging;
use crate::state::{path_str, AppState};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn find_ollama_exe(runtime_dir: &Path) -> Result<PathBuf, String> {
    let candidates = [
        runtime_dir.join("ollama.exe"),
        runtime_dir.join("bin").join("ollama.exe"),
        runtime_dir.join("Ollama").join("ollama.exe"),
    ];
    for c in candidates {
        if c.exists() {
            return Ok(c);
        }
    }
    Err(format!(
        "Could not find ollama.exe under {}",
        path_str(runtime_dir)
    ))
}

pub fn start_ollama(
    state: &AppState,
    runtime_dir: &Path,
    models_dir: &Path,
    staging_dir: &Path,
) -> Result<String, String> {
    // Ensure we don't leave a previous child running.
    let _ = stop_ollama(state);

    let exe = find_ollama_exe(runtime_dir)?;
    let port = state.settings.lock().ollama_port;
    let host = format!("127.0.0.1:{port}");
    let base_url = format!("http://{host}");

    let home_dir = staging_dir.join("ollama-home");
    std::fs::create_dir_all(&home_dir).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(models_dir).map_err(|e| e.to_string())?;

    logging::info(
        &state.usb_root,
        format!(
            "Starting portable Ollama: {} models={} host={}",
            path_str(&exe),
            path_str(models_dir),
            host
        ),
    );

    let mut cmd = Command::new(&exe);
    cmd.arg("serve")
        .env("OLLAMA_HOST", &host)
        .env("OLLAMA_MODELS", path_str(models_dir))
        .env("HOME", path_str(&home_dir))
        .env("USERPROFILE", path_str(&home_dir))
        .current_dir(runtime_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let child = cmd
        .spawn()
        .map_err(|e| format!("Failed to start Ollama: {e}"))?;

    *state.ollama_child.lock() = Some(child);
    Ok(base_url)
}

pub fn stop_ollama(state: &AppState) -> Result<(), String> {
    let mut child_opt = state.ollama_child.lock();
    if let Some(mut child) = child_opt.take() {
        logging::info(&state.usb_root, "Stopping Ollama process");
        let _ = child.kill();
        let _ = child.wait();
    }

    // Also try to terminate any ollama serving our port (best-effort, scoped).
    let port = state.settings.lock().ollama_port;
    #[cfg(windows)]
    {
        let script = format!(
            r#"
$conns = Get-NetTCPConnection -LocalPort {port} -ErrorAction SilentlyContinue |
  Where-Object {{ $_.State -eq 'Listen' }}
foreach ($c in $conns) {{
  try {{ Stop-Process -Id $c.OwningProcess -Force -ErrorAction SilentlyContinue }} catch {{}}
}}
"#
        );
        let _ = Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .creation_flags(CREATE_NO_WINDOW)
            .status();
    }

    std::thread::sleep(Duration::from_millis(400));
    Ok(())
}

pub fn wait_for_api(base_url: &str, timeout_secs: u64) -> Result<(), String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .map_err(|e| e.to_string())?;

    let deadline = std::time::Instant::now() + Duration::from_secs(timeout_secs);
    let url = format!("{base_url}/api/tags");
    let mut last_err = "Ollama did not become ready".to_string();

    while std::time::Instant::now() < deadline {
        match client.get(&url).send() {
            Ok(resp) if resp.status().is_success() => return Ok(()),
            Ok(resp) => last_err = format!("Ollama returned HTTP {}", resp.status()),
            Err(e) => last_err = e.to_string(),
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    Err(format!(
        "Ollama failed to start or become ready within {timeout_secs}s. Details: {last_err}"
    ))
}

pub fn list_models(base_url: &str) -> Result<Vec<String>, String> {
    #[derive(Deserialize)]
    struct Tags {
        models: Option<Vec<Model>>,
    }
    #[derive(Deserialize)]
    struct Model {
        name: String,
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get(format!("{base_url}/api/tags"))
        .send()
        .map_err(|e| e.to_string())?;
    let tags: Tags = resp.json().map_err(|e| e.to_string())?;
    Ok(tags
        .models
        .unwrap_or_default()
        .into_iter()
        .map(|m| m.name)
        .collect())
}
