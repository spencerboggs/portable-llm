use serde::{Deserialize, Serialize};

use crate::logging;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResult {
    pub name: String,
    pub ok: bool,
    pub content: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallRequest {
    pub name: String,
    pub arguments: serde_json::Value,
}

pub fn execute_tool(state: &AppState, request: ToolCallRequest) -> ToolResult {
    let internet = state.settings.lock().internet_enabled;
    logging::info(
        &state.usb_root,
        format!("Tool call: {}", request.name),
    );

    match request.name.as_str() {
        "web_search" => {
            if !internet {
                return ToolResult {
                    name: request.name,
                    ok: false,
                    content: "Internet tools are disabled. Enable them in Settings.".into(),
                };
            }
            let query = request
                .arguments
                .get("query")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            web_search(&query)
        }
        "fetch_url" => {
            if !internet {
                return ToolResult {
                    name: request.name,
                    ok: false,
                    content: "Internet tools are disabled. Enable them in Settings.".into(),
                };
            }
            let url = request
                .arguments
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            fetch_url(&url)
        }
        other => ToolResult {
            name: other.to_string(),
            ok: false,
            content: format!("Unknown tool: {other}"),
        },
    }
}

fn web_search(query: &str) -> ToolResult {
    if query.trim().is_empty() {
        return ToolResult {
            name: "web_search".into(),
            ok: false,
            content: "Missing query".into(),
        };
    }

    // DuckDuckGo Instant Answer API (no API key).
    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("PortableLLM/0.1")
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return ToolResult {
                name: "web_search".into(),
                ok: false,
                content: e.to_string(),
            };
        }
    };

    let url = format!(
        "https://api.duckduckgo.com/?q={}&format=json&no_html=1&skip_disambig=1",
        urlencoding_encode(query)
    );

    match client.get(&url).send() {
        Ok(resp) => match resp.json::<serde_json::Value>() {
            Ok(json) => {
                let mut parts = Vec::new();
                if let Some(heading) = json.get("Heading").and_then(|v| v.as_str()) {
                    if !heading.is_empty() {
                        parts.push(format!("Heading: {heading}"));
                    }
                }
                if let Some(abs) = json.get("AbstractText").and_then(|v| v.as_str()) {
                    if !abs.is_empty() {
                        parts.push(format!("Summary: {abs}"));
                    }
                }
                if let Some(abs_url) = json.get("AbstractURL").and_then(|v| v.as_str()) {
                    if !abs_url.is_empty() {
                        parts.push(format!("URL: {abs_url}"));
                    }
                }
                if let Some(related) = json.get("RelatedTopics").and_then(|v| v.as_array()) {
                    for item in related.iter().take(5) {
                        if let Some(text) = item.get("Text").and_then(|v| v.as_str()) {
                            parts.push(format!("- {text}"));
                        }
                    }
                }
                if parts.is_empty() {
                    ToolResult {
                        name: "web_search".into(),
                        ok: true,
                        content: format!(
                            "No structured results for '{query}'. Try fetch_url with a specific page."
                        ),
                    }
                } else {
                    ToolResult {
                        name: "web_search".into(),
                        ok: true,
                        content: parts.join("\n"),
                    }
                }
            }
            Err(e) => ToolResult {
                name: "web_search".into(),
                ok: false,
                content: e.to_string(),
            },
        },
        Err(e) => ToolResult {
            name: "web_search".into(),
            ok: false,
            content: format!("Network error: {e}"),
        },
    }
}

fn fetch_url(raw_url: &str) -> ToolResult {
    let parsed = match url::Url::parse(raw_url) {
        Ok(u) => u,
        Err(e) => {
            return ToolResult {
                name: "fetch_url".into(),
                ok: false,
                content: format!("Invalid URL: {e}"),
            };
        }
    };
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return ToolResult {
            name: "fetch_url".into(),
            ok: false,
            content: "Only http/https URLs are allowed.".into(),
        };
    }

    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("PortableLLM/0.1")
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return ToolResult {
                name: "fetch_url".into(),
                ok: false,
                content: e.to_string(),
            };
        }
    };

    match client.get(parsed).send() {
        Ok(resp) => {
            let status = resp.status();
            match resp.text() {
                Ok(text) => {
                    let clipped: String = text.chars().take(12_000).collect();
                    ToolResult {
                        name: "fetch_url".into(),
                        ok: status.is_success(),
                        content: format!("HTTP {status}\n\n{clipped}"),
                    }
                }
                Err(e) => ToolResult {
                    name: "fetch_url".into(),
                    ok: false,
                    content: e.to_string(),
                },
            }
        }
        Err(e) => ToolResult {
            name: "fetch_url".into(),
            ok: false,
            content: format!("Network error: {e}"),
        },
    }
}

fn urlencoding_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn tool_definitions(internet_enabled: bool) -> String {
    if !internet_enabled {
        return "No internet tools are currently enabled.".into();
    }
    r#"Available tools (request with a JSON block when needed):
1) web_search - arguments: {"query":"..."}
2) fetch_url - arguments: {"url":"https://..."}

To request a tool, output ONLY:
```tool
{"name":"web_search","arguments":{"query":"..."}}
```
Do not invent tool results. Wait for the application to return them.
"#
    .into()
}
