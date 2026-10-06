//! The tray icon: a live progress ring of the time left, in the colour of
//! the chosen action. Left click shows or hides the timer; right click opens
//! the app's own menu (drawn by the menu window, not the system).

use crate::power::Action;
use crate::timer::Snapshot;
use parking_lot::Mutex;
use std::f64::consts::TAU;
use tauri::image::Image;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

type Res<T> = Result<T, String>;

const ID: &str = "main";
const N: u32 = 32;
/// Ring resolution: the icon is redrawn only when the ring moves a step.
const STEPS: f64 = 96.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Look {
    Idle,
    Running { left: f64, action: Action, warning: bool },
    Paused { left: f64, action: Action },
}

#[derive(Default)]
pub struct Tray {
    last: Mutex<Option<(Look, bool)>>,
}

impl Tray {
    pub fn build(app: &AppHandle) -> Res<Tray> {
        TrayIconBuilder::with_id(ID)
            .icon(render(Look::Idle, light_taskbar()))
            .tooltip("Shut Down Timer")
            .show_menu_on_left_click(false)
            .on_tray_icon_event(|tray, e| match e {
                TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } => toggle(tray.app_handle()),
                TrayIconEvent::Click { button: MouseButton::Right, button_state: MouseButtonState::Up, .. } => {
                    if let Err(e) = tray.app_handle().emit_to("main", "tray-menu", ()) {
                        log::error!("can't open the tray menu: {e}");
                    }
                }
                _ => {}
            })
            .build(app)
            .map_err(|e| format!("Can't create the tray icon: {e}"))?;
        Ok(Tray::default())
    }

    pub fn update(&self, app: &AppHandle, s: &Snapshot) {
        let Some(tray) = app.tray_by_id(ID) else { return };
        let frac = if s.total_ms > 0 { s.remaining_ms as f64 / s.total_ms as f64 } else { 0.0 };
        let step = (frac * STEPS).ceil() / STEPS;
        let look = match s.phase {
            "running" => Look::Running { left: step, action: s.action, warning: s.warning },
            "paused" => Look::Paused { left: step, action: s.action },
            _ => Look::Idle,
        };
        let light = light_taskbar();
        let changed = {
            let mut last = self.last.lock();
            let changed = *last != Some((look, light));
            *last = Some((look, light));
            changed
        };
        if changed {
            if let Err(e) = tray.set_icon(Some(render(look, light))) {
                log::warn!("can't update the tray icon: {e}");
            }
        }
        let tip = match s.phase {
            "running" => format!("{} in {}", s.action.label(), clock(s.remaining_ms)),
            "paused" => format!("Paused - {} Left", clock(s.remaining_ms)),
            "firing" => format!("{}...", s.action.label()),
            _ => "Shut Down Timer".to_string(),
        };
        if let Err(e) = tray.set_tooltip(Some(tip)) {
            log::warn!("can't update the tray tooltip: {e}");
        }
    }
}

/// Show and focus the window, or hide it if it is showing. Hiding is left
/// to the interface, so it fades out first: the last frame a hidden window
/// painted is what flashes up when it is shown again.
pub fn toggle(app: &AppHandle) {
    let Some(w) = app.get_webview_window("main") else { return };
    let visible = w.is_visible().unwrap_or(false) && !w.is_minimized().unwrap_or(false);
    let result = if visible { app.emit_to("main", "tray-hide", ()) } else { reveal(app) };
    if let Err(e) = result {
        log::error!("can't toggle the window: {e}");
    }
}

/// Bring the window back and let the interface play its entrance.
pub fn reveal(app: &AppHandle) -> tauri::Result<()> {
    let Some(w) = app.get_webview_window("main") else { return Ok(()) };
    let was_hidden = !w.is_visible().unwrap_or(true);
    w.show()?;
    w.unminimize()?;
    w.set_focus()?;
    if was_hidden {
        app.emit_to("main", "window-shown", ())?;
    }
    Ok(())
}

/// 1:05:09, 5:09, 0:09.
pub fn clock(ms: u64) -> String {
    let s = ms.div_ceil(1000);
    let (h, m, s) = (s / 3600, s / 60 % 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

pub fn accent(action: Action) -> [u8; 3] {
    match action {
        Action::Shutdown => [255, 112, 82],
        Action::Restart => [64, 176, 255],
        Action::Sleep => [168, 134, 255],
    }
}

const WARNING: [u8; 3] = [255, 72, 84];

// ---- Drawing -----------------------------------------------------------------

/// A shape's coverage at a point, for 4x4 supersampling.
fn ring_arc(px: f64, py: f64, r: f64, t: f64, sweep: f64, from: f64) -> bool {
    let d = (px * px + py * py).sqrt();
    if (d - r).abs() > t / 2.0 {
        return caps(px, py, r, t, sweep, from);
    }
    // Clockwise from 12 o'clock.
    let mut a = px.atan2(-py);
    if a < 0.0 {
        a += TAU;
    }
    let rel = (a - from).rem_euclid(TAU);
    rel <= sweep || caps(px, py, r, t, sweep, from)
}

fn caps(px: f64, py: f64, r: f64, t: f64, sweep: f64, from: f64) -> bool {
    if sweep <= 0.0 || sweep >= TAU {
        return false;
    }
    [from, from + sweep].iter().any(|&a| {
        let (cx, cy) = (r * a.sin(), -r * a.cos());
        (px - cx).powi(2) + (py - cy).powi(2) <= (t / 2.0).powi(2)
    })
}

fn capsule(px: f64, py: f64, a: (f64, f64), b: (f64, f64), t: f64) -> bool {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let k = (((px - a.0) * dx + (py - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    let (qx, qy) = (a.0 + k * dx - px, a.1 + k * dy - py);
    qx * qx + qy * qy <= (t / 2.0).powi(2)
}

pub fn render(look: Look, light: bool) -> Image<'static> {
    let fg: [u8; 3] = if light { [24, 24, 28] } else { [245, 245, 248] };
    let track_alpha = if light { 0.20 } else { 0.30 };
    let r = 12.0;
    let t = 4.2;
    let mut rgba = vec![0u8; (N * N * 4) as usize];
    for y in 0..N {
        for x in 0..N {
            // Coverage of each layer, painted bottom to top.
            let mut layers: [(f64, [u8; 3], f64); 3] = [(0.0, fg, track_alpha), (0.0, fg, 1.0), (0.0, fg, 1.0)];
            for sy in 0..4 {
                for sx in 0..4 {
                    let px = x as f64 + (sx as f64 + 0.5) / 4.0 - N as f64 / 2.0;
                    let py = y as f64 + (sy as f64 + 0.5) / 4.0 - N as f64 / 2.0;
                    let hits: [bool; 3] = match look {
                        Look::Idle => {
                            // A power symbol: open ring and a bar through the gap.
                            let gap = 0.62;
                            [false, ring_arc(px, py, r - 1.0, t, TAU - 2.0 * gap, gap), capsule(px, py, (0.0, -14.0), (0.0, -3.0), t)]
                        }
                        Look::Running { left, .. } | Look::Paused { left, .. } => {
                            let track = ring_arc(px, py, r, t, TAU, 0.0);
                            let sweep = left * TAU;
                            let arc = sweep > 0.0 && ring_arc(px, py, r, t, sweep, 0.0);
                            let paused = matches!(look, Look::Paused { .. }) && (capsule(px, py, (-2.6, -3.6), (-2.6, 3.6), 3.0) || capsule(px, py, (2.6, -3.6), (2.6, 3.6), 3.0));
                            [track, arc, paused]
                        }
                    };
                    for (i, h) in hits.iter().enumerate() {
                        if *h {
                            layers[i].0 += 1.0 / 16.0;
                        }
                    }
                }
            }
            match look {
                Look::Running { action, warning, .. } => layers[1].1 = if warning { WARNING } else { accent(action) },
                Look::Paused { .. } => layers[1].2 = 0.55,
                Look::Idle => {}
            }
            // Source-over composite into straight alpha.
            let (mut cr, mut cg, mut cb, mut ca) = (0.0, 0.0, 0.0, 0.0);
            for (cov, c, alpha) in layers {
                let a = cov * alpha;
                if a <= 0.0 {
                    continue;
                }
                let out = a + ca * (1.0 - a);
                let mix = |src: u8, dst: f64| (src as f64 * a + dst * ca * (1.0 - a)) / out;
                cr = mix(c[0], cr);
                cg = mix(c[1], cg);
                cb = mix(c[2], cb);
                ca = out;
            }
            let i = ((y * N + x) * 4) as usize;
            rgba[i] = cr.round() as u8;
            rgba[i + 1] = cg.round() as u8;
            rgba[i + 2] = cb.round() as u8;
            rgba[i + 3] = (ca * 255.0).round() as u8;
        }
    }
    Image::new_owned(rgba, N, N)
}

#[cfg(windows)]
fn light_taskbar() -> bool {
    use windows::core::w;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
    let mut value: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    let r = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut _),
            Some(&mut size),
        )
    };
    r.is_ok() && value == 1
}

#[cfg(not(windows))]
fn light_taskbar() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha(img: &Image, x: u32, y: u32) -> u8 {
        img.rgba()[((y * N + x) * 4 + 3) as usize]
    }

    fn rgb(img: &Image, x: u32, y: u32) -> [u8; 3] {
        let i = ((y * N + x) * 4) as usize;
        [img.rgba()[i], img.rgba()[i + 1], img.rgba()[i + 2]]
    }

    #[test]
    fn clock_formats() {
        assert_eq!(clock(0), "0:00");
        assert_eq!(clock(9_000), "0:09");
        assert_eq!(clock(8_001), "0:09", "rounds up so it never shows 0:00 early");
        assert_eq!(clock(309_000), "5:09");
        assert_eq!(clock(3_909_000), "1:05:09");
    }

    #[test]
    fn half_ring_is_lit_on_the_right_only() {
        let img = render(Look::Running { left: 0.5, action: Action::Shutdown, warning: false }, false);
        // Time left runs clockwise from 12 o'clock, like the dial.
        let right_side = rgb(&img, 28, 16);
        let left_side = rgb(&img, 4, 16);
        assert_eq!(right_side, accent(Action::Shutdown));
        assert_ne!(left_side, accent(Action::Shutdown));
        assert!(alpha(&img, 4, 16) > 0, "the track still shows");
        assert_eq!(alpha(&img, 16, 16), 0, "centre is clear");
    }

    #[test]
    fn warning_is_red() {
        let img = render(Look::Running { left: 1.0, action: Action::Sleep, warning: true }, true);
        assert_eq!(rgb(&img, 28, 16), WARNING);
    }

    #[test]
    fn idle_is_a_power_symbol() {
        let img = render(Look::Idle, false);
        assert!(alpha(&img, 16, 8) > 200, "the bar");
        assert!(alpha(&img, 16, 28) > 200, "the bottom of the ring");
        assert_eq!(alpha(&img, 16, 16), 0, "the gap between bar and ring");
    }

    #[test]
    fn paused_shows_bars() {
        let img = render(Look::Paused { left: 0.3, action: Action::Restart }, false);
        assert!(alpha(&img, 13, 16) > 200);
    }
}
