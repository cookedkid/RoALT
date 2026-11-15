//! What RoALT looks at by itself while it runs, so trouble is known before a launch finds it:
//! every account's sign-in (`settings.sign_in_watch`: 90 s after start, then every 6 hours), and
//! new incoming trades (`settings.discord.trade`, off by default: every 10 minutes, told to
//! Discord). Both only with the vault unlocked, one account at a time.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use tauri::{AppHandle, Manager};

use crate::App;
use crate::commands;
use crate::model::AccountStatus;

const SIGN_IN_FIRST: Duration = Duration::from_secs(90);
const SIGN_IN_EVERY: Duration = Duration::from_secs(6 * 3600);
/// When a check couldn't run (switched off, locked, a launch under way): looked at again this soon.
const SIGN_IN_RETRY: Duration = Duration::from_secs(5 * 60);
/// A sign-in checked within this long isn't asked about again by the watch.
const JUST_CHECKED_MS: i64 = 10 * 60_000;
const TRADES_EVERY: Duration = Duration::from_secs(10 * 60);
/// Between two accounts' trade lists.
const TRADES_GAP: Duration = Duration::from_millis(500);

/// Runs for the app's lifetime.
pub fn spawn(app: AppHandle) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(SIGN_IN_FIRST).await;
        loop {
            let wait = if sign_ins(&handle).await { SIGN_IN_EVERY } else { SIGN_IN_RETRY };
            tokio::time::sleep(wait).await;
        }
    });
    tauri::async_runtime::spawn(async move {
        // The trades each account had at the last look. In memory: after a restart the first look
        // only takes stock, so trades that came while RoALT was closed aren't told.
        let mut seen = HashMap::new();
        loop {
            tokio::time::sleep(TRADES_EVERY).await;
            trades(&app, &mut seen).await;
        }
    });
}

/// Checks the sign-in of every account not known dead already (`commands::validate_accounts`:
/// one that died is marked, and told to Discord). False when it didn't run now.
async fn sign_ins(app: &AppHandle) -> bool {
    let s = app.state::<App>();
    if !s.db.settings().is_ok_and(|v| v.sign_in_watch) || s.vault.is_locked() || s.bulk.any_busy() {
        return false;
    }
    // Not one checked a moment ago (the Accounts list checks once when it first shows).
    let stale = commands::now_ms() - JUST_CHECKED_MS;
    let ids: Vec<i64> = match s.db.accounts() {
        Ok(list) => list.into_iter().filter(|a| a.status != AccountStatus::Invalid && a.status_checked_at.is_none_or(|t| t < stale)).map(|a| a.id).collect(),
        Err(e) => {
            log::info!("sign-in watch: accounts unreadable: {e}");
            return false;
        }
    };
    if ids.is_empty() {
        return true;
    }
    log::debug!("sign-in watch: checking {} accounts", ids.len());
    commands::validate_accounts(app, &s, Some(ids)).await.inspect_err(|e| log::info!("sign-in watch: not checked: {e}")).is_ok()
}

/// Looks at every valid account's incoming trades and tells Discord of the ones that weren't there
/// at the last look (`seen`). While switched off, locked or without a webhook nothing is asked and
/// `seen` is forgotten: switching it on doesn't tell old trades.
async fn trades(app: &AppHandle, seen: &mut HashMap<i64, HashSet<String>>) {
    let s = app.state::<App>();
    if !s.db.settings().is_ok_and(|v| v.discord.trade) || s.vault.is_locked() || !crate::discord::configured(&s) {
        seen.clear();
        return;
    }
    let accounts: Vec<_> = s.db.accounts().unwrap_or_default().into_iter().filter(|a| a.status == AccountStatus::Valid).collect();
    seen.retain(|id, _| accounts.iter().any(|a| a.id == *id));
    for a in &accounts {
        let rows = match commands::inbound_trades(app, &s, a.id).await {
            Ok(rows) => rows,
            Err(e) => {
                log::debug!("trade watch: account {} not read: {e}", a.id);
                continue;
            }
        };
        let now: HashSet<String> = rows.iter().map(|r| r.id.clone()).collect();
        if let Some(before) = seen.insert(a.id, now) {
            let new: Vec<_> = rows.iter().filter(|r| !before.contains(&r.id)).collect();
            if let Some(first) = new.first() {
                crate::discord::trades(app, a.id, new.len(), &first.partner_name);
            }
        }
        tokio::time::sleep(TRADES_GAP).await;
    }
}
