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
                    if let Some(html_hits) = search_html(query) {
                        return ToolResult {
                            name: "web_search".into(),
                            ok: true,
                            content: html_hits,
                        };
                    }
                    ToolResult {
                        name: "web_search".into(),
                        ok: true,
                        content: format!(
                            "The web search ran, but there were no snippets for '{query}'."
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

fn search_html(query: &str) -> Option<String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()
        .ok()?;
    let url = format!(
        "https://html.duckduckgo.com/html/?q={}",
        urlencoding_encode(query)
    );
    let page = client.get(url).send().ok()?.text().ok()?;
    let snippet = regex::Regex::new(r#"(?s)class="result__snippet"[^>]*>(.*?)</a>"#).ok()?;
    let tags = regex::Regex::new(r"<[^>]+>").ok()?;
    let mut lines = Vec::new();
    for cap in snippet.captures_iter(&page).take(6) {
        let raw = cap.get(1)?.as_str();
        let clean = tags.replace_all(raw, " ");
        let clean = clean
            .replace("&amp;", "&")
            .replace("&quot;", "\"")
            .replace("&#x27;", "'")
            .replace("&nbsp;", " ");
        let clean = clean.split_whitespace().collect::<Vec<_>>().join(" ");
        if !clean.is_empty() {
            lines.push(format!("- {clean}"));
        }
    }
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
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
        return r#"Internet tools are OFF for this message.
You cannot search or open web pages right now.
If the user asks for live information, news, a website, or anything that needs the internet, tell them they can turn internet tools on in the Settings tab.
Do not say internet access is permanently disabled, and do not say it cannot be turned on."#
            .into();
    }
    r#"Internet tools are ON for this message.
You can call web_search and fetch_url. Use them when the question needs current or online information.
Do not claim you have no internet while these tools are on.
Do not invent tool results. Wait for the tool result, then answer the user."#
        .into()
}

pub fn ollama_tool_specs() -> serde_json::Value {
    serde_json::json!([
        {
            "type": "function",
            "function": {
                "name": "web_search",
                "description": "Search the web and return short result snippets.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Search query"
                        }
                    },
                    "required": ["query"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "fetch_url",
                "description": "Download the text of one http or https page.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "url": {
                            "type": "string",
                            "description": "Full http or https URL"
                        }
                    },
                    "required": ["url"]
                }
            }
        }
    ])
}
