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
/// Restore priority left by a prior manager; governors have their own death guardian.
pub fn reset_followed(pid: u32, start: u64) {
    if process::start_time(pid) == Some(start) && unsafe { native::ro_background(pid as i32, 0) } != 0 {
        log::warn!("Could not reset background priority for tracked process {pid}");
    }
}

pub fn plan_now(s: &crate::App, v: &Settings, clients: &[(String, u32, u64, Limits)], throttled: &[String]) -> Vec<(String, u32, u64, Limits)> {
    // If account settings cannot be read, avoid imposing automatic limits on any client.
    let free = match s.db.full_quality_accounts() {
        Ok(accounts) => s.tracker.free(&accounts, crate::commands::now_ms()),
        Err(e) => { log::warn!("Could not read quality exemptions: {e}"); clients.iter().map(|c| c.0.clone()).collect() }
    };
    plan(clients, v, process::foreground_pid(), crate::commands::cores(), throttled, &free)
}

pub fn plan(clients: &[(String, u32, u64, Limits)], v: &Settings, foreground: Option<u32>, cores: u32, throttled: &[String], free: &[String]) -> Vec<(String, u32, u64, Limits)> {
    let fair = v.fair_cap.then(|| fair_cap(clients.len() as u32, cores));
    clients.iter().map(|(id,pid,start,manual)| {
        let exempt = foreground == Some(*pid) || free.contains(id);
        let mut l = effective(*manual, exempt, v, fair);
        if !exempt && v.mem_throttle.mb > 0 && throttled.contains(id) {
            l.cpu_cap = Some(throttle_cap(l.cpu_cap, v.mem_throttle.cpu_cap));
        }
        (id.clone(), *pid, *start, l)
    }).collect()
}

/// Explicit manual limits survive exemptions; automatic limits never interrupt loading/play.
pub fn effective(manual: Limits, free: bool, v: &Settings, fair: Option<u8>) -> Limits {
    if free { return manual; }
    let mut l = manual;
    if v.adaptive.enabled {
        // A manual setting overrides only its own field. `eco` is opt-in (false means
        // inherit); Full quality remains the way to opt out of all automatic limits.
        l.cpu_cap = manual.cpu_cap.or(v.adaptive.background.cpu_cap);
        l.eco |= v.adaptive.background.eco;
    }
    if let Some(cap) = fair.filter(|_| manual.cpu_cap.is_none()) {
        l.cpu_cap = Some(throttle_cap(l.cpu_cap, cap));
    }
    // No CPU affinity or memory trim is synthesized on macOS.
    l
}

pub fn throttle_cap(own: Option<u8>, throttle: u8) -> u8 { own.map_or(throttle, |c| c.min(throttle)) }

/// Same 75% total-machine budget and quarter-core floor as upstream; enforcement is cooperative.
pub fn fair_cap(n: u32, cores: u32) -> u8 {
    (75 / n.max(1)).max(25u32.div_ceil(cores.max(1))).clamp(1,100) as u8
}

#[cfg(test)]
mod planning_tests {
    use super::*;
    #[test]
    fn budget_scales_but_preserves_a_quarter_core_floor() {
        assert_eq!(fair_cap(1,8),75);
        assert_eq!(fair_cap(10,8),7);
        assert_eq!(fair_cap(50,8),4);
        assert_eq!(fair_cap(50,16),2);
        assert_eq!(fair_cap(0,0),75);
    }
    #[test]
    fn active_loading_and_quality_clients_escape_all_automatic_limits() {
        let mut v=Settings { fair_cap:true, ..Settings::default() };
        v.adaptive.enabled=true;
        v.adaptive.background=Limits { cpu_cap:Some(5), eco:true, ..Default::default() };
        v.mem_throttle.mb=1; v.mem_throttle.cpu_cap=1;
        let clients:Vec<_>=(1..=3).map(|pid|(pid.to_string(),pid,1,Limits::default())).collect();
        let result=plan(&clients,&v,Some(1),8,&["1".into(),"2".into(),"3".into()],&["2".into()]);
        assert_eq!(result[0].3,Limits::default());
        assert_eq!(result[1].3,Limits::default());
        assert_eq!(result[2].3.cpu_cap,Some(1));
        assert!(result[2].3.eco);
    }
    #[test]
    fn manual_cap_wins_and_efficiency_only_does_not_pause() {
        let mut v=Settings::default();v.adaptive.enabled=true;v.adaptive.background=Limits { eco:true, ..Default::default() };
        let own=Limits { cpu_cap:Some(30), ..Default::default() };
        assert_eq!(effective(own,true,&v,Some(5)),own);
        assert_eq!(effective(own,false,&v,Some(5)),Limits { eco:true, ..own });
        assert_eq!(effective(Limits::default(),false,&v,None),v.adaptive.background);
        assert_eq!(effective(Limits::default(),false,&v,Some(5)).cpu_cap,Some(5));
    }
}

#[cfg(test)]
mod override_tests {
    use super::*;
    #[test]
    fn manual_fields_do_not_cancel_unrelated_adaptive_settings() {
        let mut v = Settings::default();
        v.adaptive.enabled = true;
        v.adaptive.background = Limits { cpu_cap: Some(20), eco: true, ..Default::default() };
        let cpu = Limits { cpu_cap: Some(40), ..Default::default() };
        assert_eq!(effective(cpu, false, &v, Some(5)), Limits { eco: true, ..cpu });
        let priority = Limits { eco: true, ..Default::default() };
        assert_eq!(effective(priority, false, &v, None).cpu_cap, Some(20));
        assert_eq!(effective(priority, false, &v, Some(10)).cpu_cap, Some(10));
        // 100% is an explicit manual choice; it must not fall back to an automatic cap.
        let unlimited = Limits { cpu_cap: Some(100), ..Default::default() };
        assert_eq!(effective(unlimited, false, &v, Some(5)).cpu_cap, Some(100));
        assert_eq!(effective(cpu, true, &v, Some(5)), cpu);
        v.adaptive.enabled = false;
        assert_eq!(effective(cpu, false, &v, None), cpu);
        assert_eq!(effective(priority, false, &v, None), priority);
    }
}
