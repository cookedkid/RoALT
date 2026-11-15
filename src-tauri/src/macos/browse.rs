use crate::{
    App,
    error::{AppError, Result},
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use ts_rs::TS;
#[derive(Serialize, TS, Clone, Copy, Debug)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum BrowseBlocked {
    Create,
    Robux,
    Money,
    SignOut,
}
#[derive(Deserialize, TS, Clone, Copy, Debug)]
#[ts(export)]
pub struct PaneRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
#[derive(Serialize, TS, Clone, Debug)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BrowsePage {
    pub account_id: i64,
    /// Path and query on www.roblox.com, like "games/123/x" or "search/users?keyword=bob".
    pub path: String,
}
#[derive(Serialize, Deserialize, TS, Clone, Debug)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PageCard {
    pub user_id: i64,
    pub title: String,
    /// The collapse button's label.
    pub toggle: String,
    pub rows: Vec<(String, String)>,
    pub note: Option<String>,
    pub top_title: String,
    pub top: Vec<(String, String)>,
}
#[derive(Deserialize, TS, Clone, Copy, Debug)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum BrowseNav {
    Back,
    Forward,
    Reload,
}
#[derive(Serialize, TS, Clone, Debug)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BrowsePlay {
    pub account_id: i64,
    pub target: crate::model::LaunchTarget,
}
fn unavailable() -> Result<()> {
    Err(AppError::Invalid("Embedded browsing with purchase protection has not been ported to macOS. Use Open on Roblox for an isolated account browser.".into()))
}
pub fn is_open() -> bool {
    false
}
pub fn set_lock(_: bool) {}
pub fn window_shown(_: &AppHandle, _: bool) {}
pub fn close(app: &AppHandle) {
    for (label, window) in app.webview_windows() {
        if label.starts_with("session-") || label.starts_with("login-") {
            let _ = window.close();
        }
    }
}
pub fn close_if(app: &AppHandle, _: i64) {
    close(app)
}
pub fn close_then(app: &AppHandle, then: impl FnOnce()) {
    close(app);
    then()
}
#[tauri::command]
pub fn browse_card(_app: AppHandle, card: Option<PageCard>) -> Result<()> {
    let _ = card;
    unavailable()
}
#[tauri::command]
pub async fn browse_open(
    _app: AppHandle,
    _s: State<'_, App>,
    account_id: i64,
    path: String,
    rect: PaneRect,
) -> Result<()> {
    let _ = (account_id, path, rect);
    unavailable()
}
#[tauri::command]
pub fn browse_bounds(_app: AppHandle, rect: PaneRect, shown: bool) -> Result<()> {
    let _ = (rect, shown);
    unavailable()
}
#[tauri::command]
pub fn browse_nav(_app: AppHandle, action: BrowseNav) -> Result<()> {
    let _ = action;
    unavailable()
}
#[tauri::command]
pub async fn browse_close(_app: AppHandle) -> Result<()> {
    Ok(())
}
