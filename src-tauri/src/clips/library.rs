//! The saved clips: `.mp4` files in `Videos\RoALT`. What the Clips view lists, plays (through the
//! `clip` protocol, only these files), renames and deletes (to the Recycle Bin).

use std::path::{Path, PathBuf};

use serde::Serialize;
use ts_rs::TS;

use crate::error::{AppError, Result};

/// A clip being written; never listed, and swept at start.
pub const PART: &str = ".part.mp4";

#[derive(Serialize, TS, Clone, Debug)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ClipItem {
    pub name: String,
    #[ts(type = "number")]
    pub bytes: u64,
    /// Last written, Unix ms.
    #[ts(type = "number")]
    pub modified: i64,
}

/// `Videos\RoALT` (wherever Windows keeps the user's Videos).
#[cfg(windows)]
pub fn dir() -> Result<PathBuf> {
    use windows::Win32::UI::Shell::{FOLDERID_Videos, KF_FLAG_DEFAULT, SHGetKnownFolderPath};
    // SAFETY: asks for a known folder; the returned string is read once, then freed.
    let videos = unsafe {
        let p = SHGetKnownFolderPath(&FOLDERID_Videos, KF_FLAG_DEFAULT, None).map_err(|e| AppError::Internal(e.to_string()))?;
        let s = p.to_string().unwrap_or_default();
        windows::Win32::System::Com::CoTaskMemFree(Some(p.0.cast()));
        s
    };
    if videos.is_empty() {
        return Err(AppError::Internal("no Videos folder".into()));
    }
    Ok(PathBuf::from(videos).join("RoALT"))
}

/// Whether `name` is a clip file name RoALT may touch: one plain file name ending `.mp4`, no
/// path, nothing Windows would read as something else (pure).
pub fn valid(name: &str) -> bool {
    let Some(stem) = name.strip_suffix(".mp4") else { return false };
    let device = {
        let s = stem.split('.').next().unwrap_or("").trim_end_matches(' ').to_ascii_uppercase();
        ["CON", "PRN", "AUX", "NUL"].contains(&s.as_str()) || (s.len() == 4 && (s.starts_with("COM") || s.starts_with("LPT")) && s.as_bytes()[3].is_ascii_digit())
    };
    !stem.is_empty()
        && name.chars().count() <= 140
        && !name.ends_with(PART)
        && !name.chars().any(|c| c.is_control() || r#"\/:*?"<>|"#.contains(c))
        && !stem.ends_with(['.', ' '])
        && !stem.starts_with([' ', '.'])
        && !device
}

/// A clip's name from what it shows (pure): Windows-safe, at most 60 characters of it.
pub fn clean(title: &str) -> String {
    let t: String = title.chars().map(|c| if c.is_control() || r#"\/:*?"<>|"#.contains(c) { ' ' } else { c }).take(60).collect();
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    let t = t.trim_matches(['.', ' ']).to_owned();
    if t.is_empty() || !valid(&format!("{t}.mp4")) { "Roblox".into() } else { t }
}

/// A free path for a new clip named `title` and `stamp` (`2026-11-09 14-03-22`), with ` (2)` and
/// so on when taken.
pub fn new_path(dir: &Path, title: &str, stamp: &str) -> PathBuf {
    let base = format!("{} {stamp}", clean(title));
    (1..1000)
        .map(|n| dir.join(if n == 1 { format!("{base}.mp4") } else { format!("{base} ({n}).mp4") }))
        .find(|p| !p.exists())
        .unwrap_or_else(|| dir.join(format!("{base} {}.mp4", uuid::Uuid::new_v4())))
}

/// Every clip, newest first.
pub fn list() -> Result<Vec<ClipItem>> {
    let dir = dir()?;
    let Ok(entries) = std::fs::read_dir(&dir) else { return Ok(Vec::new()) };
    let mut out: Vec<ClipItem> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_owned();
            let meta = e.metadata().ok().filter(|m| m.is_file())?;
            valid(&name).then(|| ClipItem {
                name,
                bytes: meta.len(),
                modified: meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis() as i64),
            })
        })
        .collect();
    out.sort_by_key(|c| std::cmp::Reverse(c.modified));
    Ok(out)
}

/// The file of clip `name`, if it is one: only files directly in the folder.
pub fn path_of(name: &str) -> Option<PathBuf> {
    valid(name).then(|| dir().ok().map(|d| d.join(name))).flatten().filter(|p| p.is_file())
}

pub fn rename(name: &str, to: &str) -> Result<String> {
    rename_in(&dir()?, name, to)
}

/// Renames clip `name` in `dir` to `to` (".mp4" added); its new file name.
fn rename_in(dir: &Path, name: &str, to: &str) -> Result<String> {
    let from = dir.join(name);
    if !valid(name) || !from.is_file() {
        return Err(AppError::NotFound);
    }
    let to = format!("{}.mp4", to.trim().trim_end_matches(".mp4").trim());
    if !valid(&to) {
        return Err(AppError::Invalid(tr!("That name can't be a file name: leave out symbols like / : * ? < > |.")));
    }
    let target = from.with_file_name(&to);
    // Its own name, or the same in other letter case, is no clash: Windows names ignore case.
    if target.exists() && !to.eq_ignore_ascii_case(name) && to.to_lowercase() != name.to_lowercase() {
        return Err(AppError::Invalid(tr!("A clip with that name already exists.")));
    }
    std::fs::rename(&from, &target)?;
    Ok(to)
}

/// Moves clip `name` to the Recycle Bin.
#[cfg(windows)]
pub fn delete(name: &str) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::UI::Shell::{FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, FOF_WANTNUKEWARNING, SHFILEOPSTRUCTW, SHFileOperationW};
    let path = path_of(name).ok_or(AppError::NotFound)?;
    // Double-NUL-terminated, as the shell wants a list.
    let from: Vec<u16> = path.as_os_str().encode_wide().chain([0, 0]).collect();
    let mut op = SHFILEOPSTRUCTW {
        wFunc: FO_DELETE,
        pFrom: windows::core::PCWSTR(from.as_ptr()),
        // No confirmation, except Windows' own warning when the drive has no Recycle Bin.
        fFlags: (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_NOERRORUI | FOF_SILENT | FOF_WANTNUKEWARNING).0 as u16,
        ..Default::default()
    };
    // SAFETY: `from` outlives the call; no window, no confirmation.
    let done = unsafe { SHFileOperationW(&mut op) };
    if done != 0 || op.fAnyOperationsAborted.as_bool() {
        return Err(AppError::Internal(format!("the shell couldn't delete it ({done:#x})")));
    }
    Ok(())
}

/// Clips cut short by a quit or crash while being written.
pub fn sweep() {
    let Ok(dir) = dir() else { return };
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        if e.file_name().to_string_lossy().ends_with(PART) {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert!(valid("Sky Islands 2026-11-09 14-03-22.mp4"));
        assert!(valid("スカイ 2026-11-09 14-03-22 (2).mp4"));
        for bad in ["a.mp4.part.mp4", "../x.mp4", "a\\b.mp4", "nul.mp4", "nul .mp4", "COM1.mp4", "x.txt", ".mp4", " x.mp4", "x..mp4", "a:b.mp4", "x.mp4\0"] {
            assert!(!valid(bad), "{bad}");
        }
        assert_eq!(clean("Obby: Escape! <Hard>"), "Obby Escape! Hard");
        assert_eq!(clean("..."), "Roblox");
        assert_eq!(clean("nul"), "Roblox");
        assert_eq!(clean(&"x".repeat(100)).len(), 60);
    }

    #[test]
    fn renames() {
        let d = std::env::temp_dir().join(format!("roalt-clips-rn-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("Sky.mp4"), b"a").unwrap();
        std::fs::write(d.join("Obby.mp4"), b"b").unwrap();
        assert_eq!(rename_in(&d, "Sky.mp4", "Sky").unwrap(), "Sky.mp4", "its own name");
        assert_eq!(rename_in(&d, "Sky.mp4", "sky").unwrap(), "sky.mp4", "only the case");
        assert!(rename_in(&d, "sky.mp4", "Obby").is_err(), "another clip's name");
        assert!(rename_in(&d, "sky.mp4", "a/b").is_err(), "not a file name");
        assert!(matches!(rename_in(&d, "Gone.mp4", "x"), Err(AppError::NotFound)));
        assert_eq!(std::fs::read(d.join("Obby.mp4")).unwrap(), b"b", "never overwritten");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn new_names_never_overwrite() {
        let d = std::env::temp_dir().join(format!("roalt-clips-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        let a = new_path(&d, "Sky", "2026-11-09 14-03-22");
        std::fs::write(&a, b"x").unwrap();
        let b = new_path(&d, "Sky", "2026-11-09 14-03-22");
        assert_eq!(b.file_name().unwrap(), "Sky 2026-11-09 14-03-22 (2).mp4");
        std::fs::remove_dir_all(&d).unwrap();
    }
}

#[cfg(target_os="macos")]
pub fn dir()->Result<PathBuf>{ Ok(PathBuf::from(std::env::var_os("HOME").ok_or_else(||AppError::Internal("HOME missing".into()))?).join("Movies/RoALT")) }
#[cfg(target_os="macos")]
pub fn delete(name:&str)->Result<()> {
 let path=path_of(name).ok_or(AppError::NotFound)?;
 let trash=PathBuf::from(std::env::var_os("HOME").ok_or_else(||AppError::Internal("HOME missing".into()))?).join(".Trash");
 std::fs::rename(path,trash.join(format!("{}-{name}",uuid::Uuid::new_v4())))?;Ok(())
}
