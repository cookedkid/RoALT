//! The main thread runs the window, the tray icon and its menu: stuck there, RoALT can't even be
//! quit from the tray. Every 5 s it is asked to answer; after 30 s without an answer what it is
//! stuck in is logged (its call stack, as module+offset: `roalt.exe` ones are named with that
//! build's PDB), and after 60 s RoALT restarts in the tray. Its clients run on and the new RoALT
//! follows them again (`Tracker::readopt`). An open menu or a window being dragged is the user, not
//! a hang.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

use tauri::AppHandle;
use windows::Win32::Foundation::{CloseHandle, HMODULE};
use windows::Win32::System::LibraryLoader::{GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT, GetModuleFileNameW, GetModuleHandleExW};
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentThreadId, OpenProcess, OpenThread, PROCESS_SYNCHRONIZE, ResumeThread, SuspendThread, THREAD_GET_CONTEXT, THREAD_SUSPEND_RESUME, TerminateProcess,
    WaitForSingleObject,
};
use windows::Win32::UI::WindowsAndMessaging::{GUI_INMENUMODE, GUI_INMOVESIZE, GUI_POPUPMENUMODE, GUI_SYSTEMMENUMODE, GUITHREADINFO, GetGUIThreadInfo};
use windows::core::PCWSTR;

/// The restarted RoALT's arguments: wait for this pid to be gone, then start in the tray.
const AFTER_ARG: &str = "--after";
pub const TRAY_ARG: &str = "--tray";

const TICK: Duration = Duration::from_secs(5);
/// Unanswered ticks before the stack is logged (30 s), and before RoALT restarts (60 s).
const LOG_AFTER: u32 = 6;
const RESTART_AFTER: u32 = 12;

static MAIN_THREAD: AtomicU32 = AtomicU32::new(0);
/// Quitting: the main thread is busy closing, and nothing is to restart.
static EXITING: AtomicBool = AtomicBool::new(false);

pub fn exiting() {
    EXITING.store(true, Ordering::Relaxed);
}

/// Watches the main thread; called on it, once the app is set up.
pub fn spawn(app: AppHandle) {
    // SAFETY: no arguments.
    MAIN_THREAD.store(unsafe { GetCurrentThreadId() }, Ordering::Relaxed);
    let started = std::thread::Builder::new().name("watchdog".into()).spawn(move || {
        let answered = Arc::new(AtomicU64::new(0));
        let (mut asked, mut stuck) = (0u64, 0u32);
        loop {
            std::thread::sleep(TICK);
            if EXITING.load(Ordering::Relaxed) {
                return;
            }
            if answered.load(Ordering::Acquire) >= asked {
                if stuck >= LOG_AFTER {
                    log::info!("watchdog: the main thread answers again");
                }
                stuck = 0;
                asked += 1;
                let (answered, n) = (answered.clone(), asked);
                if app.run_on_main_thread(move || answered.store(n, Ordering::Release)).is_err() {
                    return;
                }
                continue;
            }
            if user_holds_it() {
                continue;
            }
            stuck += 1;
            if stuck == LOG_AFTER {
                log::warn!("watchdog: the main thread hasn't answered for {} s; RoALT {} is stuck in: {}", stuck * 5, env!("CARGO_PKG_VERSION"), main_stack());
            } else if stuck == RESTART_AFTER {
                log::error!("watchdog: the main thread hasn't answered for {} s; restarting in the tray (still in: {})", stuck * 5, main_stack());
                restart(&app);
            }
        }
    });
    if let Err(e) = started {
        log::warn!("watchdog not running: {e}");
    }
}

/// A menu is open or a window is being moved or sized on the main thread: it pumps that loop only.
fn user_holds_it() -> bool {
    let mut gui = GUITHREADINFO { cbSize: size_of::<GUITHREADINFO>() as u32, ..Default::default() };
    // SAFETY: fills a local with cbSize set.
    unsafe { GetGUIThreadInfo(MAIN_THREAD.load(Ordering::Relaxed), &mut gui) }.is_ok()
        && gui.flags.0 & (GUI_INMENUMODE.0 | GUI_POPUPMENUMODE.0 | GUI_SYSTEMMENUMODE.0 | GUI_INMOVESIZE.0) != 0
}

/// The main thread's call stack, innermost first, as `module+0xoffset`. Read on a thread of its
/// own, given 2 s: the unwinder takes a lock the stuck thread could hold.
fn main_stack() -> String {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(read_stack(MAIN_THREAD.load(Ordering::Relaxed)));
    });
    match rx.recv_timeout(Duration::from_secs(2)) {
        Ok(frames) if !frames.is_empty() => frames.into_iter().map(module_offset).collect::<Vec<_>>().join(" < "),
        Ok(_) => "(stack unreadable)".into(),
        Err(_) => "(stack reader stuck)".into(),
    }
}

/// Return addresses of thread `tid`, suspended meanwhile. Nothing is allocated while it is: it may
/// hold the heap's lock.
#[cfg(target_arch = "x86_64")]
fn read_stack(tid: u32) -> Vec<u64> {
    use windows::Win32::System::Diagnostics::Debug::{CONTEXT, CONTEXT_FULL_AMD64, GetThreadContext, RtlLookupFunctionEntry, RtlVirtualUnwind, UNW_FLAG_NHANDLER};
    // GetThreadContext wants it 16-byte aligned.
    #[repr(C, align(16))]
    struct Aligned(CONTEXT);
    let mut frames = [0u64; 48];
    let mut n = 0;
    // SAFETY: a thread of this process, resumed before its handle closes; the unwind reads only
    // its stack and the loaded modules' unwind tables, into locals.
    unsafe {
        let Ok(h) = OpenThread(THREAD_SUSPEND_RESUME | THREAD_GET_CONTEXT, false, tid) else { return Vec::new() };
        if SuspendThread(h) != u32::MAX {
            let mut ctx: Aligned = std::mem::zeroed();
            ctx.0.ContextFlags = CONTEXT_FULL_AMD64;
            if GetThreadContext(h, &mut ctx.0).is_ok() {
                let c = &mut ctx.0;
                while n < frames.len() && c.Rip != 0 && c.Rsp != 0 {
                    frames[n] = c.Rip;
                    n += 1;
                    let mut base = 0u64;
                    let f = RtlLookupFunctionEntry(c.Rip, &mut base, None);
                    if f.is_null() {
                        // A leaf function: its return address is on top of the stack.
                        c.Rip = *(c.Rsp as *const u64);
                        c.Rsp += 8;
                    } else {
                        let (mut data, mut frame) = (std::ptr::null_mut(), 0u64);
                        RtlVirtualUnwind(UNW_FLAG_NHANDLER, base, c.Rip, f, c, &mut data, &mut frame, None);
                    }
                }
            }
            ResumeThread(h);
        }
        let _ = CloseHandle(h);
    }
    frames[..n].to_vec()
}

#[cfg(not(target_arch = "x86_64"))]
fn read_stack(_: u32) -> Vec<u64> {
    Vec::new()
}

/// `user32.dll+0x1a2b`; a bare address outside any module.
fn module_offset(addr: u64) -> String {
    let mut m = HMODULE::default();
    // SAFETY: looks up the module holding an address without keeping it loaded; the name goes into
    // a local buffer.
    unsafe {
        if GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT, PCWSTR(addr as *const u16), &mut m).is_err() {
            return format!("0x{addr:x}");
        }
        let mut buf = [0u16; 260];
        let len = GetModuleFileNameW(Some(m), &mut buf) as usize;
        let path = String::from_utf16_lossy(&buf[..len]);
        let name = path.rsplit('\\').next().unwrap_or("?");
        format!("{name}+0x{:x}", addr - m.0 as u64)
    }
}

/// Does what quitting does off the main thread (at most 5 s: it may hold what that needs), starts
/// RoALT again in the tray once this one is gone, and ends this one.
fn restart(app: &AppHandle) -> ! {
    EXITING.store(true, Ordering::Relaxed);
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = app.clone();
    std::thread::spawn(move || {
        crate::commands::clear_copied_cookie(&handle);
        crate::exit_cleanup(&handle);
        let _ = tx.send(());
    });
    if rx.recv_timeout(Duration::from_secs(5)).is_err() {
        log::warn!("watchdog: quitting's clean-up didn't finish in 5 s; restarting anyway");
    }
    match std::env::current_exe() {
        Ok(exe) => {
            if let Err(e) = std::process::Command::new(exe).args([AFTER_ARG, &std::process::id().to_string(), TRAY_ARG]).spawn() {
                log::error!("watchdog: could not start RoALT again: {e}");
            }
        }
        Err(e) => log::error!("watchdog: could not start RoALT again: {e}"),
    }
    remove_tray_icon();
    log::logger().flush();
    // SAFETY: ends this process. Not `exit`: its DLL clean-up could wait on the stuck thread.
    let _ = unsafe { TerminateProcess(GetCurrentProcess(), 1) };
    std::process::abort()
}

/// Takes this process's tray icon away, which a killed process leaves behind until the pointer
/// passes over it. The shell knows it by tray-icon's window and its own id, counted from 1 per
/// process: the first few go, those not in use just fail.
fn remove_tray_icon() {
    use windows::Win32::UI::Shell::{NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW};
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowExW, GetWindowThreadProcessId};
    let mut after = None;
    // SAFETY: finds windows by class and asks the shell to drop icons of the ones in this process.
    unsafe {
        while let Ok(hwnd) = FindWindowExW(None, after, windows::core::w!("tray_icon_app"), None) {
            let mut pid = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == std::process::id() {
                for id in 1..=8 {
                    let nid = NOTIFYICONDATAW { cbSize: size_of::<NOTIFYICONDATAW>() as u32, hWnd: hwnd, uID: id, ..Default::default() };
                    let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
                }
            }
            after = Some(hwnd);
        }
    }
}

/// Started by `restart`: waits (at most 10 s) until the RoALT it replaces is gone, so that one isn't
/// the instance this start hands over to.
pub fn wait_for_previous() {
    let mut args = std::env::args().skip_while(|a| a != AFTER_ARG).skip(1);
    let Some(pid) = args.next().and_then(|p| p.parse::<u32>().ok()) else { return };
    // SAFETY: waits on a process handle, closed after.
    unsafe {
        if let Ok(h) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
            WaitForSingleObject(h, 10_000);
            let _ = CloseHandle(h);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_blocked_threads_stack() {
        let (tid_tx, tid_rx) = std::sync::mpsc::channel();
        let (go_tx, go_rx) = std::sync::mpsc::channel::<()>();
        let t = std::thread::spawn(move || {
            // SAFETY: no arguments.
            tid_tx.send(unsafe { GetCurrentThreadId() }).unwrap();
            let _ = go_rx.recv();
        });
        let tid = tid_rx.recv().unwrap();
        std::thread::sleep(Duration::from_millis(200));
        let frames: Vec<String> = read_stack(tid).into_iter().map(module_offset).collect();
        go_tx.send(()).unwrap();
        t.join().unwrap();
        println!("{}", frames.join(" < "));
        // Waiting in the kernel, called from this test's own code.
        assert!(frames.len() > 3, "{frames:?}");
        assert!(frames[0].starts_with("ntdll.dll+"), "{frames:?}");
        let me = std::env::current_exe().unwrap();
        let me = me.file_name().unwrap().to_string_lossy();
        assert!(frames.iter().any(|f| f.starts_with(&*me)), "{frames:?}");
    }
}
