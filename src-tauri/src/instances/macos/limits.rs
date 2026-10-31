use super::{cpu::Governor, native, process};
use crate::{
    error::{AppError, Result},
    model::{Limits, Settings},
};
use std::{collections::HashMap, sync::Mutex};
struct Applied {
    pid: u32,
    start: u64,
    eco: bool,
    cap: Option<u8>,
    governor: Option<Governor>,
}
struct State {
    applied: HashMap<String, Applied>,
    released: bool,
}
pub struct Limiter {
    state: Mutex<State>,
}
fn restore(a: &mut Applied) -> bool {
    a.governor.take();
    a.cap = None;
    if !a.eco || process::start_time(a.pid) != Some(a.start) {
        return true;
    }
    let ok = unsafe { native::ro_background(a.pid as i32, 0) } == 0;
    if !ok {
        log::warn!("Could not restore priority for process {}", a.pid)
    }
    ok
}
pub fn validate(l: Limits) -> Result<()> {
    if l.cpu_cap.is_some_and(|v| v == 0 || v > 100) {
        return Err(AppError::Invalid(
            "CPU cap must be between 1 and 100 percent.".into(),
        ));
    }
    if l.affinity.is_some() || l.trim_every_min.is_some_and(|v| v > 0) {
        return Err(AppError::Invalid(
            "Core affinity and memory trimming are unavailable on macOS.".into(),
        ));
    }
    Ok(())
}
impl Limiter {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(State {
                applied: HashMap::new(),
                released: false,
            }),
        }
    }
    pub fn apply(&self, id: &str, pid: u32, start: u64, l: Limits) -> Result<()> {
        validate(l)?;
        if process::start_time(pid) != Some(start) {
            return Err(AppError::Invalid(
                "The process is no longer running.".into(),
            ));
        }
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.released {
            return Ok(());
        }
        let map = &mut state.applied;
        if let Some(a) = map.get_mut(id).filter(|a| a.pid != pid || a.start != start) {
            if !restore(a) {
                return Err(AppError::Invalid(
                    "Could not restore previous process priority.".into(),
                ));
            }
            map.remove(id);
        }
        let a = map.entry(id.into()).or_insert_with(|| Applied {
            pid,
            start,
            eco: false,
            cap: None,
            governor: None,
        });
        let cap = l.cpu_cap.filter(|v| *v < 100);
        if a.cap != cap || a.governor.as_mut().is_some_and(|g| !g.alive()) {
            a.governor.take();
            a.cap = None;
            if let Some(cap) = cap {
                a.governor = Some(Governor::start(pid, start, cap)?);
                a.cap = Some(cap);
            }
        }
        if a.eco != l.eco {
            let background = l.eco;
            if unsafe { native::ro_background(pid as i32, background as i32) } != 0 {
                return Err(AppError::Invalid(format!(
                    "Could not change macOS background priority: {}",
                    std::io::Error::last_os_error()
                )));
            }
            a.eco = l.eco;
        }
        Ok(())
    }
    pub fn release_all(&self) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.released = true;
        state.applied.retain(|_, a| !restore(a));
    }
    pub fn sync(&self, clients: &[(String, u32, u64, Limits)]) {
        {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if state.released {
                return;
            }
            state
                .applied
                .retain(|id, a| clients.iter().any(|c| &c.0 == id) || !restore(a));
        }
        for (id, pid, start, l) in clients {
            if let Err(e) = self.apply(id, *pid, *start, *l) {
                log::debug!("macOS priority: {e}")
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn background_priority_round_trip_on_owned_child() {
        let mut child = std::process::Command::new("/bin/sleep")
            .arg("10")
            .spawn()
            .unwrap();
        let pid = child.id();
        let start = process::start_time(pid).unwrap();
        let l = Limiter::new();
        let before = unsafe { native::ro_thread_priority(pid as i32) };
        let result = l.apply(
            "owned",
            pid,
            start,
            Limits {
                eco: true,
                ..Default::default()
            },
        );
        let after = unsafe { native::ro_thread_priority(pid as i32) };
        l.sync(&[]);
        let detached = unsafe { native::ro_thread_priority(pid as i32) };
        l.apply(
            "owned",
            pid,
            start,
            Limits {
                eco: true,
                ..Default::default()
            },
        )
        .unwrap();
        l.release_all();
        l.sync(&[(
            "owned".into(),
            pid,
            start,
            Limits {
                eco: true,
                ..Default::default()
            },
        )]);
        let restored = unsafe { native::ro_thread_priority(pid as i32) };
        let _ = child.kill();
        let _ = child.wait();
        result.unwrap();
        assert!(
            after < before,
            "before={before}, after={after}, restored={restored}"
        );
        assert_eq!(restored, before);
        assert_eq!(detached, before);
    }
}
pub fn trim(_: u32, _: u64) -> Result<()> {
    Err(AppError::Invalid(
        "Memory trimming is unavailable on macOS.".into(),
    ))
}
pub fn plan(
    clients: &[(String, u32, u64, Limits)],
    v: &Settings,
    foreground: Option<u32>,
    cores: u32,
    throttled: &[String],
) -> Vec<(String, u32, u64, Limits)> {
    let n = clients.len() as u32;
    clients
        .iter()
        .enumerate()
        .map(|(i, (id, pid, t0, manual))| {
            let share = (v.auto_spread && n > 1).then(|| spread(i as u32, n, cores));
            let mut l = effective(*manual, foreground == Some(*pid), v, share);
            if v.mem_throttle.mb > 0 && throttled.contains(id) {
                l.cpu_cap = Some(throttle_cap(l.cpu_cap, v.mem_throttle.cpu_cap));
            }
            (id.clone(), *pid, *t0, l)
        })
        .collect()
}

/// One client's limits (pure): its own when the user set any (manual limits win), else the
/// adaptive manager's background limits while its window isn't in front; then the default
/// trim and the auto-spread `share` where it says nothing of its own.
pub fn effective(manual: Limits, foreground: bool, v: &Settings, share: Option<u64>) -> Limits {
    let mut l = if manual == Limits::default() && v.adaptive.enabled && !foreground {
        v.adaptive.background
    } else {
        manual
    };
    if l.trim_every_min.is_none() && v.trim_every_min > 0 {
        l.trim_every_min = Some(v.trim_every_min);
    }
    if l.affinity.is_none() {
        l.affinity = share;
    }
    l
}

/// The CPU cap of a throttled client (pure): the throttle's, or its own if that is tighter.
pub fn throttle_cap(own: Option<u8>, throttle: u8) -> u8 {
    own.map_or(throttle, |c| c.min(throttle))
}

/// Client `i` of `n`'s cores out of `cores` (at most 64), round-robin (pure): with at least as
/// many cores as clients each gets every n-th core; with more clients, one core each.
pub fn spread(i: u32, n: u32, cores: u32) -> u64 {
    let cores = cores.clamp(1, 64);
    if n >= cores {
        return 1 << (i % cores);
    }
    (0..cores)
        .filter(|c| c % n == i % n)
        .fold(0, |m, c| m | 1 << c)
}

/// The mask actually set: the cores asked for that exist (`system`); None when none do (pure).
pub fn clamp_affinity(mask: u64, system: u64) -> Option<u64> {
    Some(mask & system).filter(|&m| m != 0)
}
