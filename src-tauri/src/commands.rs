use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::conversations::{self, Conversation};
use crate::drives::{self, DriveInfo, HardwareInfo};
use crate::knowledge;
use crate::logging;
use crate::settings::{self, SettingsDto};
use crate::staging::{self, SpaceRequirement};
use crate::state::{path_str, AppState, RuntimeInfo};
use crate::tools::{self, ToolCallRequest, ToolResult};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppBootstrap {
    pub usb_root: String,
    pub runtime: RuntimeInfo,
    pub settings: SettingsDto,
    pub hardware: HardwareInfo,
    pub drives: Vec<DriveInfo>,
    pub model_label: String,
    pub required_space_hint: String,
}

#[tauri::command]
pub fn get_bootstrap(state: State<'_, AppState>) -> Result<AppBootstrap, String> {
    let settings = state.settings.lock().clone();
    let runtime = state.runtime.lock().clone();
    let drives = drives::list_drives().unwrap_or_default();
    let hardware = drives::detect_hardware();
    let model_bytes = staging::dir_size(&state.usb_root.join("model"));
    let runtime_bytes = staging::dir_size(&state.usb_root.join("runtime"));
    let required = if settings.portable_mode {
        model_bytes + runtime_bytes + 64 * 1024 * 1024
    } else {
        runtime_bytes + 64 * 1024 * 1024
    };

    Ok(AppBootstrap {
        usb_root: path_str(&state.usb_root),
        runtime,
        settings: SettingsDto::from(&settings),
        hardware,
        drives,
        model_label: settings.model_name,
        required_space_hint: drives::format_bytes(required),
    })
}

#[tauri::command]
pub fn refresh_drives() -> Result<Vec<DriveInfo>, String> {
    drives::list_drives()
}

#[tauri::command]
pub fn get_hardware() -> HardwareInfo {
    drives::detect_hardware()
}

#[tauri::command]
pub fn get_space_requirement(
    state: State<'_, AppState>,
    drive_letter: String,
) -> SpaceRequirement {
    staging::calculate_requirements(&state, &drive_letter)
}

#[tauri::command]
pub fn get_runtime(state: State<'_, AppState>) -> RuntimeInfo {
    state.runtime.lock().clone()
}

#[tauri::command]
pub async fn load_model(
    app: AppHandle,
    drive_letter: String,
) -> Result<RuntimeInfo, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        match staging::load_model(state.inner(), &drive_letter) {
            Ok(rt) => Ok(rt),
            Err(e) => {
                logging::error(&state.usb_root, &e);
                let mut rt = state.runtime.lock();
                rt.status = crate::state::LoadStatus::Error;
                rt.error = Some(e.clone());
                rt.progress_message = e.clone();
                Err(e)
            }
        }
    })
    .await
    .map_err(|e| format!("Load task failed: {e}"))?
}

#[tauri::command]
pub async fn remove_model(app: AppHandle) -> Result<RuntimeInfo, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        staging::remove_model(state.inner()).map_err(|e| {
            logging::error(&state.usb_root, &e);
            e
        })
    })
    .await
    .map_err(|e| format!("Remove task failed: {e}"))?
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> SettingsDto {
    SettingsDto::from(&state.settings.lock().clone())
}

#[tauri::command]
pub fn update_settings(
    state: State<'_, AppState>,
    patch: SettingsDto,
) -> Result<SettingsDto, String> {
    let mut settings = state.settings.lock();
    settings.preferred_drive = patch.preferred_drive;
    settings.internet_enabled = patch.internet_enabled;
    settings.portable_mode = patch.portable_mode;
    settings.theme = patch.theme;
    settings.model_name = patch.model_name;
    settings.ollama_port = patch.ollama_port;
    settings::save_settings(&state.usb_root, &settings)?;
    {
        let mut rt = state.runtime.lock();
        rt.model_name = settings.model_name.clone();
    }
    Ok(SettingsDto::from(&*settings))
}

#[tauri::command]
pub fn list_conversations(state: State<'_, AppState>) -> Result<Vec<Conversation>, String> {
    conversations::list_conversations(&state.usb_root)
}

#[tauri::command]
pub fn get_conversation(state: State<'_, AppState>, id: String) -> Result<Conversation, String> {
    conversations::get_conversation(&state.usb_root, &id)
}

#[tauri::command]
pub fn create_conversation(
    state: State<'_, AppState>,
    title: Option<String>,
) -> Result<Conversation, String> {
    let conv = conversations::new_conversation(title);
    conversations::save_conversation(&state.usb_root, &conv)?;
    Ok(conv)
}

#[tauri::command]
pub fn delete_conversation(state: State<'_, AppState>, id: String) -> Result<(), String> {
    conversations::delete_conversation(&state.usb_root, &id)
}

#[tauri::command]
pub fn save_conversation(
    state: State<'_, AppState>,
    conversation: Conversation,
) -> Result<(), String> {
    conversations::save_conversation(&state.usb_root, &conversation)
}

#[tauri::command]
pub async fn send_chat(
    app: AppHandle,
    request: crate::chat::ChatRequest,
) -> Result<crate::chat::ChatResponse, String> {
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app2.state::<AppState>();
        crate::chat::send_chat(&app2, state.inner(), request)
    })
    .await
    .map_err(|e| format!("Chat task failed: {e}"))?
}

#[tauri::command]
pub fn stop_generation(state: State<'_, AppState>) -> Result<(), String> {
    crate::chat::stop_generation(&state)
}

#[tauri::command]
pub fn reload_knowledge(state: State<'_, AppState>) -> Result<usize, String> {
    let index = knowledge::index_knowledge(&state.knowledge_dir())?;
    let count = index.len();
    *state.knowledge_index.lock() = index;
    logging::info(&state.usb_root, format!("Reloaded knowledge: {count} chunks"));
    Ok(count)
}

#[tauri::command]
pub fn list_knowledge(state: State<'_, AppState>) -> Result<Vec<knowledge::KnowledgeFileInfo>, String> {
    knowledge::list_knowledge_files(&state.knowledge_dir())
}

#[tauri::command]
pub fn read_knowledge_file(state: State<'_, AppState>, relative_path: String) -> Result<String, String> {
    knowledge::read_knowledge_file(&state.knowledge_dir(), &relative_path)
}

#[tauri::command]
pub fn write_knowledge_file(
    state: State<'_, AppState>,
    relative_path: String,
    content: String,
) -> Result<(), String> {
    knowledge::write_knowledge_file(&state.knowledge_dir(), &relative_path, &content)?;
    let index = knowledge::index_knowledge(&state.knowledge_dir())?;
    *state.knowledge_index.lock() = index;
    Ok(())
}

#[tauri::command]
pub fn delete_knowledge_file(state: State<'_, AppState>, relative_path: String) -> Result<(), String> {
    knowledge::delete_knowledge_file(&state.knowledge_dir(), &relative_path)?;
    let index = knowledge::index_knowledge(&state.knowledge_dir())?;
    *state.knowledge_index.lock() = index;
    Ok(())
}

#[tauri::command]
pub fn search_knowledge(
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<knowledge::RetrievedChunk>, String> {
    let index = state.knowledge_index.lock();
    Ok(knowledge::retrieve(&index, &query, 8))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptInfo {
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
}

#[tauri::command]
pub fn list_scripts(state: State<'_, AppState>) -> Result<Vec<ScriptInfo>, String> {
    let dir = state.data_dir().join("scripts");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_file() {
            let meta = entry.metadata().map_err(|e| e.to_string())?;
            out.push(ScriptInfo {
                name: entry.file_name().to_string_lossy().to_string(),
                path: path_str(&path),
                size_bytes: meta.len(),
            });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

#[tauri::command]
pub fn save_script(
    state: State<'_, AppState>,
    name: String,
    content: String,
) -> Result<ScriptInfo, String> {
    let safe = name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
    if safe.trim().is_empty() {
        return Err("Script name required".into());
    }
    let dir = state.data_dir().join("scripts");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(&safe);
    std::fs::write(&path, content).map_err(|e| e.to_string())?;
    let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
    Ok(ScriptInfo {
        name: safe,
        path: path_str(&path),
        size_bytes: meta.len(),
    })
}

#[tauri::command]
pub fn read_script(state: State<'_, AppState>, name: String) -> Result<String, String> {
    let path = state.data_dir().join("scripts").join(name);
    std::fs::read_to_string(path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_scripts_folder(state: State<'_, AppState>) -> Result<(), String> {
    let dir = state.data_dir().join("scripts");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    open::that(path_str(&dir)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn run_tool(state: State<'_, AppState>, request: ToolCallRequest) -> ToolResult {
    tools::execute_tool(&state, request)
}

#[tauri::command]
pub fn get_usb_root(state: State<'_, AppState>) -> String {
    path_str(&state.usb_root)
}

// Open a folder in Explorer when the opener plugin is not used.
mod open {
    use std::process::Command;
    pub fn that(path: String) -> std::io::Result<()> {
        #[cfg(windows)]
        {
            Command::new("explorer").arg(path).spawn()?;
            Ok(())
        }
        #[cfg(not(windows))]
        {
            Command::new("xdg-open").arg(path).spawn()?;
            Ok(())
        }
    }
}
