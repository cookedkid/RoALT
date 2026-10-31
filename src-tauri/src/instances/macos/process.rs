use crate::error::{AppError, Result};
use std::{process::Command, time::Duration};
#[derive(Clone, Copy, Debug)]
pub struct ProcInfo {
    pub pid: u32,
    pub start_time: u64,
}
pub fn start_time(pid:u32)->Option<u64>{super::native::process(pid).map(|p|p.start)}

pub fn roblox_processes() -> Vec<ProcInfo> {
    let Ok(o) = Command::new("/usr/bin/pgrep")
        .args(["-x", "RobloxPlayer"])
        .output()
    else {
        return vec![];
    };
    String::from_utf8_lossy(&o.stdout)
        .lines()
        .filter_map(|s| {
            let pid = s.parse().ok()?;
            Some(ProcInfo {
                pid,
                start_time: start_time(pid)?,
            })
        })
        .collect()
}
pub fn installer_running() -> bool {
    false
}
pub fn close(pid: u32, expected: Option<u64>, force: bool, grace: Duration) -> Result<()> {
    let Some(t) = start_time(pid) else {
        return Ok(());
    };
    if expected.is_some_and(|e| e != t) {
        return Err(AppError::Invalid(
            "Process identity changed; refusing to close it.".into(),
        ));
    }
    if !roblox_processes().iter().any(|p| p.pid == pid) {
        return Err(AppError::Invalid("Not a Roblox process.".into()));
    }
    let status = Command::new("/bin/kill")
        .args([if force { "-KILL" } else { "-TERM" }, &pid.to_string()])
        .status()?;
    if !status.success() {
        return Err(AppError::Invalid("Could not close Roblox.".into()));
    }
    let until = std::time::Instant::now() + grace;
    while std::time::Instant::now() < until && start_time(pid) == Some(t) {
        std::thread::sleep(Duration::from_millis(100));
    }
    if start_time(pid)==Some(t) && !force {
        return close(pid,Some(t),true,Duration::from_secs(3));
    }
    if start_time(pid)==Some(t){return Err(AppError::Invalid("Roblox did not exit before the deadline.".into()))}
    Ok(())
}
pub fn hung(pid:u32) -> bool {
    use std::{collections::HashMap,sync::{Mutex,OnceLock}};
    unsafe extern "C" { fn ro_responsiveness(pid:i32)->i32; }
    static RESPONSIVE: OnceLock<Mutex<HashMap<u32,u64>>> = OnceLock::new();
    let Some(start)=start_time(pid) else{return false};
    let status=unsafe{ro_responsiveness(pid as i32)};
    let mut known=RESPONSIVE.get_or_init(Default::default).lock().unwrap_or_else(|e|e.into_inner());
    if status==1 {known.insert(pid,start);false} else {status==-1 && known.get(&pid)==Some(&start)}
}
pub fn main_window(pid:u32)->Option<(usize,bool)>{(unsafe{super::native::ro_has_window(pid as i32)}!=0).then_some((pid as usize,false))}
pub fn foreground_pid()->Option<u32>{let pid=unsafe{super::native::ro_foreground()};(pid>0).then_some(pid as u32)}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_is_unix_time_and_missing_process_is_absent() {
        let before = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let started = start_time(std::process::id()).expect("current process exists");
        assert!(
            started <= before && started > before - 3600,
            "process start must use Unix seconds"
        );
        assert_eq!(start_time(std::process::id()), Some(started));
        assert_eq!(start_time(u32::MAX), None);
    }
}
