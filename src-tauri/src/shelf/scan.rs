//! Which files a Shelf will read: skip lists, supported names, and the
//! per-Shelf file limit so a linked home folder cannot stall the app.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Files a Shelf reads unless its owner picks another limit. New files past
/// the limit are left unread until some are removed or the limit goes up.
pub const DEFAULT_FILE_LIMIT: usize = 1_000;

/// The limits a Shelf can pick. The top one is bounded by the document
/// registry, which is rewritten after every file (about 7 MB at 10,000).
pub const FILE_LIMIT_CHOICES: [usize; 4] = [1_000, 2_500, 5_000, 10_000];

/// Bound how many directory entries a walk inspects, so a huge tree with
/// few documents cannot hang Add folder. Not a product-facing quota.
const MIN_WALK_ENTRIES: usize = 30_000;

/// Walk budget per file the walk may meet (new or already on the Shelf), so
/// a Shelf with a higher limit can still reach every file it may read.
const WALK_ENTRIES_PER_FILE: usize = 30;

fn walk_budget(max_new: usize, already: usize) -> usize {
    max_new
        .saturating_add(already)
        .saturating_mul(WALK_ENTRIES_PER_FILE)
        .max(MIN_WALK_ENTRIES)
}

/// Folder names that are almost never documents: package installs, language
/// caches, OS trash. Matched case-insensitively on a single path component.
///
/// Drawn from GitHub's Node/Python gitignores, Raycast File Search (skips
/// `node_modules`, hidden files, and tool caches), Spotlight's `.noindex`
/// suffix, and the macOS/Windows global gitignores. `Library`, `vendor`,
/// `dist`, `build`, and `docs` are not listed — people use those names for
/// real folders.
const SKIP_DIR_NAMES: &[&str] = &[
    "node_modules",
    "bower_components",
    "jspm_packages",
    "web_modules",
    "elm-stuff",
    "pnpm-store",
    "__pycache__",
    "__pypackages__",
    "site-packages",
    "htmlcov",
    "develop-eggs",
    "venv",
    "virtualenv",
    "deriveddata",
    "xcuserdata",
    "__macosx",
    "$recycle.bin",
    "recycle.bin",
    "system volume information",
    "lost+found",
    "temporary items",
    "network trash folder",
    "caches",
    "cacheddata",
    "tmp",
];

#[derive(Debug, Default)]
pub struct ScanOutcome {
    pub files: Vec<PathBuf>,
    /// Stopped because another new file would pass the Shelf's file limit.
    pub hit_file_cap: bool,
}

pub fn skip_dir_name(name: &str) -> bool {
    let folded = name.to_ascii_lowercase();
    folded.ends_with(".noindex") || SKIP_DIR_NAMES.iter().any(|skip| *skip == folded)
}

/// True when a path relative to a source root crosses a skipped folder or
/// a hidden segment, or the file itself is a lock/temp name. The source
/// root itself is never judged by its name.
pub fn rel_is_skipped(rel: &str) -> bool {
    let path = Path::new(rel);
    if path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy();
        name.starts_with('.') || skip_dir_name(&name)
    }) {
        return true;
    }
    path.file_name()
        .map(|name| crate::ingest::extract::skip_file_name(&name.to_string_lossy()))
        .unwrap_or(false)
}

/// Walk `root` for supported files that are not in `already`. Stops after
/// `max_new` new files or [`walk_budget`] visits. Does not skip `root`
/// even if its name is on the ignore list (the user linked it on purpose).
pub fn scan_new_files(root: &Path, max_new: usize, already: &HashSet<PathBuf>) -> ScanOutcome {
    let mut outcome = ScanOutcome::default();
    if max_new == 0 {
        outcome.hit_file_cap = true;
        return outcome;
    }
    let max_visits = walk_budget(max_new, already.len());
    let mut stack = vec![root.to_path_buf()];
    let mut visits = 0usize;
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            visits += 1;
            if visits >= max_visits {
                outcome.files.sort();
                return outcome;
            }
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') {
                continue;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                if skip_dir_name(&name) {
                    continue;
                }
                stack.push(path);
                continue;
            }
            if !file_type.is_file() || !crate::ingest::extract::is_supported_file(&path) {
                continue;
            }
            if already.contains(&path) {
                continue;
            }
            if outcome.files.len() >= max_new {
                outcome.hit_file_cap = true;
                outcome.files.sort();
                return outcome;
            }
            outcome.files.push(path);
        }
    }
    outcome.files.sort();
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan_all(root: &Path) -> Vec<PathBuf> {
        scan_new_files(root, DEFAULT_FILE_LIMIT, &HashSet::new()).files
    }

    #[test]
    fn skip_list_covers_packages_not_library() {
        assert!(skip_dir_name("node_modules"));
        assert!(skip_dir_name("Node_Modules"));
        assert!(skip_dir_name("__pycache__"));
        assert!(skip_dir_name("Caches"));
        assert!(skip_dir_name("venv"));
        assert!(skip_dir_name("Photos.noindex"));
        assert!(skip_dir_name("elm-stuff"));
        assert!(!skip_dir_name("Library"));
        assert!(!skip_dir_name("vendor"));
        assert!(!skip_dir_name("dist"));
        assert!(!skip_dir_name("build"));
        assert!(!skip_dir_name("docs"));
        assert!(!skip_dir_name("Downloads"));
    }

    #[test]
    fn rel_skip_is_relative_to_the_source() {
        assert!(rel_is_skipped("src/node_modules/pkg/readme.md"));
        assert!(rel_is_skipped(".hidden/note.md"));
        assert!(rel_is_skipped("~$report.docx"));
        assert!(rel_is_skipped("incoming/~$Book.xlsx"));
        assert!(rel_is_skipped("scratch.tmp"));
        assert!(rel_is_skipped("logs/app.TMP"));
        assert!(!rel_is_skipped("pkg/readme.md"));
        assert!(!rel_is_skipped("Library/notes.md"));
        assert!(!rel_is_skipped("report.docx"));
    }

    #[test]
    fn scan_skips_node_modules_and_dotfiles_but_reads_library() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
        std::fs::write(root.join("node_modules/pkg/readme.md"), "nope").unwrap();
        std::fs::write(root.join(".secret.md"), "nope").unwrap();
        std::fs::create_dir_all(root.join("Library")).unwrap();
        std::fs::write(root.join("Library/notes.md"), "keep").unwrap();
        std::fs::write(root.join("brief.md"), "keep").unwrap();
        let scanned = scan_all(root);
        let names: Vec<_> = scanned
            .iter()
            .map(|p| {
                p.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        assert_eq!(names, vec!["Library/notes.md", "brief.md"]);
    }

    #[test]
    fn scan_skips_office_lock_files_and_tmp() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("~$report.docx"), "lock").unwrap();
        std::fs::write(root.join("scratch.tmp"), "tmp").unwrap();
        std::fs::write(root.join("note.md"), "keep").unwrap();
        let scanned = scan_all(root);
        let names: Vec<_> = scanned
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["note.md"]);
    }

    #[test]
    fn scan_stops_at_the_new_file_cap() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for i in 0..8 {
            std::fs::write(root.join(format!("n{i}.md")), "x").unwrap();
        }
        let outcome = scan_new_files(root, 3, &HashSet::new());
        assert_eq!(outcome.files.len(), 3);
        assert!(outcome.hit_file_cap);
    }

    #[test]
    fn walk_budget_grows_with_the_file_limit() {
        assert_eq!(walk_budget(DEFAULT_FILE_LIMIT, 0), MIN_WALK_ENTRIES);
        assert_eq!(walk_budget(3, 0), MIN_WALK_ENTRIES);
        assert_eq!(walk_budget(1_000, 9_000), 300_000);
        assert_eq!(walk_budget(usize::MAX, 1), usize::MAX);
    }

    #[test]
    fn scan_does_not_recount_files_already_on_the_shelf() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let known = root.join("old.md");
        std::fs::write(&known, "old").unwrap();
        std::fs::write(root.join("new.md"), "new").unwrap();
        let mut already = HashSet::new();
        already.insert(known);
        let outcome = scan_new_files(root, 1, &already);
        assert_eq!(outcome.files.len(), 1);
        assert_eq!(outcome.files[0].file_name().unwrap(), "new.md");
        assert!(!outcome.hit_file_cap);
    }

    #[test]
    fn linking_a_folder_named_node_modules_still_reads_it() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("node_modules");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("readme.md"), "explicit").unwrap();
        std::fs::create_dir_all(root.join("nested_pkg")).unwrap();
        std::fs::write(root.join("nested_pkg/also.md"), "ok").unwrap();
        let inner = root.join("node_modules");
        std::fs::create_dir_all(&inner).unwrap();
        std::fs::write(inner.join("skip.md"), "no").unwrap();
        let scanned = scan_all(&root);
        assert_eq!(scanned.len(), 2);
        assert!(scanned.iter().all(|p| !p.starts_with(&inner)));
    }

    #[test]
    fn linking_a_folder_named_caches_still_reads_it() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Caches");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("note.md"), "explicit").unwrap();
        let scanned = scan_all(&root);
        assert_eq!(scanned.len(), 1);
    }
}
