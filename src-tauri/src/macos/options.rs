//! Mac-only options. The parent module and command registration must be cfg(macos).
use crate::{
    App,
    error::{AppError, Result},
};
use serde::{Deserialize, Serialize};
use std::{io::Write, path::Path, process::Command, sync::Mutex};
use tauri::State;

static WRITE_LOCK: Mutex<()> = Mutex::new(());
const FILE: &str = "macos-options.json";
const NAP_KEY: &str = "NSAppSleepDisabled";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Options {
    pub key_code: u16,
    pub focus_fallback: bool,
    pub disable_app_nap: bool,
    pub release_unused_memory: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            key_code: 48,
            focus_fallback: false,
            disable_app_nap: false,
            release_unused_memory: false,
        }
    }
}
impl Options {
    fn validate(&self) -> Result<()> {
        if !matches!(self.key_code, 48 | 49) {
            return Err(AppError::Invalid(
                "Choose Tab (48) or Space (49) for anti-AFK.".into(),
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct NapBackup {
    domain: String,
    previous: Option<bool>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct Stored {
    options: Options,
    nap_backup: Option<NapBackup>,
}

fn load(dir: &Path) -> Result<Stored> {
    match std::fs::read(dir.join(FILE)) {
        Ok(bytes) => {
            let stored: Stored = serde_json::from_slice(&bytes)
                .map_err(|_| AppError::Invalid("RoALT's macOS options file is invalid.".into()))?;
            stored.options.validate()?;
            Ok(stored)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Stored::default()),
        Err(e) => Err(e.into()),
    }
}
pub fn options_dir(app: &App) -> std::path::PathBuf {
    if crate::data_dir_overridden() { app.data_dir.clone() } else { crate::default_dir() }
}
pub fn read(dir: &Path) -> Result<Options> {
    Ok(load(dir)?.options)
}
fn save(dir: &Path, state: &Stored) -> Result<()> {
    use std::os::unix::fs::{OpenOptionsExt, DirBuilderExt};
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    let path = dir.join("macos-options.json.tmp");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&path)?;
    file.write_all(
        &serde_json::to_vec_pretty(state).map_err(|e| AppError::Invalid(e.to_string()))?,
    )?;
    file.sync_all()?;
    std::fs::rename(path, dir.join(FILE))?;
    Ok(())
}
fn domain_valid(domain: &str) -> bool {
    !domain.is_empty()
        && !domain.starts_with('-')
        && domain.contains('.')
        && domain
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
}
fn roblox_domain() -> Result<String> {
    let exe = crate::launcher::client::find()?;
    let contents = exe
        .path
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| AppError::Invalid("Roblox's app bundle could not be located.".into()))?;
    let output = Command::new("/usr/libexec/PlistBuddy")
        .args(["-c", "Print :CFBundleIdentifier"])
        .arg(contents.join("Info.plist"))
        .output()?;
    let domain = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if !output.status.success() || !domain_valid(&domain) {
        return Err(AppError::Invalid(
            "Roblox's bundle identifier could not be read.".into(),
        ));
    }
    Ok(domain)
}
fn defaults(args: &[&str]) -> Result<std::process::Output> {
    Ok(Command::new("/usr/bin/defaults")
        .env("LC_ALL", "C")
        .args(args)
        .output()?)
}
fn absent(output: &std::process::Output) -> bool {
    !output.status.success() && String::from_utf8_lossy(&output.stderr).contains("does not exist")
}
fn nap_value(domain: &str) -> Result<Option<bool>> {
    if !domain_valid(domain) {
        return Err(AppError::Invalid(
            "Invalid saved Roblox bundle identifier.".into(),
        ));
    }
    let kind = defaults(&["read-type", domain, NAP_KEY])?;
    if absent(&kind) {
        return Ok(None);
    }
    if !kind.status.success() || !String::from_utf8_lossy(&kind.stdout).contains("boolean") {
        return Err(AppError::Invalid("Could not read Roblox's existing App Nap preference as a boolean; it was left unchanged.".into()));
    }
    let value = defaults(&["read", domain, NAP_KEY])?;
    match String::from_utf8_lossy(&value.stdout).trim() {
        "1" if value.status.success() => Ok(Some(true)),
        "0" if value.status.success() => Ok(Some(false)),
        _ => Err(AppError::Invalid(
            "Could not read Roblox's App Nap preference.".into(),
        )),
    }
}
fn write_nap(domain: &str, value: Option<bool>) -> Result<()> {
    let output = match value {
        Some(v) => defaults(&[
            "write",
            domain,
            NAP_KEY,
            "-bool",
            if v { "YES" } else { "NO" },
        ])?,
        None => defaults(&["delete", domain, NAP_KEY])?,
    };
    if !output.status.success() && !(value.is_none() && absent(&output)) {
        return Err(AppError::Invalid(
            "macOS refused to change Roblox's App Nap preference.".into(),
        ));
    }
    if nap_value(domain)? != value {
        return Err(AppError::Invalid(
            "Roblox's App Nap preference did not match the requested value.".into(),
        ));
    }
    Ok(())
}
// Only restore a value RoALT still owns; preserve a later external change.
fn restoration(previous: Option<bool>, current: Option<bool>) -> Option<Option<bool>> {
    (current == Some(true)).then_some(previous)
}

#[tauri::command]
pub fn mac_options_get(s: State<'_, App>) -> Result<Options> {
    read(&options_dir(&s))
}

#[tauri::command]
pub fn mac_options_set(s: State<'_, App>, options: Options) -> Result<Options> {
    options.validate()?;
    let _guard = WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut stored = load(&options_dir(&s))?;
    let mut rollback: Option<(String, Option<bool>)> = None;
    if options.disable_app_nap {
        if stored.nap_backup.is_none() {
            let domain = roblox_domain()?;
            stored.nap_backup = Some(NapBackup {
                previous: nap_value(&domain)?,
                domain,
            });
            // Journal the original value before touching defaults, including across a crash.
            save(&options_dir(&s), &stored)?;
        }
        let backup = stored.nap_backup.as_ref().unwrap();
        rollback = Some((backup.domain.clone(), nap_value(&backup.domain)?));
        write_nap(&backup.domain, Some(true))?;
    } else if let Some(backup) = stored.nap_backup.as_ref() {
        let current = nap_value(&backup.domain)?;
        if let Some(value) = restoration(backup.previous, current) {
            rollback = Some((backup.domain.clone(), current));
            write_nap(&backup.domain, value)?;
        }
        stored.nap_backup = None;
    }
    stored.options = options.clone();
    if let Err(error) = save(&options_dir(&s), &stored) {
        if let Some((domain, value)) = rollback {
            if write_nap(&domain, value).is_err() {
                return Err(AppError::Invalid("Could not save Mac settings or restore the App Nap preference. The original preference remains in macos-options.json for recovery.".into()));
            }
        }
        return Err(error);
    }
    Ok(options)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn safe_defaults_and_key_validation() {
        let defaults: Options = serde_json::from_str("{}").unwrap();
        assert_eq!(defaults, Options::default());
        assert!(!defaults.disable_app_nap && !defaults.focus_fallback && !defaults.release_unused_memory);
        assert!(
            Options {
                key_code: 49,
                ..defaults.clone()
            }
            .validate()
            .is_ok()
        );
        assert!(
            Options {
                key_code: 12,
                ..defaults
            }
            .validate()
            .is_err()
        );
    }
    #[test]
    fn restores_prior_value_without_overwriting_external_changes() {
        assert_eq!(restoration(None, Some(true)), Some(None));
        assert_eq!(restoration(Some(false), Some(true)), Some(Some(false)));
        assert_eq!(restoration(Some(true), Some(true)), Some(Some(true)));
        assert_eq!(restoration(None, Some(false)), None);
        assert_eq!(restoration(Some(true), None), None);
    }
    #[test]
    fn rejects_domains_that_could_be_options_or_paths() {
        assert!(domain_valid("com.example.Player"));
        assert!(!domain_valid("-globalDomain"));
        assert!(!domain_valid("/tmp/com.example.Player"));
        assert!(!domain_valid(""));
    }
    #[test]
    fn journal_preserves_original_state() {
        let state = Stored {
            options: Options::default(),
            nap_backup: Some(NapBackup {
                domain: "com.example.Player".into(),
                previous: Some(false),
            }),
        };
        let decoded: Stored = serde_json::from_slice(&serde_json::to_vec(&state).unwrap()).unwrap();
        assert_eq!(decoded.nap_backup.unwrap().previous, Some(false));
        assert!(!decoded.options.disable_app_nap);
    }
}
