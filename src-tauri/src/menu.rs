//! macOS app menu: About, Settings, new and search conversations, view shortcuts, and
//! text size. Windows and Linux have no menu bar: the webview handles the same
//! shortcuts, and About is in Settings.

use serde::Serialize;
use tauri::menu::MenuEvent;
#[cfg(target_os = "macos")]
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu, WINDOW_SUBMENU_ID};
use tauri::{AppHandle, Emitter};

#[cfg(target_os = "macos")]
fn tr(key: &str) -> String {
    rust_i18n::t!(key).into()
}

const ABOUT: &str = "about";
const VIEW_SETTINGS: &str = "view-settings";
const NEW_CONVERSATION: &str = "new-conversation";
const SEARCH_CONVERSATIONS: &str = "search-conversations";
const VIEW_CHAT: &str = "view-chat";
const VIEW_SHELVES: &str = "view-shelves";
const VIEW_RECIPES: &str = "view-recipes";
const TEXT_LARGER: &str = "text-larger";
const TEXT_SMALLER: &str = "text-smaller";

#[derive(Clone, Serialize)]
struct MenuAction {
    action: &'static str,
}

#[cfg(target_os = "macos")]
pub fn build_menu(handle: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let pkg_info = handle.package_info();
    let about = MenuItem::with_id(handle, ABOUT, tr("menu.about"), true, None::<&str>)?;
    let settings = MenuItem::with_id(
        handle,
        VIEW_SETTINGS,
        tr("menu.settings"),
        true,
        Some("CmdOrCtrl+,"),
    )?;
    let new_conversation = MenuItem::with_id(
        handle,
        NEW_CONVERSATION,
        tr("menu.newConversation"),
        true,
        Some("CmdOrCtrl+N"),
    )?;
    let search_conversations = MenuItem::with_id(
        handle,
        SEARCH_CONVERSATIONS,
        tr("menu.searchConversations"),
        true,
        Some("CmdOrCtrl+Shift+F"),
    )?;
    let chat = MenuItem::with_id(
        handle,
        VIEW_CHAT,
        tr("menu.chat"),
        true,
        Some("CmdOrCtrl+1"),
    )?;
    let shelves = MenuItem::with_id(
        handle,
        VIEW_SHELVES,
        tr("menu.shelves"),
        true,
        Some("CmdOrCtrl+2"),
    )?;
    let recipes = MenuItem::with_id(
        handle,
        VIEW_RECIPES,
        tr("menu.recipes"),
        true,
        Some("CmdOrCtrl+3"),
    )?;
    let text_larger = MenuItem::with_id(
        handle,
        TEXT_LARGER,
        tr("menu.largerText"),
        true,
        Some("CmdOrCtrl+Plus"),
    )?;
    let text_smaller = MenuItem::with_id(
        handle,
        TEXT_SMALLER,
        tr("menu.smallerText"),
        true,
        Some("CmdOrCtrl+Minus"),
    )?;

    let window_menu = Submenu::with_id_and_items(
        handle,
        WINDOW_SUBMENU_ID,
        tr("menu.window"),
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
                    &settings,
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
                tr("menu.file"),
                true,
                &[
                    &new_conversation,
                    &search_conversations,
                    &PredefinedMenuItem::separator(handle)?,
                    &PredefinedMenuItem::close_window(handle, None)?,
                ],
            )?,
            &Submenu::with_items(
                handle,
                tr("menu.edit"),
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
                tr("menu.view"),
                true,
                &[
                    &chat,
                    &shelves,
                    &recipes,
                    &PredefinedMenuItem::separator(handle)?,
                    &text_larger,
                    &text_smaller,
                    &PredefinedMenuItem::separator(handle)?,
                    &PredefinedMenuItem::fullscreen(handle, None)?,
                ],
            )?,
            &window_menu,
        ],
    )
}

pub fn on_menu_event(app: &AppHandle, event: &MenuEvent) {
    if event.id() == ABOUT {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error) = crate::about::open(&app) {
                log::error!("about window: {error}");
            }
        });
        return;
    }

    let action = if event.id() == NEW_CONVERSATION {
        "new-conversation"
    } else if event.id() == SEARCH_CONVERSATIONS {
        "search-conversations"
    } else if event.id() == VIEW_CHAT {
        "view-chat"
    } else if event.id() == VIEW_SHELVES {
        "view-shelves"
    } else if event.id() == VIEW_RECIPES {
        "view-recipes"
    } else if event.id() == VIEW_SETTINGS {
        "view-settings"
    } else if event.id() == TEXT_LARGER {
        "text-larger"
    } else if event.id() == TEXT_SMALLER {
        "text-smaller"
    } else {
        return;
    };

    if let Err(error) = app.emit("rebost://menu", MenuAction { action }) {
        log::warn!("emit rebost://menu: {error}");
    }
}
