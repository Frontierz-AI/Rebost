//! Shelves — knowledge spaces with a managed folder plus linked external
//! folders, and the per-shelf document registry.

pub mod watcher;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::ids;
use crate::paths::Paths;
use crate::types::{DocStatus, DocumentMeta, PiiSummaryView, ShelfStats};

/// `shelf.yml` inside the managed folder — the Shelf's identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShelfConfig {
    pub schema: String,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub linked_folders: Vec<LinkedFolder>,
    #[serde(default)]
    pub settings: BTreeMap<String, serde_json::Value>,
}

impl ShelfConfig {
    pub const SCHEMA: &'static str = "rebost-shelf/v1";
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LinkedFolder {
    pub path: PathBuf,
}

/// In-memory Shelf.
#[derive(Debug, Clone, Serialize)]
pub struct Shelf {
    pub id: String,
    pub name: String,
    pub managed_path: PathBuf,
    pub linked_folders: Vec<LinkedFolder>,
}

impl Shelf {
    /// The stable source id for imported (managed-folder) files.
    pub const IMPORTED_SOURCE: &'static str = "imported";
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct RegistryFile {
    shelves: Vec<RegistryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RegistryEntry {
    id: String,
    name: String,
    managed_path: PathBuf,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct DocumentsFile {
    documents: Vec<DocumentMeta>,
}

/// All shelves plus their document registries.
pub struct Library {
    shelves: Vec<Shelf>,
    /// shelf_id → (document_id → meta)
    documents: BTreeMap<String, BTreeMap<String, DocumentMeta>>,
}

impl Library {
    pub fn load(paths: &Paths) -> Result<Self> {
        let registry: RegistryFile = read_json(&paths.shelf_registry()).unwrap_or_default();
        let mut shelves = Vec::new();
        let mut documents = BTreeMap::new();
        for entry in registry.shelves {
            // shelf.yml is the source of truth for name and linked folders.
            let config_path = entry.managed_path.join("shelf.yml");
            let shelf = match read_shelf_config(&config_path) {
                Ok(config) => Shelf {
                    id: config.id,
                    name: config.name,
                    managed_path: entry.managed_path.clone(),
                    linked_folders: config.linked_folders,
                },
                Err(error) => {
                    log::warn!(
                        "shelf.yml unreadable at {} ({error:#}); using registry entry",
                        config_path.display()
                    );
                    Shelf {
                        id: entry.id.clone(),
                        name: entry.name.clone(),
                        managed_path: entry.managed_path.clone(),
                        linked_folders: Vec::new(),
                    }
                }
            };
            let docs: DocumentsFile =
                read_json(&paths.documents_registry(&shelf.id)).unwrap_or_default();
            documents.insert(
                shelf.id.clone(),
                docs.documents
                    .into_iter()
                    .map(|d| (d.id.clone(), d))
                    .collect(),
            );
            shelves.push(shelf);
        }
        Ok(Self { shelves, documents })
    }

    pub fn shelves(&self) -> &[Shelf] {
        &self.shelves
    }

    pub fn shelf(&self, id: &str) -> Option<&Shelf> {
        self.shelves.iter().find(|s| s.id == id)
    }

    /// True when `path` is inside a managed Shelf folder or a linked source.
    pub fn allows_open_path(&self, path: &Path) -> bool {
        let Ok(canonical) = path.canonicalize() else {
            return false;
        };
        self.shelves.iter().any(|shelf| {
            path_is_under(&canonical, &shelf.managed_path)
                || shelf
                    .linked_folders
                    .iter()
                    .any(|linked| path_is_under(&canonical, &linked.path))
        })
    }

    fn shelf_mut(&mut self, id: &str) -> Option<&mut Shelf> {
        self.shelves.iter_mut().find(|s| s.id == id)
    }

    /// Create a Shelf: managed folder (collision-safe) + shelf.yml + registry.
    pub fn create_shelf(&mut self, paths: &Paths, name: &str, root: &Path) -> Result<Shelf> {
        let name = name.trim();
        if name.is_empty() {
            return Err(anyhow!("A Shelf needs a name."));
        }
        if self
            .shelves
            .iter()
            .any(|s| s.name.eq_ignore_ascii_case(name))
        {
            return Err(anyhow!("A Shelf called \"{name}\" already exists."));
        }
        let folder_name = sanitize_folder_name(name);
        std::fs::create_dir_all(root)?;
        let mut managed = root.join(&folder_name);
        let mut counter = 2;
        while managed.exists() {
            managed = root.join(format!("{folder_name} {counter}"));
            counter += 1;
        }
        std::fs::create_dir_all(&managed)?;

        let shelf = Shelf {
            id: ids::shelf_id(name),
            name: name.to_string(),
            managed_path: managed,
            linked_folders: Vec::new(),
        };
        write_shelf_config(&shelf)?;
        std::fs::create_dir_all(paths.cards_dir(&shelf.id))?;
        std::fs::create_dir_all(paths.extracted_dir(&shelf.id))?;
        self.documents.insert(shelf.id.clone(), BTreeMap::new());
        self.shelves.push(shelf.clone());
        self.save_registry(paths)?;
        Ok(shelf)
    }

    /// Remove a Shelf from Rebost. Derived data goes; the managed folder and
    /// every original file stay on disk.
    pub fn remove_shelf(&mut self, paths: &Paths, shelf_id: &str) -> Result<Vec<String>> {
        let position = self
            .shelves
            .iter()
            .position(|s| s.id == shelf_id)
            .ok_or_else(|| anyhow!("Shelf not found"))?;
        self.shelves.remove(position);
        let doc_ids: Vec<String> = self
            .documents
            .remove(shelf_id)
            .map(|docs| docs.keys().cloned().collect())
            .unwrap_or_default();
        std::fs::remove_dir_all(paths.shelf_data_dir(shelf_id)).ok();
        self.save_registry(paths)?;
        Ok(doc_ids)
    }

    pub fn add_linked_folder(&mut self, shelf_id: &str, folder: &Path) -> Result<Shelf> {
        if !folder.is_dir() {
            return Err(anyhow!("That folder can't be read."));
        }
        let shelf = self
            .shelf_mut(shelf_id)
            .ok_or_else(|| anyhow!("Shelf not found"))?;
        let canonical = folder
            .canonicalize()
            .unwrap_or_else(|_| folder.to_path_buf());
        if shelf.managed_path == canonical
            || shelf.linked_folders.iter().any(|l| l.path == canonical)
        {
            return Err(anyhow!("That folder is already part of this Shelf."));
        }
        shelf.linked_folders.push(LinkedFolder { path: canonical });
        let shelf = shelf.clone();
        write_shelf_config(&shelf)?;
        Ok(shelf)
    }

    /// Unlink a source folder: returns the document ids whose derived data
    /// must be removed. Original files are never touched.
    pub fn remove_linked_folder(&mut self, shelf_id: &str, source_id: &str) -> Result<Vec<String>> {
        let shelf = self
            .shelf_mut(shelf_id)
            .ok_or_else(|| anyhow!("Shelf not found"))?;
        shelf
            .linked_folders
            .retain(|l| ids::source_id(&l.path.to_string_lossy()) != source_id);
        let shelf_snapshot = shelf.clone();
        write_shelf_config(&shelf_snapshot)?;
        let docs = self.documents.entry(shelf_id.to_string()).or_default();
        let ids: Vec<String> = docs
            .values()
            .filter(|d| d.source_id == source_id)
            .map(|d| d.id.clone())
            .collect();
        for id in &ids {
            docs.remove(id);
        }
        Ok(ids)
    }

    pub fn documents(&self, shelf_id: &str) -> Vec<DocumentMeta> {
        self.documents
            .get(shelf_id)
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default()
    }

    pub fn document(&self, shelf_id: &str, doc_id: &str) -> Option<DocumentMeta> {
        self.documents.get(shelf_id)?.get(doc_id).cloned()
    }

    pub fn find_document_by_path(&self, shelf_id: &str, path: &Path) -> Option<DocumentMeta> {
        let path = path.to_string_lossy();
        self.documents
            .get(shelf_id)?
            .values()
            .find(|d| d.path == path)
            .cloned()
    }

    pub fn upsert_document(&mut self, meta: DocumentMeta) {
        self.documents
            .entry(meta.shelf_id.clone())
            .or_default()
            .insert(meta.id.clone(), meta);
    }

    pub fn remove_document(&mut self, shelf_id: &str, doc_id: &str) -> Option<DocumentMeta> {
        self.documents.get_mut(shelf_id)?.remove(doc_id)
    }

    pub fn save_documents(&self, paths: &Paths, shelf_id: &str) -> Result<()> {
        let documents = DocumentsFile {
            documents: self.documents(shelf_id),
        };
        write_json(&paths.documents_registry(shelf_id), &documents)
    }

    pub fn save_registry(&self, paths: &Paths) -> Result<()> {
        let registry = RegistryFile {
            shelves: self
                .shelves
                .iter()
                .map(|s| RegistryEntry {
                    id: s.id.clone(),
                    name: s.name.clone(),
                    managed_path: s.managed_path.clone(),
                })
                .collect(),
        };
        write_json(&paths.shelf_registry(), &registry)
    }

    /// Shelf overview numbers.
    pub fn stats(&self, shelf_id: &str) -> ShelfStats {
        let mut stats = ShelfStats::default();
        let Some(docs) = self.documents.get(shelf_id) else {
            return stats;
        };
        let mut categories: BTreeMap<String, u32> = BTreeMap::new();
        for doc in docs.values() {
            stats.files += 1;
            match doc.status {
                DocStatus::Ready => stats.searchable += 1,
                DocStatus::Reading => stats.reading += 1,
                DocStatus::Error => stats.errors += 1,
            }
            if doc.pii_total > 0 {
                stats.files_with_pii += 1;
            }
            for (category, count) in &doc.pii_categories {
                *categories.entry(category.clone()).or_insert(0) += count;
            }
        }
        let total = categories.values().sum();
        stats.pii = PiiSummaryView { total, categories };
        stats
    }
}

/// Copy dropped files/folders into a fresh, duplicate-safe import subfolder.
/// Returns the import folder and the list of copied files.
pub fn import_into_shelf(shelf: &Shelf, dropped: &[PathBuf]) -> Result<(PathBuf, Vec<PathBuf>)> {
    let imports_root = shelf.managed_path.join("Imports");
    std::fs::create_dir_all(&imports_root)?;
    let stamp = chrono::Local::now().format("%Y-%m-%d %H-%M").to_string();
    let mut import_dir = imports_root.join(&stamp);
    let mut counter = 2;
    while import_dir.exists() {
        import_dir = imports_root.join(format!("{stamp} ({counter})"));
        counter += 1;
    }
    std::fs::create_dir_all(&import_dir)?;

    let mut copied = Vec::new();
    for source in dropped {
        if source.is_dir() {
            let target = unique_target(&import_dir, source)?;
            copy_dir(source, &target, &mut copied)?;
        } else if source.is_file() {
            let target = unique_target(&import_dir, source)?;
            std::fs::copy(source, &target).with_context(|| format!("copy {}", source.display()))?;
            copied.push(target);
        }
    }
    Ok((import_dir, copied))
}

fn unique_target(dir: &Path, source: &Path) -> Result<PathBuf> {
    let name = source
        .file_name()
        .ok_or_else(|| anyhow!("invalid file name"))?;
    let mut target = dir.join(name);
    let stem = target
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .to_string();
    let ext = target
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_default();
    let mut counter = 2;
    while target.exists() {
        target = dir.join(format!("{stem} ({counter}){ext}"));
        counter += 1;
    }
    Ok(target)
}

fn copy_dir(source: &Path, target: &Path, copied: &mut Vec<PathBuf>) -> Result<()> {
    std::fs::create_dir_all(target)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        let dest = target.join(&name);
        if file_type.is_dir() {
            copy_dir(&path, &dest, copied)?;
        } else if file_type.is_file() {
            std::fs::copy(&path, &dest)?;
            copied.push(dest);
        }
    }
    Ok(())
}

/// Recursively list supported files under a folder (linked sources).
/// Symlinks are skipped so a linked tree cannot pull in files outside itself.
pub fn scan_folder(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            if name.to_string_lossy().starts_with('.') {
                continue;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                stack.push(path);
            } else if file_type.is_file() && crate::ingest::extract::is_supported_file(&path) {
                files.push(path);
            }
        }
        if files.len() > 20_000 {
            log::warn!(
                "linked folder scan stopped at 20k files under {}",
                root.display()
            );
            break;
        }
    }
    files.sort();
    files
}

fn path_is_under(path: &Path, root: &Path) -> bool {
    match root.canonicalize() {
        Ok(root) => path.starts_with(&root),
        Err(_) => path.starts_with(root),
    }
}

fn sanitize_folder_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => ' ',
            c => c,
        })
        .collect();
    let cleaned = cleaned.trim().trim_end_matches('.').to_string();
    if cleaned.is_empty() {
        "Shelf".to_string()
    } else {
        cleaned
    }
}

fn read_shelf_config(path: &Path) -> Result<ShelfConfig> {
    let text = std::fs::read_to_string(path)?;
    Ok(serde_yaml_ng::from_str(&text)?)
}

pub fn write_shelf_config(shelf: &Shelf) -> Result<()> {
    let config = ShelfConfig {
        schema: ShelfConfig::SCHEMA.to_string(),
        id: shelf.id.clone(),
        name: shelf.name.clone(),
        linked_folders: shelf.linked_folders.clone(),
        settings: BTreeMap::new(),
    };
    let text = serde_yaml_ng::to_string(&config)?;
    std::fs::write(shelf.managed_path.join("shelf.yml"), text)?;
    Ok(())
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
    fn create_shelf_is_collision_safe() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path().join("appdata"));
        paths.ensure().unwrap();
        let root = dir.path().join("Rebost");
        // A folder with the target name already exists.
        std::fs::create_dir_all(root.join("Finance")).unwrap();

        let mut library = Library::load(&paths).unwrap();
        let shelf = library.create_shelf(&paths, "Finance", &root).unwrap();
        assert_eq!(shelf.name, "Finance");
        assert!(shelf.managed_path.ends_with("Finance 2"));
        assert!(shelf.managed_path.join("shelf.yml").exists());

        // Same name again is rejected.
        assert!(library.create_shelf(&paths, "finance", &root).is_err());

        // Reload round-trips.
        let library2 = Library::load(&paths).unwrap();
        assert_eq!(library2.shelves().len(), 1);
        assert_eq!(library2.shelves()[0].name, "Finance");
    }

    #[test]
    fn import_creates_stamped_folder_and_copies() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path().join("appdata"));
        paths.ensure().unwrap();
        let root = dir.path().join("Rebost");
        let mut library = Library::load(&paths).unwrap();
        let shelf = library.create_shelf(&paths, "Docs", &root).unwrap();

        let source = dir.path().join("invoice.txt");
        std::fs::write(&source, "Invoice INV-1 total 100 EUR").unwrap();
        let (import_dir, copied) =
            import_into_shelf(&shelf, std::slice::from_ref(&source)).unwrap();
        assert!(import_dir.starts_with(shelf.managed_path.join("Imports")));
        assert_eq!(copied.len(), 1);
        // Original untouched.
        assert!(source.exists());

        // Second import of the same file gets its own folder — no overwrite.
        let (import_dir2, copied2) =
            import_into_shelf(&shelf, std::slice::from_ref(&source)).unwrap();
        assert_ne!(import_dir, import_dir2);
        assert_eq!(copied2.len(), 1);
    }

    #[test]
    fn linked_folder_roundtrip_and_open_allowlist() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path().join("appdata"));
        paths.ensure().unwrap();
        let root = dir.path().join("Rebost");
        let mut library = Library::load(&paths).unwrap();
        let shelf = library.create_shelf(&paths, "Docs", &root).unwrap();
        let linked = dir.path().join("incoming");
        std::fs::create_dir_all(&linked).unwrap();
        std::fs::write(linked.join("note.md"), "hi").unwrap();
        library.add_linked_folder(&shelf.id, &linked).unwrap();
        let reloaded = Library::load(&paths).unwrap();
        assert_eq!(reloaded.shelf(&shelf.id).unwrap().linked_folders.len(), 1);
        assert!(reloaded.allows_open_path(&linked.join("note.md")));
        let outside = dir.path().join("secret.txt");
        std::fs::write(&outside, "nope").unwrap();
        assert!(!reloaded.allows_open_path(&outside));
        let ids = {
            let mut library = Library::load(&paths).unwrap();
            let source = crate::ids::source_id(&linked.canonicalize().unwrap().to_string_lossy());
            library.remove_linked_folder(&shelf.id, &source).unwrap()
        };
        let _ = ids;
        let after = Library::load(&paths).unwrap();
        assert!(after.shelf(&shelf.id).unwrap().linked_folders.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_escape_is_not_opened_or_scanned() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path().join("appdata"));
        paths.ensure().unwrap();
        let root = dir.path().join("Rebost");
        let mut library = Library::load(&paths).unwrap();
        let shelf = library.create_shelf(&paths, "Docs", &root).unwrap();
        let linked = dir.path().join("incoming");
        std::fs::create_dir_all(&linked).unwrap();
        std::fs::write(linked.join("note.md"), "inside").unwrap();
        let secret = dir.path().join("secret.md");
        std::fs::write(&secret, "outside").unwrap();
        std::os::unix::fs::symlink(&secret, linked.join("escape.md")).unwrap();
        library.add_linked_folder(&shelf.id, &linked).unwrap();
        let reloaded = Library::load(&paths).unwrap();
        assert!(reloaded.allows_open_path(&linked.join("note.md")));
        assert!(!reloaded.allows_open_path(&secret));
        assert!(
            !reloaded.allows_open_path(&linked.join("escape.md")),
            "symlink to a file outside the Shelf must not pass the open allowlist"
        );
        let traversal = format!("{}/../../secret.md", linked.display());
        assert!(!reloaded.allows_open_path(std::path::Path::new(&traversal)));
        let scanned = crate::shelf::scan_folder(&linked);
        assert_eq!(
            scanned.len(),
            1,
            "symlink must not be ingested: {scanned:?}"
        );
        assert_eq!(scanned[0].file_name().unwrap(), "note.md");
    }

    #[test]
    fn remove_shelf_drops_registry_keeps_managed_folder() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path().join("appdata"));
        paths.ensure().unwrap();
        let root = dir.path().join("Rebost");
        let mut library = Library::load(&paths).unwrap();
        let shelf = library.create_shelf(&paths, "Docs", &root).unwrap();
        let managed = shelf.managed_path.clone();
        std::fs::write(managed.join("keep.md"), "original").unwrap();
        library.remove_shelf(&paths, &shelf.id).unwrap();
        assert!(Library::load(&paths).unwrap().shelf(&shelf.id).is_none());
        assert!(managed.join("keep.md").exists());
    }
}
