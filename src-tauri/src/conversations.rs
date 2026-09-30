use std::path::Path;

use chrono::Local;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub messages: Vec<ChatMessage>,
}

fn conversations_dir(usb_root: &Path) -> std::path::PathBuf {
    usb_root.join("data").join("conversations")
}

pub fn list_conversations(usb_root: &Path) -> Result<Vec<Conversation>, String> {
    let dir = conversations_dir(usb_root);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        if let Ok(conv) = serde_json::from_str::<Conversation>(&raw) {
            out.push(conv);
        }
    }
    out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(out)
}

pub fn get_conversation(usb_root: &Path, id: &str) -> Result<Conversation, String> {
    let path = conversations_dir(usb_root).join(format!("{id}.json"));
    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

pub fn save_conversation(usb_root: &Path, conversation: &Conversation) -> Result<(), String> {
    let dir = conversations_dir(usb_root);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.json", conversation.id));
    let raw = serde_json::to_string_pretty(conversation).map_err(|e| e.to_string())?;
    std::fs::write(path, raw).map_err(|e| e.to_string())
}

pub fn delete_conversation(usb_root: &Path, id: &str) -> Result<(), String> {
    let path = conversations_dir(usb_root).join(format!("{id}.json"));
    if path.exists() {
        std::fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn new_conversation(title: Option<String>) -> Conversation {
    let now = Local::now().to_rfc3339();
    Conversation {
        id: Uuid::new_v4().to_string(),
        title: title.unwrap_or_else(|| "New chat".into()),
        created_at: now.clone(),
        updated_at: now,
        messages: Vec::new(),
    }
}

pub fn append_message(conversation: &mut Conversation, role: &str, content: &str) -> ChatMessage {
    let msg = ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: role.to_string(),
        content: content.to_string(),
        created_at: Local::now().to_rfc3339(),
    };
    conversation.messages.push(msg.clone());
    conversation.updated_at = Local::now().to_rfc3339();
    if conversation.title == "New chat" && role == "user" {
        let clipped: String = content.chars().take(48).collect();
        conversation.title = if content.chars().count() > 48 {
            format!("{clipped}...")
        } else {
            clipped
        };
    }
    msg
}
