//! Read only recent macOS reports for the exact tracked Roblox process. Never export raw
//! reports (they contain usernames, identifiers and paths); emit fixed explanations only.
use crate::{App, instances::Ended, model::CrashKind};
use chrono::DateTime;
use serde_json::Value;
use std::{fs, io::Read, path::Path, time::UNIX_EPOCH};
use tauri::{AppHandle, Manager};

const MAX_REPORT: u64 = 4 * 1024 * 1024;

fn timestamp(text: &str) -> Option<i64> {
    DateTime::parse_from_str(text.trim(), "%Y-%m-%d %H:%M:%S%.f %z")
        .or_else(|_| DateTime::parse_from_rfc3339(text.trim()))
        .ok()
        .map(|d| d.timestamp())
}

fn explanation(namespace: &str, details: &str, exception: &str) -> &'static str {
    let detail = details.to_ascii_lowercase();
    if namespace == "TCC" {
        if detail.contains("nsmicrophoneusagedescription") {
            return "macOS stopped Roblox because a required microphone usage description is missing from the app bundle. Update or reinstall RoALT and Roblox. Turning microphone permission on alone does not repair missing app metadata.";
        }
        if detail.contains("nscamerausagedescription") {
            return "macOS stopped Roblox because a required camera usage description is missing from the app bundle. Update or reinstall RoALT and Roblox; changing the permission toggle alone does not repair app metadata.";
        }
        return "macOS stopped Roblox for a privacy-permission violation (TCC). Review Privacy & Security permissions and update RoALT and Roblox. This report does not establish a memory shortage.";
    }
    if namespace == "CODESIGNING" {
        return "macOS rejected Roblox's code signature. Reinstall Roblox from its official installer; the installed app may be damaged or modified.";
    }
    if namespace == "JETSAM" || (exception == "EXC_RESOURCE" && detail.contains("memory")) {
        return "The macOS report identifies a memory-resource termination. Reduce the number of simultaneous clients or the game's memory use before retrying.";
    }
    if exception == "EXC_BAD_ACCESS" {
        return "Roblox crashed while accessing invalid memory (EXC_BAD_ACCESS). This does not by itself mean the Mac ran out of RAM. Update Roblox; if it repeats, include RoALT diagnostics in a bug report.";
    }
    if detail.contains("sigabrt") {
        return "Roblox aborted (SIGABRT). The report does not identify a specific privacy or memory cause. Update Roblox and include RoALT diagnostics if it repeats.";
    }
    "A matching macOS crash report confirms Roblox terminated, but does not establish a specific cause. Update Roblox and include RoALT diagnostics if it repeats."
}

/// Modern .ips is either one JSON object or an incident header followed by the report JSON.
fn parse(text: &str, pid: u32, start: u64, ended_ms: i64) -> Option<&'static str> {
    let start = i64::try_from(start).ok()?;
    if start <= 0 {
        return None;
    }
    let mut objects = serde_json::Deserializer::from_str(text).into_iter::<Value>();
    let first = objects.next()?.ok()?;
    let report = if first.get("pid").is_some() {
        first
    } else {
        objects.next()?.ok()?
    };
    if report.get("pid")?.as_u64()? != u64::from(pid)
        || report.get("procName")?.as_str()? != "RobloxPlayer"
        || timestamp(report.get("procLaunch")?.as_str()?)?.abs_diff(start) > 1
    {
        return None;
    }
    let captured = timestamp(report.get("captureTime")?.as_str()?)?;
    // A report can be written after the exit, but not before this process existed.
    if captured < start || captured > ended_ms / 1000 + 60 {
        return None;
    }
    let namespace = report
        .pointer("/termination/namespace")
        .and_then(Value::as_str)
        .unwrap_or("");
    let exception = report
        .pointer("/exception/type")
        .and_then(Value::as_str)
        .unwrap_or("");
    // Restrict classification to OS termination/exception descriptions, not arbitrary stack data.
    let details = format!(
        "{} {} {}",
        report.get("termination").unwrap_or(&Value::Null),
        report.get("exception").unwrap_or(&Value::Null),
        report.get("asi").unwrap_or(&Value::Null)
    );
    Some(explanation(namespace, &details, exception))
}

fn find_in(dir: &Path, pid: u32, start: u64, ended_ms: i64) -> Option<&'static str> {
    let mut paths: Vec<_> = fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension()?.to_str()? != "ips"
                || !path.file_name()?.to_str()?.starts_with("RobloxPlayer")
            {
                return None;
            }
            // No symlinks or unbounded report reads.
            if !entry.file_type().ok()?.is_file() {
                return None;
            }
            let meta = entry.metadata().ok()?;
            if meta.len() > MAX_REPORT {
                return None;
            }
            let modified = meta
                .modified()
                .ok()?
                .duration_since(UNIX_EPOCH)
                .ok()?
                .as_secs();
            if modified < start || modified > (ended_ms / 1000).max(0) as u64 + 120 {
                return None;
            }
            Some((modified, path))
        })
        .collect();
    paths.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, path) in paths.into_iter().take(32) {
        let mut text = String::new();
        let Ok(file) = fs::File::open(path) else {
            continue;
        };
        if file.take(MAX_REPORT + 1).read_to_string(&mut text).is_err()
            || text.len() as u64 > MAX_REPORT
        {
            continue;
        }
        if let Some(reason) = parse(&text, pid, start, ended_ms) {
            return Some(reason);
        }
    }
    None
}

pub fn unknown(reached_game: bool) -> &'static str {
    if reached_game {
        "Roblox exited unexpectedly. No matching macOS crash report is available yet; the cause is unconfirmed. A manual close can also look like this."
    } else {
        "Roblox exited before joining a game. No matching macOS crash report is available yet; the startup failure's cause is unconfirmed."
    }
}

/// ReportCrash can write the .ips after we see the exit. Retry for 40 seconds off the tracker
/// thread, and update the existing history row rather than recording the crash again.
pub fn inspect(app: AppHandle, ended: Ended) {
    if ended.kind != CrashKind::Crash {
        return;
    }
    let Some(start) = ended.process_start.filter(|s| *s > 0) else {
        return;
    };
    let Some(home) = std::env::var_os("HOME") else {
        return;
    };
    let dir = std::path::PathBuf::from(home).join("Library/Logs/DiagnosticReports");
    tauri::async_runtime::spawn(async move {
        for delay in [0, 2, 3, 5, 10, 20] {
            tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
            let dir = dir.clone();
            let pid = ended.pid;
            let at = ended.at;
            let reason =
                tauri::async_runtime::spawn_blocking(move || find_in(&dir, pid, start, at))
                    .await
                    .ok()
                    .flatten();
            if let Some(reason) = reason {
                let s = app.state::<App>();
                match s.db.explain_crash(&ended.instance_id, reason) {
                    Ok(true) => {
                        s.tracker.explain_crash(&ended.instance_id, reason);
                        log::warn!(
                            "instance {} macOS crash explanation: {reason}",
                            ended.instance_id
                        );
                        crate::commands::instances_changed(&app);
                    }
                    Ok(false) => {} // Account/history removed while the report was being written.
                    Err(e) => log::warn!("Could not save Mac crash explanation: {e}"),
                }
                return;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn report() -> Value {
        serde_json::json!({"pid":42,"procName":"RobloxPlayer", "procLaunch":"2026-11-04 12:00:00.1234 -0800", "captureTime":"2026-11-04 12:01:00.0000 -0800", "termination":{"namespace":"TCC", "details":["NSMicrophoneUsageDescription"]},"exception":{"type":"EXC_CRASH","signal":"SIGABRT"}})
    }
    fn start() -> u64 {
        timestamp("2026-11-04T20:00:00Z").unwrap() as u64
    }
    #[test]
    fn microphone_report_is_not_called_out_of_memory() {
        let text = format!("{{\"app_name\":\"RobloxPlayer\"}}\n{}", report());
        let why = parse(&text, 42, start(), (start() as i64 + 61) * 1000).unwrap();
        assert!(why.contains("microphone usage description"));
        assert!(why.contains("permission on alone does not repair"));
    }
    #[test]
    fn unrelated_stale_or_reused_process_reports_are_rejected() {
        let r = report().to_string();
        let end = (start() as i64 + 61) * 1000;
        assert!(parse(&r, 43, start(), end).is_none());
        assert!(parse(&r, 42, start() + 120, end).is_none());
        assert!(parse(&r, 42, 0, end).is_none());
        assert!(parse("broken JSON", 42, start(), end).is_none());
        let mut wrong_app = report();
        wrong_app["procName"] = "OtherApp".into();
        assert!(parse(&wrong_app.to_string(), 42, start(), end).is_none());
        let mut missing_start = report();
        missing_start.as_object_mut().unwrap().remove("procLaunch");
        assert!(parse(&missing_start.to_string(), 42, start(), end).is_none());
    }
    #[test]
    fn memory_and_abort_are_not_conflated() {
        assert!(explanation("SIGNAL", "SIGABRT", "EXC_CRASH").contains("does not identify"));
        assert!(explanation("SIGNAL", "SIGKILL", "EXC_CRASH").contains("does not establish"));
        assert!(explanation("", "MEMORY", "EXC_RESOURCE").contains("memory-resource"));
        assert!(explanation("", "", "EXC_BAD_ACCESS").contains("does not by itself"));
        assert!(explanation("CODESIGNING", "", "").contains("code signature"));
        assert!(
            !explanation("SIGNAL", "NSMicrophoneUsageDescription", "EXC_CRASH").contains("missing")
        );
    }
    #[test]
    fn scanner_skips_unrelated_oversized_and_symlinked_reports() {
        let dir = std::env::temp_dir().join(format!("roalt-crash-report-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&dir).unwrap();
        let start = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let date = chrono::DateTime::from_timestamp(start as i64, 0)
            .unwrap()
            .to_rfc3339();
        let mut r = report();
        r["procLaunch"] = date.clone().into();
        r["captureTime"] = date.into();
        fs::write(dir.join("OtherApp.ips"), r.to_string()).unwrap();
        std::os::unix::fs::symlink(dir.join("OtherApp.ips"), dir.join("RobloxPlayer-link.ips"))
            .unwrap();
        fs::File::create(dir.join("RobloxPlayer-large.ips"))
            .unwrap()
            .set_len(MAX_REPORT + 1)
            .unwrap();
        fs::write(dir.join("RobloxPlayer-broken.ips"), "{broken").unwrap();
        assert!(find_in(&dir, 42, start, start as i64 * 1000).is_none());
        fs::write(dir.join("RobloxPlayer-match.ips"), r.to_string()).unwrap();
        assert!(
            find_in(&dir, 42, start, start as i64 * 1000)
                .unwrap()
                .contains("microphone usage description")
        );
        assert!(find_in(&dir, 999, start, start as i64 * 1000).is_none());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn timezone_and_fraction_are_handled() {
        assert_eq!(
            timestamp("2026-11-04 12:00:00.1234 -0800"),
            timestamp("2026-11-04T20:00:00Z")
        );
        assert!(timestamp("not a date").is_none());
    }
}
