//! Clips: a ShadowPlay-style replay buffer of one picked account's client: its window (H.264 on
//! the GPU) and its own sound (AAC), the last seconds kept in memory and written as an MP4 to
//! `Videos\RoALT` only when the clip key is pressed. Nothing is recorded with no account picked.
//!
//! `sync` (the tracker's 1 s loop) starts and stops the one recorder as the picked account's
//! client comes and goes. A new client process gets a new recorder; the old one's last seconds
//! stay savable until the new one has its first keyframe.

pub mod audio;
pub mod library;
pub mod mux;
pub mod ring;
pub mod serve;
pub mod video;

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use ts_rs::TS;
use windows::Graphics::Capture::{Direct3D11CaptureFrame, Direct3D11CaptureFramePool, GraphicsCaptureItem};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::System::WinRT::Direct3D11::{CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::core::{Interface, factory};

use crate::App;
use crate::model::{InstanceState, Settings};
use mux::Sample;
use ring::Ring;

/// What the Clips view shows.
#[derive(Serialize, TS, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ClipsState {
    /// No account picked.
    Off,
    /// The picked account has no client with a window.
    Waiting,
    Recording,
    /// Recording can't run here (`note` says why).
    Unavailable,
}

#[derive(Serialize, TS, Clone, Debug)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ClipsStatus {
    pub state: ClipsState,
    pub account_id: Option<i64>,
    /// Why it's unavailable, or why a recording has no sound.
    pub note: Option<String>,
    /// Recording, and its sound too.
    pub sound: bool,
}

#[derive(Default)]
struct Rings {
    video: Ring,
    audio: Ring,
}

struct Running {
    account: i64,
    pid: u32,
    stop: Arc<AtomicBool>,
    rings: Arc<Mutex<Rings>>,
    /// The video thread ended; with why, when it failed.
    ended: Arc<Mutex<Option<String>>>,
    finished: Arc<AtomicBool>,
    /// The sound couldn't be recorded: why.
    no_sound: Arc<Mutex<Option<String>>>,
    started: Instant,
}

#[derive(Default)]
struct Clips {
    running: Option<Running>,
    /// The last recorder's seconds and its account, savable until the new one has a keyframe
    /// (dropped once another account is picked, or none).
    previous: Option<(i64, Arc<Mutex<Rings>>)>,
}

static CLIPS: LazyLock<Mutex<Clips>> = LazyLock::new(Mutex::default);
/// Seconds a clip keeps (`Settings.clip_seconds`).
static KEEP: AtomicU32 = AtomicU32::new(30);
static SAVING: AtomicBool = AtomicBool::new(false);
/// The clip key, as registered (`commands::set_clip_hotkey`).
static KEY: Mutex<Option<tauri_plugin_global_shortcut::Shortcut>> = Mutex::new(None);
const FORMAT: DirectXPixelFormat = DirectXPixelFormat::B8G8R8A8UIntNormalized;
/// A recorder that stopped is started again after this, not at once.
const RETRY: Duration = Duration::from_secs(30);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn keep() -> i64 {
    KEEP.load(Ordering::Relaxed) as i64 * video::SECOND
}

pub fn set_key(key: Option<tauri_plugin_global_shortcut::Shortcut>) {
    *lock(&KEY) = key;
}

pub fn is_key(key: &tauri_plugin_global_shortcut::Shortcut) -> bool {
    lock(&KEY).as_ref() == Some(key)
}

/// The picked account's running client: its pid and window.
fn client_of(s: &App, account: i64) -> Option<(u32, HWND)> {
    let pid = s.tracker.list().into_iter().find(|i| i.account_id == account && i.state != InstanceState::Exited && i.pid != 0)?.pid;
    crate::instances::process::main_window(pid).map(|(hwnd, _)| (pid, hwnd))
}

/// Starts, keeps or stops the recorder for what `v` picks (from the tracker's 1 s loop).
pub fn sync(s: &App, v: &Settings) {
    KEEP.store(v.clip_seconds.clamp(5, 300), Ordering::Relaxed);
    let mut c = lock(&CLIPS);
    let want = v.clip_account.and_then(|a| client_of(s, a).map(|(pid, hwnd)| (a, pid, hwnd)));
    if c.previous.as_ref().is_some_and(|p| v.clip_account != Some(p.0)) || c.running.as_ref().is_some_and(|r| lock(&r.rings).video.has_key()) {
        c.previous = None;
    }
    match (want, &c.running) {
        (Some((a, pid, _)), Some(r)) if r.account == a && r.pid == pid && !(r.finished.load(Ordering::Relaxed) && r.started.elapsed() > RETRY) => {}
        (Some((a, pid, hwnd)), _) => {
            stop(&mut c);
            c.running = Some(start(a, pid, hwnd));
        }
        (None, _) => {
            stop(&mut c);
            if v.clip_account.is_none() {
                c.previous = None;
            }
        }
    }
}

fn stop(c: &mut Clips) {
    if let Some(r) = c.running.take() {
        r.stop.store(true, Ordering::Relaxed);
        if lock(&r.rings).video.has_key() {
            c.previous = Some((r.account, r.rings));
        }
        log::info!("clips: stopped recording account {}", r.account);
    }
}

fn start(account: i64, pid: u32, hwnd: HWND) -> Running {
    log::info!("clips: recording account {account} (pid {pid})");
    let r = Running {
        account,
        pid,
        stop: Arc::new(AtomicBool::new(false)),
        rings: Arc::new(Mutex::new(Rings::default())),
        ended: Arc::default(),
        finished: Arc::default(),
        no_sound: Arc::default(),
        started: Instant::now(),
    };
    let (stop, rings, ended, finished) = (r.stop.clone(), r.rings.clone(), r.ended.clone(), r.finished.clone());
    let window = hwnd.0 as isize;
    let video = std::thread::Builder::new().name("clips-video".into()).spawn(move || {
        if let Err(e) = record_video(HWND(window as _), &rings, &stop) {
            log::warn!("clips: recording stopped: {e}");
            *lock(&ended) = Some(e.message());
        }
        finished.store(true, Ordering::Relaxed);
    });
    let (stop, rings, no_sound) = (r.stop.clone(), r.rings.clone(), r.no_sound.clone());
    let sound = std::thread::Builder::new().name("clips-audio".into()).spawn(move || {
        mf_thread();
        let result = audio::record(pid, &stop, &mut |kind| lock(&rings).audio.kind = kind, &mut |time, data| {
            lock(&rings).audio.push(Sample { time, duration: audio::AAC_DURATION, key: true, data: data.into() }, keep());
        });
        if let Err(e) = result {
            log::info!("clips: no sound for pid {pid}: {e}");
            *lock(&no_sound) = Some(e.message());
        }
    });
    if let Err(e) = video.and(sound) {
        log::warn!("clips: can't start recording: {e}");
    }
    r
}

/// COM and Media Foundation on this thread (once each; failure shows in the first MF call).
fn mf_thread() {
    // SAFETY: this thread's own apartment and MF.
    unsafe {
        let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED);
        let _ = windows::Win32::Media::MediaFoundation::MFStartup(windows::Win32::Media::MediaFoundation::MF_VERSION, 0);
    }
}

/// Captures the window and encodes it into the rings until `stop`.
fn record_video(hwnd: HWND, rings: &Mutex<Rings>, stop: &AtomicBool) -> windows::core::Result<()> {
    mf_thread();
    let mut enc = video::Encoder::new(video::gpu()?)?;
    // SAFETY: wraps the encoder's own device for the WinRT capture API, then a capture item for
    // a window handle (fails for a window that is gone).
    let (winrt, item): (IDirect3DDevice, GraphicsCaptureItem) = unsafe {
        let winrt = CreateDirect3D11DeviceFromDXGIDevice(&enc.device().cast::<IDXGIDevice>()?)?.cast()?;
        let interop = factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
        (winrt, interop.CreateForWindow(hwnd)?)
    };
    let mut size = item.Size()?;
    let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(&winrt, FORMAT, 2, size)?;
    let session = pool.CreateCaptureSession(&item)?;
    // Not possible before Windows 11: the yellow frame stays there.
    let _ = session.SetIsBorderRequired(false);
    let _ = session.SetIsCursorCaptureEnabled(false);
    session.StartCapture()?;
    let mut out = Vec::new();
    let mut last: Option<i64> = None;
    let result = (|| {
        while !stop.load(Ordering::Relaxed) {
            enc.poll(&mut out)?;
            if !out.is_empty() {
                let mut r = lock(rings);
                r.video.kind = enc.out_type().to_vec();
                for e in out.drain(..) {
                    let s = Sample { time: e.time, duration: 0, key: e.key, data: e.data.into() };
                    // The encoder may send SPS/PPS only with its first keyframe: kept for later clips.
                    if s.key
                        && let Some(h) = mux::sequence_header(std::slice::from_ref(&s))
                    {
                        r.video.header = Some(h);
                    }
                    r.video.push(s, keep());
                }
            }
            // Only the newest frame matters; older ones go back to the pool.
            let mut newest: Option<Direct3D11CaptureFrame> = None;
            while let Ok(frame) = pool.TryGetNextFrame() {
                if let Some(old) = newest.replace(frame) {
                    let _ = old.Close();
                }
            }
            if let Some(frame) = newest {
                let content = frame.ContentSize()?;
                if content.Width != size.Width || content.Height != size.Height {
                    pool.Recreate(&winrt, FORMAT, 2, content)?;
                    size = content;
                } else {
                    let time = frame.SystemRelativeTime()?.Duration;
                    // At most 60 frames a second.
                    if last.is_none_or(|l| time - l >= video::SECOND / video::FPS as i64 - 10_000) {
                        // SAFETY: the frame's own texture, alive while `frame` is.
                        let texture: ID3D11Texture2D = unsafe { frame.Surface()?.cast::<IDirect3DDxgiInterfaceAccess>()?.GetInterface()? };
                        if enc.push(&texture, time)? {
                            last = Some(time);
                        }
                    }
                }
                let _ = frame.Close();
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        Ok(())
    })();
    let _ = session.Close();
    let _ = pool.Close();
    result
}

/// What the Clips view shows now.
pub fn status(v: &Settings) -> ClipsStatus {
    let c = lock(&CLIPS);
    let Some(account) = v.clip_account else {
        return ClipsStatus { state: ClipsState::Off, account_id: None, note: None, sound: false };
    };
    match &c.running {
        Some(r) if r.account == account => {
            if let Some(why) = lock(&r.ended).clone() {
                return ClipsStatus { state: ClipsState::Unavailable, account_id: Some(account), note: Some(unavailable(&why)), sound: false };
            }
            let silent = lock(&r.no_sound).clone();
            ClipsStatus { state: ClipsState::Recording, account_id: Some(account), sound: silent.is_none(), note: silent.map(|_| tr!("This client's sound can't be recorded on this PC.")) }
        }
        _ => ClipsStatus { state: ClipsState::Waiting, account_id: Some(account), note: None, sound: false },
    }
}

fn unavailable(why: &str) -> String {
    if why.starts_with("find encoder") {
        tr!("This PC's graphics can't record video.")
    } else {
        tr!("Recording stopped ({why}). RoALT tries again shortly.", why = why)
    }
}

/// Saves the last seconds as a clip (the clip key, or Save clip). One at a time; a beep says how
/// it went, as RoALT's window may be hidden behind the game.
pub fn save(app: &AppHandle) {
    if SAVING.swap(true, Ordering::AcqRel) {
        return;
    }
    let (rings, account) = {
        let c = lock(&CLIPS);
        let current = c.running.as_ref().filter(|r| lock(&r.rings).video.has_key()).map(|r| (r.rings.clone(), Some(r.account)));
        match current.or_else(|| c.previous.clone().map(|(a, p)| (p, Some(a)))) {
            Some(x) => x,
            None => {
                SAVING.store(false, Ordering::Release);
                return failed(app, tr!("Nothing to save yet: the clip starts once the picked client is recording."));
            }
        }
    };
    let cut = {
        let r = lock(&rings);
        ring::cut(&r.video, &r.audio, keep(), audio::now(), video::SECOND / video::FPS as i64, audio::AAC_DURATION)
    };
    let Some((video_track, audio_track)) = cut else {
        SAVING.store(false, Ordering::Release);
        return failed(app, tr!("Nothing to save yet: the clip starts once the picked client is recording."));
    };
    let s = app.state::<App>();
    let title = account.and_then(|a| title_of(&s, a)).unwrap_or_else(|| "Roblox".into());
    let app = app.clone();
    let spawned = std::thread::Builder::new().name("clips-save".into()).spawn(move || {
        mf_thread();
        let done = write(&title, &video_track, &audio_track);
        SAVING.store(false, Ordering::Release);
        match done {
            Ok(name) => {
                log::info!("clips: saved {name}");
                beep(true);
                crate::commands::notice(&app, false, tr!("Clip saved: {name}", name = name));
                let _ = app.emit("clips://changed", ());
            }
            Err(e) => {
                log::warn!("clips: save failed: {e}");
                failed(&app, tr!("The clip wasn't saved: {why}", why = e));
            }
        }
    });
    if spawned.is_err() {
        SAVING.store(false, Ordering::Release);
    }
}

fn failed(app: &AppHandle, why: String) {
    beep(false);
    crate::commands::notice(app, true, why);
}

fn beep(ok: bool) {
    use windows::Win32::System::Diagnostics::Debug::MessageBeep;
    use windows::Win32::UI::WindowsAndMessaging::{MB_ICONHAND, MB_OK};
    // SAFETY: plays a system sound.
    let _ = unsafe { MessageBeep(if ok { MB_OK } else { MB_ICONHAND }) };
}

/// The game the account plays now, else its name.
fn title_of(s: &App, account: i64) -> Option<String> {
    let place = s.tracker.list().into_iter().find(|i| i.account_id == account && i.state != InstanceState::Exited).and_then(|i| i.place_id);
    place.and_then(|p| crate::commands::cached_game_name(s, p)).or_else(|| s.db.account(account).ok().map(|a| a.username))
}

/// Writes the clip to `Videos\RoALT` through a `.part.mp4`, renamed once whole. Its file name.
fn write(title: &str, video: &mux::Track, audio: &mux::Track) -> crate::error::Result<String> {
    let dir = library::dir()?;
    std::fs::create_dir_all(&dir)?;
    let path = library::new_path(&dir, title, &stamp());
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let part = path.with_file_name(format!("{}{}", name.trim_end_matches(".mp4"), library::PART));
    let written = mux::write(&part, video, Some(audio)).map_err(|e| crate::error::AppError::Internal(e.message()));
    let done = written.and_then(|()| crate::launcher::client_settings::retry(|| std::fs::rename(&part, &path)).map_err(Into::into));
    if done.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    done.map(|()| name)
}

/// Local date and time for a file name: `2026-11-09 14-03-22`.
fn stamp() -> String {
    // SAFETY: fills a local struct.
    let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    format!("{:04}-{:02}-{:02} {:02}-{:02}-{:02}", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond)
}
