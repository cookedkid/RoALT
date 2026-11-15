use crate::{App, model::Settings};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use ts_rs::TS;
#[path = "../clips/library.rs"]
pub mod library;
#[path = "../clips/serve.rs"]
pub mod serve;
/// What the Clips view shows.
#[derive(Serialize, TS, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ClipsState {
    /// No account picked.
    Off,
    /// The picked account has no client with a window.
    Waiting,
    Recording,
    /// Recording can't run here (`note` says why).
    Unavailable,
}

#[derive(Serialize, TS, Clone, Debug)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ClipsStatus {
    pub state: ClipsState,
    pub account_id: Option<i64>,
    /// Why it's unavailable, or why a recording has no sound.
    pub note: Option<String>,
    /// Recording, and its sound too.
    pub sound: bool,
}

pub fn set_key(_: Option<tauri_plugin_global_shortcut::Shortcut>) {}
pub fn is_key(_: &tauri_plugin_global_shortcut::Shortcut) -> bool {
    false
}
pub fn sync(_: &App, _: &Settings) {}
pub fn status(v: &Settings) -> ClipsStatus {
    ClipsStatus {
        state: ClipsState::Unavailable,
        account_id: v.clip_account,
        note: Some("Replay recording has not been ported to macOS.".into()),
        sound: false,
    }
}
pub fn save(app: &AppHandle) {
    let _ = app.emit(
        "clip://error",
        "Replay recording has not been ported to macOS.",
    );
}
