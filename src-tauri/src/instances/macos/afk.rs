use crate::{
    App,
    commands::{instances_changed, now_ms},
    error::{AppError, Result},
    model::AfkMode,
};
use tauri::{AppHandle, Manager};
#[derive(Clone, Debug, PartialEq)]
pub struct AfkClient {
    pub id: String,
    pub pid: u32,
    pub since: i64,
}
unsafe extern "C" {
    fn ro_afk_ready() -> i32;
    fn ro_afk_press_mode(pid: i32, start: u64, background: bool, keycode: i32) -> i32;
}
pub fn validate(_: AfkMode) -> Result<()> {
    if unsafe { ro_afk_ready() } != 0 {
        return Err(AppError::Invalid("Enable RoALT in System Settings → Privacy & Security → Accessibility and Input Monitoring, then restart RoALT before enabling anti-AFK.".into()));
    }
    Ok(())
}
pub fn spawn(app: AppHandle) {
    let _ = std::thread::Builder::new()
        .name("anti-afk".into())
        .spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(10));
                let s = app.state::<App>();
                let Ok(v) = s.db.settings() else { continue };
                if !v.anti_afk.enabled {
                    continue;
                }
                let options = match crate::mac_options::read(&crate::mac_options::options_dir(&s)) {
                    Ok(options) => options,
                    Err(e) => { log::warn!("Anti-AFK settings unavailable: {e}"); continue; }
                };
                let clients = s.tracker.clients();
                let mut changed = false;
                for c in s.tracker.afk_clients() {
                    if now_ms() - c.since < i64::from(v.anti_afk.interval_min) * 60000 {
                        continue;
                    }
                    let Some((_, pid, start, _)) =
                        clients.iter().find(|a| a.0 == c.id && a.1 == c.pid)
                    else {
                        continue;
                    };
                    // Recheck opt-in each client so switching off takes effect during a round.
                    if !s.db.settings().is_ok_and(|v| v.anti_afk.enabled) {
                        break;
                    }
                    if !s
                        .tracker
                        .afk_clients()
                        .iter()
                        .any(|a| a.id == c.id && a.pid == *pid)
                    {
                        continue;
                    }
                    let background = v.anti_afk.mode == AfkMode::Background;
                    let mut result = unsafe { ro_afk_press_mode(*pid as i32, *start, background, i32::from(options.key_code)) };
                    if should_fallback(background, options.focus_fallback, result) {
                        log::warn!("Background anti-AFK event creation failed; using opted-in focus fallback for tracked PID {pid}");
                        result = unsafe { ro_afk_press_mode(*pid as i32, *start, false, i32::from(options.key_code)) };
                    }
                    if result < 0 {
                        log::warn!("Anti-AFK skipped tracked PID {pid}: {}", if result == -2 { "Accessibility or Input Monitoring access is missing" } else { "could not create input events" });
                    }
                    // Posting has no delivery receipt: this records an attempt, not game acceptance.
                    match result {
                        0 => {
                            s.tracker.mark_afk(&c.id, now_ms());
                            changed = true;
                        }
                        2 => continue,
                        _ => break,
                    }
                }
                if changed {
                    instances_changed(&app);
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_identity_never_injects_input() {
        assert_eq!(unsafe { ro_afk_press_mode(std::process::id() as i32, 1,false,48) }, 2);
    }
}

fn should_fallback(background: bool, allowed: bool, result: i32) -> bool {
    background && allowed && result == -1
}
#[cfg(test)]
mod fallback_tests {
    use super::*;
    #[test]
    fn never_focus_for_missing_permission_activity_stale_pid_or_success() {
        for result in [-3, -2, 0, 1, 2] { assert!(!should_fallback(true, true, result)); }
        assert!(should_fallback(true, true, -1));
        assert!(!should_fallback(true, false, -1));
        assert!(!should_fallback(false, true, -1));
    }
}
