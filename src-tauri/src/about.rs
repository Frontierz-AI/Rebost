//! Custom About window and the desktop menu that opens it.
//!
//! Replaces the system About panel so we can describe Rebost and link to
//! the source repository.

use serde::{Deserialize, Serialize};
use tauri::menu::{Menu, MenuEvent, MenuItem, Submenu};
#[cfg(target_os = "macos")]
use tauri::menu::{PredefinedMenuItem, WINDOW_SUBMENU_ID};
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

/// App menu. macOS matches the usual desktop shape; other platforms get Help.
#[cfg(target_os = "macos")]
pub fn build_menu(handle: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    build_macos_menu(handle)
}

#[cfg(not(target_os = "macos"))]
pub fn build_menu(handle: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let about = MenuItem::with_id(handle, ABOUT_LABEL, "&About Rebost", true, None::<&str>)?;
    Menu::with_items(
        handle,
        &[&Submenu::with_items(handle, "&Help", true, &[&about])?],
    )
}

/// macOS menubar: same shape as Tauri's default, with our About item.
#[cfg(target_os = "macos")]
fn build_macos_menu(handle: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let pkg_info = handle.package_info();
    let about = MenuItem::with_id(handle, ABOUT_LABEL, "About Rebost", true, None::<&str>)?;

    let window_menu = Submenu::with_id_and_items(
        handle,
        WINDOW_SUBMENU_ID,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(handle, None)?,
            &PredefinedMenuItem::maximize(handle, None)?,
            &PredefinedMenuItem::separator(handle)?,
            &PredefinedMenuItem::close_window(handle, None)?,
        ],
    )?;

    Menu::with_items(
        handle,
        &[
            &Submenu::with_items(
                handle,
                pkg_info.name.clone(),
                true,
                &[
                    &about,
                    &PredefinedMenuItem::separator(handle)?,
                    &PredefinedMenuItem::services(handle, None)?,
                    &PredefinedMenuItem::separator(handle)?,
                    &PredefinedMenuItem::hide(handle, None)?,
                    &PredefinedMenuItem::hide_others(handle, None)?,
                    &PredefinedMenuItem::show_all(handle, None)?,
                    &PredefinedMenuItem::separator(handle)?,
                    &PredefinedMenuItem::quit(handle, None)?,
                ],
            )?,
            &Submenu::with_items(
                handle,
                "File",
                true,
                &[&PredefinedMenuItem::close_window(handle, None)?],
            )?,
            &Submenu::with_items(
                handle,
                "Edit",
                true,
                &[
                    &PredefinedMenuItem::undo(handle, None)?,
                    &PredefinedMenuItem::redo(handle, None)?,
                    &PredefinedMenuItem::separator(handle)?,
                    &PredefinedMenuItem::cut(handle, None)?,
                    &PredefinedMenuItem::copy(handle, None)?,
                    &PredefinedMenuItem::paste(handle, None)?,
                    &PredefinedMenuItem::select_all(handle, None)?,
                ],
            )?,
            &Submenu::with_items(
                handle,
                "View",
                true,
                &[&PredefinedMenuItem::fullscreen(handle, None)?],
            )?,
            &window_menu,
        ],
    )
}

pub fn on_menu_event(app: &AppHandle, event: &MenuEvent) {
    if event.id() != ABOUT_LABEL {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = open(&app) {
            log::error!("about window: {error}");
        }
    });
}

#[tauri::command]
pub fn about_info(app: AppHandle) -> AboutInfo {
    AboutInfo {
        version: app.package_info().version.to_string(),
        repository_url: env!("CARGO_PKG_REPOSITORY").to_string(),
    }
}

#[tauri::command]
pub async fn show_about_window(app: AppHandle) -> CmdResult<()> {
    open(&app)
}

#[tauri::command]
pub fn open_external(app: AppHandle, link: ExternalLink) -> CmdResult<()> {
    app.opener()
        .open_url(link.url(), None::<String>)
        .map_err(friendly)
}

fn open(app: &AppHandle) -> CmdResult<()> {
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
