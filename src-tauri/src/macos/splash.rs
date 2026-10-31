use tauri::{AppHandle, LogicalSize, WebviewWindow};
use tauri_plugin_window_state::{StateFlags, WindowExt};
pub const DEFAULT: (f64, f64) = (1280.0, 800.0);
#[derive(Default)]
pub struct Pending;
pub fn start(_: &AppHandle, w: &WebviewWindow, _: bool) -> tauri::Result<()> {
    w.restore_state(StateFlags::all() - StateFlags::VISIBLE)?;
    w.set_min_size(Some(LogicalSize::new(960.0, 600.0)))?;
    w.show()?;
    w.set_focus()
}
#[tauri::command]
pub async fn splash_done(_: AppHandle) {}
