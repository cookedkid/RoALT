use crate::{
    App,
    error::{AppError, Result},
    model::{LauncherKind, RobloxLauncher},
};
use tauri::AppHandle;
pub fn list() -> Vec<RobloxLauncher> {
    Vec::new()
}
pub fn set(_: &App, _: &str) -> Result<LauncherKind> {
    Err(AppError::Invalid(
        "Default Roblox launcher registration has not been ported to macOS.".into(),
    ))
}
pub fn reclaim(_: &App) {}
pub fn open(_: &AppHandle, _: &str) {}
pub async fn started() {}
