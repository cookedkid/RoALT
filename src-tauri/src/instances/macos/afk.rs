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
    fn ro_afk_press_mode(pid: i32, start: u64, background: bool) -> i32;
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
                    match unsafe { ro_afk_press_mode(*pid as i32, *start, v.anti_afk.mode == AfkMode::Background) } {
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
        assert_eq!(unsafe { ro_afk_press_mode(std::process::id() as i32, 1,false) }, 2);
    }
}
