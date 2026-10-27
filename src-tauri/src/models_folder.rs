//! Where AI models live, and moving them to another folder.
//!
//! Models default to `<app-data>/models`. A person short of space can choose
//! another folder, on another drive, say (issue #29). Rules:
//!
//! - Rebost never creates a chosen folder. On macOS an unplugged drive's
//!   `/Volumes/<name>` path would otherwise quietly appear on the startup disk.
//! - A missing chosen folder is "unavailable", never "empty": nothing is
//!   downloaded into it and no installed AI is forgotten.
//! - A move never loses a model. Each file is staged in the new folder (a hard
//!   link on the same disk, otherwise a copy checked against the original),
//!   the setting is saved, and only then are the old files removed. Until
//!   that save, a failure removes only what was staged.

use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Error code while the chosen folder can't be found (an unplugged drive).
pub const MISSING: &str = "models-folder-missing";

/// Room left free on the target disk after a move, so the computer is not
/// left with a full disk.
pub const SPARE_BYTES: u64 = 1024 * 1024 * 1024;

const CHUNK: usize = 8 * 1024 * 1024;

/// Free space on the disk that holds `path`, when the OS reports it. A path
/// that does not exist yet is measured on its nearest existing parent.
pub fn free_space(path: &Path) -> Option<u64> {
    let probe = existing_ancestor(path);
    let disks = sysinfo::Disks::new_with_refreshed_list();
    disk_for(
        &probe,
        disks.iter().map(|d| (d.mount_point(), d.available_space())),
    )
    .map(|(_, free)| free)
}

/// Whether `needed` bytes fit. An unknown free space never blocks: a wrong
/// "full" would stop a download that would have worked.
pub fn has_room(free: Option<u64>, needed: u64) -> bool {
    free.is_none_or(|free| free >= needed)
}

/// Whether two folders sit on the same disk (so a move needs no extra room).
pub fn same_disk(a: &Path, b: &Path) -> bool {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let mounts: Vec<_> = disks.iter().map(|d| (d.mount_point(), 0)).collect();
    let a = disk_for(&existing_ancestor(a), mounts.iter().copied()).map(|(m, _)| m);
    let b = disk_for(&existing_ancestor(b), mounts.iter().copied()).map(|(m, _)| m);
    a.is_some() && a == b
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

/// Whether two paths name the same file or folder: a symlink, junction,
/// `subst` drive or another casing of the same place. Paths that don't
/// exist are only the same when they are spelled the same.
pub fn same_entry(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        match (std::fs::metadata(a), std::fs::metadata(b)) {
            (Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
            _ => false,
        }
    }
    #[cfg(not(unix))]
    {
        // The final path resolves junctions and `subst`; NTFS ignores case.
        let resolved =
            |p: &Path| std::fs::canonicalize(p).map(|p| p.to_string_lossy().to_lowercase());
        matches!((resolved(a), resolved(b)), (Ok(a), Ok(b)) if a == b)
    }
}

fn same_folder(a: &Path, b: &Path) -> bool {
    match (a.parent(), b.parent()) {
        (Some(a), Some(b)) => same_entry(a, b),
        _ => false,
    }
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
}

impl FolderProblem {
    pub fn code(self) -> &'static str {
        match self {
            Self::NotWritable => "models-folder-not-writable",
            Self::NotEnoughSpace => "models-folder-full",
            Self::InsideAppData => "models-folder-app-data",
            Self::NameTaken => "models-folder-name-taken",
            Self::NoAisThere => "models-folder-no-ais",
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
        let free = free_space(to);
        let default = same_entry(to, &paths.default_models_dir());
        let problem = if !default && to.starts_with(paths.base()) {
            Some(FolderProblem::InsideAppData)
        } else if same_entry(to, current) {
            None
        } else if let Err(problem) = check_writable_or_default(default, to) {
            Some(problem)
        } else if plan.to_move.iter().any(|(file, bytes)| {
            // Same-size files are compared byte for byte when the move runs.
            // A link is never a copy (see `stage_one`).
            std::fs::symlink_metadata(to.join(file))
                .is_ok_and(|m| !m.is_file() || m.len() != *bytes)
        }) {
            Some(FolderProblem::NameTaken)
        } else if !has_room(free, plan.bytes_needed(same_disk(current, to))) {
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

/// Files placed in the new folder, removed again if the move is abandoned.
#[must_use = "commit the setting, then finish(); or roll_back()"]
pub struct Staged {
    from: PathBuf,
    moved: Vec<String>,
    created: Vec<PathBuf>,
}

impl Staged {
    /// Remove what this move put in the new folder. The originals are untouched.
    pub fn roll_back(self) {
        for path in &self.created {
            if let Err(error) = std::fs::remove_file(path) {
                log::warn!("could not remove staged {}: {error}", path.display());
            }
        }
    }

    /// The new folder is in use: remove the originals. A file that can't be
    /// removed only costs space, so it is logged, not returned.
    pub fn finish(self) -> usize {
        let mut left = 0;
        for file in &self.moved {
            let old = self.from.join(file);
            if let Err(error) = std::fs::remove_file(&old) {
                if error.kind() != std::io::ErrorKind::NotFound {
                    log::warn!("could not remove moved AI {}: {error}", old.display());
                    left += 1;
                }
            }
        }
        left
    }
}

/// Stage every file of `plan` in `plan.to`. `progress(done, total)` is
/// called as bytes are placed and checked; `cancelled()` aborts between
/// chunks. On error nothing staged is left behind.
pub fn stage(
    plan: &MovePlan,
    progress: &mut dyn FnMut(u64, u64),
    cancelled: &dyn Fn() -> bool,
) -> Result<Staged> {
    let mut staged = Staged {
        from: plan.from.clone(),
        moved: Vec::new(),
        created: Vec::new(),
    };
    let total = plan.bytes();
    let mut done = 0u64;
    progress(0, total);
    for (file, bytes) in &plan.to_move {
        let result = stage_one(
            &plan.from.join(file),
            &plan.to.join(file),
            *bytes,
            &mut |n| progress(done + n, total),
            cancelled,
        );
        match result {
            Ok(created) => {
                staged.created.extend(created);
                staged.moved.push(file.clone());
                done += bytes;
                progress(done, total);
            }
            Err(error) => {
                staged.roll_back();
                return Err(error);
            }
        }
    }
    Ok(staged)
}

/// Place one file at `dst`. Returns the path it created, or `None` when an
/// identical copy was already there.
fn stage_one(
    src: &Path,
    dst: &Path,
    bytes: u64,
    progress: &mut dyn FnMut(u64),
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<PathBuf>> {
    // `symlink_metadata`, not `exists`: a link (even a dangling one) is a name.
    if let Ok(meta) = std::fs::symlink_metadata(dst) {
        // Only a regular file can be a copy. A symlink to the original would
        // dangle once the original is removed.
        if !meta.is_file() {
            return Err(anyhow!(FolderProblem::NameTaken.code()));
        }
        // The same folder under another path: "reusing" the file would
        // delete the only copy. (A hard link in another folder is a real
        // second name, left by an earlier move, and is fine to reuse.)
        if same_entry(src, dst) && same_folder(src, dst) {
            return Err(anyhow!(
                "{} and {} are the same file",
                src.display(),
                dst.display()
            ));
        }
        // Picked a folder that already holds this AI (an earlier copy).
        if files_identical(src, dst, progress, cancelled)? {
            return Ok(None);
        }
        return Err(anyhow!(FolderProblem::NameTaken.code()));
    }
    if std::fs::hard_link(src, dst).is_ok() {
        return Ok(Some(dst.to_path_buf()));
    }
    let tmp = dst.with_file_name(format!(
        ".{}.rebost-moving",
        dst.file_name().unwrap_or_default().to_string_lossy()
    ));
    let result = copy_checked(src, &tmp, bytes, progress, cancelled)
        .and_then(|()| std::fs::rename(&tmp, dst).context("rename staged copy"));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result.map(|()| Some(dst.to_path_buf()))
}

fn cancelled_error() -> anyhow::Error {
    anyhow!("cancelled")
}

/// Copy `src` to `dst`, flush it, then read the copy back and compare
/// checksums. Progress counts the copy as the first half, the check as the second.
fn copy_checked(
    src: &Path,
    dst: &Path,
    bytes: u64,
    progress: &mut dyn FnMut(u64),
    cancelled: &dyn Fn() -> bool,
) -> Result<()> {
    let mut input = std::fs::File::open(src).with_context(|| format!("open {}", src.display()))?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut output = options
        .open(dst)
        .with_context(|| format!("create {}", dst.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; CHUNK];
    let mut copied = 0u64;
    loop {
        if cancelled() {
            return Err(cancelled_error());
        }
        let n = input.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        output.write_all(&buf[..n]).map_err(|error| {
            if is_disk_full(&error) {
                anyhow!(FolderProblem::NotEnoughSpace.code())
            } else {
                anyhow!(error).context("write copy")
            }
        })?;
        copied += n as u64;
        progress(copied / 2);
    }
    output.sync_all()?;
    drop(output);
    if copied != bytes {
        return Err(anyhow!("model changed while it was being moved"));
    }
    let expected = hasher.finalize();
    let actual = checksum(dst, &mut |n| progress(bytes / 2 + n.div_ceil(2)), cancelled)?;
    if actual != expected.as_slice() {
        return Err(anyhow!("moved copy did not match the original"));
    }
    Ok(())
}

fn is_disk_full(error: &std::io::Error) -> bool {
    // ENOSPC on Unix, ERROR_DISK_FULL / ERROR_HANDLE_DISK_FULL on Windows.
    error.kind() == std::io::ErrorKind::StorageFull
        || (cfg!(windows) && matches!(error.raw_os_error(), Some(39 | 112)))
}

fn checksum(
    path: &Path,
    progress: &mut dyn FnMut(u64),
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; CHUNK];
    let mut read = 0u64;
    loop {
        if cancelled() {
            return Err(cancelled_error());
        }
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        read += n as u64;
        progress(read);
    }
    Ok(hasher.finalize().to_vec())
}

fn files_identical(
    a: &Path,
    b: &Path,
    progress: &mut dyn FnMut(u64),
    cancelled: &dyn Fn() -> bool,
) -> Result<bool> {
    let (a_len, b_len) = (std::fs::metadata(a)?.len(), std::fs::metadata(b)?.len());
    if a_len != b_len {
        return Ok(false);
    }
    let left = checksum(a, &mut |n| progress(n / 2), cancelled)?;
    let right = checksum(b, &mut |n| progress(a_len / 2 + n.div_ceil(2)), cancelled)?;
    Ok(left == right)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, bytes: &[u8]) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(name), bytes).unwrap();
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    fn files(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn disk_is_the_longest_matching_mount() {
        let disks = [
            (Path::new("/"), 10),
            (Path::new("/Volumes/Data"), 500),
            (Path::new("/Volumes/Data Two"), 7),
        ];
        let pick = |path: &str| disk_for(Path::new(path), disks.iter().copied());
        assert_eq!(
            pick("/Volumes/Data/AI"),
            Some((Path::new("/Volumes/Data"), 500))
        );
        assert_eq!(
            pick("/Volumes/Data Two/AI"),
            Some((Path::new("/Volumes/Data Two"), 7))
        );
        assert_eq!(pick("/Users/me/Library"), Some((Path::new("/"), 10)));
        // A sibling whose name only starts the same is a different disk.
        assert_eq!(pick("/Volumes/DataX"), Some((Path::new("/"), 10)));
        assert_eq!(disk_for(Path::new("/x"), []), None);
    }

    #[cfg(windows)]
    #[test]
    fn windows_drive_letters_match_in_any_case() {
        let disks = [(Path::new("C:\\"), 3), (Path::new("D:\\"), 900)];
        let pick = |path: &str| disk_for(Path::new(path), disks.iter().copied()).map(|d| d.1);
        assert_eq!(pick(r"D:\Rebost\Modelos"), Some(900));
        assert_eq!(pick(r"d:\Rebost\Modelos"), Some(900));
        assert_eq!(pick(r"C:\Users\me\AppData\Roaming"), Some(3));
    }

    #[test]
    fn free_space_measures_the_nearest_existing_folder() {
        let dir = tempfile::tempdir().unwrap();
        let not_yet = dir.path().join("a").join("b");
        // Free space moves while other programs write, so compare disks, not bytes.
        assert!(free_space(&not_yet).is_some_and(|free| free > 0));
        assert_eq!(existing_ancestor(&not_yet), dir.path());
        assert!(same_disk(dir.path(), &not_yet));
    }

    #[test]
    fn room_check_never_blocks_on_unknown_space() {
        assert!(has_room(Some(10), 10));
        assert!(!has_room(Some(9), 10));
        assert!(has_room(None, u64::MAX));
    }

    #[test]
    fn plain_path_drops_the_verbatim_prefix() {
        assert_eq!(
            plain_path(PathBuf::from(r"\\?\D:\Rebost\Modelos")),
            PathBuf::from(r"D:\Rebost\Modelos")
        );
        assert_eq!(
            plain_path(PathBuf::from(r"\\?\UNC\nas\share\AI")),
            PathBuf::from(r"\\nas\share\AI")
        );
        assert_eq!(
            plain_path(PathBuf::from("/Volumes/Data/AI")),
            PathBuf::from("/Volumes/Data/AI")
        );
    }

    #[test]
    fn plan_sorts_files_by_where_they_are() {
        let dir = tempfile::tempdir().unwrap();
        let (from, to) = (dir.path().join("from"), dir.path().join("to"));
        write(&from, "a.gguf", b"aaaa");
        write(&to, "b.gguf", b"bb");
        let plan = MovePlan::new(&files(&["a.gguf", "b.gguf", "c.gguf"]), &from, &to);
        assert_eq!(plan.to_move, [("a.gguf".to_string(), 4)]);
        assert_eq!(plan.already_there, ["b.gguf"]);
        assert_eq!(plan.missing, ["c.gguf"]);
        assert_eq!(plan.bytes(), 4);
        assert_eq!(plan.bytes_needed(true), 0);
        assert_eq!(plan.bytes_needed(false), 4 + SPARE_BYTES);
        let nothing = MovePlan::new(&files(&["b.gguf"]), &from, &to);
        assert_eq!(nothing.bytes_needed(false), 0);
    }

    #[test]
    fn preflight_spots_folders_that_cannot_hold_the_ais() {
        let dir = tempfile::tempdir().unwrap();
        let paths = crate::paths::Paths::new(dir.path().join("io.rebost.desktop"));
        paths.ensure().unwrap();
        let current = paths.default_models_dir();
        write(&current, "a.gguf", b"aaaa");
        let installed = files(&["a.gguf"]);
        let check = |to: &Path, missing: bool| {
            Preflight::check(&paths, to, &installed, &current, missing).problem
        };

        let drive = dir.path().join("Drive");
        std::fs::create_dir(&drive).unwrap();
        let ok = Preflight::check(&paths, &drive, &installed, &current, false);
        assert_eq!(ok.problem, None);
        assert_eq!(ok.plan.to_move, [("a.gguf".to_string(), 4)]);
        assert!(ok.free.is_some());

        assert_eq!(check(&current, false), None, "staying put is fine");
        assert_eq!(
            check(&paths.shelves_dir(), false),
            Some(FolderProblem::InsideAppData)
        );
        assert_eq!(
            check(&dir.path().join("Unplugged"), false),
            Some(FolderProblem::NotWritable)
        );
        // The current folder is gone and the picked one is empty.
        let gone = dir.path().join("Gone");
        let lost = Preflight::check(&paths, &drive, &installed, &gone, true);
        assert_eq!(lost.problem, Some(FolderProblem::NoAisThere));
        // ...but a folder that holds them is the fix.
        write(&drive, "a.gguf", b"aaaa");
        let found = Preflight::check(&paths, &drive, &installed, &gone, true);
        assert_eq!(found.problem, None);
        assert_eq!(found.plan.already_there, ["a.gguf"]);
    }

    #[test]
    fn preflight_recreates_only_the_default_folder() {
        let dir = tempfile::tempdir().unwrap();
        let paths = crate::paths::Paths::new(dir.path().join("io.rebost.desktop"));
        let gone = dir.path().join("Gone");
        let default = paths.default_models_dir();
        let back = Preflight::check(&paths, &default, &files(&["a.gguf"]), &gone, true);
        assert_eq!(
            back.problem, None,
            "going back is allowed even with nothing there"
        );
        assert_eq!(back.plan.missing, ["a.gguf"]);
        assert!(default.is_dir());
    }

    #[test]
    fn preflight_spots_a_different_file_with_an_ai_name() {
        let dir = tempfile::tempdir().unwrap();
        let paths = crate::paths::Paths::new(dir.path().join("io.rebost.desktop"));
        paths.ensure().unwrap();
        let current = paths.default_models_dir();
        write(&current, "a.gguf", b"aaaa");
        let drive = dir.path().join("Drive");
        write(&drive, "a.gguf", b"someone else's");
        let check = Preflight::check(&paths, &drive, &files(&["a.gguf"]), &current, false);
        assert_eq!(check.problem, Some(FolderProblem::NameTaken));
        // Same size could be the same AI: the move compares it then.
        write(&drive, "a.gguf", b"aaaa");
        let check = Preflight::check(&paths, &drive, &files(&["a.gguf"]), &current, false);
        assert_eq!(check.problem, None);
    }

    #[cfg(unix)]
    #[test]
    fn the_same_folder_under_another_path_is_never_a_move() {
        let dir = tempfile::tempdir().unwrap();
        let paths = crate::paths::Paths::new(dir.path().join("io.rebost.desktop"));
        let real = dir.path().join("Real");
        write(&real, "a.gguf", b"only copy");
        let alias = dir.path().join("Alias");
        std::os::unix::fs::symlink(&real, &alias).unwrap();
        assert!(same_entry(&real, &alias));
        assert!(same_entry(&real.join("a.gguf"), &alias.join("a.gguf")));
        assert!(!same_entry(&real, dir.path()));
        assert!(!same_entry(&dir.path().join("x"), &dir.path().join("y")));

        let installed = files(&["a.gguf"]);
        assert_eq!(
            Preflight::check(&paths, &alias, &installed, &real, false).problem,
            None
        );
        // Even if a caller got this far, staging refuses and keeps the file.
        let plan = MovePlan::new(&installed, &real, &alias);
        assert!(stage(&plan, &mut |_, _| {}, &|| false).is_err());
        assert_eq!(std::fs::read(real.join("a.gguf")).unwrap(), b"only copy");
    }

    #[cfg(unix)]
    #[test]
    fn rebosts_own_folder_under_another_path_is_still_rebosts() {
        let dir = tempfile::tempdir().unwrap();
        let paths = crate::paths::Paths::new(dir.path().join("io.rebost.desktop"));
        paths.ensure().unwrap();
        let alias = dir.path().join("Alias");
        std::os::unix::fs::symlink(paths.default_models_dir(), &alias).unwrap();
        let gone = dir.path().join("Gone");
        let back = Preflight::check(&paths, &alias, &files(&["a.gguf"]), &gone, true);
        assert_eq!(back.problem, None);
    }

    #[cfg(unix)]
    #[test]
    fn a_hard_link_left_in_the_new_folder_is_reused() {
        let dir = tempfile::tempdir().unwrap();
        let (from, to) = (dir.path().join("from"), dir.path().join("to"));
        write(&from, "a.gguf", b"weights");
        std::fs::create_dir_all(&to).unwrap();
        std::fs::hard_link(from.join("a.gguf"), to.join("a.gguf")).unwrap();
        let plan = MovePlan::new(&files(&["a.gguf"]), &from, &to);
        stage(&plan, &mut |_, _| {}, &|| false).unwrap().finish();
        assert!(!from.join("a.gguf").exists());
        assert_eq!(std::fs::read(to.join("a.gguf")).unwrap(), b"weights");
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_to_the_original_is_never_taken_for_a_copy() {
        let dir = tempfile::tempdir().unwrap();
        let paths = crate::paths::Paths::new(dir.path().join("io.rebost.desktop"));
        let (from, to) = (dir.path().join("from"), dir.path().join("to"));
        write(&from, "a.gguf", b"only copy");
        std::fs::create_dir_all(&to).unwrap();
        std::os::unix::fs::symlink(from.join("a.gguf"), to.join("a.gguf")).unwrap();
        let installed = files(&["a.gguf"]);
        assert_eq!(
            Preflight::check(&paths, &to, &installed, &from, false).problem,
            Some(FolderProblem::NameTaken)
        );
        let plan = MovePlan::new(&installed, &from, &to);
        let error = stage(&plan, &mut |_, _| {}, &|| false).err().unwrap();
        assert!(error.to_string().contains(FolderProblem::NameTaken.code()));
        assert_eq!(std::fs::read(from.join("a.gguf")).unwrap(), b"only copy");
    }

    #[test]
    fn writable_check_needs_an_existing_folder() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(check_writable(dir.path()), Ok(()));
        assert!(names(dir.path()).is_empty(), "probe file is removed");
        assert_eq!(
            check_writable(&dir.path().join("unplugged")),
            Err(FolderProblem::NotWritable)
        );
    }

    #[cfg(unix)]
    #[test]
    fn writable_check_spots_a_read_only_folder() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let locked = dir.path().join("locked");
        std::fs::create_dir(&locked).unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o500)).unwrap();
        let result = check_writable(&locked);
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(result, Err(FolderProblem::NotWritable));
    }

    #[test]
    fn move_keeps_originals_until_finished() {
        let dir = tempfile::tempdir().unwrap();
        let (from, to) = (dir.path().join("from"), dir.path().join("to"));
        write(&from, "a.gguf", &vec![7u8; CHUNK + 3]);
        write(&from, "vision-1.gguf", b"vv");
        std::fs::create_dir_all(&to).unwrap();
        let plan = MovePlan::new(&files(&["a.gguf", "vision-1.gguf"]), &from, &to);
        let mut last = (0, 0);
        let staged = stage(&plan, &mut |done, total| last = (done, total), &|| false).unwrap();
        assert_eq!(last, (plan.bytes(), plan.bytes()));
        // Both folders hold the AIs until the new folder is in use.
        assert_eq!(names(&from), ["a.gguf", "vision-1.gguf"]);
        assert_eq!(names(&to), ["a.gguf", "vision-1.gguf"]);
        assert_eq!(staged.finish(), 0);
        assert!(names(&from).is_empty());
        assert_eq!(
            std::fs::read(to.join("a.gguf")).unwrap(),
            vec![7u8; CHUNK + 3]
        );
    }

    #[test]
    fn roll_back_removes_only_what_the_move_created() {
        let dir = tempfile::tempdir().unwrap();
        let (from, to) = (dir.path().join("from"), dir.path().join("to"));
        write(&from, "a.gguf", b"aaaa");
        write(&to, "notes.txt", b"mine");
        let plan = MovePlan::new(&files(&["a.gguf"]), &from, &to);
        stage(&plan, &mut |_, _| {}, &|| false).unwrap().roll_back();
        assert_eq!(names(&from), ["a.gguf"]);
        assert_eq!(names(&to), ["notes.txt"]);
    }

    #[test]
    fn copy_is_checked_and_cancel_leaves_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();
        let (src, dst) = (dir.path().join("a.gguf"), dir.path().join("b.gguf"));
        std::fs::write(&src, vec![1u8; CHUNK * 2 + 5]).unwrap();
        let mut seen = Vec::new();
        copy_checked(
            &src,
            &dst,
            (CHUNK * 2 + 5) as u64,
            &mut |n| seen.push(n),
            &|| false,
        )
        .unwrap();
        assert_eq!(std::fs::read(&dst).unwrap(), std::fs::read(&src).unwrap());
        assert!(seen.windows(2).all(|w| w[0] <= w[1]), "progress only grows");
        assert_eq!(*seen.last().unwrap(), (CHUNK * 2 + 5) as u64);

        let cancelled = copy_checked(&src, &dir.path().join("c.gguf"), 1, &mut |_| {}, &|| true);
        assert_eq!(cancelled.unwrap_err().to_string(), "cancelled");
        let size_lie = copy_checked(&src, &dir.path().join("d.gguf"), 3, &mut |_| {}, &|| false);
        assert!(size_lie.is_err());
    }

    #[test]
    fn a_cancelled_move_keeps_every_original() {
        let dir = tempfile::tempdir().unwrap();
        let (from, to) = (dir.path().join("from"), dir.path().join("to"));
        write(&from, "a.gguf", b"aaaa");
        write(&from, "b.gguf", b"bbbb");
        std::fs::create_dir_all(&to).unwrap();
        let plan = MovePlan::new(&files(&["a.gguf", "b.gguf"]), &from, &to);
        // Fail on the second file: an identical-name file with other bytes.
        write(&to, "b.gguf", b"different");
        assert!(stage(&plan, &mut |_, _| {}, &|| false).is_err());
        assert_eq!(names(&from), ["a.gguf", "b.gguf"]);
        assert_eq!(names(&to), ["b.gguf"]);
        assert_eq!(std::fs::read(to.join("b.gguf")).unwrap(), b"different");
    }

    #[test]
    fn an_identical_copy_already_there_is_reused() {
        let dir = tempfile::tempdir().unwrap();
        let (from, to) = (dir.path().join("from"), dir.path().join("to"));
        write(&from, "a.gguf", b"same");
        write(&to, "a.gguf", b"same");
        let plan = MovePlan::new(&files(&["a.gguf"]), &from, &to);
        let staged = stage(&plan, &mut |_, _| {}, &|| false).unwrap();
        staged.finish();
        assert!(names(&from).is_empty());
        assert_eq!(std::fs::read(to.join("a.gguf")).unwrap(), b"same");
    }

    #[test]
    fn rolling_back_never_removes_a_reused_copy() {
        let dir = tempfile::tempdir().unwrap();
        let (from, to) = (dir.path().join("from"), dir.path().join("to"));
        write(&from, "a.gguf", b"same");
        write(&to, "a.gguf", b"same");
        let plan = MovePlan::new(&files(&["a.gguf"]), &from, &to);
        stage(&plan, &mut |_, _| {}, &|| false).unwrap().roll_back();
        assert_eq!(names(&from), ["a.gguf"]);
        assert_eq!(names(&to), ["a.gguf"]);
    }
}
