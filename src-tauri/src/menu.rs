//! The right-click menu lives in its own small window, so it can open
//! anywhere on screen (the pill is far too small to hold it), and it is kept
//! inside the work area of the monitor the pointer is on.
//!
//! Flow: the main window asks `menu_open` with the items; the menu window is
//! moved (hidden) to the pointer's monitor and gets the items, measures
//! itself and calls `menu_place`, which positions, shows and focuses it. A
//! pick or a click elsewhere hides it again.

use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

type Res<T> = Result<T, String>;

/// Logical px of transparent margin around the menu, for its shadow.
pub const MARGIN: f64 = 16.0;
/// Logical px kept between the menu and the work-area edge.
const EDGE_GAP: f64 = 6.0;

#[derive(Default)]
pub struct MenuState {
    /// Physical screen point the menu opens from.
    anchor: Mutex<(i32, i32)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Placement {
    /// Top-left of the menu itself (not its window), physical px.
    #[serde(skip)]
    pub x: i32,
    #[serde(skip)]
    pub y: i32,
    /// Opened to the left of / above the pointer.
    pub flip_x: bool,
    pub flip_y: bool,
}

/// Open right and down from the pointer; flip left or up when that would
/// leave the work area; clamp as a last resort (a menu taller than the
/// screen still starts on it).
pub fn place(anchor: (i32, i32), w: i32, h: i32, work: PRect, gap: i32) -> Placement {
    let (ax, ay) = anchor;
    let right = work.x + work.w - gap;
    let bottom = work.y + work.h - gap;
    let flip_x = ax + w > right && ax - w >= work.x + gap;
    let flip_y = ay + h > bottom && ay - h >= work.y + gap;
    let x = if flip_x { ax - w } else { ax };
    let y = if flip_y { ay - h } else { ay };
    let x = x.min(right - w).max(work.x + gap);
    let y = y.min(bottom - h).max(work.y + gap);
    Placement { x, y, flip_x, flip_y }
}

#[tauri::command]
pub fn menu_open(app: AppHandle, state: tauri::State<'_, MenuState>, items: serde_json::Value, action: String) -> Res<()> {
    let menu = app.get_webview_window("menu").ok_or("The menu window is missing")?;
    let anchor = match crate::window::cursor() {
        Some(p) => p,
        None => {
            let p = menu.cursor_position().map_err(|e| e.to_string())?;
            (p.x as i32, p.y as i32)
        }
    };
    *state.anchor.lock() = anchor;
    // Move it (still hidden) to the pointer's monitor first so it lays out
    // at that monitor's scale.
    menu.set_position(tauri::PhysicalPosition::new(anchor.0, anchor.1)).map_err(|e| e.to_string())?;
    app.emit_to("menu", "menu-show", serde_json::json!({ "items": items, "action": action })).map_err(|e| e.to_string())
}

/// Called by the menu window with its measured size (logical px, without
/// the shadow margin).
#[tauri::command]
pub fn menu_place(window: WebviewWindow, state: tauri::State<'_, MenuState>, w: f64, h: f64) -> Res<Placement> {
    let anchor = *state.anchor.lock();
    let monitor = window
        .monitor_from_point(anchor.0 as f64, anchor.1 as f64)
        .map_err(|e| e.to_string())?
        .or(window.primary_monitor().map_err(|e| e.to_string())?)
        .ok_or("No monitor found")?;
    let scale = monitor.scale_factor();
    let a = monitor.work_area();
    let work = PRect { x: a.position.x, y: a.position.y, w: a.size.width as i32, h: a.size.height as i32 };
    let p = |v: f64| (v * scale).ceil() as i32;
    let at = place(anchor, p(w), p(h), work, p(EDGE_GAP));
    let m = p(MARGIN);
    let outer = PRect { x: at.x - m, y: at.y - m, w: p(w) + 2 * m, h: p(h) + 2 * m };
    apply(&window, outer)?;
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    // Showing on a monitor with another scale can resize the window to
    // "fit" the new DPI; put the exact rect back.
    apply(&window, outer)?;
    Ok(at)
}

fn apply(window: &WebviewWindow, r: PRect) -> Res<()> {
    window.set_position(tauri::PhysicalPosition::new(r.x, r.y)).map_err(|e| e.to_string())?;
    window.set_size(tauri::PhysicalSize::new(r.w as u32, r.h as u32)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn menu_pick(app: AppHandle, id: String) -> Res<()> {
    hide(&app)?;
    app.emit_to("main", "menu-pick", id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn menu_close(app: AppHandle) -> Res<()> {
    hide(&app)
}

fn hide(app: &AppHandle) -> Res<()> {
    let menu = app.get_webview_window("menu").ok_or("The menu window is missing")?;
    menu.hide().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORK: PRect = PRect { x: 0, y: 0, w: 1920, h: 1040 };

    #[test]
    fn opens_right_and_down_when_it_fits() {
        let p = place((100, 100), 220, 300, WORK, 6);
        assert_eq!((p.x, p.y, p.flip_x, p.flip_y), (100, 100, false, false));
    }

    #[test]
    fn flips_left_at_the_right_edge() {
        let p = place((1900, 100), 220, 300, WORK, 6);
        assert_eq!((p.x, p.flip_x), (1680, true));
    }

    #[test]
    fn flips_up_at_the_bottom_like_a_tray_menu() {
        let p = place((1850, 1070), 220, 300, PRect { x: 0, y: 0, w: 1920, h: 1032 }, 6);
        assert!(p.flip_y && p.flip_x);
        assert!(p.y + 300 <= 1032 - 6);
        assert!(p.x + 220 <= 1920 - 6);
    }

    #[test]
    fn clamps_when_neither_side_fits() {
        let p = place((500, 400), 220, 1000, WORK, 6);
        assert_eq!(p.y, 1040 - 6 - 1000);
        assert!(!p.flip_y);
    }

    #[test]
    fn stays_on_a_secondary_monitor_with_negative_coordinates() {
        let left = PRect { x: -1280, y: 200, w: 1280, h: 984 };
        let p = place((-1275, 210), 220, 300, left, 6);
        assert!(p.x >= -1280 + 6 && p.y >= 200 + 6);
        let p = place((-5, 1180), 220, 300, left, 6);
        assert!(p.x + 220 <= -6 && p.y + 300 <= 200 + 984 - 6);
    }

    #[test]
    fn taller_than_the_screen_starts_on_it() {
        let p = place((10, 10), 220, 5000, WORK, 6);
        assert_eq!(p.y, 6);
    }
}
