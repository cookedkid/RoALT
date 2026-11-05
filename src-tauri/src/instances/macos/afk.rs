//! Foreground F13 anti-AFK, matching Windows v0.9.2 using macOS input APIs.
use crate::{
    App,
    commands::{instances_changed, now_ms},
    error::{AppError, Result},
    model::AfkMode,
};
use std::{thread::sleep, time::Duration};
use tauri::{AppHandle, Manager};

const TICK: Duration = Duration::from_secs(10);
const RETRY: Duration = Duration::from_secs(30);
const BETWEEN: Duration = Duration::from_millis(100);

#[derive(Clone, Debug, PartialEq)]
pub struct AfkClient {
    pub id: String,
    pub pid: u32,
    pub since: i64,
}
unsafe extern "C" {
    fn ro_afk_ready() -> i32;
    fn ro_afk_press(pid: i32, start: u64) -> i32;
}
pub fn validate(_: AfkMode) -> Result<()> {
    if unsafe { ro_afk_ready() } != 0 {
        return Err(AppError::Invalid("Enable RoALT in System Settings → Privacy & Security → Accessibility and Input Monitoring, then restart RoALT before enabling anti-AFK.".into()));
    }
    Ok(())
}

fn due(since: i64, now: i64, interval_min: u32) -> bool {
    now.saturating_sub(since) >= i64::from(interval_min) * 60_000
}

pub fn spawn(app: AppHandle) {
    let started = std::thread::Builder::new()
        .name("anti-afk".into())
        .spawn(move || {
            loop {
                sleep(tick(&app));
            }
        });
    if let Err(e) = started {
        log::warn!("anti-AFK not running: {e}");
    }
}

fn tick(app: &AppHandle) -> Duration {
    let s = app.state::<App>();
    let Ok(settings) = s.db.settings() else {
        return TICK;
    };
    if !settings.anti_afk.enabled {
        return TICK;
    }
    let clients = s.tracker.clients();
    let mut changed = false;
    let mut retry = false;
    for c in s.tracker.afk_clients() {
        // Recheck the toggle AND interval so edits take effect between clients in a round.
        let Ok(settings) = s.db.settings() else { break };
        if !settings.anti_afk.enabled {
            break;
        }
        let Some(current) = s
            .tracker
            .afk_clients()
            .into_iter()
            .find(|a| a.id == c.id && a.pid == c.pid)
        else {
            continue;
        };
        if !due(current.since, now_ms(), settings.anti_afk.interval_min) {
            continue;
        }
        let Some((_, pid, start, _)) = clients.iter().find(|a| a.0 == c.id && a.1 == c.pid) else {
            continue;
        };
        // Native code checks process birth time, permissions, manual idle and foreground window.
        let result = unsafe { ro_afk_press(*pid as i32, *start) };
        match result {
            0 => {
                // Posting has no delivery receipt: record an attempt, not game acceptance.
                log::info!("anti-AFK: posted foreground F13 in instance {}", c.id);
                s.tracker.mark_afk(&c.id, now_ms());
                changed = true;
            }
            2 => {
                log::debug!("anti-AFK: instance {} has no matching live window", c.id);
                retry = true;
                continue;
            }
            _ => {
                if result < 0 {
                    log::warn!(
                        "Anti-AFK skipped tracked PID {pid}: {}",
                        if result == -2 {
                            "Accessibility or Input Monitoring access is missing"
                        } else {
                            "could not create input events"
                        }
                    );
                }
                retry = true;
                break;
            }
        }
        sleep(BETWEEN);
    }
    if changed {
        instances_changed(app);
    }
    if retry { RETRY } else { TICK }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_boundary_and_changed_interval() {
        let since = 1_000;
        assert!(!due(since, since + 15 * 60_000 - 1, 15));
        assert!(due(since, since + 15 * 60_000, 15));
        assert!(!due(since, since + 15 * 60_000, 19));
        assert!(due(since, since + 60_000, 1));
        assert!(!due(since, since - 1, 1));
        // A recorded attempt starts a fresh interval.
        assert!(!due(since + 15 * 60_000, since + 16 * 60_000, 15));
    }

    #[test]
    fn stale_identity_never_injects_input() {
        assert_eq!(unsafe { ro_afk_press(std::process::id() as i32, 1) }, 2);
    }
}
