//! The `clip` protocol (`http://clip.localhost/<name>`): the Clips view's player reads a clip
//! through it, in parts (`Range`), so it can seek. Only names `library::valid` takes, only files
//! directly in the clips folder; `main.rs` answers only the main window.

use std::io::{Read, Seek, SeekFrom};

use tauri::http::{Request, Response, StatusCode, header};

/// The most one answer carries; the player asks again for the rest.
const CHUNK: u64 = 4 << 20;

pub fn refuse(status: u16) -> Response<Vec<u8>> {
    Response::builder().status(status).body(Vec::new()).unwrap_or_default()
}

pub fn clip(request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    let name = percent_decode(request.uri().path().trim_start_matches('/'));
    let Some(path) = super::library::path_of(&name) else { return refuse(404) };
    let Ok(mut file) = std::fs::File::open(&path) else { return refuse(404) };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    if len == 0 {
        return refuse(404);
    }
    let range = request.headers().get(header::RANGE).and_then(|v| v.to_str().ok()).and_then(|r| parse_range(r, len));
    let (start, end) = range.unwrap_or((0, len.saturating_sub(1)));
    let end = end.min(start + CHUNK - 1).min(len.saturating_sub(1));
    let mut body = vec![0; (end + 1).saturating_sub(start) as usize];
    if file.seek(SeekFrom::Start(start)).is_err() || file.read_exact(&mut body).is_err() {
        return refuse(500);
    }
    let partial = range.is_some() || end + 1 < len;
    let mut r = Response::builder()
        .status(if partial { StatusCode::PARTIAL_CONTENT } else { StatusCode::OK })
        .header(header::CONTENT_TYPE, "video/mp4")
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_LENGTH, body.len().to_string());
    if partial {
        r = r.header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{len}"));
    }
    r.body(body).unwrap_or_else(|_| refuse(500))
}

/// `bytes=a-b`, `bytes=a-` or `bytes=-n` within a file of `len` bytes (pure).
fn parse_range(r: &str, len: u64) -> Option<(u64, u64)> {
    let (a, b) = r.strip_prefix("bytes=")?.split(',').next()?.trim().split_once('-')?;
    let last = len.checked_sub(1)?;
    let (start, end) = match (a.parse::<u64>().ok(), b.parse::<u64>().ok()) {
        (Some(s), Some(e)) => (s, e.min(last)),
        (Some(s), None) => (s, last),
        (None, Some(n)) => (len.saturating_sub(n), last),
        (None, None) => return None,
    };
    (start <= end).then_some((start, end))
}

/// `%20` and the like back to text (pure); bad escapes stay as they are.
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && let Some(v) = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges() {
        assert_eq!(parse_range("bytes=0-", 100), Some((0, 99)));
        assert_eq!(parse_range("bytes=10-19", 100), Some((10, 19)));
        assert_eq!(parse_range("bytes=90-500", 100), Some((90, 99)));
        assert_eq!(parse_range("bytes=-10", 100), Some((90, 99)));
        assert_eq!(parse_range("bytes=100-", 100), None);
        assert_eq!(parse_range("bytes=5-1", 100), None);
        assert_eq!(parse_range("bytes=0-", 0), None);
        assert_eq!(parse_range("items=0-1", 100), None);
    }

    #[test]
    fn names() {
        assert_eq!(percent_decode("Sky%20Islands%202026-11-09.mp4"), "Sky Islands 2026-11-09.mp4");
        assert_eq!(percent_decode("%E3%82%B9.mp4"), "ス.mp4");
        assert_eq!(percent_decode("..%2F..%2Fx.mp4"), "../../x.mp4");
        assert!(!super::super::library::valid(&percent_decode("..%2F..%2Fx.mp4")), "a path never passes");
    }
}
