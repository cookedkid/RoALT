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

/// Only a player log actually open by this verified process, never a timestamp guess.
pub fn open_log(pid: u32, start: u64, dir: &std::path::Path) -> Option<std::path::PathBuf> {
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    unsafe extern "C" {
        fn ro_open_log(pid: i32, start: u64, directory: *const std::ffi::c_char, out: *mut std::ffi::c_char, capacity: i32) -> i32;
    }
    let dir = dir.canonicalize().ok()?;
    let directory = std::ffi::CString::new(dir.as_os_str().as_bytes()).ok()?;
    let mut out = [0u8; 4096];
    if unsafe { ro_open_log(pid as i32, start, directory.as_ptr(), out.as_mut_ptr().cast(), out.len() as i32) } != 1 { return None; }
    let end = out.iter().position(|&b| b == 0)?;
    Some(std::ffi::OsString::from_vec(out[..end].to_vec()).into())
}

#[cfg(test)]
mod log_tests {
    use super::*;
    #[test]
    fn binds_only_the_verified_process_player_log() {
        let dir = std::env::temp_dir().join(format!("roalt-pid-log-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let dir = dir.canonicalize().unwrap();
        let path = dir.join("sample_Player_one_last.log");
        let file = std::fs::File::create(&path).unwrap();
        let pid = std::process::id();
        let start = start_time(pid).unwrap();
        assert_eq!(open_log(pid, start, &dir), Some(path.clone()));
        assert_eq!(open_log(pid, start + 1, &dir), None);
        let crash = std::fs::File::create(dir.join("sample_Player_CrashHandler_last.log")).unwrap();
        assert_eq!(open_log(pid, start, &dir), Some(path));
        let other = std::fs::File::create(dir.join("sample_Player_two_last.log")).unwrap();
        assert_eq!(open_log(pid, start, &dir), None, "ambiguous logs must not be guessed");
        drop((file, crash, other));
        assert_eq!(open_log(pid, start, &dir), None);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
