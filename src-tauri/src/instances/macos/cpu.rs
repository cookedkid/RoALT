use crate::error::{AppError, Result};
use std::{
    io::Read,
    os::unix::process::CommandExt,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
unsafe extern "C" {
    fn ro_cpu_governor(pid: i32, started: u64, percent: i32) -> i32;
}
pub fn helper_main() -> Option<i32> {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) != Some("--roalt-cpu-governor") {
        return None;
    }
    let parsed = || {
        Some((
            args.get(2)?.parse::<i32>().ok()?,
            args.get(3)?.parse::<u64>().ok()?,
            args.get(4)?.parse::<i32>().ok()?,
        ))
    };
    Some(match parsed() {
        Some((pid, start, cap)) => unsafe { ro_cpu_governor(pid, start, cap) },
        None => 2,
    })
}
pub struct Governor {
    child: Child,
}
impl Governor {
    pub fn start(pid: u32, start: u64, percent: u8) -> Result<Self> {
        #[cfg(test)]
        let mut cmd = Command::new(env!("ROALT_CPU_TEST_HELPER"));
        #[cfg(not(test))]
        let mut cmd = {
            let mut c = Command::new(std::env::current_exe()?);
            c.arg("--roalt-cpu-governor");
            c
        };
        let mut child = cmd
            .args([pid.to_string(), start.to_string(), percent.to_string()])
            .process_group(0)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let mut output = child.stdout.take().unwrap();
        let (send, receive) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut ready = [0];
            let ok = output.read_exact(&mut ready).is_ok() && ready == [b'R'];
            let _ = send.send(ok);
        });
        let mut g = Self { child };
        if receive.recv_timeout(Duration::from_secs(2)) != Ok(true) {
            g.stop();
            return Err(AppError::Invalid(
                "CPU governor could not start safely. The client was left running.".into(),
            ));
        }
        Ok(g)
    }
    pub fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }
    pub fn stop(&mut self) {
        drop(self.child.stdin.take());
        let deadline = Instant::now() + Duration::from_millis(600);
        while Instant::now() < deadline {
            if !self.alive() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        // The independent guardian resumes the target if this governor is unresponsive.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Drop for Governor {
    fn drop(&mut self) {
        self.stop();
    }
}
#[cfg(test)]
mod tests {
    use super::super::{native, process};
    use super::*;
    struct Owned(Child);
    impl Drop for Owned {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    fn busy() -> Owned {
        Owned(
            Command::new("/usr/bin/yes")
                .stdout(Stdio::null())
                .spawn()
                .unwrap(),
        )
    }
    fn measure(pid: u32, duration: Duration) -> u64 {
        let a = native::process(pid).unwrap().cpu;
        std::thread::sleep(duration);
        native::process(pid).unwrap().cpu.saturating_sub(a)
    }
    #[test]
    fn cap_and_governor_death_resume_owned_process() {
        let child = busy();
        let pid = child.0.id();
        let start = process::start_time(pid).unwrap();
        let full = measure(pid, Duration::from_millis(300));
        let mut g = Governor::start(pid, start, 1).unwrap();
        let limited = measure(pid, Duration::from_millis(600));
        assert!(
            limited < full * 18 / 10,
            "CPU was not reduced: full300ms={full}, limited600ms={limited}"
        );
        let _ = g.child.kill();
        let _ = g.child.wait();
        std::thread::sleep(Duration::from_millis(400));
        assert!(
            measure(pid, Duration::from_millis(200)) > 50_000_000,
            "watchdog failed to resume client"
        );
    }
    #[test]
    fn parent_pipe_close_restores_owned_process() {
        let child = busy();
        let pid = child.0.id();
        let mut g = Governor::start(pid, process::start_time(pid).unwrap(), 1).unwrap();
        std::thread::sleep(Duration::from_millis(100));
        drop(g.child.stdin.take());
        std::thread::sleep(Duration::from_millis(400));
        assert!(!g.alive());
        assert!(measure(pid, Duration::from_millis(150)) > 40_000_000);
    }
    #[test]
    fn stopped_governor_lease_resumes_owned_process() {
        let child = busy();
        let pid = child.0.id();
        let mut g = Governor::start(pid, process::start_time(pid).unwrap(), 1).unwrap();
        std::thread::sleep(Duration::from_millis(150));
        Command::new("/bin/kill")
            .args(["-STOP", &g.child.id().to_string()])
            .status()
            .unwrap();
        std::thread::sleep(Duration::from_millis(500));
        let progress = measure(pid, Duration::from_millis(150));
        Command::new("/bin/kill")
            .args(["-CONT", &g.child.id().to_string()])
            .status()
            .unwrap();
        g.stop();
        assert!(
            progress > 40_000_000,
            "expired lease did not resume child: {progress}"
        );
    }
    #[test]
    fn already_stopped_process_is_not_resumed() {
        let child = busy();
        let pid = child.0.id();
        Command::new("/bin/kill")
            .args(["-STOP", &pid.to_string()])
            .status()
            .unwrap();
        std::thread::sleep(Duration::from_millis(50));
        let rejected = Governor::start(pid, process::start_time(pid).unwrap(), 1).is_err();
        let progress = measure(pid, Duration::from_millis(100));
        Command::new("/bin/kill")
            .args(["-CONT", &pid.to_string()])
            .status()
            .unwrap();
        assert!(rejected);
        assert_eq!(progress, 0);
    }
    #[test]
    fn stale_identity_is_rejected() {
        let child = busy();
        assert!(Governor::start(child.0.id(), 1, 25).is_err());
    }
}
