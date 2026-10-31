use crate::{
    error::{AppError, Result},
    vault::Secret,
};
use std::path::PathBuf;
#[derive(Clone, Debug)]
pub struct ClientExe {
    pub path: PathBuf,
    pub bootstrapper: bool,
}
pub fn find() -> Result<ClientExe> {
    let mut paths = vec![PathBuf::from(
        "/Applications/Roblox.app/Contents/MacOS/RobloxPlayer",
    )];
    if let Some(home) = std::env::var_os("HOME") {
        paths.push(PathBuf::from(home).join("Applications/Roblox.app/Contents/MacOS/RobloxPlayer"));
    }
    paths
        .into_iter()
        .find(|p| p.is_file())
        .map(|path| ClientExe {
            path,
            bootstrapper: false,
        })
        .ok_or_else(|| AppError::Invalid("Install Roblox in Applications first.".into()))
}
/// Prevent the child from reading or changing another client's persisted native cookies.
/// `sandbox-exec` applies the deny rules to the child and descendants for their lifetime.
fn cookie_sandbox(home:&std::path::Path)->Result<String> {
 let home=home.to_str().ok_or_else(||AppError::Invalid("The home directory cannot be represented as text.".into()))?;
 let escaped=home.replace('\\',"\\\\").replace('"',"\\\"");
 Ok(format!(r#"(version 1)
(allow default)
(deny file-read-data file-write* (subpath "{escaped}/Library/Cookies"))
(deny file-read-data file-write* (subpath "{escaped}/Library/HTTPStorages"))
"#))
}
pub fn start(exe:&ClientExe,uri:&Secret,extra:&[String],multi:bool,protect:bool)->Result<(u32,u64)> {
 if !multi && !crate::instances::process::roblox_processes().is_empty(){return Err(AppError::Invalid("Enable multi-instance to launch another Roblox client.".into()))}
 if multi {
  // Unlink only the named singleton. Existing clients keep their open semaphore and continue.
  let error=unsafe{crate::instances::native::ro_unlink_singleton()};
  if error!=0{return Err(AppError::Invalid(format!("Could not prepare another Roblox instance: {}",std::io::Error::from_raw_os_error(error))))}
 }
 let mut command=if protect {
  let home=std::env::var_os("HOME").map(PathBuf::from).ok_or_else(||AppError::Invalid("HOME is not set.".into()))?;
  let mut c=std::process::Command::new("/usr/bin/sandbox-exec");
  c.arg("-p").arg(cookie_sandbox(&home)?).arg(&exe.path);c
 }else{std::process::Command::new(&exe.path)};
 // Roblox logs the complete protocol argument to stdout. Never inherit those streams.
 let mut child=command.arg("-protocolString").arg(uri.expose()).args(extra).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn()?;
 let pid=child.id();
 let started=(||->Result<u64>{
  let start=crate::instances::process::start_time(pid).ok_or_else(||AppError::Invalid("Roblox exited before it could be tracked.".into()))?;
  std::thread::sleep(std::time::Duration::from_millis(1500));
  if child.try_wait()?.is_some(){return Err(AppError::Invalid("Roblox exited during startup. Check its installation and the macOS console.".into()))}
  Ok(start)
 })();
 let start=match started{Ok(t)=>t,Err(e)=>{let _=child.kill();let _=child.wait();return Err(e)}};
 std::thread::spawn(move||{let _=child.wait();});
 Ok((pid,start))
}
#[cfg(test)] mod tests {
 use super::*;
 #[test] fn sandbox_blocks_shared_cookie_reads_and_writes(){
  let temp=std::env::temp_dir().join(format!("roalt-sandbox-{}",uuid::Uuid::new_v4()));
  let home=std::fs::canonicalize(std::env::temp_dir()).unwrap().join(temp.file_name().unwrap());
  let cookies=home.join("Library/HTTPStorages/com.roblox.RobloxPlayer.binarycookies");
  std::fs::create_dir_all(cookies.parent().unwrap()).unwrap();std::fs::write(&cookies,b"test-data").unwrap();
  let profile=cookie_sandbox(&home).unwrap();
  let read=std::process::Command::new("/usr/bin/sandbox-exec").args(["-p",&profile,"/bin/cat"]).arg(&cookies).output().unwrap();
  let write=std::process::Command::new("/usr/bin/sandbox-exec").args(["-p",&profile,"/usr/bin/touch"]).arg(&cookies).output().unwrap();
  std::fs::remove_dir_all(&home).unwrap();assert!(!read.status.success());assert!(!write.status.success());
 }
}
pub fn managed_by_bootstrapper(_: &ClientExe) -> bool {
    false
}
pub fn channel() -> Option<String> {
    None
}
pub fn bootstrapper_version(_: &ClientExe) -> Option<String> {
    None
}

pub(super) fn versions_dirs() -> Vec<PathBuf> {
    vec![]
}
