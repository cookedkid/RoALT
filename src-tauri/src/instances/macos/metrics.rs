use std::{collections::HashMap,sync::Mutex,time::{Instant,Duration}};
use crate::model::{ClientMetrics,Limits,SystemUsage};
use super::native;
const MB:f64=1048576.0;
#[derive(Clone,Copy)] pub struct Reading{pub private:u64}
pub struct Sampler {last:Mutex<HashMap<String,(u32,u64,Instant,native::Process)>>,system:Mutex<Option<(Instant,native::System,SystemUsage)>>}
impl Sampler {
 pub fn new()->Self{Self{last:Mutex::new(HashMap::new()),system:Mutex::new(None)}}
 pub fn system(&self)->Option<SystemUsage>{
  let mut prev=self.system.lock().unwrap_or_else(|e|e.into_inner());
  if let Some((at,_,usage))=&*prev{if at.elapsed()<Duration::from_secs(2){return Some(*usage)}}
  let r=native::system()?;
  let cpu_pct=prev.as_ref().and_then(|(_,p,_)|{let total=r.total.checked_sub(p.total)?;if total==0{return None}Some(100.0*(1.0-r.idle.saturating_sub(p.idle).min(total) as f64/total as f64))});
  unsafe extern "C" {fn ro_gpu_pct()->f64;}
  let gpu=unsafe{ro_gpu_pct()};
  let usage=SystemUsage{gpu_pct:(0.0..=100.0).contains(&gpu).then_some(gpu),cpu_pct,mem_used_mb:r.used as f64/MB,mem_total_mb:r.memory as f64/MB};
  *prev=Some((Instant::now(),r,usage));Some(usage)
 }
 pub fn sample(&self,clients:&[(String,u32,u64,Limits)])->Vec<ClientMetrics>{
  let mut last=self.last.lock().unwrap_or_else(|e|e.into_inner());
  last.retain(|id,_|clients.iter().any(|c|&c.0==id));
  let cores=std::thread::available_parallelism().map_or(1,|n|n.get()) as f64;
  let mut out=vec![];
  for (id,pid,start,_) in clients {
   let Some(r)=native::process(*pid).filter(|r|r.start==*start) else{continue};
   let now=Instant::now();let prev=last.get(id).filter(|p|p.0==*pid&&p.1==*start);
   let rate=|get:fn(native::Process)->u64|prev.and_then(|p|{let seconds=now.duration_since(p.2).as_secs_f64();(seconds>0.0).then(||get(r).saturating_sub(get(p.3)) as f64/seconds)});
   out.push(ClientMetrics{instance_id:id.clone(),pid:*pid,cpu_pct:rate(|r|r.cpu).map(|n|(n/1e9/cores*100.0).clamp(0.0,100.0)),working_set_mb:r.resident as f64/MB,private_mb:r.footprint as f64/MB,read_bps:rate(|r|r.read),write_bps:rate(|r|r.written),gpu_pct:None,gpu_mem_mb:None});
   last.insert(id.clone(),(*pid,*start,now,r));
  }
  out
 }
}
pub fn read(pid:u32,t0:u64)->Option<Reading>{native::process(pid).filter(|r|r.start==t0).map(|r|Reading{private:r.footprint})}
#[cfg(test)] mod tests{
 use super::*;
 #[test] fn cpu_counters_are_converted_to_nanoseconds(){
  let start=unsafe{native::ro_self_cpu_ns()};let deadline=Instant::now()+Duration::from_secs(2);
  while unsafe{native::ro_self_cpu_ns()}.saturating_sub(start)<100_000_000 && Instant::now()<deadline{std::hint::spin_loop();}
  let expected=unsafe{native::ro_self_cpu_ns()};let actual=native::process(std::process::id()).unwrap().cpu;
  assert!(actual.abs_diff(expected)<50_000_000,"libproc CPU {actual} ns differs from POSIX CPU {expected} ns");
 }
 #[test] fn real_native_metrics_and_stale_identity(){
  let pid=std::process::id();let start=native::process(pid).unwrap().start;
  let sampler=Sampler::new();let clients=vec![("self".into(),pid,start,Limits::default())];
  let a=sampler.sample(&clients);assert_eq!(a.len(),1);assert!(a[0].working_set_mb>0.0);assert!(a[0].private_mb>0.0);assert_eq!(a[0].cpu_pct,None);
  std::thread::sleep(Duration::from_millis(10));assert!(sampler.sample(&clients)[0].cpu_pct.is_some());
  assert!(sampler.system().unwrap().mem_total_mb>0.0);assert!(read(pid,start+1).is_none());
 }
}
pub fn mem_strikes(prev: u32, private_mb: u64, limit_mb: u32) -> u32 {
    if limit_mb > 0 && private_mb > u64::from(limit_mb) {
        prev.saturating_add(1)
    } else {
        0
    }
}

/// Whether a client is held by the memory throttle after a sample of `private_mb` (pure): on
/// over `limit_mb`, off again under 90 % of it, else as it was (`prev`). Never with 0 (off).
pub fn mem_throttled(prev: bool, private_mb: u64, limit_mb: u32) -> bool {
    let limit = u64::from(limit_mb);
    match limit_mb {
        0 => false,
        _ if private_mb > limit => true,
        _ if private_mb * 10 < limit * 9 => false,
        _ => prev,
    }
}

/// Samples over the limit in a row that restart a client.
pub const MEM_STRIKES: u32 = 3;
