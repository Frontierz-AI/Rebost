//! Application data layout.
//!
//! ```text
//! <app-data>/
//! ├── shelves/
//! │   └── <shelf-id>/
//! │       ├── cards/          one YAML Card per document
//! │       ├── extracted/      extracted-text cache, one .md per document
//! │       └── documents.json  per-shelf document registry
//! ├── search/tantivy/         the one application-level index
//! ├── conversations/          threads.json + one JSONL per thread
//! ├── models/                 GGUF files
//! ├── engine/                 pinned llama.cpp build
//! ├── recipes.json            the saved-prompt library
//! ├── settings.json
//! └── logs/
//! ```

use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Paths {
    base: PathBuf,
    bundled_engine_archive: Option<PathBuf>,
}

impl Paths {
    pub fn new(base: impl Into<PathBuf>) -> Self {
        Self {
            base: base.into(),
            bundled_engine_archive: None,
        }
    }

    pub fn set_bundled_engine_archive(&mut self, path: Option<PathBuf>) {
        self.bundled_engine_archive = path;
    }

    pub fn bundled_engine_archive(&self) -> Option<&Path> {
        self.bundled_engine_archive.as_deref()
    }

    /// Ensure every directory of the layout exists (owner-only on Unix).
    pub fn ensure(&self) -> std::io::Result<()> {
        for dir in [
            self.base.clone(),
            self.shelves_dir(),
            self.search_dir(),
            self.conversations_dir(),
            self.models_dir(),
            self.engine_dir(),
            self.logs_dir(),
        ] {
            std::fs::create_dir_all(&dir)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
            }
        }
        Ok(())
    }

    pub fn base(&self) -> &Path {
        &self.base
    }

    pub fn shelves_dir(&self) -> PathBuf {
        self.base.join("shelves")
    }

    pub fn shelf_registry(&self) -> PathBuf {
        self.shelves_dir().join("registry.json")
    }

    pub fn shelf_data_dir(&self, shelf_id: &str) -> PathBuf {
        self.shelves_dir().join(shelf_id)
    }

    pub fn cards_dir(&self, shelf_id: &str) -> PathBuf {
        self.shelf_data_dir(shelf_id).join("cards")
    }

    pub fn card_path(&self, shelf_id: &str, doc_id: &str) -> PathBuf {
        self.cards_dir(shelf_id).join(format!("{doc_id}.yml"))
    }

    pub fn extracted_dir(&self, shelf_id: &str) -> PathBuf {
        self.shelf_data_dir(shelf_id).join("extracted")
    }

    pub fn extracted_path(&self, shelf_id: &str, doc_id: &str) -> PathBuf {
        self.extracted_dir(shelf_id).join(format!("{doc_id}.md"))
    }

    pub fn documents_registry(&self, shelf_id: &str) -> PathBuf {
        self.shelf_data_dir(shelf_id).join("documents.json")
    }

    pub fn search_dir(&self) -> PathBuf {
        self.base.join("search").join("tantivy")
    }

    pub fn conversations_dir(&self) -> PathBuf {
        self.base.join("conversations")
    }

    pub fn threads_index(&self) -> PathBuf {
        self.conversations_dir().join("threads.json")
    }

    pub fn thread_path(&self, thread_id: &str) -> PathBuf {
        self.conversations_dir().join(format!("{thread_id}.jsonl"))
    }

    pub fn models_dir(&self) -> PathBuf {
        self.base.join("models")
    }

    pub fn engine_dir(&self) -> PathBuf {
        self.base.join("engine")
    }

    pub fn recipes_path(&self) -> PathBuf {
        self.base.join("recipes.json")
    }

    pub fn settings_path(&self) -> PathBuf {
        self.base.join("settings.json")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.base.join("logs")
    }

    /// Default root where managed Shelf folders are created,
    /// e.g. `~/Documents/Rebost/Projects`.
    pub fn default_shelf_root() -> PathBuf {
        dirs::document_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
            .join("Rebost")
    }
}

/// Write `contents` to `path` via a sibling `.tmp` file, then rename.
pub fn atomic_write(path: &Path, contents: impl AsRef<[u8]>) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = {
        let mut name = path
            .file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("file"))
            .to_os_string();
        name.push(".tmp");
        match path.parent() {
            Some(parent) => parent.join(name),
            None => PathBuf::from(name),
        }
    };
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_replaces_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes.json");
        atomic_write(&path, b"one").unwrap();
        atomic_write(&path, b"two").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "two");
        assert!(!dir.path().join("notes.json.tmp").exists());
    }
}
