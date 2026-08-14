//! Short, stable identifiers.

use anyhow::bail;
use sha2::{Digest, Sha256};

fn short_hash(parts: &[&str], len: usize) -> String {
    let mut hasher = Sha256::new();
    for p in parts {
        hasher.update(p.as_bytes());
        hasher.update([0u8]);
    }
    let digest = hasher.finalize();
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    hex[..len].to_string()
}

/// Stable document id derived from its shelf, source and relative path —
/// content changes keep the same id (the Card records the content hash).
pub fn document_id(shelf_id: &str, source_id: &str, rel_path: &str) -> String {
    format!("d_{}", short_hash(&[shelf_id, source_id, rel_path], 10))
}

pub fn shelf_id(name: &str) -> String {
    format!(
        "s_{}",
        short_hash(&[name, &chrono::Utc::now().to_rfc3339()], 8)
    )
}

pub fn source_id(path: &str) -> String {
    format!("src_{}", short_hash(&[path], 8))
}

pub fn thread_id() -> String {
    format!("t_{}", uuid::Uuid::new_v4().simple())
}

pub fn message_id() -> String {
    format!("m_{}", uuid::Uuid::new_v4().simple())
}

/// True when `id` is safe to join onto an app-data path (no `..` or separators).
pub fn is_safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

pub fn require_safe_id(id: &str) -> anyhow::Result<()> {
    if is_safe_id(id) {
        Ok(())
    } else {
        bail!("invalid id")
    }
}

pub fn content_hash(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("sha256:{hex}")
}

/// Content hash of a file, streamed, without loading it whole into memory.
pub fn content_hash_file(path: &std::path::Path) -> std::io::Result<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 1 << 16];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let digest = hasher.finalize();
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    Ok(format!("sha256:{hex}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_ids_are_stable_and_distinct() {
        let a = document_id("s_1", "src_1", "a/b.pdf");
        let b = document_id("s_1", "src_1", "a/b.pdf");
        let c = document_id("s_1", "src_1", "a/c.pdf");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(a.starts_with("d_"));
    }

    #[test]
    fn content_hash_format() {
        let h = content_hash(b"hello");
        assert!(h.starts_with("sha256:"));
        assert_eq!(h.len(), "sha256:".len() + 64);
    }

    #[test]
    fn rejects_path_traversal_ids() {
        assert!(is_safe_id("s_ab12cd34"));
        assert!(is_safe_id("imported"));
        assert!(is_safe_id(&document_id("s_1", "src_1", "a/b.pdf")));
        assert!(!is_safe_id(""));
        assert!(!is_safe_id("../etc"));
        assert!(!is_safe_id("a/b"));
        assert!(!is_safe_id("a\\b"));
        assert!(!is_safe_id("x..y"));
    }
}
