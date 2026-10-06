//! The countdown.
//!
//! The backend owns the clock: a hidden webview's timers get throttled, so
//! the interface only draws what this module reports. Deadlines are wall
//! clock times, so a timer keeps its meaning across sleep; if the deadline
//! passed while the computer was asleep, the user gets a last minute instead
//! of a surprise shut down the moment the computer wakes.

use crate::power::{self, Action};
use parking_lot::Mutex;
use serde::Serialize;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The last minute: the warning shows and the window comes back on top.
pub const WARN_MS: u64 = 60_000;
/// Longest timer: 99 hours, 59 minutes, 59 seconds.
pub const MAX_MS: u64 = (99 * 3600 + 59 * 60 + 59) * 1000;
/// More late than this and the computer must have been asleep (or the clock
/// jumped): give a last minute instead of acting at once.
const LATE_MS: u64 = 10_000;
/// A forced shut down normally ends this process within seconds. If it is
/// still running after this, something stopped Windows.
const FIRE_WATCHDOG_MS: u64 = 120_000;

pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Running { deadline: u64 },
    Paused { remaining: u64 },
    Firing { since: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tick {
    Nothing,
    /// The deadline passed while the computer was asleep; a last minute was
    /// granted.
    Late,
    Fire,
}

#[derive(Debug, Clone)]
pub struct Core {
    pub phase: Phase,
    /// Length of the whole run (grows with Add Time); the ring's 100%.
    pub total: u64,
    pub action: Action,
    pub late: bool,
}

impl Default for Core {
    fn default() -> Self {
        Core { phase: Phase::Idle, total: 0, action: Action::Shutdown, late: false }
    }
}

impl Core {
    pub fn start(&mut self, ms: u64, action: Action, now: u64) -> Result<(), String> {
        if matches!(self.phase, Phase::Firing { .. }) {
            return Err("The timer has already gone off.".into());
        }
        if ms < 1000 {
            return Err("Set a time first.".into());
        }
        let ms = ms.min(MAX_MS);
        self.phase = Phase::Running { deadline: now + ms };
        self.total = ms;
        self.action = action;
        self.late = false;
        Ok(())
    }

    pub fn pause(&mut self, now: u64) {
        if let Phase::Running { deadline } = self.phase {
            self.phase = Phase::Paused { remaining: deadline.saturating_sub(now).max(1000) };
        }
    }

    pub fn resume(&mut self, now: u64) {
        if let Phase::Paused { remaining } = self.phase {
            self.phase = Phase::Running { deadline: now + remaining };
        }
    }

    /// Add time to a running or paused timer, up to the maximum.
    pub fn extend(&mut self, ms: u64, now: u64) {
        match &mut self.phase {
            Phase::Running { deadline } => {
                let left = deadline.saturating_sub(now);
                let add = ms.min(MAX_MS.saturating_sub(left));
                *deadline += add;
                self.total = (self.total + add).max(left + add);
            }
            Phase::Paused { remaining } => {
                let add = ms.min(MAX_MS.saturating_sub(*remaining));
                *remaining += add;
                self.total = (self.total + add).max(*remaining);
            }
            _ => {}
        }
    }

    pub fn cancel(&mut self) {
        self.phase = Phase::Idle;
        self.total = 0;
        self.late = false;
    }

    pub fn set_action(&mut self, action: Action) {
        if !matches!(self.phase, Phase::Firing { .. }) {
            self.action = action;
        }
    }

    pub fn remaining(&self, now: u64) -> u64 {
        match self.phase {
            Phase::Idle | Phase::Firing { .. } => 0,
            Phase::Running { deadline } => deadline.saturating_sub(now),
            Phase::Paused { remaining } => remaining,
        }
    }

    pub fn warning(&self, now: u64) -> bool {
        matches!(self.phase, Phase::Running { .. }) && self.remaining(now) <= WARN_MS
    }

    pub fn tick(&mut self, now: u64) -> Tick {
        let Phase::Running { deadline } = self.phase else { return Tick::Nothing };
        if now < deadline {
            return Tick::Nothing;
        }
        if now - deadline > LATE_MS && !self.late {
            self.late = true;
            self.phase = Phase::Running { deadline: now + WARN_MS };
            self.total = WARN_MS;
            return Tick::Late;
        }
        self.phase = Phase::Firing { since: now };
        Tick::Fire
    }

    pub fn snapshot(&self, now: u64, dry_run: bool) -> Snapshot {
        let (phase, deadline) = match self.phase {
            Phase::Idle => ("idle", None),
            Phase::Running { deadline } => ("running", Some(deadline)),
            Phase::Paused { .. } => ("paused", None),
            Phase::Firing { .. } => ("firing", None),
        };
        Snapshot {
            phase,
            remaining_ms: self.remaining(now),
            total_ms: self.total,
            deadline,
            now,
            action: self.action,
            warning: self.warning(now),
            late: self.late,
            dry_run,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub phase: &'static str,
    pub remaining_ms: u64,
    pub total_ms: u64,
    /// Wall-clock deadline (Unix ms) while running; the interface animates
    /// toward it between ticks.
    pub deadline: Option<u64>,
    pub now: u64,
    pub action: Action,
    pub warning: bool,
    pub late: bool,
    pub dry_run: bool,
}

/// Something the interface should tell the user once.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Notice {
    pub error: bool,
    pub text: String,
}

// ---- Engine ----------------------------------------------------------------

pub trait Sink: Send + Sync + 'static {
    /// Called on every change and once a second while running.
    fn snapshot(&self, s: &Snapshot);
    fn notice(&self, n: Notice);
    /// The last minute started: make sure the user can see the timer.
    fn reveal(&self);
    /// Native window handle used by Sleep on Modern Standby machines.
    fn window(&self) -> Option<isize>;
    /// Whether to keep the computer from sleeping while a timer runs.
    fn keep_awake(&self) -> bool;
}

enum Wake {
    Changed,
}

#[derive(Clone)]
pub struct Timer {
    core: Arc<Mutex<Core>>,
    wake: Sender<Wake>,
    sink: Arc<dyn Sink>,
    dry_run: bool,
}

impl Timer {
    pub fn spawn(sink: Arc<dyn Sink>, dry_run: bool) -> Result<Timer, String> {
        let (tx, rx) = mpsc::channel();
        let timer = Timer { core: Arc::default(), wake: tx, sink, dry_run };
        let t = timer.clone();
        std::thread::Builder::new()
            .name("timer".into())
            .spawn(move || {
                let mut awake = false;
                let mut was_warning = false;
                let mut last_second = u64::MAX;
                loop {
                    // Wake up just after the next whole second of the
                    // countdown, so the display and the tray stay exact.
                    let wait = {
                        let core = t.core.lock();
                        match core.phase {
                            Phase::Running { .. } => {
                                let left = core.remaining(now_ms());
                                Duration::from_millis((left % 1000).clamp(20, 1000) + 5)
                            }
                            Phase::Firing { .. } => Duration::from_millis(1000),
                            _ => Duration::from_secs(60),
                        }
                    };
                    match rx.recv_timeout(wait) {
                        Ok(Wake::Changed) | Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                    let now = now_ms();
                    let (tick, snap, firing_since) = {
                        let mut core = t.core.lock();
                        let tick = core.tick(now);
                        let since = if let Phase::Firing { since } = core.phase { Some(since) } else { None };
                        (tick, core.snapshot(now, t.dry_run), since)
                    };

                    let want_awake = snap.phase == "running" && t.sink.keep_awake();
                    if want_awake != awake {
                        set_keep_awake(want_awake);
                        awake = want_awake;
                    }
                    if snap.warning && !was_warning {
                        t.sink.reveal();
                    }
                    was_warning = snap.warning;

                    match tick {
                        Tick::Late => t.sink.notice(Notice {
                            error: false,
                            text: format!("The timer ran out while the computer was asleep. {} in one minute.", verb(snap.action)),
                        }),
                        Tick::Fire => {
                            // Let the computer act right away.
                            if awake {
                                set_keep_awake(false);
                                awake = false;
                            }
                            t.sink.snapshot(&snap);
                            t.fire(snap.action);
                            continue;
                        }
                        Tick::Nothing => {}
                    }
                    if let Some(since) = firing_since {
                        if now.saturating_sub(since) > FIRE_WATCHDOG_MS {
                            t.core.lock().cancel();
                            t.sink.notice(Notice {
                                error: true,
                                text: format!("Windows was asked to {} but didn't. Another program may have stopped it.", snap.action.label().to_lowercase()),
                            });
                            t.emit();
                            continue;
                        }
                    }
                    let second = snap.remaining_ms / 1000;
                    if second != last_second || snap.phase != "running" {
                        last_second = second;
                        t.sink.snapshot(&snap);
                    }
                }
                set_keep_awake(false);
            })
            .map_err(|e| format!("Can't start the timer thread: {e}"))?;
        Ok(timer)
    }

    fn fire(&self, action: Action) {
        let result = power::perform(action, self.dry_run, self.sink.window());
        match result {
            Ok(()) if self.dry_run => {
                self.core.lock().cancel();
                self.sink.notice(Notice { error: false, text: format!("Dry Run: the computer would {} now.", action.label().to_lowercase()) });
            }
            // Sleep returns (or, on Modern Standby, starts) right away; the
            // timer's work is done.
            Ok(()) if action == Action::Sleep => self.core.lock().cancel(),
            // Shut down and restart end this process. Stay in "firing".
            Ok(()) => {}
            Err(e) => {
                log::error!("{} failed: {e}", action.label());
                self.core.lock().cancel();
                self.sink.notice(Notice { error: true, text: e });
            }
        }
        self.emit();
    }

    pub fn update(&self, f: impl FnOnce(&mut Core, u64) -> Result<(), String>) -> Result<Snapshot, String> {
        let now = now_ms();
        let snap = {
            let mut core = self.core.lock();
            f(&mut core, now)?;
            core.snapshot(now, self.dry_run)
        };
        self.sink.snapshot(&snap);
        let _ = self.wake.send(Wake::Changed);
        Ok(snap)
    }

    pub fn snapshot(&self) -> Snapshot {
        self.core.lock().snapshot(now_ms(), self.dry_run)
    }

    fn emit(&self) {
        let snap = self.snapshot();
        self.sink.snapshot(&snap);
        let _ = self.wake.send(Wake::Changed);
    }
}

fn verb(action: Action) -> &'static str {
    match action {
        Action::Shutdown => "Shutting down",
        Action::Restart => "Restarting",
        Action::Sleep => "Going to sleep",
    }
}

/// Keeps the system (not the display) awake. Must be called from the timer
/// thread: the request belongs to the thread that made it.
#[cfg(windows)]
fn set_keep_awake(on: bool) {
    use windows::Win32::System::Power::{SetThreadExecutionState, ES_CONTINUOUS, ES_SYSTEM_REQUIRED};
    let flags = if on { ES_CONTINUOUS | ES_SYSTEM_REQUIRED } else { ES_CONTINUOUS };
    if unsafe { SetThreadExecutionState(flags) }.0 == 0 {
        log::warn!("SetThreadExecutionState failed; the computer may sleep before the timer ends");
    }
}

#[cfg(not(windows))]
fn set_keep_awake(_on: bool) {}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: u64 = 1_700_000_000_000;

    fn started(ms: u64) -> Core {
        let mut c = Core::default();
        c.start(ms, Action::Shutdown, T0).unwrap();
        c
    }

    #[test]
    fn start_rejects_zero_and_caps_at_max() {
        let mut c = Core::default();
        assert!(c.start(0, Action::Shutdown, T0).is_err());
        assert!(c.start(999, Action::Shutdown, T0).is_err());
        c.start(MAX_MS * 3, Action::Restart, T0).unwrap();
        assert_eq!(c.remaining(T0), MAX_MS);
        assert_eq!(c.action, Action::Restart);
    }

    #[test]
    fn counts_down_and_fires_at_deadline() {
        let mut c = started(5_000);
        assert_eq!(c.remaining(T0 + 2_000), 3_000);
        assert_eq!(c.tick(T0 + 4_999), Tick::Nothing);
        assert_eq!(c.tick(T0 + 5_000), Tick::Fire);
        assert!(matches!(c.phase, Phase::Firing { .. }));
        // Firing is final: no restart, no further fire.
        assert_eq!(c.tick(T0 + 6_000), Tick::Nothing);
        assert!(c.start(5_000, Action::Shutdown, T0 + 6_000).is_err());
    }

    #[test]
    fn pause_freezes_and_resume_continues() {
        let mut c = started(60_000);
        c.pause(T0 + 10_000);
        assert_eq!(c.remaining(T0 + 50_000), 50_000);
        assert_eq!(c.tick(T0 + 999_999), Tick::Nothing);
        c.resume(T0 + 100_000);
        assert_eq!(c.remaining(T0 + 100_000), 50_000);
        assert_eq!(c.tick(T0 + 150_000), Tick::Fire);
    }

    #[test]
    fn pause_near_zero_keeps_a_second() {
        let mut c = started(5_000);
        c.pause(T0 + 4_900);
        assert_eq!(c.remaining(0), 1_000);
    }

    #[test]
    fn extend_adds_time_and_grows_the_total() {
        let mut c = started(60_000);
        c.extend(300_000, T0 + 30_000);
        assert_eq!(c.remaining(T0 + 30_000), 330_000);
        assert_eq!(c.total, 360_000);
        c.pause(T0 + 30_000);
        c.extend(60_000, T0);
        assert_eq!(c.remaining(0), 390_000);
    }

    #[test]
    fn extend_never_passes_the_maximum() {
        let mut c = started(MAX_MS - 1_000);
        c.extend(3_600_000, T0);
        assert_eq!(c.remaining(T0), MAX_MS);
    }

    #[test]
    fn extend_does_nothing_when_idle() {
        let mut c = Core::default();
        c.extend(60_000, T0);
        assert_eq!(c.phase, Phase::Idle);
    }

    #[test]
    fn warning_is_the_last_minute_while_running() {
        let mut c = started(120_000);
        assert!(!c.warning(T0 + 59_999));
        assert!(c.warning(T0 + 60_000));
        c.pause(T0 + 70_000);
        assert!(!c.warning(T0 + 70_000));
    }

    #[test]
    fn late_deadline_grants_one_minute_once() {
        let mut c = started(60_000);
        // Asleep for an hour past the deadline.
        let woke = T0 + 60_000 + 3_600_000;
        assert_eq!(c.tick(woke), Tick::Late);
        assert!(c.late);
        assert_eq!(c.remaining(woke), WARN_MS);
        assert!(c.warning(woke));
        // Asleep again through the granted minute: now it acts.
        assert_eq!(c.tick(woke + 10 * WARN_MS), Tick::Fire);
    }

    #[test]
    fn slightly_late_tick_still_fires() {
        let mut c = started(60_000);
        assert_eq!(c.tick(T0 + 60_000 + LATE_MS), Tick::Fire);
    }

    #[test]
    fn cancel_resets_everything() {
        let mut c = started(60_000);
        c.tick(T0 + 999_999);
        c.cancel();
        assert_eq!(c.phase, Phase::Idle);
        assert_eq!(c.total, 0);
        assert!(!c.late);
    }

    #[test]
    fn action_is_locked_once_firing() {
        let mut c = started(1_000);
        c.set_action(Action::Sleep);
        assert_eq!(c.action, Action::Sleep);
        c.tick(T0 + 1_000);
        c.set_action(Action::Restart);
        assert_eq!(c.action, Action::Sleep);
    }

    #[test]
    fn snapshot_reports_phase_and_deadline() {
        let c = started(90_000);
        let s = c.snapshot(T0 + 1_000, true);
        assert_eq!(s.phase, "running");
        assert_eq!(s.deadline, Some(T0 + 90_000));
        assert_eq!(s.remaining_ms, 89_000);
        assert!(s.dry_run);
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["remainingMs"], 89_000);
        assert_eq!(json["action"], "shutdown");
    }
}
