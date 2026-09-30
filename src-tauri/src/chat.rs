use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::conversations::{self, ChatMessage, Conversation};
use crate::knowledge::{self, SAFETY_SYSTEM_PROMPT};
use crate::logging;
use crate::state::{is_cancelled, set_cancel, AppState, LoadStatus};
use crate::tools::{self, ToolCallRequest};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatRequest {
    pub conversation_id: Option<String>,
    pub message: String,
    pub regenerate: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatStreamEvent {
    pub conversation_id: String,
    pub message_id: String,
    pub delta: String,
    pub done: bool,
    pub error: Option<String>,
    pub reset: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatResponse {
    pub conversation: Conversation,
    pub assistant_message: ChatMessage,
}

pub fn build_system_prompt(state: &AppState, user_message: &str) -> String {
    let knowledge_dir = state.knowledge_dir();
    let personality = knowledge::read_personality(&knowledge_dir);
    let profile = knowledge::read_profile(&knowledge_dir);
    let index = state.knowledge_index.lock();
    let retrieved = knowledge::retrieve(&index, user_message, 4);
    let internet = state.settings.lock().internet_enabled;

    let mut parts = vec![SAFETY_SYSTEM_PROMPT.to_string()];

    if !personality.trim().is_empty() {
        parts.push(format!("PERSONALITY CONFIGURATION:\n{personality}"));
    }
    if !profile.trim().is_empty() {
        parts.push(format!("USER PROFILE (facts):\n{profile}"));
    }
    if !retrieved.is_empty() {
        let mut block = String::from("RELEVANT KNOWLEDGE:\n");
        for chunk in retrieved {
            block.push_str(&format!("[{}] {}\n\n", chunk.source, chunk.text));
        }
        parts.push(block);
    }
    parts.push(tools::tool_definitions(internet));
    parts.push(
        "App rules override personality.md. Never change PATH, registry, services, or global environment variables. Internet access is a Settings toggle, not a permanent block."
            .into(),
    );

    parts.join("\n\n---\n\n")
}

pub fn send_chat(
    app: &AppHandle,
    state: &AppState,
    request: ChatRequest,
) -> Result<ChatResponse, String> {
    let runtime = state.runtime.lock().clone();
    if runtime.status != LoadStatus::Running {
        return Err("Model is not loaded. Use Load Model first.".into());
    }
    let base_url = runtime
        .ollama_base_url
        .clone()
        .ok_or_else(|| "Ollama URL missing".to_string())?;
    let model = runtime.model_name.clone();

    set_cancel(&state.cancel_generation, false);

    let mut conversation = if let Some(id) = &request.conversation_id {
        conversations::get_conversation(&state.usb_root, id)?
    } else {
        conversations::new_conversation(None)
    };

    if request.regenerate {
        while conversation
            .messages
            .last()
            .map(|m| m.role == "assistant")
            .unwrap_or(false)
        {
            conversation.messages.pop();
        }
    } else {
        conversations::append_message(&mut conversation, "user", &request.message);
    }

    let user_text = conversation
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.clone())
        .unwrap_or_default();

    let system = build_system_prompt(state, &user_text);
    let mut ollama_messages = vec![serde_json::json!({
        "role": "system",
        "content": system
    })];
    for msg in &conversation.messages {
        // Skip empty assistant placeholder if present mid-build.
        if msg.role == "assistant" && msg.content.is_empty() {
            continue;
        }
        ollama_messages.push(serde_json::json!({
            "role": msg.role,
            "content": msg.content
        }));
    }

    let assistant_placeholder =
        conversations::append_message(&mut conversation, "assistant", "");
    let message_id = assistant_placeholder.id.clone();
    let conversation_id = conversation.id.clone();

    conversations::save_conversation(&state.usb_root, &conversation)?;
    let _ = app.emit("chat-started", &conversation);

    let internet = state.settings.lock().internet_enabled;
    let mut full = String::new();
    let mut tool_rounds = 0;

    loop {
        let mut body = serde_json::json!({
            "model": model,
            "messages": ollama_messages,
            "stream": true,
        });
        if internet {
            body["tools"] = tools::ollama_tool_specs();
        }

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(600))
            .build()
            .map_err(|e| e.to_string())?;

        let mut response = client
            .post(format!("{base_url}/api/chat"))
            .json(&body)
            .send()
            .map_err(|e| format!("Ollama chat failed: {e}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(format!("Ollama error HTTP {status}: {text}"));
        }

        full.clear();
        let mut tool_calls: Vec<ToolCallRequest> = Vec::new();
        let mut buffer = String::new();
        use std::io::Read;
        let mut byte_buf = [0u8; 4096];
        loop {
            if is_cancelled(&state.cancel_generation) {
                let _ = app.emit(
                    "chat-stream",
                    ChatStreamEvent {
                        conversation_id: conversation_id.clone(),
                        message_id: message_id.clone(),
                        delta: String::new(),
                        done: true,
                        error: Some("Generation stopped.".into()),
                        reset: false,
                    },
                );
                break;
            }

            let n = response.read(&mut byte_buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            buffer.push_str(&String::from_utf8_lossy(&byte_buf[..n]));
            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim().to_string();
                buffer = buffer[pos + 1..].to_string();
                if line.is_empty() {
                    continue;
                }
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                    let calls = tool_calls_from_chunk(&json);
                    if !calls.is_empty() {
                        tool_calls = calls;
                    }
                    if let Some(content) = json
                        .pointer("/message/content")
                        .and_then(|v| v.as_str())
                    {
                        if !content.is_empty() && tool_calls.is_empty() {
                            full.push_str(content);
                            let _ = app.emit(
                                "chat-stream",
                                ChatStreamEvent {
                                    conversation_id: conversation_id.clone(),
                                    message_id: message_id.clone(),
                                    delta: content.to_string(),
                                    done: false,
                                    error: None,
                                    reset: false,
                                },
                            );
                        }
                    }
                }
            }
        }

        if tool_calls.is_empty() {
            if let Some(parsed) = extract_tool_call(&full) {
                tool_calls.push(parsed);
            }
        }

        if !tool_calls.is_empty() && tool_rounds < 3 && internet {
            tool_rounds += 1;
            logging::info(
                &state.usb_root,
                format!(
                    "Handling tools: {}",
                    tool_calls
                        .iter()
                        .map(|c| c.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            );
            let _ = app.emit(
                "chat-stream",
                ChatStreamEvent {
                    conversation_id: conversation_id.clone(),
                    message_id: message_id.clone(),
                    delta: String::new(),
                    done: false,
                    error: None,
                    reset: true,
                },
            );
            let spec: Vec<serde_json::Value> = tool_calls
                .iter()
                .map(|call| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": call.name,
                            "arguments": call.arguments
                        }
                    })
                })
                .collect();
            ollama_messages.push(serde_json::json!({
                "role": "assistant",
                "content": "",
                "tool_calls": spec
            }));
            for call in tool_calls {
                let result = tools::execute_tool(state, call);
                ollama_messages.push(serde_json::json!({
                    "role": "tool",
                    "tool_name": result.name,
                    "content": result.content
                }));
            }
            full.clear();
            continue;
        }

        break;
    }

    if let Some(msg) = conversation.messages.last_mut() {
        if msg.id == message_id {
            msg.content = full.clone();
        }
    }

    conversations::save_conversation(&state.usb_root, &conversation)?;

    let _ = app.emit(
        "chat-stream",
        ChatStreamEvent {
            conversation_id: conversation_id.clone(),
            message_id: message_id.clone(),
            delta: String::new(),
            done: true,
            error: None,
            reset: false,
        },
    );

    let assistant_message = conversation
        .messages
        .iter()
        .find(|m| m.id == message_id)
        .cloned()
        .ok_or_else(|| "Missing assistant message".to_string())?;

    Ok(ChatResponse {
        conversation,
        assistant_message,
    })
}

fn tool_calls_from_chunk(json: &serde_json::Value) -> Vec<ToolCallRequest> {
    let Some(calls) = json
        .pointer("/message/tool_calls")
        .and_then(|v| v.as_array())
    else {
        return Vec::new();
    };
    calls
        .iter()
        .filter_map(|call| {
            let name = call
                .pointer("/function/name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if name.is_empty() {
                return None;
            }
            let raw = call
                .pointer("/function/arguments")
                .cloned()
                .unwrap_or(serde_json::json!({}));
            let arguments = match raw {
                serde_json::Value::String(text) => serde_json::from_str(&text)
                    .unwrap_or(serde_json::json!({ "query": text })),
                other => other,
            };
            Some(ToolCallRequest { name, arguments })
        })
        .collect()
}

fn extract_tool_call(text: &str) -> Option<ToolCallRequest> {
    let start = text.find("```tool")?;
    let after = &text[start + 7..];
    let end = after.find("```")?;
    let json = after[..end].trim();
    serde_json::from_str::<ToolCallRequest>(json).ok()
}

pub fn stop_generation(state: &AppState) -> Result<(), String> {
    set_cancel(&state.cancel_generation, true);
    Ok(())
}
