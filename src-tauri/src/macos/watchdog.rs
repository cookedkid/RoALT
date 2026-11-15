//! Native watchdog restart is not ported. Shared tray responsiveness fixes remain active.
pub const TRAY_ARG: &str = "--tray";
pub fn exiting() {}
pub fn spawn(_: tauri::AppHandle) {}
pub fn wait_for_previous() {}
