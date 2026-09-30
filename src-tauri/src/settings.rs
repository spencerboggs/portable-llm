use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::state::AppSettings;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    pub preferred_drive: Option<String>,
    pub internet_enabled: bool,
    pub portable_mode: bool,
    pub theme: String,
    pub model_name: String,
    pub ollama_port: u16,
}

impl From<&AppSettings> for SettingsDto {
    fn from(s: &AppSettings) -> Self {
        Self {
            preferred_drive: s.preferred_drive.clone(),
            internet_enabled: s.internet_enabled,
            portable_mode: s.portable_mode,
            theme: s.theme.clone(),
            model_name: s.model_name.clone(),
            ollama_port: s.ollama_port,
        }
    }
}

pub fn settings_path(usb_root: &Path) -> std::path::PathBuf {
    usb_root.join("data").join("settings.json")
}

pub fn load_settings(usb_root: &Path) -> Option<AppSettings> {
    let path = settings_path(usb_root);
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn save_settings(usb_root: &Path, settings: &AppSettings) -> Result<(), String> {
    let path = settings_path(usb_root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let raw = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(path, raw).map_err(|e| e.to_string())
}
