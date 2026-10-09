//! Windows only: a minidump of the app when it crashes outright (an access
//! violation and the like, not a Rust panic), written beside the logs as
//! `pkmn_xp_router_crash_<unix time>.dmp`. Windows Error Reporting still runs
//! afterwards. Only the newest few dumps are kept.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use windows_sys::Win32::Foundation::{CloseHandle, GENERIC_WRITE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{CreateFileW, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL};
use windows_sys::Win32::System::Diagnostics::Debug::{
    MiniDumpWithDataSegs, MiniDumpWithIndirectlyReferencedMemory, MiniDumpWithThreadInfo, MiniDumpWithUnloadedModules, MiniDumpWriteDump, SetUnhandledExceptionFilter, EXCEPTION_POINTERS,
    MINIDUMP_EXCEPTION_INFORMATION,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetCurrentProcessId, GetCurrentThreadId};

const PREFIX: &str = "pkmn_xp_router_crash_";
const KEEP: usize = 3;

/// the dump path up to the time stamp, as UTF-16 (no allocating in the handler)
static BASE: OnceLock<Vec<u16>> = OnceLock::new();
static WRITING: AtomicBool = AtomicBool::new(false);

pub fn install(dir: &Path) {
    prune(dir);
    let base: Vec<u16> = dir.join(PREFIX).as_os_str().to_string_lossy().encode_utf16().collect();
    if BASE.set(base).is_ok() {
        unsafe { SetUnhandledExceptionFilter(Some(filter)) };
    }
}

/// Log the dumps left by earlier crashes and delete all but the newest few.
fn prune(dir: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut dumps: Vec<_> = rd.flatten().map(|e| e.path()).filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with(PREFIX) && n.ends_with(".dmp"))).collect();
    dumps.sort();
    if let Some(last) = dumps.last() {
        log::info!("crash dump from an earlier run: {}", last.display());
    }
    while dumps.len() > KEEP {
        let _ = std::fs::remove_file(dumps.remove(0));
    }
}

unsafe extern "system" fn filter(info: *const EXCEPTION_POINTERS) -> i32 {
    const EXCEPTION_CONTINUE_SEARCH: i32 = 0;
    let Some(base) = BASE.get() else { return EXCEPTION_CONTINUE_SEARCH };
    if WRITING.swap(true, Ordering::SeqCst) {
        return EXCEPTION_CONTINUE_SEARCH;
    }
    // base + digits + ".dmp" + NUL, built on the stack
    let mut path = [0u16; 1024];
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let mut digits = [0u16; 20];
    let mut n = 0;
    let mut v = secs;
    loop {
        digits[n] = b'0' as u16 + (v % 10) as u16;
        n += 1;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    let ext: [u16; 4] = [b'.' as u16, b'd' as u16, b'm' as u16, b'p' as u16];
    if base.len() + n + ext.len() + 1 > path.len() {
        return EXCEPTION_CONTINUE_SEARCH;
    }
    path[..base.len()].copy_from_slice(base);
    let mut at = base.len();
    for i in (0..n).rev() {
        path[at] = digits[i];
        at += 1;
    }
    path[at..at + ext.len()].copy_from_slice(&ext);
    let file = CreateFileW(path.as_ptr(), GENERIC_WRITE, 0, std::ptr::null(), CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, 0);
    if file == INVALID_HANDLE_VALUE {
        return EXCEPTION_CONTINUE_SEARCH;
    }
    let exception = MINIDUMP_EXCEPTION_INFORMATION { ThreadId: GetCurrentThreadId(), ExceptionPointers: info as *mut _, ClientPointers: 0 };
    let kind = MiniDumpWithIndirectlyReferencedMemory | MiniDumpWithThreadInfo | MiniDumpWithUnloadedModules | MiniDumpWithDataSegs;
    MiniDumpWriteDump(GetCurrentProcess(), GetCurrentProcessId(), file, kind, &exception, std::ptr::null(), std::ptr::null());
    CloseHandle(file);
    EXCEPTION_CONTINUE_SEARCH
}
