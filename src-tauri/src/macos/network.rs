use crate::{App, model::NetworkStatus};
pub struct Network;
impl Network {
    pub fn new() -> Self {
        Self
    }
    pub fn get(&self) -> NetworkStatus {
        NetworkStatus { public: false }
    }
}
pub fn blocked(s: &App) -> Option<String> {
    s.db.settings()
        .ok()
        .filter(|s| !s.allow_public_networks)
        .map(|_| {
            "macOS cannot classify public networks. Enable public networks in Settings to launch."
                .into()
        })
}
pub fn spawn(_: tauri::AppHandle) {}
