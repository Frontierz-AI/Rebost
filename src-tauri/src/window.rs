//! Keep the main window on a visible display.

use tauri::{LogicalSize, PhysicalPosition, PhysicalSize, WebviewWindow};

const DESIGN_MIN_WIDTH: f64 = 1000.0;
const DESIGN_MIN_HEIGHT: f64 = 660.0;
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
    let _ = window.set_size(PhysicalSize::new(w, h));
    let _ = window.set_position(PhysicalPosition::new(x, y));
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
        let (w, h) = min_size_for_work(911.0, 512.0);
        assert!(w < 1000.0);
        assert!(h < 660.0);
        assert!(w > 800.0);
        let (wide_w, wide_h) = min_size_for_work(1600.0, 1000.0);
        assert_eq!((wide_w, wide_h), (1000.0, 660.0));
    }

    #[test]
    fn offscreen_frame_moves_onto_the_work_area() {
        let (x, y, w, h) = fit_frame(0, 0, 1440, 900, -400, -80, 1320, 860);
        assert_eq!((x, y, w, h), (0, 0, 1320, 860));
        let (x, y, w, h) = fit_frame(0, 0, 1280, 720, 100, 100, 1600, 900);
        assert_eq!((x, y, w, h), (0, 0, 1280, 720));
    }
}
