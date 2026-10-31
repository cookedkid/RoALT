use std::sync::atomic::{AtomicBool,AtomicUsize,Ordering};
use crate::{error::Result,model::MultiInstanceStatus};
pub struct Multi { enabled:AtomicBool, launching:AtomicUsize }
pub struct Launching<'a>(&'a Multi);
impl Drop for Launching<'_>{fn drop(&mut self){self.0.launching.fetch_sub(1,Ordering::SeqCst);}}
impl Multi {
 pub fn new()->Self{Self{enabled:AtomicBool::new(false),launching:AtomicUsize::new(0)}}
 pub fn enable(&self)->Result<()>{self.enabled.store(true,Ordering::SeqCst);Ok(())}
 pub fn take_over(&self)->Result<()>{self.enable()}
 pub fn disable(&self){self.enabled.store(false,Ordering::SeqCst);}
 pub fn is_active(&self)->bool{self.enabled.load(Ordering::SeqCst)}
 // Shared-cookie protection belongs to each child's OS sandbox, not a parent-held file lock.
 pub fn lock_shared(&self){} pub fn release_shared(&self){}
 pub fn launching(&self,_:bool)->Launching<'_>{self.launching.fetch_add(1,Ordering::SeqCst);Launching(self)}
 pub fn release_if(&self,_:impl FnOnce()->bool){}
 pub fn in_flight(&self)->bool{self.launching.load(Ordering::SeqCst)>0}
 pub fn adopt(&self,_:&[(u32,u64)]){}
 pub fn has_survivors(&self)->bool{!super::process::roblox_processes().is_empty()}
 pub fn status(&self,enabled:bool,protect_shared_login:bool)->MultiInstanceStatus {
  MultiInstanceStatus{enabled,active:self.is_active(),protect_shared_login,blocked_by:vec![],message:Some("macOS launches separate clients directly. Shared-cookie protection is enforced for newly launched clients; existing clients are not changed.".into())}
 }
}
