//! Debug-only window snapshot for website screenshots.

use std::sync::mpsc;
use tauri::{AppHandle, Manager};

use crate::commands::{friendly, CmdResult};

#[tauri::command]
pub fn dev_snapshot(app: AppHandle, path: String, label: Option<String>) -> CmdResult<()> {
    #[cfg(not(all(debug_assertions, target_os = "macos")))]
    {
        let _ = (app, path, label);
        return Err("Snapshots are only available in a debug Mac build.".into());
    }
    #[cfg(all(debug_assertions, target_os = "macos"))]
    {
        macos_snapshot(&app, &path, label.as_deref().unwrap_or("main"))
    }
}

#[cfg(all(debug_assertions, target_os = "macos"))]
fn macos_snapshot(app: &AppHandle, path: &str, label: &str) -> CmdResult<()> {
    let window = app
        .get_webview_window(label)
        .ok_or_else(|| format!("No window labelled {label}."))?;
    let (tx, rx) = mpsc::channel();
    window
        .with_webview(move |webview| {
            // SAFETY: Tauri hands a live WKWebView pointer for this window.
            let result = unsafe { png_from_view(webview.inner()) };
            let _ = tx.send(result);
        })
        .map_err(friendly)?;
    let png = rx
        .recv()
        .map_err(|_| "The snapshot did not finish.".to_string())??;
    std::fs::write(path, png).map_err(friendly)?;
    Ok(())
}

/// # Safety
/// `ptr` must be a live `NSView` (WKWebView).
#[cfg(all(debug_assertions, target_os = "macos"))]
unsafe fn png_from_view(ptr: *mut std::ffi::c_void) -> CmdResult<Vec<u8>> {
    use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRepPropertyKey, NSView};
    use objc2_foundation::NSDictionary;

    if ptr.is_null() {
        return Err("The window has no webview.".into());
    }
    // SAFETY: caller passes the WKWebView pointer from Tauri's PlatformWebview.
    let view = unsafe { &*ptr.cast::<NSView>() };
    let bounds = view.bounds();
    let Some(rep) = view.bitmapImageRepForCachingDisplayInRect(bounds) else {
        return Err("Couldn't make a bitmap of the window.".into());
    };
    view.cacheDisplayInRect_toBitmapImageRep(bounds, &rep);
    let props = NSDictionary::<NSBitmapImageRepPropertyKey, objc2::runtime::AnyObject>::new();
    // SAFETY: empty properties dictionary is valid for PNG encoding.
    let Some(data) =
        (unsafe { rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &props) })
    else {
        return Err("Couldn't encode the snapshot.".into());
    };
    Ok(data.to_vec())
}
