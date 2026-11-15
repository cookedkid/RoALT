use crate::{App, launcher::client::ClientExe};
use serde::Serialize;
use ts_rs::TS;
#[derive(Serialize, TS, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ModsLast {
    pub applied: bool,
    pub note: String,
}

pub fn last() -> Option<ModsLast> {
    Some(ModsLast {
        applied: false,
        note: "Modpack application has not been ported to macOS signed app bundles.".into(),
    })
}
pub fn prepare(s: &App, exe: ClientExe, _: bool) -> (ClientExe, Option<String>) {
    let note =
        s.db.settings()
            .ok()
            .filter(|v| v.mods_on)
            .map(|_| last().unwrap().note);
    (exe, note)
}
