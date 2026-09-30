use std::path::{Path, PathBuf};

use regex::Regex;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeChunk {
    pub id: String,
    pub source: String,
    pub title: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeFileInfo {
    pub relative_path: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrievedChunk {
    pub source: String,
    pub score: f32,
    pub text: String,
}

pub fn index_knowledge(knowledge_dir: &Path) -> Result<Vec<KnowledgeChunk>, String> {
    if !knowledge_dir.exists() {
        return Ok(Vec::new());
    }

    let mut chunks = Vec::new();
    for entry in WalkDir::new(knowledge_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if !matches!(ext.as_str(), "md" | "txt" | "markdown") {
            continue;
        }

        let rel = path
            .strip_prefix(knowledge_dir)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");

        // profile.md and personality.md are loaded separately into the system prompt.
        if rel.eq_ignore_ascii_case("personality.md") || rel.eq_ignore_ascii_case("profile.md") {
            continue;
        }

        let content = std::fs::read_to_string(path).unwrap_or_default();
        if content.trim().is_empty() {
            continue;
        }

        let title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled")
            .to_string();

        for (i, chunk) in chunk_text(&content, 900, 120).into_iter().enumerate() {
            chunks.push(KnowledgeChunk {
                id: format!("{rel}#{i}"),
                source: rel.clone(),
                title: title.clone(),
                text: chunk,
            });
        }
    }

    Ok(chunks)
}

pub fn chunk_text(text: &str, size: usize, overlap: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let end = (start + size).min(chars.len());
        let slice: String = chars[start..end].iter().collect();
        out.push(slice.trim().to_string());
        if end == chars.len() {
            break;
        }
        start = end.saturating_sub(overlap);
        if start == 0 {
            start = end;
        }
    }
    out.into_iter().filter(|s| !s.is_empty()).collect()
}

pub fn retrieve(chunks: &[KnowledgeChunk], query: &str, limit: usize) -> Vec<RetrievedChunk> {
    let terms: Vec<String> = query
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() > 2)
        .map(|s| s.to_string())
        .collect();

    if terms.is_empty() {
        return Vec::new();
    }

    let mut scored: Vec<(f32, &KnowledgeChunk)> = chunks
        .iter()
        .map(|chunk| {
            let lower = chunk.text.to_lowercase();
            let mut score = 0.0_f32;
            for term in &terms {
                if lower.contains(term) {
                    score += 1.0;
                    // Bonus for repeated mentions.
                    score += lower.matches(term.as_str()).count() as f32 * 0.1;
                }
            }
            if chunk.source.contains("programming") && terms.iter().any(|t| is_code_term(t)) {
                score += 0.5;
            }
            (score, chunk)
        })
        .filter(|(s, _)| *s > 0.0)
        .collect();

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(limit);

    scored
        .into_iter()
        .map(|(score, chunk)| RetrievedChunk {
            source: chunk.source.clone(),
            score,
            text: chunk.text.clone(),
        })
        .collect()
}

fn is_code_term(term: &str) -> bool {
    matches!(
        term,
        "code"
            | "function"
            | "class"
            | "rust"
            | "python"
            | "typescript"
            | "javascript"
            | "csharp"
            | "cpp"
            | "bug"
            | "error"
            | "compile"
            | "api"
            | "debug"
    )
}

pub fn read_profile(knowledge_dir: &Path) -> String {
    read_optional(knowledge_dir.join("profile.md"))
}

pub fn read_personality(knowledge_dir: &Path) -> String {
    read_optional(knowledge_dir.join("personality.md"))
}

pub fn write_knowledge_file(knowledge_dir: &Path, relative: &str, content: &str) -> Result<(), String> {
    validate_relative(relative)?;
    let path = knowledge_dir.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, content).map_err(|e| e.to_string())
}

pub fn delete_knowledge_file(knowledge_dir: &Path, relative: &str) -> Result<(), String> {
    validate_relative(relative)?;
    let path = knowledge_dir.join(relative);
    if path.exists() {
        std::fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn list_knowledge_files(knowledge_dir: &Path) -> Result<Vec<KnowledgeFileInfo>, String> {
    if !knowledge_dir.exists() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    for entry in WalkDir::new(knowledge_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let rel = path
            .strip_prefix(knowledge_dir)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
        files.push(KnowledgeFileInfo {
            relative_path: rel,
            size_bytes: meta.len(),
        });
    }
    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(files)
}

pub fn read_knowledge_file(knowledge_dir: &Path, relative: &str) -> Result<String, String> {
    validate_relative(relative)?;
    let path = knowledge_dir.join(relative);
    std::fs::read_to_string(path).map_err(|e| e.to_string())
}

fn validate_relative(relative: &str) -> Result<(), String> {
    if relative.is_empty()
        || relative.contains("..")
        || Path::new(relative).is_absolute()
        || relative.contains(':')
    {
        return Err("Invalid knowledge path".into());
    }
    let re = Regex::new(r"^[\w\-./ ]+$").unwrap();
    if !re.is_match(relative) {
        return Err("Invalid knowledge path characters".into());
    }
    Ok(())
}

fn read_optional(path: PathBuf) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

pub const SAFETY_SYSTEM_PROMPT: &str = r#"You are PortableLLM, a local assistant running from a USB-based app on the current computer.

Rules enforced by the application (follow them):
- Do not change the host system configuration.
- Do not change PATH, environment variables, registry keys, services, drivers, firewall rules, or Windows settings.
- Do not install global software or services, and do not modify an existing Ollama installation.
- Do not delete or overwrite files outside a workspace the user clearly selected.
- Do not claim you ran system changes you did not run.
- You cannot execute shell commands in this version. Scripts are for the user to save and run themselves.
- If PATH, env, registry, or other system changes would help, explain the manual steps. Do not pretend you already did them.
- Give complete working code when asked for programming help.
- Stay concise and accurate.
- Separate facts from guesses.
"#;
