//! Typed boundary for the system-SDK shim in native/macos.m.
#[repr(C)] #[derive(Default,Clone,Copy)]
pub struct Process { pub start:u64,pub cpu:u64,pub resident:u64,pub footprint:u64,pub read:u64,pub written:u64 }
#[repr(C)] #[derive(Default,Clone,Copy)]
pub struct System { pub idle:u64,pub total:u64,pub used:u64,pub memory:u64 }
unsafe extern "C" {
 #[cfg(test)] pub fn ro_self_cpu_ns()->u64;
 fn ro_process(pid:i32,out:*mut Process)->i32;
 fn ro_system(out:*mut System)->i32;
 pub fn ro_background(pid:i32,on:i32)->i32;
 pub fn ro_thread_priority(pid:i32)->i32;
 pub fn ro_unlink_singleton()->i32;
 pub fn ro_foreground()->i32;
 pub fn ro_has_window(pid:i32)->i32;
 pub fn ro_window(pid:i32,action:i32,x:i32,y:i32,width:i32,height:i32)->i32;
}
pub fn process(pid:u32)->Option<Process>{let pid=i32::try_from(pid).ok()?;let mut r=Process::default();(unsafe{ro_process(pid,&mut r)}==0).then_some(r)}
pub fn system()->Option<System>{let mut r=System::default();(unsafe{ro_system(&mut r)}==0).then_some(r)}
