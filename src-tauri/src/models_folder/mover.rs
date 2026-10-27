//! Moving installed AIs between folders under the one rule (see the module
//! above): a name is removed only while the same bytes stay reachable under
//! another checked name in a different place.
//!
//! One linear algorithm:
//!
//! 1. Write the journal (`models-move.json` in app data) before touching a
//!    file, and update it after each file is placed and checked.
//! 2. Place each file in the new folder: reuse a regular file already there
//!    when it is the same file or has the same SHA-256; otherwise make a hard
//!    link (same drive) or a copy that is flushed and read back.
//! 3. The caller saves the setting. Only then does [`Mover::finish`] remove
//!    the old names; [`Mover::roll_back`] instead removes the new names this
//!    move created. Both go through [`redundant`].
//! 4. A restart that finds the journal finishes the move when the setting
//!    already points at the new folder, and rolls it back otherwise.

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use super::{FolderProblem, MovePlan};

const CHUNK: usize = 8 * 1024 * 1024;
const JOURNAL: &str = "models-move.json";
/// Half-written copies. Never an installed file name (those end in `.gguf`).
const STAGING_SUFFIX: &str = ".rebost-moving";

/// Where an unfinished move is recorded.
pub fn journal_path(base: &Path) -> PathBuf {
    base.join(JOURNAL)
}

/// How a file came to be in the new folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Placed {
    /// A hard link this move made: the very same file.
    Linked,
    /// A copy this move wrote, flushed and read back with the original's SHA-256.
    Copied,
    /// A regular file that was already there: the same file, or the same SHA-256.
    Found,
}

impl Placed {
    fn created(self) -> bool {
        self != Self::Found
    }
}

/// A move in progress, as written to disk.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Journal {
    pub from: PathBuf,
    pub to: PathBuf,
    /// Every file the move set out to place.
    pub files: Vec<String>,
    /// Files placed and checked in `to`, in order.
    pub placed: Vec<(String, Placed)>,
}

/// How hard staging may try. Tests force copies to stand in for another drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Strategy {
    LinkOrCopy,
    #[cfg_attr(not(test), allow(dead_code))]
    Copy,
}

/// Files placed in the new folder, waiting for the setting to switch.
#[must_use = "save the setting, then finish(); or roll_back()"]
#[derive(Debug)]
pub struct Mover {
    journal: Journal,
    path: PathBuf,
}

/// What a restart did with a move it found unfinished.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recovered {
    /// The setting had switched: old names removed, `kept` left in place.
    Finished { kept: usize },
    /// The setting had not switched: names this move created removed.
    RolledBack,
}

/// Place every file of `plan` in `plan.to`, recording each in the journal at
/// `journal`. `progress(done, total)` follows bytes placed and checked;
/// `cancelled()` stops between chunks. On error the move is rolled back.
pub fn stage(
    plan: &MovePlan,
    journal: &Path,
    progress: &mut dyn FnMut(u64, u64),
    cancelled: &dyn Fn() -> bool,
) -> Result<Mover> {
    stage_with(plan, journal, Strategy::LinkOrCopy, progress, cancelled)
}

pub(crate) fn stage_with(
    plan: &MovePlan,
    journal: &Path,
    strategy: Strategy,
    progress: &mut dyn FnMut(u64, u64),
    cancelled: &dyn Fn() -> bool,
) -> Result<Mover> {
    let mut mover = Mover {
        journal: Journal {
            from: plan.from.clone(),
            to: plan.to.clone(),
            files: plan.to_move.iter().map(|(file, _)| file.clone()).collect(),
            placed: Vec::new(),
        },
        path: journal.to_path_buf(),
    };
    mover.save()?;
    let total = plan.bytes();
    let mut done = 0u64;
    progress(0, total);
    for (file, bytes) in &plan.to_move {
        let placed = place(
            &plan.from.join(file),
            &plan.to.join(file),
            *bytes,
            strategy,
            &mut |n| progress(done + n, total),
            cancelled,
        )
        .and_then(|how| {
            mover.journal.placed.push((file.clone(), how));
            mover.save()
        });
        if let Err(error) = placed {
            mover.roll_back();
            return Err(error);
        }
        done += bytes;
        progress(done, total);
    }
    Ok(mover)
}

impl Mover {
    fn save(&self) -> Result<()> {
        crate::paths::write_json(&self.path, &self.journal)
    }

    /// The setting points at the new folder: remove the old names the rule
    /// allows. Returns how many old names were kept.
    pub fn finish(self) -> usize {
        let kept = finish(&self.journal);
        forget(&self.path);
        kept
    }

    /// The setting did not switch: remove the new names this move created
    /// that the rule allows, and any half-written copies.
    pub fn roll_back(self) {
        roll_back(&self.journal);
        forget(&self.path);
    }
}

/// Finish or undo a move a previous run left unfinished. `current` is the
/// folder the setting points at now.
pub fn recover(base: &Path, current: &Path) -> Option<Recovered> {
    let path = journal_path(base);
    let journal: Journal = crate::paths::read_json(&path)?;
    let outcome = if journal.to == current || super::is_same(&journal.to, current) {
        Recovered::Finished {
            kept: finish(&journal),
        }
    } else {
        roll_back(&journal);
        Recovered::RolledBack
    };
    forget(&path);
    Some(outcome)
}

fn finish(journal: &Journal) -> usize {
    let mut kept = 0;
    for (file, _) in &journal.placed {
        let old = journal.from.join(file);
        if std::fs::symlink_metadata(&old).is_err() {
            continue;
        }
        if !remove_if_redundant(&old, &journal.to.join(file)) {
            kept += 1;
        }
    }
    kept
}

fn roll_back(journal: &Journal) {
    for file in &journal.files {
        let _ = std::fs::remove_file(staging_name(&journal.to.join(file)));
    }
    for (file, how) in &journal.placed {
        if how.created() {
            remove_if_redundant(&journal.to.join(file), &journal.from.join(file));
        }
    }
}

fn forget(journal: &Path) {
    for path in [journal.to_path_buf(), crate::paths::backup_path(journal)] {
        let _ = std::fs::remove_file(path);
    }
}

/// The rule, at the moment of removal. `keep` was checked when it was placed
/// (it is in the journal); here both names must still be regular files of
/// the same size in folders proven to be different places.
pub(crate) fn redundant(remove: &Path, keep: &Path) -> bool {
    let (Ok(gone), Ok(kept)) = (
        std::fs::symlink_metadata(remove),
        std::fs::symlink_metadata(keep),
    ) else {
        return false;
    };
    let folders = remove.parent().zip(keep.parent());
    gone.is_file()
        && kept.is_file()
        && gone.len() == kept.len()
        && folders.is_some_and(|(a, b)| super::is_different(a, b))
}

fn remove_if_redundant(remove: &Path, keep: &Path) -> bool {
    if !redundant(remove, keep) {
        log::warn!(
            "kept {}: can't prove {} holds the same AI",
            remove.display(),
            keep.display()
        );
        return false;
    }
    match std::fs::remove_file(remove) {
        Ok(()) => true,
        Err(error) => {
            log::warn!("could not remove {}: {error}", remove.display());
            false
        }
    }
}

fn staging_name(dst: &Path) -> PathBuf {
    dst.with_file_name(format!(
        ".{}{STAGING_SUFFIX}",
        dst.file_name().unwrap_or_default().to_string_lossy()
    ))
}

/// Make `dst` hold the bytes of `src`.
fn place(
    src: &Path,
    dst: &Path,
    bytes: u64,
    strategy: Strategy,
    progress: &mut dyn FnMut(u64),
    cancelled: &dyn Fn() -> bool,
) -> Result<Placed> {
    // `symlink_metadata`: a link (even a dangling one) is a name, never a copy.
    match std::fs::symlink_metadata(dst) {
        Ok(meta) => {
            if !meta.is_file() || meta.len() != bytes {
                return Err(anyhow!(FolderProblem::NameTaken.code()));
            }
            // A hard link left by an earlier move is the same file; anything
            // else must match byte for byte.
            if super::is_same(src, dst) || files_identical(src, dst, progress, cancelled)? {
                return Ok(Placed::Found);
            }
            return Err(anyhow!(FolderProblem::NameTaken.code()));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("look at the new folder"),
    }
    // `hard_link` never replaces an existing name.
    if strategy == Strategy::LinkOrCopy && std::fs::hard_link(src, dst).is_ok() {
        return Ok(Placed::Linked);
    }
    let staging = staging_name(dst);
    let _ = std::fs::remove_file(&staging);
    let copied = copy_checked(src, &staging, bytes, progress, cancelled).and_then(|()| {
        // A name may have appeared while copying: never write over it.
        if std::fs::symlink_metadata(dst).is_ok() {
            return Err(anyhow!(FolderProblem::NameTaken.code()));
        }
        std::fs::rename(&staging, dst).context("rename the checked copy")
    });
    if copied.is_err() {
        let _ = std::fs::remove_file(&staging);
    }
    copied.map(|()| Placed::Copied)
}

fn cancelled_error() -> anyhow::Error {
    anyhow!("cancelled")
}

/// Copy `src` to a new file `dst`, flush it, then read the copy back and
/// compare SHA-256s. Progress counts the copy as the first half, the check
/// as the second.
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
    let (copied, expected) =
        copy_stream(&mut input, &mut output, &mut |n| progress(n / 2), cancelled)?;
    // A full disk can surface only when the data is flushed.
    output.sync_all().map_err(write_error)?;
    drop(output);
    if copied != bytes {
        return Err(anyhow!("model changed while it was being moved"));
    }
    let actual = checksum(dst, &mut |n| progress(bytes / 2 + n.div_ceil(2)), cancelled)?;
    if actual != expected {
        return Err(anyhow!("moved copy did not match the original"));
    }
    Ok(())
}

/// Copy all of `input` into `output`, returning the bytes and their SHA-256.
/// A full disk becomes [`FolderProblem::NotEnoughSpace`].
pub(crate) fn copy_stream(
    input: &mut dyn Read,
    output: &mut dyn Write,
    progress: &mut dyn FnMut(u64),
    cancelled: &dyn Fn() -> bool,
) -> Result<(u64, Vec<u8>)> {
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
        output.write_all(&buf[..n]).map_err(write_error)?;
        copied += n as u64;
        progress(copied);
    }
    output.flush().map_err(write_error)?;
    Ok((copied, hasher.finalize().to_vec()))
}

/// A full disk becomes [`FolderProblem::NotEnoughSpace`].
fn write_error(error: std::io::Error) -> anyhow::Error {
    if is_disk_full(&error) {
        anyhow!(FolderProblem::NotEnoughSpace.code())
    } else {
        anyhow!(error).context("write copy")
    }
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
    let (_, hash) = copy_stream(&mut file, &mut std::io::sink(), progress, cancelled)?;
    Ok(hash)
}

fn files_identical(
    a: &Path,
    b: &Path,
    progress: &mut dyn FnMut(u64),
    cancelled: &dyn Fn() -> bool,
) -> Result<bool> {
    let len = std::fs::metadata(a)?.len();
    if std::fs::metadata(b)?.len() != len {
        return Ok(false);
    }
    let left = checksum(a, &mut |n| progress(n / 2), cancelled)?;
    let right = checksum(b, &mut |n| progress(len / 2 + n.div_ceil(2)), cancelled)?;
    Ok(left == right)
}
