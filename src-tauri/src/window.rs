//! Keep the main window on a visible display.

use tauri::{LogicalSize, PhysicalPosition, PhysicalSize, WebviewWindow};

/// Screenshot runner: hang the window off the right edge instead of centering it.
pub fn shot_park_enabled() -> bool {
    matches!(std::env::var("REBOST_SHOT_PARK").as_deref(), Ok("1"))
}

const DESIGN_MIN_WIDTH: f64 = 640.0;
const DESIGN_MIN_HEIGHT: f64 = 540.0;
const WORK_FIT: f64 = 0.92;

/// After the window-state plugin restores the last frame, shrink the min
/// size on a small display and move the window onto the work area.
pub fn place_main_window(window: &WebviewWindow) {
    let Ok(Some(monitor)) = window
        .current_monitor()
        .or_else(|_| window.primary_monitor())
    else {
        return;
    };
    let scale = monitor.scale_factor();
    let work = monitor.work_area();
    let work_w = work.size.width as f64 / scale;
    let work_h = work.size.height as f64 / scale;
    let (min_w, min_h) = min_size_for_work(work_w, work_h);
    let _ = window.set_min_size(Some(LogicalSize::new(min_w, min_h)));

    let Ok(pos) = window.outer_position() else {
        return;
    };
    let Ok(size) = window.outer_size() else {
        return;
    };
    let inner = window.inner_size().unwrap_or(size);
    let (x, y, w, h) = fit_frame(
        work.position.x,
        work.position.y,
        work.size.width,
        work.size.height,
        pos.x,
        pos.y,
        size.width,
        size.height,
    );
    // Only touch what has to change. GTK can report 0×0 for a window that is
    // not mapped yet; resizing to that would open it at the minimum size.
    if (w, h) != (size.width, size.height) {
        let (w, h) = content_size(w, h, size, inner);
        let _ = window.set_size(PhysicalSize::new(w, h));
    }
    if (x, y) != (pos.x, pos.y) {
        let _ = window.set_position(PhysicalPosition::new(x, y));
    }
}

/// Event the webviews get when the desktop switches between light and dark.
#[cfg(target_os = "linux")]
pub const THEME_EVENT: &str = "rebost://theme";

/// Tell the webviews when the desktop switches light/dark. tao applies the
/// portal's colour scheme to GTK (title bar, menus) but reports it for no
/// window, so the page never hears of it. Call on the main thread.
#[cfg(target_os = "linux")]
pub fn follow_desktop_theme(app: &tauri::AppHandle) {
    use gtk::prelude::GtkSettingsExt;
    use tauri::Emitter;

    let Some(settings) = gtk::Settings::default() else {
        return;
    };
    let notify = {
        let app = app.clone();
        move |settings: &gtk::Settings| {
            if let Err(error) = app.emit(THEME_EVENT, gtk_scheme(settings)) {
                log::warn!("emit {THEME_EVENT}: {error}");
            }
        }
    };
    let on_name = notify.clone();
    settings.connect_gtk_application_prefer_dark_theme_notify(notify);
    settings.connect_gtk_theme_name_notify(on_name);
}

#[cfg(target_os = "linux")]
fn gtk_scheme(settings: &gtk::Settings) -> &'static str {
    use gtk::prelude::GtkSettingsExt;
    let theme_name = settings
        .gtk_theme_name()
        .map(|name| name.to_string())
        .unwrap_or_default();
    scheme_for(settings.is_gtk_application_prefer_dark_theme(), &theme_name)
}

/// `dark` when GTK prefers dark or the GTK theme is a dark variant.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn scheme_for(prefer_dark: bool, theme_name: &str) -> &'static str {
    let theme_name = theme_name.to_lowercase();
    if prefer_dark || theme_name.ends_with("-dark") || theme_name.ends_with(":dark") {
        "dark"
    } else {
        "light"
    }
}

/// The desktop's light/dark scheme as GTK sees it (Linux), read once when the
/// page starts. tao's own window theme only asks the portal, which misses a
/// dark GTK theme or `gtk-application-prefer-dark-theme` in settings.ini.
/// `None` elsewhere: the window theme is right there.
#[tauri::command]
pub fn desktop_color_scheme() -> Option<&'static str> {
    #[cfg(target_os = "linux")]
    {
        gtk::Settings::default().map(|settings| gtk_scheme(&settings))
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// Move a window so only a few pixels stay on the work area (right edge).
/// Leaves size alone so marketing frames stay 1320×860 (or About's fixed size).
pub fn park_for_shots(window: &WebviewWindow) {
    let Ok(Some(monitor)) = window
        .primary_monitor()
        .or_else(|_| window.current_monitor())
    else {
        return;
    };
    let work = monitor.work_area();
    let Ok(size) = window.outer_size() else {
        return;
    };
    let sliver = shot_park_sliver();
    let (x, y, _w, _h) = park_frame(
        work.position.x,
        work.position.y,
        work.size.width,
        work.size.height,
        size.width,
        size.height,
        sliver,
    );
    let _ = window.set_position(PhysicalPosition::new(x, y));
    let _ = window.set_ignore_cursor_events(true);
}

fn shot_park_sliver() -> i32 {
    std::env::var("REBOST_SHOT_SLIVER")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value: &i32| *value > 0)
        .unwrap_or(8)
}

/// Hang `w`×`h` off the right of the work area, keeping `sliver` pixels on-screen
/// so the compositor still paints the webview.
pub(crate) fn park_frame(
    work_x: i32,
    work_y: i32,
    work_w: u32,
    _work_h: u32,
    w: u32,
    h: u32,
    sliver: i32,
) -> (i32, i32, u32, u32) {
    let sliver = sliver.max(1);
    let x = work_x.saturating_add(work_w as i32).saturating_sub(sliver);
    (x, work_y, w, h)
}

/// `set_size` takes the content size; take the title bar and borders off a
/// frame size measured with them.
pub(crate) fn content_size(
    w: u32,
    h: u32,
    outer: PhysicalSize<u32>,
    inner: PhysicalSize<u32>,
) -> (u32, u32) {
    if inner.width == 0 || inner.height == 0 {
        return (w, h);
    }
    (
        w.saturating_sub(outer.width.saturating_sub(inner.width)),
        h.saturating_sub(outer.height.saturating_sub(inner.height)),
    )
}

pub(crate) fn min_size_for_work(work_w: f64, work_h: f64) -> (f64, f64) {
    (
        DESIGN_MIN_WIDTH.min(work_w * WORK_FIT),
        DESIGN_MIN_HEIGHT.min(work_h * WORK_FIT),
    )
}

pub(crate) fn fit_frame(
    work_x: i32,
    work_y: i32,
    work_w: u32,
    work_h: u32,
    mut x: i32,
    mut y: i32,
    mut w: u32,
    mut h: u32,
) -> (i32, i32, u32, u32) {
    if w > work_w {
        w = work_w;
    }
    if h > work_h {
        h = work_h;
    }
    let max_x = work_x
        .saturating_add(work_w as i32)
        .saturating_sub(w as i32);
    let max_y = work_y
        .saturating_add(work_h as i32)
        .saturating_sub(h as i32);
    if x < work_x {
        x = work_x;
    } else if x > max_x {
        x = max_x;
    }
    if y < work_y {
        y = work_y;
    } else if y > max_y {
        y = max_y;
    }
    (x, y, w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_display_lowers_the_design_minimum() {
        let (w, h) = min_size_for_work(600.0, 512.0);
        assert!(w < 640.0);
        assert!(h < 540.0);
        assert!(w > 500.0);
        let (wide_w, wide_h) = min_size_for_work(1600.0, 1000.0);
        assert_eq!((wide_w, wide_h), (640.0, 540.0));
    }

    #[test]
    fn offscreen_frame_moves_onto_the_work_area() {
        let (x, y, w, h) = fit_frame(0, 0, 1440, 900, -400, -80, 1320, 860);
        assert_eq!((x, y, w, h), (0, 0, 1320, 860));
        let (x, y, w, h) = fit_frame(0, 0, 1280, 720, 100, 100, 1600, 900);
        assert_eq!((x, y, w, h), (0, 0, 1280, 720));
    }

    #[test]
    fn content_size_leaves_room_for_the_title_bar() {
        let outer = PhysicalSize::new(1372, 949);
        let inner = PhysicalSize::new(1320, 860);
        assert_eq!(content_size(1280, 720, outer, inner), (1228, 631));
        let unmapped = PhysicalSize::new(0, 0);
        assert_eq!(content_size(1280, 720, outer, unmapped), (1280, 720));
    }

    #[test]
    fn dark_gtk_settings_mean_a_dark_page() {
        assert_eq!(scheme_for(true, "Adwaita"), "dark");
        assert_eq!(scheme_for(false, "Adwaita-dark"), "dark");
        assert_eq!(scheme_for(false, "Adwaita:dark"), "dark");
        assert_eq!(scheme_for(false, "Breeze-Dark"), "dark");
        assert_eq!(scheme_for(false, "Adwaita"), "light");
        assert_eq!(scheme_for(false, ""), "light");
    }

    #[test]
    fn park_frame_keeps_a_sliver_on_the_right() {
        let (x, y, w, h) = park_frame(0, 0, 2560, 1440, 1320, 860, 8);
        assert_eq!((x, y, w, h), (2552, 0, 1320, 860));
    }
}
