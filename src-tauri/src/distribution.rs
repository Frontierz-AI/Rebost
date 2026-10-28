//! Who updates this copy of Rebost.
//!
//! The in-app updater installs a release the way each GitHub package does
//! (`pkexec dpkg -i`, `rpm -U`, a new AppImage file). A copy a package manager
//! installed must leave that to the package manager: inside a Flatpak the app
//! cannot replace itself, and over an AUR install `dpkg` would write files
//! pacman owns. Those copies never ask GitHub for `latest.json`, so its
//! `linux-*` keys are only ever read by the GitHub packages.
//!
//! - Flatpak: the sandbox always has `/.flatpak-info`. `FLATPAK_ID` is not
//!   used: a program started on the host from a sandboxed terminal or editor
//!   can inherit it.
//! - Repackaged builds (the AUR's `rebost-bin`) ship a [`MARKER_FILE`] next
//!   to the bundled resources (`/usr/lib/Rebost/package-manager`). Its first
//!   line names the package manager for the log.

use std::path::Path;
use std::sync::OnceLock;

/// File a repackaged build installs in Tauri's resource folder.
pub const MARKER_FILE: &str = "package-manager";

const FLATPAK_INFO: &str = "/.flatpak-info";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateSource {
    /// GitHub Releases, through the in-app updater.
    App,
    /// Flatpak: GNOME Software, KDE Discover, or `flatpak update`.
    SoftwareCentre,
    /// A system package manager, named by the marker file.
    PackageManager(String),
}

impl UpdateSource {
    /// Value the About window receives.
    pub fn kind(&self) -> &'static str {
        match self {
            UpdateSource::App => "app",
            UpdateSource::SoftwareCentre => "softwareCentre",
            UpdateSource::PackageManager(_) => "packageManager",
        }
    }

    /// Who installs updates, when it is not Rebost itself.
    pub fn manager(&self) -> Option<&str> {
        match self {
            UpdateSource::App => None,
            UpdateSource::SoftwareCentre => Some("Flatpak"),
            UpdateSource::PackageManager(name) => Some(name),
        }
    }
}

/// Detected once per launch.
pub fn current(app: &tauri::AppHandle) -> &'static UpdateSource {
    static SOURCE: OnceLock<UpdateSource> = OnceLock::new();
    SOURCE.get_or_init(|| {
        if !cfg!(target_os = "linux") {
            return UpdateSource::App;
        }
        use tauri::Manager;
        let resource_dir = app.path().resource_dir().ok();
        detect(Path::new(FLATPAK_INFO), resource_dir.as_deref())
    })
}

fn detect(flatpak_info: &Path, resource_dir: Option<&Path>) -> UpdateSource {
    if flatpak_info.is_file() {
        return UpdateSource::SoftwareCentre;
    }
    let Some(marker) = resource_dir
        .map(|dir| dir.join(MARKER_FILE))
        .filter(|path| path.is_file())
    else {
        return UpdateSource::App;
    };
    // An unreadable marker still means someone packaged this copy.
    let text = std::fs::read_to_string(&marker).unwrap_or_default();
    let name = text.lines().next().unwrap_or("").trim();
    let name = if name.is_empty() {
        "the package manager"
    } else {
        name
    };
    UpdateSource::PackageManager(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_packages_update_in_app() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("flatpak-info");
        assert_eq!(detect(&missing, Some(dir.path())), UpdateSource::App);
        assert_eq!(detect(&missing, None), UpdateSource::App);
        assert_eq!(UpdateSource::App.manager(), None);
        assert_eq!(UpdateSource::App.kind(), "app");
    }

    #[test]
    fn flatpak_sandbox_leaves_updates_to_the_software_centre() {
        let dir = tempfile::tempdir().unwrap();
        let info = dir.path().join("flatpak-info");
        std::fs::write(&info, "[Application]\nname=ai.rebost.Rebost\n").unwrap();
        // Flatpak wins over a marker: the sandbox is what decides.
        std::fs::write(dir.path().join(MARKER_FILE), "pacman\n").unwrap();
        let source = detect(&info, Some(dir.path()));
        assert_eq!(source, UpdateSource::SoftwareCentre);
        assert_eq!(source.kind(), "softwareCentre");
        assert_eq!(source.manager(), Some("Flatpak"));
    }

    #[test]
    fn marker_file_names_the_package_manager() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("flatpak-info");
        std::fs::write(
            dir.path().join(MARKER_FILE),
            "pacman (AUR rebost-bin)\nextra\n",
        )
        .unwrap();
        let source = detect(&missing, Some(dir.path()));
        assert_eq!(
            source,
            UpdateSource::PackageManager("pacman (AUR rebost-bin)".into())
        );
        assert_eq!(source.kind(), "packageManager");
        assert_eq!(source.manager(), Some("pacman (AUR rebost-bin)"));
    }

    #[test]
    fn empty_marker_still_counts() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("flatpak-info");
        std::fs::write(dir.path().join(MARKER_FILE), "\n").unwrap();
        assert_eq!(
            detect(&missing, Some(dir.path())),
            UpdateSource::PackageManager("the package manager".into())
        );
    }

    #[test]
    fn only_a_marker_file_counts() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("flatpak-info");
        std::fs::create_dir(dir.path().join(MARKER_FILE)).unwrap();
        assert_eq!(detect(&missing, Some(dir.path())), UpdateSource::App);
        // A resource folder that is not a folder has no marker in it.
        let file = dir.path().join("not-a-folder");
        std::fs::write(&file, "").unwrap();
        assert_eq!(detect(&missing, Some(&file)), UpdateSource::App);
    }

    #[test]
    fn a_folder_named_like_flatpak_info_is_not_a_sandbox() {
        let dir = tempfile::tempdir().unwrap();
        let info = dir.path().join("flatpak-info");
        std::fs::create_dir(&info).unwrap();
        assert_eq!(detect(&info, None), UpdateSource::App);
    }
}
