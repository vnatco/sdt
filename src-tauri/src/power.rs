//! The power actions the timer ends with.
//!
//! On Windows a shut down or restart is forced: open apps are closed without
//! asking, the way `shutdown /s /f /t 0` does it. Sleep uses S1-S3 where the
//! firmware has them; on Modern Standby machines (S0 low power idle), which
//! have no API to enter standby directly, it turns the display off, which is
//! what makes Windows enter Modern Standby.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    #[default]
    Shutdown,
    Restart,
    Sleep,
}

impl Action {
    pub fn label(self) -> &'static str {
        match self {
            Action::Shutdown => "Shut Down",
            Action::Restart => "Restart",
            Action::Sleep => "Sleep",
        }
    }
}

/// Run `action` now. With `dry_run` nothing happens to the computer; the
/// action is only logged (used for testing the app without losing work).
pub fn perform(action: Action, dry_run: bool, window: Option<isize>) -> Result<(), String> {
    if dry_run {
        log::warn!("dry run: would {} now", action.label());
        return Ok(());
    }
    log::info!("performing {}", action.label());
    imp::perform(action, window)
}

#[cfg(windows)]
mod imp {
    use super::Action;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_NOT_ALL_ASSIGNED, HANDLE, HWND, LPARAM, LUID, WIN32_ERROR, WPARAM};
    use windows::Win32::Security::{AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES, SE_PRIVILEGE_ENABLED, SE_SHUTDOWN_NAME, TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY};
    use windows::Win32::System::Power::{GetPwrCapabilities, SetSuspendState, SYSTEM_POWER_CAPABILITIES};
    use windows::Win32::System::Shutdown::{
        InitiateShutdownW, SHTDN_REASON_FLAG_PLANNED, SHTDN_REASON_MAJOR_OTHER, SHTDN_REASON_MINOR_OTHER, SHUTDOWN_FLAGS, SHUTDOWN_FORCE_OTHERS, SHUTDOWN_FORCE_SELF, SHUTDOWN_POWEROFF, SHUTDOWN_RESTART,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, SC_MONITORPOWER, WM_SYSCOMMAND};

    pub fn perform(action: Action, window: Option<isize>) -> Result<(), String> {
        match action {
            Action::Shutdown => shutdown(SHUTDOWN_POWEROFF, 0),
            Action::Restart => shutdown(SHUTDOWN_RESTART, 0),
            Action::Sleep => sleep(window),
        }
    }

    fn win32_message(code: WIN32_ERROR) -> String {
        windows::core::Error::from(code.to_hresult()).message()
    }

    /// Normal users hold SeShutdownPrivilege, but it is disabled in their
    /// token until a process enables it.
    pub fn enable_shutdown_privilege() -> Result<(), String> {
        unsafe {
            let mut token = HANDLE::default();
            OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token).map_err(|e| format!("Can't open the process token: {}", e.message()))?;
            let result = (|| {
                let mut luid = LUID::default();
                LookupPrivilegeValueW(PCWSTR::null(), SE_SHUTDOWN_NAME, &mut luid).map_err(|e| format!("Can't look up the shutdown privilege: {}", e.message()))?;
                let tp = TOKEN_PRIVILEGES { PrivilegeCount: 1, Privileges: [LUID_AND_ATTRIBUTES { Luid: luid, Attributes: SE_PRIVILEGE_ENABLED }] };
                AdjustTokenPrivileges(token, false, Some(&tp), 0, None, None).map_err(|e| format!("Can't enable the shutdown privilege: {}", e.message()))?;
                // AdjustTokenPrivileges "succeeds" when the account doesn't
                // hold the privilege at all; the real answer is in GetLastError.
                if GetLastError() == ERROR_NOT_ALL_ASSIGNED {
                    return Err("This account isn't allowed to shut down the computer.".to_string());
                }
                Ok(())
            })();
            if let Err(e) = CloseHandle(token) {
                log::warn!("CloseHandle(token) failed: {e}");
            }
            result
        }
    }

    /// `grace` > 0 is only used by the test that proves the call is accepted
    /// and then aborts it.
    pub fn shutdown(kind: SHUTDOWN_FLAGS, grace: u32) -> Result<(), String> {
        enable_shutdown_privilege()?;
        let flags = kind | SHUTDOWN_FORCE_OTHERS | SHUTDOWN_FORCE_SELF;
        let reason = SHTDN_REASON_MAJOR_OTHER | SHTDN_REASON_MINOR_OTHER | SHTDN_REASON_FLAG_PLANNED;
        let code = unsafe { InitiateShutdownW(PCWSTR::null(), PCWSTR::null(), grace, flags, reason) };
        if code == 0 {
            Ok(())
        } else {
            Err(format!("Windows refused to {}: {}", if kind == SHUTDOWN_RESTART { "restart" } else { "shut down" }, win32_message(WIN32_ERROR(code))))
        }
    }

    fn sleep(window: Option<isize>) -> Result<(), String> {
        let mut caps = SYSTEM_POWER_CAPABILITIES::default();
        if !unsafe { GetPwrCapabilities(&mut caps) } {
            return Err(format!("Can't read the power capabilities: {}", win32_message(unsafe { GetLastError() })));
        }
        if caps.SystemS3 || caps.SystemS2 || caps.SystemS1 {
            enable_shutdown_privilege()?;
            // Blocks until the computer wakes up again.
            if unsafe { SetSuspendState(false, false, false) } {
                return Ok(());
            }
            return Err(format!("Windows refused to sleep: {}", win32_message(unsafe { GetLastError() })));
        }
        if caps.AoAc {
            let hwnd = window.ok_or("No window to turn the display off with")?;
            // 2 = display off. On Modern Standby this is how standby starts.
            unsafe { PostMessageW(Some(HWND(hwnd as _)), WM_SYSCOMMAND, WPARAM(SC_MONITORPOWER as usize), LPARAM(2)) }.map_err(|e| format!("Can't turn the display off: {}", e.message()))?;
            return Ok(());
        }
        Err("This computer doesn't support sleep.".to_string())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use windows::Win32::System::Shutdown::AbortSystemShutdownW;

        #[test]
        fn privilege_can_be_enabled() {
            enable_shutdown_privilege().unwrap();
        }

        /// Asks Windows to shut down in 10 minutes and aborts it right away.
        /// This proves the exact call the timer makes is accepted, without
        /// turning the computer off. Run explicitly:
        /// `cargo test -- --ignored scheduled_shutdown_is_accepted`
        #[test]
        #[ignore]
        fn scheduled_shutdown_is_accepted_then_aborted() {
            shutdown(SHUTDOWN_POWEROFF, 600).unwrap();
            unsafe { AbortSystemShutdownW(PCWSTR::null()) }.expect("abort the test shutdown");
            shutdown(SHUTDOWN_RESTART, 600).unwrap();
            unsafe { AbortSystemShutdownW(PCWSTR::null()) }.expect("abort the test restart");
        }
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use super::Action;
    use std::process::Command;

    pub fn perform(action: Action, _window: Option<isize>) -> Result<(), String> {
        // -i ignores inhibitors and other logged-in users: a forced action.
        let verb = match action {
            Action::Shutdown => "poweroff",
            Action::Restart => "reboot",
            Action::Sleep => "suspend",
        };
        run(Command::new("systemctl").args([verb, "-i"]))
    }

    fn run(cmd: &mut Command) -> Result<(), String> {
        let out = cmd.output().map_err(|e| format!("Can't run systemctl: {e}"))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
        }
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use super::Action;
    use std::process::Command;

    pub fn perform(action: Action, _window: Option<isize>) -> Result<(), String> {
        // Forcing apps closed needs root on macOS; System Events asks them
        // to quit instead.
        let mut cmd = match action {
            Action::Shutdown => osascript("tell application \"System Events\" to shut down"),
            Action::Restart => osascript("tell application \"System Events\" to restart"),
            Action::Sleep => {
                let mut c = Command::new("pmset");
                c.arg("sleepnow");
                c
            }
        };
        let out = cmd.output().map_err(|e| format!("Can't run the power command: {e}"))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
        }
    }

    fn osascript(script: &str) -> Command {
        let mut c = Command::new("osascript");
        c.args(["-e", script]);
        c
    }
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
mod imp {
    use super::Action;
    pub fn perform(_action: Action, _window: Option<isize>) -> Result<(), String> {
        Err("Power actions are not supported on this system.".to_string())
    }
}
