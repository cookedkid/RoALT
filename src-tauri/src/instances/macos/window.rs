use crate::error::{AppError, Result};
pub struct Thumb {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Show {
    Focus,
    Minimize,
    Restore,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Rect {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}
unsafe extern "C" {
    fn ro_rect(pid: i32, start: u64, rect: *mut Rect, set: i32) -> i32;
    fn ro_workarea(rect: *mut Rect) -> i32;
    fn ro_capture(
        pid: i32,
        start: u64,
        maxwidth: i32,
        bytes: *mut *mut u8,
        width: *mut i32,
        height: *mut i32,
    ) -> i32;
    fn ro_capture_free(bytes: *mut u8);
}
fn error(code: i32) -> AppError {
    AppError::Invalid(match code{-2=>"Enable RoALT in System Settings → Privacy & Security → Accessibility, then restart RoALT. If already enabled after an update, remove the old RoALT entry and add the current RoALT.app again.",-3=>"Enable RoALT in System Settings → Privacy & Security → Screen Recording, then restart RoALT to show previews.",_=>"macOS could not access this Roblox window."}.into())
}
pub fn thumbnail(pid: u32, start: u64, maxwidth: u32) -> Result<Option<(Thumb, bool)>> {
    let (mut bytes, mut w, mut h) = (std::ptr::null_mut(), 0, 0);
    let code = unsafe {
        ro_capture(
            pid as i32,
            start,
            maxwidth as i32,
            &mut bytes,
            &mut w,
            &mut h,
        )
    };
    if code == 1 {
        return Ok(None);
    }
    if code != 0 {
        return Err(error(code));
    }
    let length = (w as usize)
        .checked_mul(h as usize)
        .and_then(|n| n.checked_mul(4));
    if bytes.is_null() || w <= 0 || h <= 0 || w > 1280 || h > 8192 || length.is_none() {
        if !bytes.is_null() {
            unsafe { ro_capture_free(bytes) }
        }
        return Err(error(-4));
    }
    let rgba = unsafe {
        let copy = std::slice::from_raw_parts(bytes, length.unwrap()).to_vec();
        ro_capture_free(bytes);
        copy
    };
    Ok(Some((
        Thumb {
            width: w as u32,
            height: h as u32,
            rgba,
        },
        false,
    )))
}
pub fn unpark_client(_: u32) {}
fn action(pid: u32, what: i32, size: Option<(u32, u32)>) -> Result<()> {
    if super::process::start_time(pid).is_none() {
        return Err(AppError::NotFound);
    }
    let (w, h) = size.unwrap_or((0, 0));
    match unsafe{super::native::ro_window(pid as i32,what,80,80,w as i32,h as i32)} {
  0=>Ok(()),-2=>Err(AppError::Invalid("Enable RoALT in System Settings → Privacy & Security → Accessibility, then restart RoALT. If already enabled after an update, remove the old RoALT entry and add the current RoALT.app again.".into())),
  _=>Err(AppError::Invalid("macOS could not control this Roblox window.".into()))
 }
}
pub fn show(pid: u32, what: Show, _: bool) -> Result<()> {
    action(
        pid,
        match what {
            Show::Focus => 0,
            Show::Minimize => 1,
            Show::Restore => 2,
        },
        None,
    )
}
pub fn minimize_quietly(pid: u32, _: bool) {
    if let Err(e) = action(pid, 1, None) {
        log::debug!("minimize: {e}")
    }
}
fn saved() -> &'static std::sync::Mutex<std::collections::HashMap<String, (u32, u64, Rect)>> {
    static SAVED: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, (u32, u64, Rect)>>,
    > = std::sync::OnceLock::new();
    SAVED.get_or_init(Default::default)
}
pub fn micro(id: &str, pid: u32, on: bool, size: (u32, u32)) -> Result<(u32, u32)> {
    let start = super::process::start_time(pid).ok_or(AppError::NotFound)?;
    let mut rect = Rect::default();
    let code = unsafe { ro_rect(pid as i32, start, &mut rect, 0) };
    if code != 0 {
        return Err(error(code));
    }
    let mut saved = saved().lock().unwrap_or_else(|e| e.into_inner());
    if on {
        if !saved
            .get(id)
            .is_some_and(|(p, s, _)| *p == pid && *s == start)
        {
            saved.insert(id.into(), (pid, start, rect));
        }
        rect.w = size.0 as i32;
        rect.h = size.1 as i32;
    } else if let Some((p, s, r)) = saved.get(id) {
        if *p == pid && *s == start {
            rect = *r;
        }
    }
    let code = unsafe { ro_rect(pid as i32, start, &mut rect, 1) };
    if code != 0 {
        return Err(error(code));
    }
    if !on {
        saved.remove(id);
    }
    Ok((rect.w as u32, rect.h as u32))
}
pub fn tile(clients: &[(String, u32, u64)], size: Option<(u32, u32)>) -> Result<Vec<String>> {
    let mut area = Rect::default();
    if unsafe { ro_workarea(&mut area) } != 0 {
        return Err(error(-1));
    }
    let mut windows = Vec::new();
    for (id, pid, start) in clients {
        let mut rect = Rect::default();
        match unsafe { ro_rect(*pid as i32, *start, &mut rect, 0) } {
            0 => windows.push((id, pid, start, rect)),
            -2 => return Err(error(-2)),
            _ => {}
        }
    }
    let cells = grid(
        windows.len(),
        (area.x, area.y, area.w, area.h),
        size.map(|(w, h)| (w as i32, h as i32)),
    );
    let mut tiled = Vec::new();
    let mut saved = saved().lock().unwrap_or_else(|e| e.into_inner());
    for ((id, pid, start, old), (x, y, w, h)) in windows.into_iter().zip(cells) {
        let mut rect = Rect { x, y, w, h };
        if unsafe { ro_rect(*pid as i32, *start, &mut rect, 1) } == 0 {
            saved
                .entry(id.clone())
                .and_modify(|v| {
                    if v.0 != *pid || v.1 != *start {
                        *v = (*pid, *start, old)
                    }
                })
                .or_insert((*pid, *start, old));
            tiled.push(id.clone());
        }
    }
    Ok(tiled)
}
pub fn grid(
    n: usize,
    area: (i32, i32, i32, i32),
    cell: Option<(i32, i32)>,
) -> Vec<(i32, i32, i32, i32)> {
    let (ax, ay, aw, ah) = area;
    if n == 0 || aw <= 0 || ah <= 0 {
        return Vec::new();
    }
    let (cols, cw, ch, per) = match cell {
        Some((cw, ch)) => {
            let (cw, ch) = (cw.max(1), ch.max(1));
            let cols = (aw / cw).max(1) as usize;
            (cols, cw, ch, cols * (ah / ch).max(1) as usize)
        }
        None => {
            let picture = |c: usize| (aw / c as i32).min(ah / n.div_ceil(c) as i32 * 4 / 3);
            let cols = (1..=n).fold(
                1,
                |best, c| if picture(c) > picture(best) { c } else { best },
            );
            (cols, aw / cols as i32, ah / n.div_ceil(cols) as i32, n)
        }
    };
    (0..n)
        .map(|i| {
            let slot = i % per;
            (
                ax + (slot % cols) as i32 * cw,
                ay + (slot / cols) as i32 * ch,
                cw,
                ch,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grid_fits_and_empty_is_safe() {
        assert!(grid(0, (0, 0, 1000, 800), None).is_empty());
        for n in 1..20 {
            let cells = grid(n, (20, 40, 1200, 800), None);
            assert_eq!(cells.len(), n);
            assert!(
                cells
                    .iter()
                    .all(|&(x, y, w, h)| x >= 20 && y >= 40 && x + w <= 1220 && y + h <= 840)
            );
        }
    }
    #[test]
    fn stale_identity_cannot_capture_or_resize() {
        let pid = std::process::id();
        assert!(thumbnail(pid, 1, 320).is_err());
        let mut r = Rect::default();
        assert_eq!(unsafe { ro_rect(pid as i32, 1, &mut r, 1) }, -1);
    }
}
