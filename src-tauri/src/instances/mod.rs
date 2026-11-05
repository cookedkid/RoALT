//! Roblox clients RoALT started: the multi-instance lock, process tracking, log watching
//! and closing. Nothing here reads or writes another process's memory or handles; anti-AFK
//! (`afk`) presses a key the way a keyboard does.

#[cfg(windows)]
pub mod afk;
#[cfg(target_os = "macos")]
#[path = "macos/afk.rs"]
pub mod afk;
#[cfg(windows)]
mod capture;
#[cfg(windows)]
mod gpu;
#[cfg(windows)]
pub mod limits;
#[cfg(target_os = "macos")]
#[path = "macos/limits.rs"]
pub mod limits;
pub mod logwatch;
#[cfg(windows)]
pub mod metrics;
#[cfg(target_os = "macos")]
#[path = "macos/metrics.rs"]
pub mod metrics;
#[cfg(windows)]
pub mod multi;
#[cfg(target_os = "macos")]
#[path = "macos/multi.rs"]
pub mod multi;
#[cfg(windows)]
pub mod process;
#[cfg(target_os = "macos")]
#[path = "macos/process.rs"]
pub mod process;
#[cfg(windows)]
pub mod qos;
#[cfg(target_os = "macos")]
#[path = "macos/qos.rs"]
pub mod qos;
#[cfg(windows)]
pub mod resize;
#[cfg(target_os = "macos")]
#[path = "macos/resize.rs"]
pub mod resize;
#[cfg(windows)]
pub mod window;
#[cfg(target_os = "macos")]
#[path = "macos/window.rs"]
pub mod window;
mod tracker;
#[cfg(target_os = "macos")]
pub(crate) use tracker::Ended;

pub mod rejoin;
pub use tracker::{Tracker, Transition, challenged, closed, hung, unexpected, wrong_account};

#[cfg(target_os = "macos")]
#[path = "macos/native.rs"]
pub(crate) mod native;

#[cfg(target_os = "macos")]
#[path = "macos/cpu.rs"]
pub(crate) mod cpu;
