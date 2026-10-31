use super::client;
use crate::{
    App,
    error::{AppError, Result},
    model::RobloxClient,
};
pub struct Updater;
impl Updater {
    pub fn new() -> Self {
        Self
    }
    pub fn installing(&self) -> bool {
        false
    }
}
pub async fn launch_client(_: &App) -> Result<(client::ClientExe, bool)> {
    Ok((client::find()?, true))
}
pub async fn update_hold(_: &App, _: usize) -> Option<String> {
    None
}
pub fn client(_: &App) -> RobloxClient {
    RobloxClient {
        path: client::find().ok().map(|e| e.path.display().to_string()),
        bootstrapper: false,
        version: None,
        latest: None,
        outdated: false,
    }
}
pub fn spawn(_: tauri::AppHandle) {}
pub fn publish(app: &tauri::AppHandle) -> RobloxClient {
    use tauri::Manager;
    client(&app.state::<App>())
}
pub async fn install(_: &tauri::AppHandle) -> Result<RobloxClient> {
    Err(AppError::Invalid(
        "Install or update Roblox using the official Roblox website on macOS.".into(),
    ))
}

pub const INSTALL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(600);
pub fn is_version(s: &str) -> bool {
    s.strip_prefix("version-").is_some_and(|h| {
        h.len() == 16
            && h.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
