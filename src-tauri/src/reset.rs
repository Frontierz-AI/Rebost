//! Wipe Rebost app data back to first-run.
//!
//! The running app holds file handles (Tantivy, the engine, logs), so the
//! command only writes a marker and restarts. `setup` applies the wipe
//! before anything in app data is opened. Shelf folders outside app data
//! (the default is `Documents/Rebost`) are not deleted.

use std::path::{Path, PathBuf};
use std::time::Duration;

/// Confirmation the UI requires before invoking the reset command.
pub const CONFIRMATION: &str = "DELETE";

pub const MARKER_NAME: &str = ".reset-pending";

pub fn marker_path(data_dir: &Path) -> PathBuf {
    data_dir.join(MARKER_NAME)
}

pub fn mark_pending(data_dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(data_dir)?;
    std::fs::write(marker_path(data_dir), b"")
}

pub fn is_pending(data_dir: &Path) -> bool {
    marker_path(data_dir).is_file()
}

/// Cache, logs, and OS-specific extras next to the app-data directory.
/// Extra paths are deleted only when they contain the bundle identifier.
pub fn extra_paths(
    identifier: &str,
    cache: Option<PathBuf>,
    logs: Option<PathBuf>,
    local_data: Option<PathBuf>,
) -> Vec<PathBuf> {
    let mut extras = Vec::new();
    extras.extend(cache);
    extras.extend(logs);
    extras.extend(local_data);
    extras.extend(platform_extras(identifier));
    extras
}

#[cfg(target_os = "macos")]
fn platform_extras(identifier: &str) -> Vec<PathBuf> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    vec![
        home.join("Library/WebKit").join(identifier),
        home.join("Library/Preferences")
            .join(format!("{identifier}.plist")),
        home.join("Library/Saved Application State")
            .join(format!("{identifier}.savedState")),
    ]
}

#[cfg(not(target_os = "macos"))]
fn platform_extras(_identifier: &str) -> Vec<PathBuf> {
    Vec::new()
}

/// True when a path looks like Rebost app state, not a user folder.
pub fn is_safe_reset_target(path: &Path, identifier: &str) -> bool {
    if identifier.is_empty() || path.parent().is_none() {
        return false;
    }
    path.components()
        .any(|c| c.as_os_str().to_string_lossy().contains(identifier))
}

/// If a reset was requested on the previous run, delete app data and extras.
/// Returns whether a wipe ran.
pub fn apply_pending(
    data_dir: &Path,
    extras: &[PathBuf],
    identifier: &str,
) -> std::io::Result<bool> {
    if !is_pending(data_dir) {
        return Ok(false);
    }
    wipe_dir_contents(data_dir, &[MARKER_NAME])?;
    for extra in extras {
        if extra == data_dir || extra.starts_with(data_dir) {
            continue;
        }
        if !is_safe_reset_target(extra, identifier) {
            log::warn!("skipping reset path {}", extra.display());
            continue;
        }
        if let Err(error) = remove_path_retry(extra) {
            log::warn!("could not remove {}: {error}", extra.display());
        }
    }
    let marker = marker_path(data_dir);
    if marker.exists() {
        std::fs::remove_file(&marker)?;
    }
    Ok(true)
}

fn wipe_dir_contents(dir: &Path, keep_names: &[&str]) -> std::io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        if keep_names.iter().any(|keep| name == *keep) {
            continue;
        }
        remove_path_retry(&entry.path())?;
    }
    Ok(())
}

fn remove_path_retry(path: &Path) -> std::io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    }
    let mut last_error = None;
    for attempt in 0..10 {
        match remove_path(path) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                last_error = Some(error);
                std::thread::sleep(Duration::from_millis(40 * (attempt + 1)));
            }
        }
    }
    Err(last_error.unwrap_or_else(|| std::io::Error::other("reset path still in use")))
}

fn remove_path(path: &Path) -> std::io::Result<()> {
    let meta = std::fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() || meta.is_file() {
        std::fs::remove_file(path)
    } else {
        std::fs::remove_dir_all(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn populated_data_dir() -> (tempfile::TempDir, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join("appdata");
        std::fs::create_dir_all(data.join("models")).unwrap();
        std::fs::write(data.join("settings.json"), b"{\"onboardingDone\":true}").unwrap();
        std::fs::write(data.join("models").join("model.gguf"), b"gguf").unwrap();
        (root, data)
    }

    #[test]
    fn apply_pending_is_noop_without_marker() {
        let (_root, data) = populated_data_dir();
        assert!(!apply_pending(&data, &[], "io.rebost.app").unwrap());
        assert!(data.join("settings.json").is_file());
    }

    #[test]
    fn apply_pending_wipes_app_data_and_removes_marker() {
        let (_root, data) = populated_data_dir();
        mark_pending(&data).unwrap();
        assert!(apply_pending(&data, &[], "io.rebost.app").unwrap());
        assert!(!data.join("settings.json").exists());
        assert!(!data.join("models").exists());
        assert!(!marker_path(&data).exists());
        assert!(data.is_dir());
    }

    #[test]
    fn apply_pending_leaves_files_outside_app_data() {
        let (root, data) = populated_data_dir();
        let documents = root.path().join("Documents").join("Rebost").join("Notes");
        std::fs::create_dir_all(&documents).unwrap();
        let kept = documents.join("letter.txt");
        std::fs::write(&kept, b"keep me").unwrap();
        mark_pending(&data).unwrap();
        apply_pending(&data, &[], "io.rebost.app").unwrap();
        assert_eq!(std::fs::read_to_string(&kept).unwrap(), "keep me");
    }

    #[test]
    fn extras_with_identifier_are_removed() {
        let (_root, data) = populated_data_dir();
        let cache_root = tempfile::tempdir().unwrap();
        let cache = cache_root.path().join("io.rebost.app");
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::write(cache.join("blob"), b"x").unwrap();
        mark_pending(&data).unwrap();
        apply_pending(&data, std::slice::from_ref(&cache), "io.rebost.app").unwrap();
        assert!(!cache.exists());
    }

    #[test]
    fn extras_without_identifier_are_left_alone() {
        let (_root, data) = populated_data_dir();
        let other_root = tempfile::tempdir().unwrap();
        let other = other_root.path().join("Documents");
        std::fs::create_dir_all(&other).unwrap();
        let kept = other.join("photo.jpg");
        std::fs::write(&kept, b"img").unwrap();
        mark_pending(&data).unwrap();
        apply_pending(&data, std::slice::from_ref(&other), "io.rebost.app").unwrap();
        assert!(kept.is_file());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_in_app_data_is_unlinked_not_followed() {
        let (_root, data) = populated_data_dir();
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("real-file");
        std::fs::write(&target, b"untouched").unwrap();
        let link = data.join("sneaky");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        mark_pending(&data).unwrap();
        apply_pending(&data, &[], "io.rebost.app").unwrap();
        assert!(!link.exists());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "untouched");
    }

    #[test]
    fn confirmation_is_delete() {
        assert_eq!(CONFIRMATION, "DELETE");
    }

    #[test]
    fn unsafe_root_paths_are_rejected() {
        assert!(!is_safe_reset_target(Path::new("/"), "io.rebost.app"));
        assert!(!is_safe_reset_target(
            Path::new("/Users/someone/Documents"),
            "io.rebost.app"
        ));
        assert!(is_safe_reset_target(
            Path::new("/Users/someone/Library/Application Support/io.rebost.app"),
            "io.rebost.app"
        ));
    }
}
