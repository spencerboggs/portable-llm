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
        let raw = match cap.get(1) {
            Some(m) => m.as_str(),
            None => continue,
        };
        if raw.len() > 500 {
            continue;
        }
        let clean = tags.replace_all(raw, " ");
        let clean = clean
            .replace("&amp;", "&")
            .replace("&quot;", "\"")
            .replace("&#x27;", "'")
            .replace("&nbsp;", " ");
        let clean = clean.split_whitespace().collect::<Vec<_>>().join(" ");
        if clean.is_empty() || clean.len() > 240 || looks_like_code(&clean) {
            continue;
        }
        lines.push(format!("- {clean}"));
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
    if parsed.host_str().unwrap_or("").contains("yahoo.com") {
        if let Some(symbol) = symbol_from_path(parsed.path()) {
            return quote_result(&symbol);
        }
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
                    let visible = visible_text(&text);
                    ToolResult {
                        name: "fetch_url".into(),
                        ok: status.is_success(),
                        content: format!("HTTP {status}\n\n{visible}"),
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

pub fn asks_about_internet(text: &str) -> bool {
    let lower = text.to_lowercase();
    let mentions = lower.contains("internet")
        || lower.contains("online")
        || lower.contains("web access");
    let asks = lower.contains("test")
        || lower.contains("access")
        || lower.contains("connect")
        || lower.contains("can you")
        || lower.contains("check")
        || lower.contains("reach");
    mentions && asks
}

pub fn probe_internet() -> String {
    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .user_agent("PortableLLM/0.1")
        .build()
    {
        Ok(c) => c,
        Err(e) => return format!("Internet check failed. {e}"),
    };
    match client.get("https://example.com").send() {
        Ok(resp) => format!(
            "Internet check succeeded. HTTP {} from https://example.com.",
            resp.status()
        ),
        Err(e) => format!("Internet check failed. {e}"),
    }
}

pub fn stock_lookup_note(text: &str, internet_enabled: bool) -> Option<String> {
    let symbol = ticker_from_question(text)?;
    if !internet_enabled {
        return Some(
            "The user wants a live stock price. Internet tools are off. Tell them they can turn internet tools on in the Settings tab. Do not give sample code.".into(),
        );
    }
    let quote = fetch_quote(&symbol);
    Some(format!(
        "The user wants a live stock price. The app already looked it up.\n{quote}\nAnswer with this price in one or two plain sentences. Do not use emojis. Do not give sample code."
    ))
}

fn ticker_from_question(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    let wants = lower.contains("stock")
        || lower.contains("share")
        || lower.contains("ticker")
        || lower.contains("price");
    if !wants {
        return None;
    }
    let re = regex::Regex::new(r"\b([A-Z]{1,5})\b").ok()?;
    let skip = ["A", "I", "OK", "USD", "HTTP", "HTML", "API", "NYSE", "NASDAQ"];
    for cap in re.captures_iter(text) {
        let symbol = cap.get(1)?.as_str();
        if !skip.contains(&symbol) {
            return Some(symbol.to_string());
        }
    }
    let near_stock = regex::Regex::new(r"(?i)\b([a-z]{2,5})\s+stock\b").ok()?;
    let word_skip = ["the", "this", "that", "live", "share", "current", "stock"];
    for cap in near_stock.captures_iter(text) {
        let symbol = cap.get(1)?.as_str();
        if !word_skip.contains(&symbol.to_lowercase().as_str()) {
            return Some(symbol.to_uppercase());
        }
    }
    None
}

fn symbol_from_path(path: &str) -> Option<String> {
    let re = regex::Regex::new(r"/([A-Za-z.]{1,10})/?$").ok()?;
    let symbol = re.captures(path)?.get(1)?.as_str();
    if symbol.eq_ignore_ascii_case("chart") || symbol.eq_ignore_ascii_case("quote") {
        return None;
    }
    Some(symbol.to_uppercase())
}

fn quote_result(symbol: &str) -> ToolResult {
    ToolResult {
        name: "web_search".into(),
        ok: true,
        content: fetch_quote(symbol),
    }
}

fn fetch_quote(symbol: &str) -> String {
    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36")
        .build()
    {
        Ok(c) => c,
        Err(e) => return format!("Quote lookup failed for {symbol}. {e}"),
    };
    let url = format!(
        "https://query1.finance.yahoo.com/v8/finance/chart/{symbol}?interval=1d&range=1d"
    );
    let resp = match client.get(&url).send() {
        Ok(r) => r,
        Err(e) => return format!("Quote lookup failed for {symbol}. {e}"),
    };
    if !resp.status().is_success() {
        return format!("Quote lookup failed for {symbol}. HTTP {}", resp.status());
    }
    let json: serde_json::Value = match resp.json() {
        Ok(v) => v,
        Err(e) => return format!("Quote lookup failed for {symbol}. {e}"),
    };
    let meta = match json.pointer("/chart/result/0/meta") {
        Some(v) => v,
        None => return format!("Quote lookup failed for {symbol}. No price in the response."),
    };
    let price = match meta.get("regularMarketPrice").and_then(|v| v.as_f64()) {
        Some(p) => p,
        None => return format!("Quote lookup failed for {symbol}. No price in the response."),
    };
    let name = meta
        .get("longName")
        .or_else(|| meta.get("shortName"))
        .and_then(|v| v.as_str())
        .unwrap_or(symbol);
    let currency = meta.get("currency").and_then(|v| v.as_str()).unwrap_or("USD");
    let exchange = meta
        .get("fullExchangeName")
        .or_else(|| meta.get("exchangeName"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let change = meta
        .get("regularMarketChangePercent")
        .and_then(|v| v.as_f64());
    match change {
        Some(pct) => format!(
            "{symbol} ({name}) last price {price:.2} {currency} on {exchange}. Change {pct:.2}%."
        ),
        None => format!("{symbol} ({name}) last price {price:.2} {currency} on {exchange}."),
    }
}

fn looks_like_code(text: &str) -> bool {
    text.contains("function")
        || text.contains("_.")
        || text.contains('{')
        || text.contains("var ")
        || text.contains("<script")
}

fn visible_text(html: &str) -> String {
    let script = regex::Regex::new(r"(?is)<script[^>]*>.*?</script>").ok();
    let style = regex::Regex::new(r"(?is)<style[^>]*>.*?</style>").ok();
    let tags = regex::Regex::new(r"<[^>]+>").ok();
    let mut text = html.to_string();
    if let Some(script) = script {
        text = script.replace_all(&text, " ").into_owned();
    }
    if let Some(style) = style {
        text = style.replace_all(&text, " ").into_owned();
    }
    if let Some(tags) = tags {
        text = tags.replace_all(&text, " ").into_owned();
    }
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    text.chars().take(3_000).collect()
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
When a live number is already in the conversation, answer with that number.
Do not replace a price lookup with sample code unless the user asked for code.
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
