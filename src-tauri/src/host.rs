//! Start programs that belong to the desktop, not to Rebost.
//!
//! An AppImage runs with its own library and GTK paths in the environment
//! (`LD_LIBRARY_PATH`, `GTK_PATH`, `GDK_PIXBUF_MODULE_FILE`, `XDG_DATA_DIRS`,
//! …) pointing into its mount. A document viewer or file manager started with
//! those loads the AppImage's older GLib and dies before showing a window,
//! while `xdg-open` still reports success. Children get the desktop's own
//! environment back, and opening a file on Linux waits long enough to report
//! an opener that fails.

use std::ffi::{OsStr, OsString};
use std::process::Command;

/// Set by the AppImage GTK hook for the bundled GTK only. Children never need
/// them, and the hook overwrites any value the desktop had.
const APPIMAGE_ONLY: &[&str] = &[
    "APPDIR",
    "APPIMAGE",
    "ARGV0",
    "OWD",
    "GDK_BACKEND",
    "GTK_THEME",
    "GTK_DATA_PREFIX",
    "GTK_EXE_PREFIX",
    "GTK_PATH",
    "GTK_IM_MODULE_FILE",
    "GDK_PIXBUF_MODULE_FILE",
    "GSETTINGS_SCHEMA_DIR",
    "GI_TYPELIB_PATH",
    "GIO_EXTRA_MODULES",
];

/// Mount point of the running AppImage, if this is one.
fn appimage_dir() -> Option<String> {
    std::env::var_os("APPIMAGE")?;
    if let Some(dir) = std::env::var("APPDIR").ok().filter(|dir| !dir.is_empty()) {
        return Some(dir);
    }
    // `<APPDIR>/usr/bin/rebost`
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?.parent()?.parent()?;
    Some(dir.to_str()?.to_string())
}

/// What to change in `vars` so a program outside the AppImage at `appdir`
/// sees the desktop's environment: `None` removes the variable.
pub(crate) fn desktop_env_changes(
    vars: impl IntoIterator<Item = (OsString, OsString)>,
    appdir: &str,
) -> Vec<(OsString, Option<OsString>)> {
    let appdir = appdir.trim_end_matches('/');
    if appdir.is_empty() {
        return Vec::new();
    }
    let inside = |entry: &str| {
        entry == appdir
            || entry
                .strip_prefix(appdir)
                .is_some_and(|rest| rest.starts_with('/'))
    };
    let mut changes = Vec::new();
    for (key, value) in vars {
        if APPIMAGE_ONLY.iter().any(|name| key == OsStr::new(name)) {
            changes.push((key, None));
            continue;
        }
        let Some(text) = value.to_str() else {
            continue;
        };
        if key == OsStr::new("XDG_DATA_DIRS") {
            // The hooks prepend the mount's share folders and `/usr/share`
            // (`$APPDIR/usr/share/:$APPDIR/usr/share:/usr/share:<desktop>`).
            // What follows is the desktop's value, empty when it relied on
            // the default.
            let mut rest = text;
            let mut prepended = false;
            while let Some((entry, tail)) = rest.split_once(':') {
                if !inside(entry) {
                    break;
                }
                rest = tail;
                prepended = true;
            }
            if prepended {
                if let Some(tail) = rest.strip_prefix("/usr/share:") {
                    let desktop = (!tail.is_empty()).then(|| tail.into());
                    changes.push((key, desktop));
                    continue;
                }
            }
        }
        if !text.split(':').any(inside) {
            continue;
        }
        let kept: Vec<&str> = text
            .split(':')
            .filter(|entry| !entry.is_empty() && !inside(entry))
            .collect();
        if kept.is_empty() {
            changes.push((key, None));
        } else {
            changes.push((key, Some(kept.join(":").into())));
        }
    }
    changes
}

/// Prepare a program Rebost starts for the desktop: on Linux it gets no open
/// files past stdin/stdout/stderr, and as an AppImage the desktop's
/// environment instead of the AppImage's.
pub fn use_desktop_env(command: &mut Command) {
    #[cfg(target_os = "linux")]
    close_inherited_fds(command);
    let Some(appdir) = appimage_dir() else {
        return;
    };
    for (key, value) in desktop_env_changes(std::env::vars_os(), &appdir) {
        match value {
            Some(value) => command.env(key, value),
            None => command.env_remove(key),
        };
    }
}

/// Mark every file descriptor from 3 up close-on-exec in the child, so the
/// program gets none of Rebost's. An AppImage process holds its mount's root
/// open without close-on-exec (as do some GPU and cgroup files); a restarted
/// AppImage that inherited it kept the old mount busy, so the old AppImage
/// could never unmount and exit.
#[cfg(target_os = "linux")]
fn close_inherited_fds(command: &mut Command) {
    use std::os::unix::process::CommandExt;

    // SAFETY: sysconf has no preconditions. Read before fork: the child may
    // only make async-signal-safe calls.
    let open_max = unsafe { libc::sysconf(libc::_SC_OPEN_MAX) };
    let limit = if open_max > 0 {
        open_max.min(1 << 16) as libc::c_int
    } else {
        1024
    };
    let hook = move || {
        // SAFETY: close_range and fcntl are async-signal-safe and change only
        // this forked child's descriptor flags. Descriptors 0-2 are already
        // the program's stdio.
        unsafe {
            let first: libc::c_uint = 3;
            if libc::syscall(
                libc::SYS_close_range,
                first,
                libc::c_uint::MAX,
                libc::CLOSE_RANGE_CLOEXEC,
            ) == 0
            {
                return Ok(());
            }
            // Kernels before 5.11 (older than any supported distribution):
            // descriptors up to the open-file limit, capped at 65536.
            for fd in 3..limit {
                let flags = libc::fcntl(fd, libc::F_GETFD);
                if flags >= 0 && flags & libc::FD_CLOEXEC == 0 {
                    libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC);
                }
            }
        }
        Ok(())
    };
    // SAFETY: the hook only makes async-signal-safe system calls.
    unsafe {
        command.pre_exec(hook);
    }
}

/// Quit and start Rebost again. An AppImage starts its file afresh with the
/// desktop's environment: Tauri's restart passes on this mount's library
/// paths, so the new copy kept loading from the old mount and the old
/// AppImage could never unmount and exit.
pub fn restart(app: &tauri::AppHandle) {
    #[cfg(target_os = "linux")]
    if let Some(appimage) = std::env::var_os("APPIMAGE") {
        use std::os::unix::process::CommandExt;
        let mut command = Command::new(&appimage);
        command
            .args(std::env::args_os().skip(1))
            .stdin(std::process::Stdio::null())
            .process_group(0);
        if let Some(home) = dirs::home_dir() {
            command.current_dir(home);
        }
        use_desktop_env(&mut command);
        match command.spawn() {
            Ok(_) => {
                app.exit(0);
                return;
            }
            Err(error) => log::error!("restart {}: {error}", appimage.to_string_lossy()),
        }
    }
    app.request_restart();
}

/// Open a file, folder, or URL in the desktop's default app.
pub async fn open(app: &tauri::AppHandle, target: String) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let _ = app;
        tauri::async_runtime::spawn_blocking(move || open_on_linux(&target))
            .await
            .map_err(crate::commands::friendly)?
    }
    #[cfg(not(target_os = "linux"))]
    {
        use tauri_plugin_opener::OpenerExt;
        let opener = app.opener();
        if target.contains("://") {
            opener.open_url(target, None::<String>)
        } else {
            opener.open_path(target, None::<String>)
        }
        .map_err(crate::commands::friendly)
    }
}

/// Seconds `xdg-open` gets to fail before the open counts as started. A
/// handler that keeps running (some open the app in the foreground) is fine.
#[cfg(target_os = "linux")]
const OPENER_WAIT: std::time::Duration = std::time::Duration::from_secs(4);

#[cfg(target_os = "linux")]
fn open_on_linux(target: &str) -> Result<(), String> {
    use std::process::Stdio;
    use std::time::Instant;

    let failed = |reason: String| {
        log::error!("open {target}: {reason}");
        Err(rust_i18n::t!("errors.openFailed").to_string())
    };
    let mut last_error = None;
    let mut child = None;
    for (program, args) in [("xdg-open", &[][..]), ("gio", &["open"][..])] {
        let mut command = Command::new(program);
        command
            .args(args)
            .arg(target)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            // Not a pipe: the app the opener starts may inherit it.
            .stderr(Stdio::null());
        if let Some(home) = dirs::home_dir() {
            command.current_dir(home);
        }
        use_desktop_env(&mut command);
        match command.spawn() {
            Ok(spawned) => {
                child = Some((program, spawned));
                break;
            }
            Err(error) => last_error = Some(format!("{program}: {error}")),
        }
    }
    let Some((program, mut child)) = child else {
        return failed(last_error.unwrap_or_default());
    };
    let deadline = Instant::now() + OPENER_WAIT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => {
                return failed(format!(
                    "{program} exited with {status} ({})",
                    opener_failure(status.code())
                ));
            }
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Ok(None) => {
                let _ = std::thread::Builder::new()
                    .name("rebost-opener".into())
                    .spawn(move || {
                        let _ = child.wait();
                    });
                return Ok(());
            }
            Err(error) => return failed(format!("{program}: {error}")),
        }
    }
}

/// What an `xdg-open` exit code means (xdg-utils manual).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn opener_failure(code: Option<i32>) -> &'static str {
    match code {
        Some(1) => "bad arguments",
        Some(2) => "file not found",
        Some(3) => "no opener tool installed",
        Some(4) => "no app for this type, or the app failed",
        _ => "unknown failure",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
        pairs
            .iter()
            .map(|(key, value)| (OsString::from(key), OsString::from(value)))
            .collect()
    }

    fn change<'a>(
        changes: &'a [(OsString, Option<OsString>)],
        key: &str,
    ) -> Option<&'a Option<OsString>> {
        changes
            .iter()
            .find(|(name, _)| name == OsStr::new(key))
            .map(|(_, value)| value)
    }

    #[test]
    fn appimage_paths_are_dropped_and_desktop_paths_kept() {
        let mount = "/tmp/.mount_RebostAb12";
        let changes = desktop_env_changes(
            env(&[
                ("LD_LIBRARY_PATH", "/tmp/.mount_RebostAb12/usr/lib"),
                (
                    "XDG_DATA_DIRS",
                    "/tmp/.mount_RebostAb12/usr/share:/usr/share:/usr/local/share:/var/lib/flatpak/exports/share",
                ),
                ("PATH", "/tmp/.mount_RebostAb12/usr/bin:/usr/bin:/bin"),
                ("GTK_PATH", "/tmp/.mount_RebostAb12//usr/lib/gtk-3.0"),
                ("GDK_BACKEND", "x11"),
                ("APPIMAGE", "/home/a/Rebost.AppImage"),
                ("HOME", "/home/a"),
                ("XDG_CONFIG_DIRS", "/etc/xdg"),
            ]),
            mount,
        );
        assert_eq!(change(&changes, "LD_LIBRARY_PATH"), Some(&None));
        assert_eq!(
            change(&changes, "XDG_DATA_DIRS"),
            Some(&Some(
                "/usr/local/share:/var/lib/flatpak/exports/share".into()
            ))
        );
        assert_eq!(
            change(&changes, "PATH"),
            Some(&Some("/usr/bin:/bin".into()))
        );
        assert_eq!(change(&changes, "GTK_PATH"), Some(&None));
        assert_eq!(change(&changes, "GDK_BACKEND"), Some(&None));
        assert_eq!(change(&changes, "APPIMAGE"), Some(&None));
        assert_eq!(change(&changes, "HOME"), None);
        assert_eq!(change(&changes, "XDG_CONFIG_DIRS"), None);
    }

    #[test]
    fn xdg_data_dirs_comes_back_exactly_as_the_desktop_had_it() {
        let changes = desktop_env_changes(
            env(&[(
                "XDG_DATA_DIRS",
                "/tmp/.mount_RebostNM/usr/share/:/tmp/.mount_RebostNM/usr/share:/usr/share:/usr/local/share:/usr/share",
            )]),
            "/tmp/.mount_RebostNM",
        );
        assert_eq!(
            change(&changes, "XDG_DATA_DIRS"),
            Some(&Some("/usr/local/share:/usr/share".into()))
        );
    }

    #[test]
    fn unset_xdg_data_dirs_goes_back_to_unset() {
        let changes = desktop_env_changes(
            env(&[("XDG_DATA_DIRS", "/tmp/.mount_Rb/usr/share:/usr/share:")]),
            "/tmp/.mount_Rb",
        );
        assert_eq!(change(&changes, "XDG_DATA_DIRS"), Some(&None));
    }

    #[test]
    fn a_sibling_folder_with_the_same_prefix_is_not_the_mount() {
        let changes = desktop_env_changes(
            env(&[("XDG_DATA_DIRS", "/tmp/.mount_RebostAb12x/share:/usr/share")]),
            "/tmp/.mount_RebostAb12/",
        );
        assert!(changes.is_empty());
    }

    #[test]
    fn extracted_appimage_dir_is_handled_like_the_mount() {
        let changes = desktop_env_changes(
            env(&[(
                "LD_LIBRARY_PATH",
                "/tmp/appimage_extracted_0f1e/usr/lib:/opt/cuda/lib64",
            )]),
            "/tmp/appimage_extracted_0f1e",
        );
        assert_eq!(
            change(&changes, "LD_LIBRARY_PATH"),
            Some(&Some("/opt/cuda/lib64".into()))
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn children_get_no_descriptors_past_stdio() {
        // SAFETY: open a directory without O_CLOEXEC and move it to a free
        // descriptor of 900 or more (F_DUPFD never replaces an open one).
        let fd = unsafe {
            let raw = libc::open(c"/".as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY);
            assert!(raw >= 0);
            let parked = libc::fcntl(raw, libc::F_DUPFD, 900);
            libc::close(raw);
            parked
        };
        assert!(fd >= 900);
        let name = fd.to_string();
        let listed = |prepare: bool| {
            let mut command = Command::new("sh");
            command.args(["-c", "ls /proc/self/fd"]);
            if prepare {
                close_inherited_fds(&mut command);
            }
            let output = command.output().unwrap();
            String::from_utf8(output.stdout)
                .unwrap()
                .lines()
                .any(|line| line.trim() == name)
        };
        assert!(
            listed(false),
            "control: the descriptor is inherited by default"
        );
        assert!(!listed(true));
        // SAFETY: close the descriptor this test opened.
        unsafe {
            libc::close(fd);
        }
    }

    #[test]
    fn nothing_changes_without_an_appimage_dir() {
        assert!(desktop_env_changes(env(&[("GDK_BACKEND", "x11")]), "").is_empty());
    }
}
