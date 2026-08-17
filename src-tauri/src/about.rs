//! Custom About window. The desktop menu that opens it lives in `menu.rs`.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt;

use crate::commands::{friendly, CmdResult};

const ABOUT_LABEL: &str = "about";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutInfo {
    pub version: String,
    pub repository_url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExternalLink {
    Repository,
}

impl ExternalLink {
    fn url(self) -> &'static str {
        match self {
            ExternalLink::Repository => env!("CARGO_PKG_REPOSITORY"),
        }
    }
}

/// Version and source URL for the About window.
#[tauri::command]
pub fn about_info(app: AppHandle) -> AboutInfo {
    AboutInfo {
        version: app.package_info().version.to_string(),
        repository_url: env!("CARGO_PKG_REPOSITORY").to_string(),
    }
}

/// Open or focus the About window.
#[tauri::command]
pub async fn show_about_window(app: AppHandle) -> CmdResult<()> {
    open(&app)
}

/// Open an allowlisted URL in the system browser.
#[tauri::command]
pub fn open_external(app: AppHandle, link: ExternalLink) -> CmdResult<()> {
    app.opener()
        .open_url(link.url(), None::<String>)
        .map_err(friendly)
}

pub(crate) fn open(app: &AppHandle) -> CmdResult<()> {
    if let Some(existing) = app.get_webview_window(ABOUT_LABEL) {
        let _ = existing.unminimize();
        let _ = existing.show();
        let _ = existing.set_focus();
        return Ok(());
    }
    create(app).map_err(friendly)
}

fn create(app: &AppHandle) -> tauri::Result<()> {
    let builder = WebviewWindowBuilder::new(
        app,
        ABOUT_LABEL,
        WebviewUrl::App("index.html?window=about".into()),
    )
    .title("About Rebost")
    .inner_size(400.0, 356.0)
    .min_inner_size(400.0, 356.0)
    .max_inner_size(400.0, 356.0)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .skip_taskbar(true)
    .accept_first_mouse(true)
    .center()
    .focused(true);

    #[cfg(windows)]
    let builder = builder.drag_and_drop(false);

    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);

    builder.build()?;
    Ok(())
}
