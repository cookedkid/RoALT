//! Isolated, ephemeral WKWebView sessions. Roblox pages cannot invoke main-window commands.
use crate::{
    error::{AppError, Result},
    vault::Secret,
};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};
fn allowed(url: &url::Url) -> bool {
    url.scheme() == "about"
        || (url.scheme() == "https"
            && url.host_str().is_some_and(|h| {
                h == "roblox.com"
                    || h.ends_with(".roblox.com")
                    || h == "arkoselabs.com"
                    || h.ends_with(".arkoselabs.com")
            }))
}
fn cookie(w: &tauri::WebviewWindow) -> Option<Secret> {
    w.cookies()
        .ok()?
        .into_iter()
        .find(|c| {
            c.name() == ".ROBLOSECURITY"
                && matches!(
                    c.domain(),
                    Some(".roblox.com" | "roblox.com" | "www.roblox.com")
                )
                && !c.value().is_empty()
        })
        .map(|c| Secret::new(c.value()))
}
fn window(
    app: &AppHandle,
    prefix: &str,
    title: &str,
    script: &str,
) -> Result<(tauri::WebviewWindow, Arc<AtomicBool>)> {
    let w = WebviewWindowBuilder::new(
        app,
        format!("{prefix}-{}", uuid::Uuid::new_v4()),
        WebviewUrl::External("about:blank".parse().unwrap()),
    )
    .title(title)
    .inner_size(1100.0, 800.0)
    .incognito(true)
    .initialization_script(script)
    .on_navigation(allowed)
    .build()?;
    let closed = Arc::new(AtomicBool::new(false));
    let flag = closed.clone();
    w.on_window_event(move |e| {
        if matches!(e, WindowEvent::Destroyed) {
            flag.store(true, Ordering::Release);
        }
    });
    Ok((w, closed))
}
pub async fn capture_login(app: &AppHandle, _: &Path, fill: Option<&LoginFill>) -> Result<Secret> {
    let script = fill.map(|f| fill_script(&f.username, f.password.expose()));
    let (w, closed) = window(
        app,
        "login",
        "Sign in to Roblox",
        script.as_deref().map_or("", |s| s.as_str()),
    )?;
    w.navigate("https://www.roblox.com/login".parse().unwrap())?;
    loop {
        if closed.load(Ordering::Acquire) {
            return Err(AppError::Cancelled);
        }
        if let Some(c) = cookie(&w) {
            let _ = w.close();
            return Ok(c);
        }
        tokio::time::sleep(Duration::from_millis(750)).await;
    }
}
pub async fn open_session(
    app: &AppHandle,
    _: &Path,
    title: &str,
    secret: Secret,
    on_close: impl FnOnce(Option<Secret>) + Send + 'static,
) -> Result<()> {
    let (w, closed) = window(app, "session", title, "")?;
    let c = tauri::webview::Cookie::build((".ROBLOSECURITY", secret.expose().to_owned()))
        .domain(".roblox.com")
        .path("/")
        .secure(true)
        .http_only(true)
        .build();
    if let Err(e) = w.set_cookie(c) {
        let _ = w.close();
        return Err(e.into());
    }
    w.navigate("https://www.roblox.com/home".parse().unwrap())?;
    tauri::async_runtime::spawn(async move {
        let mut last = None;
        while !closed.load(Ordering::Acquire) {
            if let Some(c) = cookie(&w) {
                last = Some(c)
            }
            tokio::time::sleep(Duration::from_millis(750)).await;
        }
        let _ = tauri::async_runtime::spawn_blocking(move || on_close(last)).await;
    });
    Ok(())
}
pub fn sweep_temp_profiles(_: &Path) {}
pub fn clear_webview2_env() {}
pub fn log_process_failures(_: &tauri::WebviewWindow) {}

use zeroize::Zeroizing;
pub struct LoginFill {
    pub username: String,
    pub password: Secret,
}

/// What fills Roblox's login form (pure): on www.roblox.com's top frame only, once its fields
/// show (up to 20 s), each one still empty; the input events tell its React state. Never presses
/// Log In: the user does, after any captcha. Values are JSON-escaped.
fn fill_script(username: &str, password: &str) -> Zeroizing<String> {
    const BODY: &str = r#"if (location.hostname !== "www.roblox.com" || window.top !== window) return;
const set = (el, v) => {
  Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set.call(el, v);
  el.dispatchEvent(new Event("input", { bubbles: true }));
  el.dispatchEvent(new Event("change", { bubbles: true }));
};
let tries = 0;
const timer = setInterval(() => {
  const u = document.getElementById("login-username"), p = document.getElementById("login-password");
  if (u && p) {
    clearInterval(timer);
    if (!u.value) set(u, user);
    if (!p.value) set(p, pass);
    p.focus();
  } else if (++tries >= 100) clearInterval(timer);
}, 200);"#;
    let json = |s: &str| Zeroizing::new(serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into()));
    Zeroizing::new(format!(
        "(() => {{\nconst user = {}, pass = {};\n{BODY}\n}})();",
        *json(username),
        *json(password)
    ))
}

pub fn close_login(app: &AppHandle) {
    for (label, w) in app.webview_windows() {
        if label.starts_with("login-") {
            let _ = w.close();
        }
    }
}
pub fn signing_in(app: &AppHandle) -> bool {
    app.webview_windows()
        .keys()
        .any(|l| l.starts_with("login-"))
}
pub fn browsing(app: &AppHandle) -> bool {
    app.webview_windows()
        .keys()
        .any(|l| l.starts_with("login-") || l.starts_with("session-"))
}
// WKWebView sessions are nonpersistent; there are no on-disk account profiles to remove.
pub fn remove_account_profile(_: &AppHandle, _: &Path, _: i64) {}
pub fn reset_profiles(app: &AppHandle, _: &Path) -> bool {
    !browsing(app)
}
pub fn sweep_dirty_profiles(_: &Path) {}
pub fn sweep_orphan_profiles(_: &Path, _: &[i64]) {}
// Hiding the webview and frontend visibility events suspend polls. WKWebView has no WebView2 memory target API.
pub fn memory_target(_: &tauri::Webview, _: bool) {}
