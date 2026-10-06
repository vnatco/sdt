//! Settings, saved as JSON in the app's config folder.

use crate::power::Action;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub action: Action,
    /// "in" (a duration) or "at" (a clock time).
    pub mode: String,
    /// Last duration set, in seconds.
    pub duration: u64,
    /// Last clock time set, in minutes after midnight.
    pub at: u32,
    /// Window positions (logical px, top-left of the window).
    pub card_pos: Option<Point>,
    pub pill_pos: Option<Point>,
    pub on_top: bool,
    /// Shrink to the pill when a timer starts.
    pub auto_mini: bool,
    /// Chime at the last minute and tick through the last ten seconds.
    pub sound: bool,
    pub keep_awake: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            action: Action::Shutdown,
            mode: "in".into(),
            duration: 30 * 60,
            at: 23 * 60,
            card_pos: None,
            pill_pos: None,
            on_top: false,
            auto_mini: false,
            sound: true,
            keep_awake: true,
        }
    }
}

pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new(dir: &Path) -> Self {
        SettingsStore { path: dir.join("settings.json") }
    }

    /// Missing file: defaults. Unreadable file: defaults, and the bad file
    /// is kept beside it so nothing is silently lost.
    pub fn load(&self) -> Settings {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Settings::default(),
            Err(e) => {
                log::error!("can't read settings ({e}); using defaults");
                return Settings::default();
            }
        };
        match serde_json::from_str(&text) {
            Ok(s) => s,
            Err(e) => {
                log::error!("settings file is damaged ({e}); using defaults");
                let _ = std::fs::rename(&self.path, self.path.with_extension("json.bad"));
                Settings::default()
            }
        }
    }

    /// Written to a temporary file and renamed over the old one, so a crash
    /// mid-write never leaves a half-written file.
    pub fn save(&self, s: &Settings) -> Result<(), String> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("Can't create the settings folder: {e}"))?;
        }
        let json = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, json).map_err(|e| format!("Can't save settings: {e}"))?;
        std::fs::rename(&tmp, &self.path).map_err(|e| format!("Can't save settings: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sdt-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn round_trips() {
        let dir = temp_dir("roundtrip");
        let store = SettingsStore::new(&dir);
        assert_eq!(store.load(), Settings::default());
        let s = Settings { action: Action::Restart, duration: 5400, pill_pos: Some(Point { x: 10.0, y: -200.0 }), sound: false, ..Default::default() };
        store.save(&s).unwrap();
        assert_eq!(store.load(), s);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_fields_take_defaults() {
        let s: Settings = serde_json::from_str(r#"{"action":"sleep"}"#).unwrap();
        assert_eq!(s.action, Action::Sleep);
        assert!(s.keep_awake);
        assert_eq!(s.mode, "in");
    }

    #[test]
    fn damaged_file_is_kept_aside() {
        let dir = temp_dir("damaged");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("settings.json"), "{ not json").unwrap();
        let store = SettingsStore::new(&dir);
        assert_eq!(store.load(), Settings::default());
        assert!(dir.join("settings.json.bad").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
