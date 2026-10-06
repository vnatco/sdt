//! Window shaping and motion.
//!
//! The window is transparent and undecorated; the card or the pill is drawn
//! inside it with room for its shadow. A webview can't be click-through per
//! pixel, so a small thread polls the cursor and makes the window
//! click-through whenever the pointer is outside the shapes the interface
//! reports. A click-through window gets no mouse events, so the same thread
//! tells the interface when the pointer enters or leaves.
//!
//! Moves are done here rather than by the system move loop: Windows keeps a
//! dragged window's top edge on screen, and the pill's window extends above
//! the pill so it can grow in either direction. Glides are paced by DWM
//! composition, so they run at the display's refresh rate.

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

type Res<T> = Result<T, String>;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
}

// ---- Click-through ---------------------------------------------------------

#[derive(Default)]
struct Hit {
    /// Interactive areas in logical px relative to the window's top-left.
    rects: Vec<Rect>,
    ignoring: bool,
    inside: bool,
}

#[derive(Clone, Default)]
pub struct HitTest(Arc<Mutex<Hit>>);

#[derive(Debug, Clone, Serialize)]
pub struct PointerInside {
    pub inside: bool,
}

pub fn spawn_hit_test(app: &AppHandle, window: WebviewWindow) -> HitTest {
    let hit = HitTest::default();
    let shared = hit.0.clone();
    let app = app.clone();
    let spawned = std::thread::Builder::new().name("hit-test".into()).spawn(move || loop {
        std::thread::sleep(Duration::from_millis(16));
        let Some(local) = cursor_in_window(&window) else { continue };
        let mut h = shared.lock();
        let over = h.rects.iter().any(|r| r.contains(local.0, local.1));
        // Keep input while a button is held so a drag past the edge of the
        // shape doesn't drop.
        let inside = over || (!h.ignoring && mouse_button_down());
        if inside == h.ignoring {
            match window.set_ignore_cursor_events(!inside) {
                Ok(()) => h.ignoring = !inside,
                Err(e) => log::warn!("click-through toggle failed: {e}"),
            }
        }
        if inside != h.inside {
            h.inside = inside;
            if let Err(e) = app.emit_to("main", "pointer-inside", PointerInside { inside }) {
                log::warn!("can't send pointer-inside: {e}");
            }
        }
    });
    if let Err(e) = spawned {
        log::error!("can't start the hit-test thread ({e}); the window's transparent margin will block clicks");
    }
    hit
}

#[tauri::command]
pub fn window_hit(hit: tauri::State<'_, HitTest>, rects: Vec<Rect>) {
    hit.0.lock().rects = rects;
}

// ---- Frames ----------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Frame {
    /// Outer rect in logical screen px.
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub scale: f64,
    /// Work area (screen minus taskbar) of the monitor under the anchor point.
    pub work: Rect,
}

/// The window's frame, with the work area of the monitor under the point
/// (`ax`, `ay`) of the window (logical px from its top-left). The interface
/// passes the pill's centre, since the window itself hangs off screen edges.
#[tauri::command]
pub fn window_frame(window: WebviewWindow, ax: f64, ay: f64) -> Res<Frame> {
    frame(&window, ax, ay)
}

fn frame(window: &WebviewWindow, ax: f64, ay: f64) -> Res<Frame> {
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    let size = window.outer_size().map_err(|e| e.to_string())?;
    let px = pos.x as f64 + ax * scale;
    let py = pos.y as f64 + ay * scale;
    let work = work_area_at(window, px, py, scale)?;
    Ok(Frame { x: pos.x as f64 / scale, y: pos.y as f64 / scale, w: size.width as f64 / scale, h: size.height as f64 / scale, scale, work })
}

/// Logical work area of the monitor at a physical point (or the nearest one).
fn work_area_at(window: &WebviewWindow, px: f64, py: f64, scale: f64) -> Res<Rect> {
    let monitor = match window.monitor_from_point(px, py).map_err(|e| e.to_string())? {
        Some(m) => Some(m),
        None => match window.current_monitor().map_err(|e| e.to_string())? {
            Some(m) => Some(m),
            None => window.primary_monitor().map_err(|e| e.to_string())?,
        },
    };
    let m = monitor.ok_or("No monitor found")?;
    let a = m.work_area();
    Ok(Rect { x: a.position.x as f64 / scale, y: a.position.y as f64 / scale, w: a.size.width as f64 / scale, h: a.size.height as f64 / scale })
}

/// Whether a logical screen point is on any monitor (used to check that a
/// remembered position is still reachable).
#[tauri::command]
pub fn point_on_screen(window: WebviewWindow, x: f64, y: f64) -> Res<bool> {
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    Ok(window.monitor_from_point(x * scale, y * scale).map_err(|e| e.to_string())?.is_some())
}

/// Move and resize in one step (logical screen px).
#[tauri::command]
pub fn window_set_frame(window: WebviewWindow, motion: tauri::State<'_, Motion>, x: f64, y: f64, w: f64, h: f64) -> Res<()> {
    motion.cancel();
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let r = |v: f64| (v * scale).round() as i32;
    set_pos(&window, r(x), r(y), Some((r(w), r(h))))
}

#[cfg(windows)]
fn hwnd(window: &WebviewWindow) -> Res<windows::Win32::Foundation::HWND> {
    Ok(windows::Win32::Foundation::HWND(window.hwnd().map_err(|e| e.to_string())?.0 as _))
}

#[cfg(windows)]
fn set_pos(window: &WebviewWindow, x: i32, y: i32, size: Option<(i32, i32)>) -> Res<()> {
    use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER};
    let (w, h, flags) = match size {
        Some((w, h)) => (w, h, SWP_NOZORDER | SWP_NOACTIVATE),
        None => (0, 0, SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOSIZE),
    };
    unsafe { SetWindowPos(hwnd(window)?, None, x, y, w, h, flags) }.map_err(|e| format!("Can't move the window: {e}"))
}

#[cfg(not(windows))]
fn set_pos(window: &WebviewWindow, x: i32, y: i32, size: Option<(i32, i32)>) -> Res<()> {
    window.set_position(tauri::PhysicalPosition::new(x, y)).map_err(|e| e.to_string())?;
    if let Some((w, h)) = size {
        window.set_size(tauri::PhysicalSize::new(w as u32, h as u32)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Physical top-left of the window.
fn position(window: &WebviewWindow) -> Res<(i32, i32)> {
    let p = window.outer_position().map_err(|e| e.to_string())?;
    Ok((p.x, p.y))
}

// ---- Motion ----------------------------------------------------------------

/// Generation counter: starting a new glide or drag stops the previous one.
#[derive(Default)]
pub struct Motion(AtomicU64);

impl Motion {
    fn next(&self) -> u64 {
        self.0.fetch_add(1, Ordering::SeqCst) + 1
    }
    fn current(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
    pub fn cancel(&self) {
        self.next();
    }
}

/// Wait for the next DWM composition (one display refresh).
fn frame_wait() {
    #[cfg(windows)]
    {
        if unsafe { windows::Win32::Graphics::Dwm::DwmFlush() }.is_ok() {
            return;
        }
    }
    std::thread::sleep(Duration::from_millis(8));
}

/// A slightly under-damped spring from 0 to 1: starts gently, lands with a
/// hint of overshoot. `t` is in units of the glide's length.
pub fn spring(t: f64) -> f64 {
    if t >= 1.0 {
        return 1.0;
    }
    let zeta: f64 = 0.82;
    let omega = 9.5;
    let wd = omega * (1.0 - zeta * zeta).sqrt();
    let decay = (-zeta * omega * t).exp();
    1.0 - decay * ((wd * t).cos() + (zeta * omega / wd) * (wd * t).sin())
}

/// Glide the window's top-left to (x, y) logical px.
#[tauri::command]
pub async fn window_glide(app: AppHandle, window: WebviewWindow, x: f64, y: f64, ms: u64) -> Res<()> {
    let gen = app.state::<Motion>().next();
    let task = tauri::async_runtime::spawn_blocking(move || {
        let motion = app.state::<Motion>();
        let scale = window.scale_factor().map_err(|e| e.to_string())?;
        let (fx, fy) = position(&window)?;
        let (tx, ty) = ((x * scale).round() as i32, (y * scale).round() as i32);
        if ms == 0 || (fx == tx && fy == ty) {
            return set_pos(&window, tx, ty, None);
        }
        let start = Instant::now();
        loop {
            if motion.current() != gen {
                return Ok(());
            }
            let t = start.elapsed().as_secs_f64() * 1000.0 / ms as f64;
            let e = spring(t);
            let nx = fx + ((tx - fx) as f64 * e).round() as i32;
            let ny = fy + ((ty - fy) as f64 * e).round() as i32;
            set_pos(&window, nx, ny, None)?;
            if t >= 1.0 {
                return Ok(());
            }
            frame_wait();
        }
    });
    task.await.map_err(|e| e.to_string())?
}

/// The shape being grabbed changes size when a drag starts (the pill
/// shrinks to its small form while carried). `from` is the shape when it was
/// pressed and `to` the shape while carried, both logical px in the window;
/// `ms` and `ease` are the interface's transition for that change.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Grab {
    pub from: Rect,
    pub to: Rect,
    pub ms: f64,
    /// CSS cubic-bezier control points.
    pub ease: [f64; 4],
}

/// Where the window must shift (logical px) so the point grabbed on `from`
/// stays under the cursor on `to`: the same relative spot, so a pill
/// grabbed near its left end is still held near its left end once small.
pub fn grab_shift(grab: &Grab, lx: f64, ly: f64) -> (f64, f64) {
    let fx = ((lx - grab.from.x) / grab.from.w).clamp(0.0, 1.0);
    let fy = ((ly - grab.from.y) / grab.from.h).clamp(0.0, 1.0);
    let tx = grab.to.x + fx * grab.to.w;
    let ty = grab.to.y + fy * grab.to.h;
    (lx - tx, ly - ty)
}

/// CSS `cubic-bezier(x1, y1, x2, y2)` at time `t` (0..1), so the window's
/// slide follows the shape's own transition exactly.
pub fn cubic_bezier(p: [f64; 4], t: f64) -> f64 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }
    let [x1, y1, x2, y2] = p;
    let bez = |a: f64, b: f64, s: f64| 3.0 * a * s * (1.0 - s) * (1.0 - s) + 3.0 * b * s * s * (1.0 - s) + s * s * s;
    // Solve x(s) = t by bisection (x is monotonic for valid CSS curves).
    let (mut lo, mut hi) = (0.0, 1.0);
    let mut s = t;
    for _ in 0..40 {
        let x = bez(x1, x2, s);
        if (x - t).abs() < 1e-6 {
            break;
        }
        if x < t {
            lo = s;
        } else {
            hi = s;
        }
        s = (lo + hi) / 2.0;
    }
    bez(y1, y2, s)
}

/// Move the window with the mouse until the button is released. Returns
/// whether it actually moved (a press without movement is a click).
#[tauri::command]
pub async fn window_drag(app: AppHandle, window: WebviewWindow, grab: Option<Grab>) -> Res<bool> {
    let gen = app.state::<Motion>().next();
    let task = tauri::async_runtime::spawn_blocking(move || {
        let motion = app.state::<Motion>();
        let Some(start) = cursor() else { return Ok(false) };
        let (wx, wy) = position(&window)?;
        let scale = window.scale_factor().map_err(|e| e.to_string())?;
        // Where the shape was grabbed, in logical px within the window.
        let (lx, ly) = ((start.0 - wx) as f64 / scale, (start.1 - wy) as f64 / scale);
        let shift = grab.map(|g| (grab_shift(&g, lx, ly), g));
        let mut moved_at: Option<Instant> = None;
        while mouse_button_down() && motion.current() == gen {
            if let Some(c) = cursor() {
                let (dx, dy) = (c.0 - start.0, c.1 - start.1);
                if moved_at.is_none() && dx.abs() + dy.abs() >= 4 {
                    moved_at = Some(Instant::now());
                    // The interface shows the small pill while it is carried.
                    if let Err(e) = app.emit_to("main", "drag-moved", ()) {
                        log::warn!("can't send drag-moved: {e}");
                    }
                }
                if let Some(t0) = moved_at {
                    // Slide under the cursor in step with the shape's change.
                    let (ox, oy) = match shift {
                        Some(((sx, sy), g)) => {
                            let e = cubic_bezier(g.ease, t0.elapsed().as_secs_f64() * 1000.0 / g.ms);
                            ((sx * e * scale).round() as i32, (sy * e * scale).round() as i32)
                        }
                        None => (0, 0),
                    };
                    set_pos(&window, wx + dx + ox, wy + dy + oy, None)?;
                }
            }
            frame_wait();
        }
        Ok(moved_at.is_some())
    });
    task.await.map_err(|e| e.to_string())?
}

// ---- Window roles ------------------------------------------------------------

/// The pill floats over everything and stays off the taskbar; the card is a
/// normal window, on top only if the user asked.
#[tauri::command]
pub fn window_mode(window: WebviewWindow, pill: bool, on_top: bool) -> Res<()> {
    window.set_skip_taskbar(pill).map_err(|e| e.to_string())?;
    window.set_always_on_top(pill || on_top).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn window_show(window: WebviewWindow) -> Res<()> {
    window.show().map_err(|e| e.to_string())?;
    window.unminimize().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn window_hide(window: WebviewWindow) -> Res<()> {
    window.hide().map_err(|e| e.to_string())
}

// ---- Input -------------------------------------------------------------------

/// Cursor position in logical px relative to the window's top-left, or None
/// if the window is minimized or hidden.
#[cfg(windows)]
fn cursor_in_window(window: &WebviewWindow) -> Option<(f64, f64)> {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowRect, IsIconic, IsWindowVisible};
    let hwnd = hwnd(window).ok()?;
    unsafe {
        if IsIconic(hwnd).as_bool() || !IsWindowVisible(hwnd).as_bool() {
            return None;
        }
        let p = cursor()?;
        let mut r = RECT::default();
        GetWindowRect(hwnd, &mut r).ok()?;
        let scale = window.scale_factor().ok()?;
        Some(((p.0 - r.left) as f64 / scale, (p.1 - r.top) as f64 / scale))
    }
}

/// Physical cursor position.
#[cfg(windows)]
pub fn cursor() -> Option<(i32, i32)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
    let mut p = POINT::default();
    unsafe { GetCursorPos(&mut p) }.ok()?;
    Some((p.x, p.y))
}

/// The primary button, physically: GetAsyncKeyState reads physical buttons,
/// so left-handed (swapped) mice use the right one.
#[cfg(windows)]
fn mouse_button_down() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_RBUTTON};
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_SWAPBUTTON};
    let key = if unsafe { GetSystemMetrics(SM_SWAPBUTTON) } != 0 { VK_RBUTTON } else { VK_LBUTTON };
    unsafe { (GetAsyncKeyState(key.0 as i32) as u16 & 0x8000) != 0 }
}

#[cfg(not(windows))]
fn cursor_in_window(_: &WebviewWindow) -> Option<(f64, f64)> {
    None
}

#[cfg(not(windows))]
pub fn cursor() -> Option<(i32, i32)> {
    None
}

#[cfg(not(windows))]
fn mouse_button_down() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::spring;

    use super::{cubic_bezier, grab_shift, Grab, Rect};

    #[test]
    fn bezier_matches_css_endpoints_and_linear() {
        assert_eq!(cubic_bezier([0.3, 1.18, 0.5, 1.0], 0.0), 0.0);
        assert_eq!(cubic_bezier([0.3, 1.18, 0.5, 1.0], 1.0), 1.0);
        // cubic-bezier(0, 0, 1, 1) is linear.
        assert!((cubic_bezier([0.0, 0.0, 1.0, 1.0], 0.37) - 0.37).abs() < 1e-4);
        // An overshooting curve passes 1 before landing.
        let peak = (1..100).map(|i| cubic_bezier([0.34, 1.32, 0.5, 1.0], i as f64 / 100.0)).fold(0.0, f64::max);
        assert!(peak > 1.0);
    }

    #[test]
    fn grabbed_spot_stays_relative() {
        let g = Grab { from: Rect { x: 124.0, y: 200.0, w: 340.0, h: 58.0 }, to: Rect { x: 188.0, y: 200.0, w: 212.0, h: 44.0 }, ms: 440.0, ease: [0.3, 1.18, 0.5, 1.0] };
        // Grabbed at the left end of the wide pill: held at the left end of the small one.
        let (sx, _) = grab_shift(&g, 124.0 + 34.0, 229.0);
        assert!((158.0 - sx - (188.0 + 0.1 * 212.0)).abs() < 1e-9);
        // Grabbed in the middle: nothing moves sideways.
        let (sx, _) = grab_shift(&g, 294.0, 229.0);
        assert!(sx.abs() < 1e-9);
    }

    #[test]
    fn spring_starts_at_rest_and_lands_exactly() {
        assert!(spring(0.0).abs() < 1e-9);
        assert!(spring(0.02) < 0.05, "starts gently");
        assert_eq!(spring(1.0), 1.0);
        let peak = (0..100).map(|i| spring(i as f64 / 100.0)).fold(0.0, f64::max);
        assert!(peak > 1.0 && peak < 1.03, "overshoot stays subtle: {peak}");
        assert!((spring(0.99) - 1.0).abs() < 0.01, "settled by the end");
    }
}
