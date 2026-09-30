mod chat;
mod commands;
mod conversations;
mod drives;
mod knowledge;
mod logging;
mod ollama;
mod settings;
mod staging;
mod state;
mod tools;

use state::{resolve_usb_root, AppState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let usb_root = resolve_usb_root();
    let app_state = AppState::new(usb_root.clone());
    let _ = app_state.ensure_dirs();
    logging::info(&usb_root, format!("PortableLLM starting. USB root: {}", state::path_str(&usb_root)));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::get_bootstrap,
            commands::refresh_drives,
            commands::get_hardware,
            commands::get_space_requirement,
            commands::get_runtime,
            commands::load_model,
            commands::remove_model,
            commands::get_settings,
            commands::update_settings,
            commands::list_conversations,
            commands::get_conversation,
            commands::create_conversation,
            commands::delete_conversation,
            commands::save_conversation,
            commands::send_chat,
            commands::stop_generation,
            commands::reload_knowledge,
            commands::list_knowledge,
            commands::read_knowledge_file,
            commands::write_knowledge_file,
            commands::delete_knowledge_file,
            commands::search_knowledge,
            commands::list_scripts,
            commands::save_script,
            commands::read_script,
            commands::open_scripts_folder,
            commands::run_tool,
            commands::get_usb_root,
        ])
        .run(tauri::generate_context!())
        .expect("error while running PortableLLM");
}
