//! Mac previews are GitHub prereleases. Trust metadata from the fixed fork over TLS;
//! verify the asset digest, app identity, version, arm64 binary and code signature
//! before handing the replacement to an embedded installer.
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::App;
use crate::error::{AppError, Result};
use crate::model::AppUpdate;

const CURRENT: &str = env!("CARGO_PKG_VERSION");
const FEED: &str = "https://api.github.com/repos/cookedkid/RoALT/releases?per_page=100";
const ARCHIVE: &str = "RoALT-macOS-Apple-Silicon.zip";
const INSTALLER: &str = include_str!("../../../scripts/install-macos-release.sh");

#[derive(Default)]
pub struct SelfUpdater {
    available: Mutex<Option<String>>,
    installing: AtomicBool,
}
impl SelfUpdater {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn installing(&self) -> bool {
        self.installing.load(Ordering::SeqCst)
    }
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    assets: Vec<Asset>,
}
#[derive(Deserialize)]
struct Asset {
    name: String,
    digest: Option<String>,
}
struct Update {
    version: String,
    tag: String,
    digest: String,
}

fn version_key(version: &str) -> Option<[u32; 3]> {
    let parts: Vec<_> = version
        .split('.')
        .map(|s| {
            let n: u32 = s.parse().ok()?;
            (s == n.to_string()).then_some(n)
        })
        .collect::<Option<_>>()?;
    parts.try_into().ok()
}

fn pick(releases: Vec<Release>, current: &str) -> Result<Option<Update>> {
    let here = version_key(current)
        .ok_or_else(|| AppError::Invalid("Invalid installed version.".into()))?;
    let best = releases
        .into_iter()
        .filter(|r| !r.draft)
        .filter_map(|r| {
            let (version, run) = r
                .tag_name
                .strip_prefix("macos-preview-")?
                .rsplit_once('-')?;
            let key = version_key(version)?;
            let run: u64 = run.parse().ok()?;
            let version = version.to_owned();
            (key > here).then_some(((key, run), version, r))
        })
        .max_by_key(|(key, _, _)| *key);
    let Some((_, version, release)) = best else {
        return Ok(None);
    };
    let digest = release
        .assets
        .iter()
        .find(|a| a.name == ARCHIVE)
        .and_then(|a| a.digest.as_deref()?.strip_prefix("sha256:"))
        .filter(|s| {
            s.len() == 64
                && s.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
        .ok_or_else(|| {
            AppError::Invalid(
                "The new release has no verified Mac ZIP yet. Try again later.".into(),
            )
        })?;
    if !release.assets.iter().any(|a| a.name == "SHA256SUMS.txt") {
        return Err(AppError::Invalid(
            "The Mac release checksum is missing.".into(),
        ));
    }
    Ok(Some(Update {
        version,
        tag: release.tag_name,
        digest: digest.to_owned(),
    }))
}

async fn latest() -> Result<Option<Update>> {
    let client = reqwest::Client::builder()
        .user_agent("RoALT-Mac-Updater")
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(25))
        .build()
        .map_err(|e| AppError::Network(e.to_string()))?;
    let mut response = client
        .get(FEED)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| AppError::Network(format!("Could not check GitHub releases: {e}")))?;
    let mut data = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| AppError::Network(e.to_string()))?
    {
        if data.len() + chunk.len() > 4_000_000 {
            return Err(AppError::Invalid("Release list too large.".into()));
        }
        data.extend_from_slice(&chunk);
    }
    pick(
        serde_json::from_slice(&data)
            .map_err(|_| AppError::Invalid("Invalid Mac release list.".into()))?,
        CURRENT,
    )
}

pub fn status(app: &App) -> AppUpdate {
    AppUpdate {
        current: CURRENT.into(),
        available: app
            .self_update
            .available
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone(),
    }
}
pub async fn check(app: &AppHandle) -> Result<AppUpdate> {
    let version = latest().await?.map(|u| u.version);
    let s = app.state::<App>();
    *s.self_update
        .available
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = version;
    let result = status(&s);
    let _ = app.emit("app://update", &result);
    Ok(result)
}
async fn background_check(app: &AppHandle) {
    if let Err(e) = check(app).await {
        log::info!("Mac update check: {e}");
    }
}
pub fn switched(app: &AppHandle, on: bool) {
    if on {
        let handle = app.clone();
        tauri::async_runtime::spawn(async move { background_check(&handle).await });
    } else {
        let s = app.state::<App>();
        s.self_update
            .available
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        let _ = app.emit("app://update", status(&s));
    }
}
pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            if app
                .state::<App>()
                .db
                .settings()
                .is_ok_and(|s| s.self_update)
            {
                background_check(&app).await;
            }
            tokio::time::sleep(Duration::from_secs(6 * 3600)).await;
        }
    });
}

fn idle(app: &AppHandle) -> Result<()> {
    if app.state::<App>().bulk.any_busy() {
        return Err(AppError::Wait(
            "Finish the current launch or rejoin before updating RoALT.".into(),
        ));
    }
    Ok(())
}
fn destination() -> Result<PathBuf> {
    let exe = std::env::current_exe()?.canonicalize()?;
    let bundle = exe
        .ancestors()
        .nth(3)
        .ok_or_else(|| AppError::Invalid("Run the installed Mac app to update.".into()))?;
    let user_apps = PathBuf::from(
        std::env::var_os("HOME")
            .ok_or_else(|| AppError::Invalid("Missing home directory.".into()))?,
    )
    .join("Applications/RoALT.app");
    if !(bundle == user_apps || bundle == std::path::Path::new("/Applications/RoALT.app"))
        || exe != bundle.join("Contents/MacOS/roalt")
    {
        return Err(AppError::Invalid(
            "Move RoALT.app into Applications or ~/Applications, reopen it there, then update."
                .into(),
        ));
    }
    Ok(bundle.to_owned())
}
fn stage(app: &AppHandle, update: Update, destination: PathBuf) -> Result<()> {
    // Merge diagnostics and readiness onto one pipe. The embedded installer only
    // replaces the bundle after both an explicit commit and this process's exit.
    let script = format!("exec 2>&1\n{INSTALLER}");
    let mut child = Command::new("/bin/bash")
        .args([
            "-c",
            &script,
            "roalt-updater",
            "--in-app",
            &update.tag,
            &update.version,
            &update.digest,
        ])
        .arg(destination)
        .arg(std::process::id().to_string())
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env_remove("BASH_ENV")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let mut input = child.stdin.take().expect("piped stdin");
    let output = child.stdout.take().expect("piped stdout");
    let mut detail = String::new();
    let result = (|| {
        for line in BufReader::new(output).lines() {
            let line = line?;
            if line == "ROALT_UPDATE_READY" {
                idle(app)?;
                writeln!(input, "install")?;
                return Ok(());
            }
            if !line.trim().is_empty() {
                detail = line.chars().take(500).collect();
            }
        }
        Err(AppError::Invalid(format!(
            "Could not stage the update. {detail}"
        )))
    })();
    drop(input);
    if result.is_err() {
        let _ = child.wait();
    }
    result
}
pub async fn install(app: &AppHandle) -> Result<()> {
    let s = app.state::<App>();
    if s.self_update.installing.swap(true, Ordering::SeqCst) {
        return Err(AppError::Wait("An update is already in progress.".into()));
    }
    let result = async {
        idle(app)?;
        let dest = destination()?;
        let update = latest()
            .await?
            .ok_or_else(|| AppError::Invalid("RoALT is already up to date.".into()))?;
        if status(&s).available.as_deref() != Some(&update.version) {
            return Err(AppError::Invalid(
                "The available release changed. Click Check now again.".into(),
            ));
        }
        let handle = app.clone();
        tauri::async_runtime::spawn_blocking(move || stage(&handle, update, dest))
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?
    }
    .await;
    if result.is_ok() {
        app.exit(0);
    } else {
        s.self_update.installing.store(false, Ordering::SeqCst);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn release(tag: &str) -> Release {
        Release {
            tag_name: tag.into(),
            draft: false,
            assets: vec![
                Asset {
                    name: ARCHIVE.into(),
                    digest: Some(format!("sha256:{}", "a".repeat(64))),
                },
                Asset {
                    name: "SHA256SUMS.txt".into(),
                    digest: None,
                },
            ],
        }
    }
    #[test]
    fn selects_newest_complete_mac_preview_only() {
        let mut draft = release("macos-preview-99.0.0-1");
        draft.draft = true;
        let choices = vec![
            draft,
            release("v99.0.0"),
            release("macos-preview-0.10.0-2"),
            release("macos-preview-0.9.0-9"),
            release("macos-preview-0.10.0-3"),
        ];
        assert_eq!(
            pick(choices, "0.8.1").unwrap().unwrap().tag,
            "macos-preview-0.10.0-3"
        );
        assert!(
            pick(
                vec![
                    release("macos-preview-0.8.1-5"),
                    release("macos-preview-0.8.0-4")
                ],
                "0.8.1"
            )
            .unwrap()
            .is_none()
        );
    }
    #[test]
    fn rejects_missing_digest_and_checksum() {
        let mut r = release("macos-preview-1.0.0-1");
        r.assets[0].digest = None;
        assert!(pick(vec![r], "0.8.1").is_err());
        let mut r = release("macos-preview-1.0.0-1");
        r.assets.pop();
        assert!(pick(vec![r], "0.8.1").is_err());
    }
}
