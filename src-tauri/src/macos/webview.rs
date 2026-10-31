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
use tauri::{AppHandle, WebviewUrl, WebviewWindowBuilder, WindowEvent};
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
) -> Result<(tauri::WebviewWindow, Arc<AtomicBool>)> {
    let w = WebviewWindowBuilder::new(
        app,
        format!("{prefix}-{}", uuid::Uuid::new_v4()),
        WebviewUrl::External("about:blank".parse().unwrap()),
    )
    .title(title)
    .inner_size(1100.0, 800.0)
    .incognito(true)
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
pub async fn capture_login(app: &AppHandle, _: &Path) -> Result<Secret> {
    let (w, closed) = window(app, "login", "Sign in to Roblox")?;
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
    let (w, closed) = window(app, "session", title)?;
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
