use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub preferred_drive: Option<String>,
    pub internet_enabled: bool,
    pub portable_mode: bool,
    pub theme: String,
    pub model_name: String,
    pub ollama_port: u16,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            preferred_drive: None,
            internet_enabled: false,
            portable_mode: true,
            theme: "dark".into(),
            model_name: "qwen3:4b".into(),
            ollama_port: 11435,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LoadStatus {
    NotLoaded,
    Loading,
    Running,
    Removing,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInfo {
    pub status: LoadStatus,
    pub target_drive: Option<String>,
    pub staging_path: Option<String>,
    pub model_name: String,
    pub progress_message: String,
    pub progress_percent: u8,
    pub error: Option<String>,
    pub ollama_base_url: Option<String>,
    pub using_gpu: Option<bool>,
}

impl Default for RuntimeInfo {
    fn default() -> Self {
        Self {
            status: LoadStatus::NotLoaded,
            target_drive: None,
            staging_path: None,
            model_name: "qwen3:4b".into(),
            progress_message: String::new(),
            progress_percent: 0,
            error: None,
            ollama_base_url: None,
            using_gpu: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub version: String,
    pub source_path: String,
    pub source_id: String,
    pub target_drive: String,
    pub staging_path: String,
    pub runtime_version: String,
    pub model_identifier: String,
    pub created_at: String,
    pub portable_mode: bool,
    pub created_paths: Vec<String>,
}

pub struct AppState {
    pub usb_root: PathBuf,
    pub settings: Mutex<AppSettings>,
    pub runtime: Mutex<RuntimeInfo>,
    pub ollama_child: Mutex<Option<std::process::Child>>,
    pub cancel_generation: Arc<AtomicBool>,
    pub knowledge_index: Mutex<Vec<crate::knowledge::KnowledgeChunk>>,
}

impl AppState {
    pub fn new(usb_root: PathBuf) -> Self {
        let settings = crate::settings::load_settings(&usb_root).unwrap_or_default();
        let mut runtime = RuntimeInfo::default();
        runtime.model_name = settings.model_name.clone();

        let knowledge_index = crate::knowledge::index_knowledge(&usb_root.join("knowledge"))
            .unwrap_or_default();

        Self {
            usb_root,
            settings: Mutex::new(settings),
            runtime: Mutex::new(runtime),
            ollama_child: Mutex::new(None),
            cancel_generation: Arc::new(AtomicBool::new(false)),
            knowledge_index: Mutex::new(knowledge_index),
        }
    }

    pub fn data_dir(&self) -> PathBuf {
        self.usb_root.join("data")
    }

    pub fn knowledge_dir(&self) -> PathBuf {
        self.usb_root.join("knowledge")
    }

    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self.data_dir().join("conversations"))?;
        std::fs::create_dir_all(self.data_dir().join("logs"))?;
        std::fs::create_dir_all(self.data_dir().join("scripts"))?;
        std::fs::create_dir_all(self.usb_root.join("config"))?;
        Ok(())
    }
}

pub fn resolve_usb_root() -> PathBuf {
    // PORTABLELLM_ROOT for local overrides.
    if let Ok(p) = std::env::var("PORTABLELLM_ROOT") {
        let path = PathBuf::from(p);
        if path.exists() {
            return path;
        }
    }

    // Dev: repo portable/ folder.
    if let Ok(cwd) = std::env::current_dir() {
        let candidate = cwd.join("portable");
        if candidate.exists() {
            return candidate;
        }
        // cargo run from src-tauri.
        let candidate = cwd.join("..").join("portable");
        if candidate.exists() {
            return candidate.canonicalize().unwrap_or(candidate);
        }
    }

    // Release: directory next to the exe.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let sibling = parent.join("portable");
            if sibling.exists() {
                return sibling;
            }
            return parent.to_path_buf();
        }
    }

    PathBuf::from(".")
}

pub fn path_str(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

pub fn set_cancel(flag: &AtomicBool, value: bool) {
    flag.store(value, Ordering::SeqCst);
}

pub fn is_cancelled(flag: &AtomicBool) -> bool {
    flag.load(Ordering::SeqCst)
}
