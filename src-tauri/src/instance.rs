//! One process may own a Rebost library. The lock lives in app data so a
//! second copy of the app cannot open the same files. That copy asks the
//! existing window to come forward instead of starting ingest and the AI.

use std::fs::{File, OpenOptions, TryLockError};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

/// Exclusive lock in the app-data directory. Held for the process lifetime.
pub const LOCK_NAME: &str = "instance.lock";
const FOCUS_NAME: &str = "instance.focus";
const RELAUNCH_NAME: &str = "instance.relaunch";
const MAIN_WINDOW: &str = "main";
const RELAUNCH_WAIT: Duration = Duration::from_secs(15);
const FOCUS_POLL: Duration = Duration::from_millis(150);

/// Why a library lock could not be taken.
#[derive(Debug)]
pub enum AcquireError {
    Busy,
    Io(io::Error),
}

impl std::fmt::Display for AcquireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Busy => write!(f, "Rebost is already open"),
            Self::Io(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for AcquireError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Busy => None,
            Self::Io(error) => Some(error),
        }
    }
}

/// Guard that holds the exclusive library lock until dropped.
pub struct InstanceLock {
    file: Mutex<File>,
    data_dir: PathBuf,
}

impl InstanceLock {
    /// App-data directory this lock belongs to.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }
}

impl Drop for InstanceLock {
    fn drop(&mut self) {
        let file = self
            .file
            .get_mut()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _ = file.unlock();
    }
}

/// Take the library lock, waiting if this process was started by a relaunch.
pub fn acquire_for_launch(data_dir: &Path) -> Result<InstanceLock, AcquireError> {
    let relaunch = data_dir.join(RELAUNCH_NAME).is_file();
    let wait = if relaunch {
        RELAUNCH_WAIT
    } else {
        Duration::ZERO
    };
    let lock = acquire(data_dir, wait)?;
    let _ = std::fs::remove_file(data_dir.join(RELAUNCH_NAME));
    // A request left from an earlier session must not raise this one.
    let _ = std::fs::remove_file(data_dir.join(FOCUS_NAME));
    Ok(lock)
}

/// Non-blocking acquire. Used by seed and tests.
pub fn try_acquire(data_dir: &Path) -> Result<InstanceLock, AcquireError> {
    acquire(data_dir, Duration::ZERO)
}

fn acquire(data_dir: &Path, wait: Duration) -> Result<InstanceLock, AcquireError> {
    std::fs::create_dir_all(data_dir).map_err(AcquireError::Io)?;
    let path = data_dir.join(LOCK_NAME);
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&path)
        .map_err(AcquireError::Io)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = file.set_permissions(std::fs::Permissions::from_mode(0o600));
    }
    let deadline = Instant::now() + wait;
    loop {
        match file.try_lock() {
            Ok(()) => {
                return Ok(InstanceLock {
                    file: Mutex::new(file),
                    data_dir: data_dir.to_path_buf(),
                });
            }
            Err(TryLockError::WouldBlock) => {
                if Instant::now() >= deadline {
                    return Err(AcquireError::Busy);
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(TryLockError::Error(error)) => return Err(AcquireError::Io(error)),
        }
    }
}

/// Next launch waits for this process to exit before taking the lock.
pub fn mark_relaunch(data_dir: &Path) -> io::Result<()> {
    std::fs::create_dir_all(data_dir)?;
    std::fs::write(data_dir.join(RELAUNCH_NAME), b"")
}

/// Tell the process that holds the lock to bring its main window forward.
/// Passes on this launch's activation token: Wayland compositors only raise
/// a window for a token the user's click produced.
pub fn request_focus(data_dir: &Path) {
    let _ = std::fs::create_dir_all(data_dir);
    let token = ["XDG_ACTIVATION_TOKEN", "DESKTOP_STARTUP_ID"]
        .iter()
        .find_map(|name| std::env::var(name).ok());
    let _ = std::fs::write(
        data_dir.join(FOCUS_NAME),
        focus_request(&uuid::Uuid::new_v4().to_string(), token.as_deref()),
    );
}

fn focus_request(nonce: &str, token: Option<&str>) -> String {
    match token.and_then(clean_token) {
        Some(token) => format!("{nonce}\n{token}"),
        None => nonce.to_string(),
    }
}

/// Activation token in a focus request, if the launch had one.
fn activation_token(request: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(request).ok()?;
    clean_token(text.split_once('\n')?.1).map(str::to_string)
}

fn clean_token(token: &str) -> Option<&str> {
    let token = token.trim();
    let valid =
        !token.is_empty() && token.len() <= 512 && token.chars().all(|c| c.is_ascii_graphic());
    valid.then_some(token)
}

/// Watch for a second copy asking this window to come forward.
pub fn spawn_focus_listener(app: AppHandle, data_dir: PathBuf) {
    let path = data_dir.join(FOCUS_NAME);
    let _ = thread::Builder::new()
        .name("rebost-focus".into())
        .spawn(move || {
            let mut last = Vec::new();
            loop {
                let now = std::fs::read(&path).unwrap_or_default();
                if now != last {
                    if now.is_empty() {
                        last = now;
                    } else {
                        let handle = app.clone();
                        let token = activation_token(&now);
                        if app
                            .run_on_main_thread(move || focus_main_with(&handle, token.as_deref()))
                            .is_ok()
                        {
                            last = now;
                        }
                    }
                }
                thread::sleep(FOCUS_POLL);
            }
        });
}

/// Unminimize, show, and focus the main window.
pub fn focus_main(app: &AppHandle) {
    focus_main_with(app, None);
}

/// [`focus_main`] with the activation token of the launch that asked.
pub fn focus_main_with(app: &AppHandle, token: Option<&str>) {
    #[cfg(target_os = "macos")]
    {
        let _ = app.show();
    }
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.unminimize();
        let _ = window.show();
        #[cfg(target_os = "linux")]
        if let (Some(token), Ok(gtk_window)) = (token, window.gtk_window()) {
            // GTK hands the token to xdg-activation (Wayland) or uses it as
            // the startup id and timestamp (X11).
            use gtk::prelude::GtkWindowExt;
            gtk_window.set_startup_id(token);
        }
        #[cfg(not(target_os = "linux"))]
        let _ = token;
        let _ = window.set_focus();
        // Hyprland and sway ignore a raise without a token; ask them directly.
        #[cfg(target_os = "linux")]
        let _ = thread::Builder::new()
            .name("rebost-raise".into())
            .spawn(crate::compositor::focus_own_window);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_acquire_fails_while_the_first_is_held() {
        let dir = tempfile::tempdir().unwrap();
        let first = try_acquire(dir.path()).unwrap();
        let start = Instant::now();
        assert!(matches!(try_acquire(dir.path()), Err(AcquireError::Busy)));
        assert!(start.elapsed() < Duration::from_millis(200));
        drop(first);
        assert!(try_acquire(dir.path()).is_ok());
    }

    #[test]
    fn leftover_lock_file_does_not_block() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(LOCK_NAME), b"").unwrap();
        assert!(try_acquire(dir.path()).is_ok());
    }

    #[test]
    fn two_libraries_do_not_share_a_lock() {
        let one = tempfile::tempdir().unwrap();
        let two = tempfile::tempdir().unwrap();
        let _a = try_acquire(one.path()).unwrap();
        assert!(try_acquire(two.path()).is_ok());
    }

    #[test]
    fn request_focus_writes_a_new_token() {
        let dir = tempfile::tempdir().unwrap();
        request_focus(dir.path());
        let first = std::fs::read_to_string(dir.path().join(FOCUS_NAME)).unwrap();
        request_focus(dir.path());
        let second = std::fs::read_to_string(dir.path().join(FOCUS_NAME)).unwrap();
        assert_ne!(first, second);
        assert!(!second.is_empty());
    }

    #[test]
    fn focus_request_carries_the_activation_token() {
        let request = focus_request("nonce", Some(" gnome-shell/Rebost/1234-0-host_TIME5678 "));
        assert_eq!(
            activation_token(request.as_bytes()).as_deref(),
            Some("gnome-shell/Rebost/1234-0-host_TIME5678")
        );
        assert_eq!(focus_request("nonce", None), "nonce");
        assert_eq!(activation_token(b"nonce"), None);
        assert_eq!(focus_request("nonce", Some("bad token\n")), "nonce");
        assert_eq!(activation_token(b"nonce\n"), None);
    }

    #[test]
    fn relaunch_waits_until_the_lock_is_free() {
        let dir = tempfile::tempdir().unwrap();
        let first = try_acquire(dir.path()).unwrap();
        mark_relaunch(dir.path()).unwrap();
        let data = dir.path().to_path_buf();
        let waiter = thread::spawn(move || acquire_for_launch(&data));
        thread::sleep(Duration::from_millis(80));
        drop(first);
        waiter
            .join()
            .expect("relaunch waiter")
            .expect("lock after relaunch");
        assert!(!dir.path().join(RELAUNCH_NAME).exists());
    }
}
