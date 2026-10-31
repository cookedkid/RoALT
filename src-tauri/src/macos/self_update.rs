use crate::{
    App,
    error::{AppError, Result},
    model::AppUpdate,
};
pub struct SelfUpdater;
impl SelfUpdater {
    pub fn new() -> Self {
        Self
    }
    pub fn installing(&self) -> bool {
        false
    }
}
pub fn status(_: &App) -> AppUpdate {
    AppUpdate {
        current: env!("CARGO_PKG_VERSION").into(),
        available: None,
    }
}
pub async fn check(_: &tauri::AppHandle) -> Result<AppUpdate> {
    Err(AppError::Invalid(
        "Automatic updates are unavailable for this macOS build.".into(),
    ))
}
pub async fn install(_: &tauri::AppHandle) -> Result<()> {
    Err(AppError::Invalid(
        "Install a new macOS build manually.".into(),
    ))
}
pub fn switched(_: &tauri::AppHandle, _: bool) {}
pub fn spawn(_: tauri::AppHandle) {}
