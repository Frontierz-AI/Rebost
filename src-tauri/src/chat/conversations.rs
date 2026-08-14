//! Conversations — append-oriented JSONL per thread (inspectable,
//! recoverable, easy to re-index) plus a small threads.json index.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;

use crate::paths::Paths;
use crate::types::SourcePassage;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadMeta {
    pub id: String,
    pub title: String,
    /// Shelf remembered for this conversation (None = No Shelf).
    #[serde(default)]
    pub shelf_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub message_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredMessage {
    pub id: String,
    /// "user" | "assistant"
    pub role: String,
    pub text: String,
    /// Reasoning trace, when the model produced one (shown collapsed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
    pub ts: String,
    #[serde(default)]
    pub shelf_id: Option<String>,
    #[serde(default)]
    pub sources: Vec<SourcePassage>,
    /// "done" | "stopped" | "error"
    #[serde(default = "default_status")]
    pub status: String,
}

fn default_status() -> String {
    "done".into()
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ThreadsFile {
    threads: Vec<ThreadMeta>,
}

pub struct Conversations;

impl Conversations {
    pub fn list(paths: &Paths) -> Vec<ThreadMeta> {
        let mut threads = read_threads(paths).threads;
        threads.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        threads
    }

    pub fn get(paths: &Paths, thread_id: &str) -> Option<ThreadMeta> {
        read_threads(paths)
            .threads
            .into_iter()
            .find(|t| t.id == thread_id)
    }

    pub fn create(paths: &Paths, shelf_id: Option<String>) -> Result<ThreadMeta> {
        let now = chrono::Utc::now().to_rfc3339();
        let meta = ThreadMeta {
            id: crate::ids::thread_id(),
            title: "New conversation".to_string(),
            shelf_id,
            created_at: now.clone(),
            updated_at: now,
            message_count: 0,
        };
        let mut file = read_threads(paths);
        file.threads.push(meta.clone());
        write_threads(paths, &file)?;
        Ok(meta)
    }

    pub fn set_shelf(paths: &Paths, thread_id: &str, shelf_id: Option<String>) -> Result<()> {
        let mut file = read_threads(paths);
        if let Some(thread) = file.threads.iter_mut().find(|t| t.id == thread_id) {
            thread.shelf_id = shelf_id;
        }
        write_threads(paths, &file)
    }

    pub fn delete(paths: &Paths, thread_id: &str) -> Result<()> {
        let mut file = read_threads(paths);
        file.threads.retain(|t| t.id != thread_id);
        write_threads(paths, &file)?;
        std::fs::remove_file(paths.thread_path(thread_id)).ok();
        Ok(())
    }

    /// Append a message and refresh thread metadata (title comes from the
    /// first user message).
    pub fn append(paths: &Paths, thread_id: &str, message: &StoredMessage) -> Result<()> {
        let path = paths.thread_path(thread_id);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("open {}", path.display()))?;
        writeln!(file, "{}", serde_json::to_string(message)?)?;

        let mut threads = read_threads(paths);
        if let Some(thread) = threads.threads.iter_mut().find(|t| t.id == thread_id) {
            thread.updated_at = chrono::Utc::now().to_rfc3339();
            thread.message_count += 1;
            if thread.title == "New conversation" && message.role == "user" {
                thread.title = title_from(&message.text);
            }
            if message.shelf_id.is_some() {
                thread.shelf_id = message.shelf_id.clone();
            }
        }
        write_threads(paths, &threads)?;
        Ok(())
    }

    pub fn messages(paths: &Paths, thread_id: &str) -> Vec<StoredMessage> {
        let Ok(text) = std::fs::read_to_string(paths.thread_path(thread_id)) else {
            return Vec::new();
        };
        text.lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    }

    /// Recent turns for the model — newest last, bounded in size.
    pub fn recent_history(
        paths: &Paths,
        thread_id: &str,
        max_messages: usize,
        max_chars: usize,
    ) -> Vec<StoredMessage> {
        let all = Self::messages(paths, thread_id);
        let mut selected: Vec<StoredMessage> = Vec::new();
        let mut used = 0usize;
        for message in all.into_iter().rev() {
            if message.status == "error" {
                continue;
            }
            let cost = message.text.chars().count().min(1600);
            if selected.len() >= max_messages || used + cost > max_chars {
                break;
            }
            used += cost;
            selected.push(message);
        }
        selected.reverse();
        selected
    }
}

fn title_from(text: &str) -> String {
    let clean = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut title: String = clean.chars().take(48).collect();
    if clean.chars().count() > 48 {
        // Cut at a word boundary.
        if let Some(pos) = title.rfind(' ') {
            title.truncate(pos);
        }
        title.push('…');
    }
    if title.is_empty() {
        "New conversation".to_string()
    } else {
        title
    }
}

fn read_threads(paths: &Paths) -> ThreadsFile {
    read_json(&paths.threads_index()).unwrap_or_default()
}

fn write_threads(paths: &Paths, file: &ThreadsFile) -> Result<()> {
    write_json(&paths.threads_index(), file)
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    crate::paths::atomic_write(path, serde_json::to_string_pretty(value)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threads_roundtrip_and_title() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        paths.ensure().unwrap();

        let thread = Conversations::create(&paths, None).unwrap();
        let message = StoredMessage {
            id: crate::ids::message_id(),
            role: "user".into(),
            text: "Explain EBITDA in simple terms for our monthly board meeting".into(),
            thinking: None,
            ts: chrono::Utc::now().to_rfc3339(),
            shelf_id: None,
            sources: Vec::new(),
            status: "done".into(),
        };
        Conversations::append(&paths, &thread.id, &message).unwrap();

        let listed = Conversations::list(&paths);
        assert_eq!(listed.len(), 1);
        assert!(listed[0].title.starts_with("Explain EBITDA"));
        assert_eq!(listed[0].message_count, 1);

        let messages = Conversations::messages(&paths, &thread.id);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].text, message.text);
    }
}
