pub mod menu;
pub mod power;
pub mod settings;
pub mod timer;
pub mod tray;
pub mod window;

use parking_lot::Mutex;
use power::Action;
use serde::Serialize;
use settings::{Settings, SettingsStore};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use timer::{Notice, Snapshot, Sink, Timer};

type Res<T> = Result<T, String>;

struct AppState {
    timer: Timer,
    settings: Arc<Mutex<Settings>>,
    store: SettingsStore,
    dry_run: bool,
}

/// Where the timer's news goes: the interface, the tray, and the taskbar.
struct AppSink {
    app: AppHandle,
    tray: tray::Tray,
    settings: Arc<Mutex<Settings>>,
}

impl Sink for AppSink {
    fn snapshot(&self, s: &Snapshot) {
        if let Err(e) = self.app.emit_to("main", "timer", s) {
            log::warn!("can't send the timer state: {e}");
        }
        self.tray.update(&self.app, s);
        taskbar_progress(&self.app, s);
    }

    fn notice(&self, n: Notice) {
        if n.error {
            log::error!("{}", n.text);
        }
        if let Err(e) = self.app.emit_to("main", "notice", n) {
            log::warn!("can't send a notice: {e}");
        }
    }

    fn reveal(&self) {
        if let Err(e) = tray::reveal(&self.app) {
            log::error!("can't show the window for the last minute: {e}");
        }
        if let Some(w) = self.app.get_webview_window("main") {
            if let Err(e) = w.set_always_on_top(true) {
                log::warn!("can't keep the warning on top: {e}");
            }
        }
    }

    fn window(&self) -> Option<isize> {
        self.app.get_webview_window("main").and_then(|w| w.hwnd().ok()).map(|h| h.0 as isize)
    }

    fn keep_awake(&self) -> bool {
        self.settings.lock().keep_awake
    }
}

/// The taskbar button fills up as time passes; yellow when paused, red in
/// the last minute.
fn taskbar_progress(app: &AppHandle, s: &Snapshot) {
    use tauri::window::{ProgressBarState, ProgressBarStatus};
    let Some(w) = app.get_webview_window("main") else { return };
    let done = if s.total_ms > 0 { 100 - (s.remaining_ms * 100 / s.total_ms).min(100) } else { 0 };
    let status = match s.phase {
        "running" if s.warning => ProgressBarStatus::Error,
        "running" => ProgressBarStatus::Normal,
        "paused" => ProgressBarStatus::Paused,
        "firing" => ProgressBarStatus::Indeterminate,
        _ => ProgressBarStatus::None,
    };
    if let Err(e) = w.set_progress_bar(ProgressBarState { status: Some(status), progress: Some(done.max(1)) }) {
        log::warn!("can't update the taskbar progress: {e}");
    }
}

// ---- Commands ------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Boot {
    settings: Settings,
    timer: Snapshot,
    version: String,
    dry_run: bool,
}

#[tauri::command]
fn boot(app: AppHandle, state: tauri::State<'_, AppState>) -> Boot {
    Boot { settings: state.settings.lock().clone(), timer: state.timer.snapshot(), version: app.package_info().version.to_string(), dry_run: state.dry_run }
}

#[tauri::command]
fn settings_set(state: tauri::State<'_, AppState>, settings: Settings) -> Res<()> {
    *state.settings.lock() = settings.clone();
    state.store.save(&settings)
}

#[tauri::command]
fn timer_start(state: tauri::State<'_, AppState>, ms: u64, action: Action) -> Res<Snapshot> {
    state.timer.update(|c, now| c.start(ms, action, now))
}

#[tauri::command]
fn timer_pause(state: tauri::State<'_, AppState>) -> Res<Snapshot> {
    state.timer.update(|c, now| {
        c.pause(now);
        Ok(())
    })
}

#[tauri::command]
fn timer_resume(state: tauri::State<'_, AppState>) -> Res<Snapshot> {
    state.timer.update(|c, now| {
        c.resume(now);
        Ok(())
    })
}

#[tauri::command]
fn timer_extend(state: tauri::State<'_, AppState>, ms: u64) -> Res<Snapshot> {
    state.timer.update(|c, now| {
        c.extend(ms, now);
        Ok(())
    })
}

#[tauri::command]
fn timer_cancel(state: tauri::State<'_, AppState>) -> Res<Snapshot> {
    state.timer.update(|c, _| {
        c.cancel();
        Ok(())
    })
}

#[tauri::command]
fn timer_set_action(state: tauri::State<'_, AppState>, action: Action) -> Res<Snapshot> {
    state.timer.update(|c, _| {
        c.set_action(action);
        Ok(())
    })
}

#[tauri::command]
fn app_quit(app: AppHandle) {
    app.exit(0);
}

// ---- Startup --------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be first: a second launch brings the running timer forward.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Err(e) = tray::reveal(app) {
                log::error!("can't show the window: {e}");
            }
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .max_file_size(1_000_000)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepOne)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            setup(app.handle()).map_err(|e| {
                log::error!("startup failed: {e}");
                e
            })?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            boot,
            settings_set,
            timer_start,
            timer_pause,
            timer_resume,
            timer_extend,
            timer_cancel,
            timer_set_action,
            app_quit,
            window::window_hit,
            window::window_frame,
            window::window_set_frame,
            window::window_glide,
            window::window_drag,
            window::window_mode,
            window::window_show,
            window::window_hide,
            window::point_on_screen,
            menu::menu_open,
            menu::menu_place,
            menu::menu_pick,
            menu::menu_close,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Shut Down Timer");
}

fn setup(app: &AppHandle) -> Result<(), String> {
    let dir = app.path().app_config_dir().map_err(|e| format!("No settings folder: {e}"))?;
    let store = SettingsStore::new(&dir);
    let settings = Arc::new(Mutex::new(store.load()));

    // Test switch: count down for real but only log the power action.
    let dry_run = std::env::args().any(|a| a == "--dry-run") || std::env::var("SDT_DRY_RUN").is_ok_and(|v| v == "1");
    if dry_run {
        log::warn!("dry run: the computer will not be shut down");
    }

    let tray = tray::Tray::build(app)?;
    let sink = Arc::new(AppSink { app: app.clone(), tray, settings: settings.clone() });
    let timer = Timer::spawn(sink, dry_run)?;
    {
        let action = settings.lock().action;
        if let Err(e) = timer.update(|c, _| {
            c.set_action(action);
            Ok(())
        }) {
            log::warn!("can't restore the action: {e}");
        }
    }

    let main = app.get_webview_window("main").ok_or("The main window is missing")?;
    app.manage(window::spawn_hit_test(app, main));
    app.manage(window::Motion::default());
    app.manage(menu::MenuState::default());
    app.manage(AppState { timer, settings, store, dry_run });
    Ok(())
}
