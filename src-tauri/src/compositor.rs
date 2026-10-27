//! Ask a tiling Wayland compositor to bring Rebost's window forward.
//!
//! Hyprland and sway only raise a window for an xdg-activation token, and a
//! second copy started from a launcher, a terminal, or a keybinding often has
//! none. Both take commands on a local socket, so the running copy asks them
//! directly to focus the window of its own process (switching workspace if
//! needed). Other desktops ignore this.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

const IPC_TIMEOUT: Duration = Duration::from_secs(1);

/// Title of the main window (`tauri.conf.json`); About and Update differ.
const MAIN_TITLE: &str = "Rebost";

/// Focus this process's main window through the compositor's IPC, if it has one.
pub fn focus_own_window() {
    let pid = std::process::id();
    if let Some(signature) = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE") {
        let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
        for socket in hyprland_sockets(runtime.as_deref(), Path::new(&signature)) {
            // Prefer the main window's address; `pid:` alone may pick About.
            let target = hyprland_request(&socket, "j/clients")
                .ok()
                .and_then(|clients| main_window_address(&clients, pid))
                .map_or_else(
                    || format!("pid:{pid}"),
                    |address| format!("address:{address}"),
                );
            match hyprland_request(&socket, &format!("/dispatch focuswindow {target}")) {
                Ok(reply) if reply.trim() == "ok" => return,
                Ok(reply) => log::warn!("Hyprland did not focus the window: {}", reply.trim()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => log::warn!("Hyprland IPC {}: {error}", socket.display()),
            }
        }
    }
    if let Some(socket) = std::env::var_os("SWAYSOCK") {
        let command = format!("[pid={pid} title=\"^{MAIN_TITLE}$\"] focus");
        match sway_command(Path::new(&socket), &command) {
            Ok(true) => {}
            Ok(false) => log::warn!("sway did not focus the window"),
            Err(error) => log::warn!("sway IPC: {error}"),
        }
    }
}

/// Address of this process's main window in Hyprland's `j/clients` list.
fn main_window_address(clients: &str, pid: u32) -> Option<String> {
    let clients: Vec<serde_json::Value> = serde_json::from_str(clients).ok()?;
    clients
        .iter()
        .find(|client| {
            client["pid"].as_u64() == Some(u64::from(pid))
                && client["title"].as_str() == Some(MAIN_TITLE)
        })
        .and_then(|client| client["address"].as_str())
        .map(str::to_string)
}

/// Hyprland 0.40+ keeps its socket under `XDG_RUNTIME_DIR`; older ones in /tmp.
fn hyprland_sockets(runtime: Option<&Path>, signature: &Path) -> Vec<PathBuf> {
    let mut sockets = Vec::new();
    if let Some(runtime) = runtime {
        sockets.push(runtime.join("hypr").join(signature).join(".socket.sock"));
    }
    sockets.push(Path::new("/tmp/hypr").join(signature).join(".socket.sock"));
    sockets
}

/// One request the way `hyprctl` sends it (`[flags]/command`); Hyprland
/// replies and closes the socket. `dispatch` replies `ok` on success.
fn hyprland_request(socket: &Path, request: &str) -> std::io::Result<String> {
    let mut stream = UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(IPC_TIMEOUT))?;
    stream.set_write_timeout(Some(IPC_TIMEOUT))?;
    stream.write_all(request.as_bytes())?;
    let mut reply = String::new();
    stream.read_to_string(&mut reply)?;
    Ok(reply)
}

/// i3/sway IPC: `i3-ipc`, payload length and type (0, RUN_COMMAND) as
/// native-endian u32, then the command. The reply is a JSON list of results.
fn sway_command(socket: &Path, command: &str) -> std::io::Result<bool> {
    const MAGIC: &[u8] = b"i3-ipc";
    let mut stream = UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(IPC_TIMEOUT))?;
    stream.set_write_timeout(Some(IPC_TIMEOUT))?;
    let mut message = MAGIC.to_vec();
    message.extend_from_slice(&(command.len() as u32).to_ne_bytes());
    message.extend_from_slice(&0u32.to_ne_bytes());
    message.extend_from_slice(command.as_bytes());
    stream.write_all(&message)?;
    let mut header = [0u8; 14];
    stream.read_exact(&mut header)?;
    if &header[..6] != MAGIC {
        return Ok(false);
    }
    let len = u32::from_ne_bytes([header[6], header[7], header[8], header[9]]) as usize;
    let mut reply = vec![0u8; len.min(64 * 1024)];
    stream.read_exact(&mut reply)?;
    Ok(String::from_utf8_lossy(&reply).contains("\"success\":true"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;

    #[test]
    fn hyprland_socket_follows_the_runtime_dir_then_tmp() {
        let sockets = hyprland_sockets(Some(Path::new("/run/user/1000")), Path::new("abc_1"));
        assert_eq!(
            sockets,
            vec![
                PathBuf::from("/run/user/1000/hypr/abc_1/.socket.sock"),
                PathBuf::from("/tmp/hypr/abc_1/.socket.sock"),
            ]
        );
    }

    #[test]
    fn main_window_is_found_by_pid_and_title() {
        let clients = r#"[
            {"address": "0x1", "pid": 42, "title": "About Rebost"},
            {"address": "0x2", "pid": 7, "title": "Rebost"},
            {"address": "0x3", "pid": 42, "title": "Rebost"}
        ]"#;
        assert_eq!(main_window_address(clients, 42).as_deref(), Some("0x3"));
        assert_eq!(main_window_address(clients, 9), None);
        assert_eq!(main_window_address("not json", 42), None);
    }

    #[test]
    fn hyprland_request_sends_what_hyprctl_sends() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".socket.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 128];
            let n = stream.read(&mut buf).unwrap();
            stream.write_all(b"ok").unwrap();
            String::from_utf8_lossy(&buf[..n]).to_string()
        });
        let reply = hyprland_request(&path, "/dispatch focuswindow pid:42").unwrap();
        assert_eq!(reply, "ok");
        assert_eq!(server.join().unwrap(), "/dispatch focuswindow pid:42");
    }

    #[test]
    fn sway_command_frames_the_request_and_reads_success() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sway.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut header = [0u8; 14];
            stream.read_exact(&mut header).unwrap();
            let len = u32::from_ne_bytes([header[6], header[7], header[8], header[9]]) as usize;
            let kind = u32::from_ne_bytes([header[10], header[11], header[12], header[13]]);
            let mut body = vec![0u8; len];
            stream.read_exact(&mut body).unwrap();
            let reply = br#"[{"success":true}]"#;
            let mut out = b"i3-ipc".to_vec();
            out.extend_from_slice(&(reply.len() as u32).to_ne_bytes());
            out.extend_from_slice(&0u32.to_ne_bytes());
            out.extend_from_slice(reply);
            stream.write_all(&out).unwrap();
            (kind, String::from_utf8(body).unwrap())
        });
        assert!(sway_command(&path, "[pid=42] focus").unwrap());
        assert_eq!(server.join().unwrap(), (0, "[pid=42] focus".to_string()));
    }
}
