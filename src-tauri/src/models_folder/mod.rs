//! Where AI models live, and moving them to another folder.
//!
//! Models default to `<app-data>/models`. A person short of space can choose
//! another folder, on another drive, say (issue #29).
//!
//! # The one rule
//!
//! A name of an AI file is removed only when the same bytes stay reachable
//! under another name Rebost checked, in a folder proven to be a different
//! place:
//!
//! 1. Both names are regular files (not links) of the same size.
//! 2. The two folders are different places by file identity (device and
//!    inode on macOS and Linux, volume serial and file index on Windows),
//!    never by comparing path text.
//! 3. The other name is the very same file (a hard link), or an independent
//!    copy that was written, flushed to disk and read back with the
//!    original's SHA-256 before it was recorded.
//! 4. What Rebost can't prove keeps both names. A kept copy costs disk
//!    space; it never costs a model.
//!
//! The rule guards every removal: the old names after a move, the new names
//! when a move is undone, and both when a restart finds a half-done move.
//! The setting points at the new folder only after every file is recorded
//! there. See [`mover`].
//!
//! Also:
//!
//! - A folder that is the current one under another path is no change; one
//!   inside the current folder, or holding it, is refused.
//! - Rebost never creates a chosen folder. On macOS an unplugged drive's
//!   `/Volumes/<name>` path would otherwise quietly appear on the startup disk.
//! - A missing chosen folder is "unavailable", never "empty": nothing is
//!   downloaded into it and no installed AI is forgotten.

use serde::Serialize;
use std::path::{Path, PathBuf};

pub mod mover;
#[cfg(test)]
mod tests;

pub use mover::{journal_path, recover, stage, Mover, Recovered};

/// Error code while the chosen folder can't be found (an unplugged drive).
pub const MISSING: &str = "models-folder-missing";

/// Room left free on the target disk after a move, so the computer is not
/// left with a full disk.
pub const SPARE_BYTES: u64 = 1024 * 1024 * 1024;

/// Mounted disks and their free space, read once.
struct DiskList(Vec<(PathBuf, u64)>);

impl DiskList {
    fn read() -> Self {
        let disks = sysinfo::Disks::new_with_refreshed_list();
        Self(
            disks
                .iter()
                .map(|d| (d.mount_point().to_path_buf(), d.available_space()))
                .collect(),
        )
    }

    fn disk_of(&self, path: &Path) -> Option<(&Path, u64)> {
        let probe = existing_ancestor(path);
        disk_for(&probe, self.0.iter().map(|(m, free)| (m.as_path(), *free)))
    }
}

/// Free space on the disk that holds `path`, when the OS reports it. A path
/// that does not exist yet is measured on its nearest existing parent.
pub fn free_space(path: &Path) -> Option<u64> {
    DiskList::read().disk_of(path).map(|(_, free)| free)
}

/// Whether `needed` bytes fit. An unknown free space never blocks: a wrong
/// "full" would stop a download that would have worked.
pub fn has_room(free: Option<u64>, needed: u64) -> bool {
    free.is_none_or(|free| free >= needed)
}

/// Whether two folders sit on the same disk (so a move needs no extra room).
pub fn same_disk(a: &Path, b: &Path) -> bool {
    let disks = DiskList::read();
    let (a, b) = (disks.disk_of(a), disks.disk_of(b));
    a.is_some() && a.map(|(m, _)| m) == b.map(|(m, _)| m)
}

/// The disk that holds `path`: the mount point that is its longest prefix.
/// `/Volumes/Data/AI` is on `/Volumes/Data`, not `/`.
pub(crate) fn disk_for<'a>(
    path: &Path,
    disks: impl IntoIterator<Item = (&'a Path, u64)>,
) -> Option<(&'a Path, u64)> {
    disks
        .into_iter()
        .filter(|(mount, _)| on_mount(path, mount))
        .max_by_key(|(mount, _)| mount.components().count())
}

fn on_mount(path: &Path, mount: &Path) -> bool {
    if cfg!(windows) {
        // Drive letters and folder names are case-insensitive there.
        let lower = |p: &Path| PathBuf::from(p.to_string_lossy().to_lowercase());
        lower(path).starts_with(lower(mount))
    } else {
        path.starts_with(mount)
    }
}

fn existing_ancestor(path: &Path) -> PathBuf {
    path.ancestors()
        .find(|p| p.exists())
        .unwrap_or(path)
        .to_path_buf()
}

/// How two folders relate, by file identity rather than path text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relation {
    /// One place, maybe spelled differently: a symlink, junction, `subst`
    /// drive, `..`, a trailing separator or another casing.
    Same,
    /// One holds the other.
    Nested,
    /// Different places, or one can't be reached and the spellings differ.
    Separate,
}

/// How folders `a` and `b` relate. A folder that can't be reached (an
/// unplugged drive) is compared by its spelling.
pub fn relation(a: &Path, b: &Path) -> Relation {
    if a == b || is_same(a, b) {
        Relation::Same
    } else if within(a, b) || within(b, a) {
        Relation::Nested
    } else {
        Relation::Separate
    }
}

/// Proven to be one file or folder. An error proves nothing.
pub(crate) fn is_same(a: &Path, b: &Path) -> bool {
    matches!(same_file::is_same_file(a, b), Ok(true))
}

/// Proven to be two different places. An error proves nothing.
pub(crate) fn is_different(a: &Path, b: &Path) -> bool {
    matches!(same_file::is_same_file(a, b), Ok(false))
}

/// `inner` sits somewhere below `outer`.
fn within(inner: &Path, outer: &Path) -> bool {
    let Ok(outer_id) = same_file::Handle::from_path(outer) else {
        return inner.starts_with(outer) && inner != outer;
    };
    let inner = std::fs::canonicalize(inner).unwrap_or_else(|_| inner.to_path_buf());
    inner
        .ancestors()
        .skip(1)
        .any(|parent| same_file::Handle::from_path(parent).is_ok_and(|id| id == outer_id))
}

/// Drop the `\\?\` prefix Windows' `canonicalize` adds, so the path reads as
/// the person sees it in Explorer and matches mount points.
pub fn plain_path(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    if let Some(rest) = text.strip_prefix(r"\\?\") {
        return PathBuf::from(rest);
    }
    path
}

/// Why a picked folder can't hold the AIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FolderProblem {
    /// Rebost can't write there (read-only drive, no permission).
    NotWritable,
    /// Not enough room for the AIs that would move there.
    NotEnoughSpace,
    /// Inside Rebost's own data (other than its default models folder).
    InsideAppData,
    /// A different file with the same name as an installed AI is there.
    NameTaken,
    /// The current folder is unavailable and the picked one holds none of
    /// the installed AIs: switching would only forget them.
    NoAisThere,
    /// Inside the current folder, or holding it.
    Nested,
}

impl FolderProblem {
    pub fn code(self) -> &'static str {
        match self {
            Self::NotWritable => "models-folder-not-writable",
            Self::NotEnoughSpace => "models-folder-full",
            Self::InsideAppData => "models-folder-app-data",
            Self::NameTaken => "models-folder-name-taken",
            Self::NoAisThere => "models-folder-no-ais",
            Self::Nested => "models-folder-nested",
        }
    }
}

/// What moving the installed AIs from one folder to another involves.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MovePlan {
    pub from: PathBuf,
    pub to: PathBuf,
    /// Files in `from`, with their sizes.
    pub to_move: Vec<(String, u64)>,
    /// Files already in `to` and not in `from` (a folder picked again after
    /// its drive letter changed, say). Nothing to do.
    pub already_there: Vec<String>,
    /// Files in neither folder.
    pub missing: Vec<String>,
}

impl MovePlan {
    pub fn new(files: &[String], from: &Path, to: &Path) -> Self {
        let mut plan = Self {
            from: from.to_path_buf(),
            to: to.to_path_buf(),
            ..Self::default()
        };
        for file in files {
            match std::fs::metadata(from.join(file)) {
                Ok(meta) if meta.is_file() => plan.to_move.push((file.clone(), meta.len())),
                _ if to.join(file).is_file() => plan.already_there.push(file.clone()),
                _ => plan.missing.push(file.clone()),
            }
        }
        plan
    }

    pub fn bytes(&self) -> u64 {
        self.to_move.iter().map(|(_, bytes)| bytes).sum()
    }

    /// Bytes the target disk needs free: nothing when hard links can stage.
    pub fn bytes_needed(&self, same_disk: bool) -> u64 {
        if self.to_move.is_empty() || same_disk {
            0
        } else {
            self.bytes() + SPARE_BYTES
        }
    }
}

/// What switching to a folder would do, checked before anything changes.
#[derive(Debug, Clone)]
pub struct Preflight {
    pub plan: MovePlan,
    pub free: Option<u64>,
    pub problem: Option<FolderProblem>,
}

impl Preflight {
    /// `files` are the installed AIs' files; `current_missing` says the
    /// current folder is unavailable.
    pub fn check(
        paths: &crate::paths::Paths,
        to: &Path,
        files: &[String],
        current: &Path,
        current_missing: bool,
    ) -> Self {
        let plan = MovePlan::new(files, current, to);
        let disks = DiskList::read();
        let free = disks.disk_of(to).map(|(_, free)| free);
        let one_disk = disks
            .disk_of(current)
            .is_some_and(|(mount, _)| disks.disk_of(to).is_some_and(|(other, _)| other == mount));
        let default = relation(to, &paths.default_models_dir()) == Relation::Same;
        let here = relation(to, current);
        let problem = if here == Relation::Same {
            None
        } else if here == Relation::Nested {
            Some(FolderProblem::Nested)
        } else if !default
            && (relation(to, paths.base()) == Relation::Same || within(to, paths.base()))
        {
            // Folders that merely hold app data (the home folder) are fine.
            Some(FolderProblem::InsideAppData)
        } else if let Err(problem) = check_writable_or_default(default, to) {
            Some(problem)
        } else if plan.to_move.iter().any(|(file, bytes)| {
            // Same-size files are compared byte for byte when the move runs.
            // A link is never a copy (rule 1).
            std::fs::symlink_metadata(to.join(file))
                .is_ok_and(|m| !m.is_file() || m.len() != *bytes)
        }) {
            Some(FolderProblem::NameTaken)
        } else if !has_room(free, plan.bytes_needed(one_disk)) {
            Some(FolderProblem::NotEnoughSpace)
        } else if current_missing
            && !default
            && plan.already_there.is_empty()
            && !plan.missing.is_empty()
        {
            // Rebost's own folder is the explicit way out; it forgets after a confirm.
            Some(FolderProblem::NoAisThere)
        } else {
            None
        };
        Self {
            plan,
            free,
            problem,
        }
    }
}

/// The default folder is Rebost's own and may be created; a chosen one never is.
fn check_writable_or_default(default: bool, dir: &Path) -> Result<(), FolderProblem> {
    if default {
        std::fs::create_dir_all(dir).map_err(|_| FolderProblem::NotWritable)?;
    }
    check_writable(dir)
}

/// Check that `dir` exists and Rebost can create files in it.
pub fn check_writable(dir: &Path) -> Result<(), FolderProblem> {
    if !dir.is_dir() {
        return Err(FolderProblem::NotWritable);
    }
    let probe = dir.join(format!(".rebost-write-check-{}", uuid::Uuid::new_v4()));
    match std::fs::File::create(&probe) {
        Ok(file) => {
            drop(file);
            let _ = std::fs::remove_file(&probe);
            Ok(())
        }
        Err(_) => Err(FolderProblem::NotWritable),
    }
}
