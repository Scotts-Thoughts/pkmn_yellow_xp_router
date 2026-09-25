//! Windows only: keeps the main window usable when it crosses between
//! monitors with different display scales.
//!
//! winit answers `WM_DPICHANGED` by keeping the window's logical size and
//! then nudging it a pixel at a time until Windows agrees it is on the new
//! monitor. A window bigger than that monitor never gets there and slides
//! far off it. During an edge resize the window also jumps as if dragged by
//! its title bar. This subclasses the window procedure and, once winit has
//! handled the message, puts the window somewhere sensible:
//! - edge resize: back to the rect under the user's hand (only the content
//!   scale changes, so the edge stays under the cursor)
//! - title-bar drag: winit's new size, capped to the new monitor's work
//!   area, with the cursor kept on the same spot of the title bar
//! - anything else (Win+Shift+Arrow, a display settings change): fitted
//!   inside its monitor's work area
//!
//! A drag that changed the scale settles fully onto the monitor the window
//! is mostly on when it ends, so the window is not left spanning two
//! scales (resizing it there flips the scale back and forth).

use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering::Relaxed};

use egui::Pos2;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromRect, MonitorFromWindow, HMONITOR, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, DefWindowProcW, GetCursorPos, GetWindowRect, IsIconic, IsZoomed, SetWindowLongPtrW, SetWindowPos, GWLP_WNDPROC, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, WM_DPICHANGED, WM_ENTERSIZEMOVE, WM_EXITSIZEMOVE, WM_SIZING, WNDPROC,
};

/// winit's window procedure, which ours hands every message to
static ORIG_PROC: AtomicIsize = AtomicIsize::new(0);
/// inside the modal move/size loop
static IN_SIZE_MOVE: AtomicBool = AtomicBool::new(false);
/// ... and it is an edge resize, not a title-bar drag
static SIZING: AtomicBool = AtomicBool::new(false);
/// the scale changed during the current move/size loop
static SCALE_CHANGED_IN_MOVE: AtomicBool = AtomicBool::new(false);
/// guards against our own `SetWindowPos` re-entering the handler
static HANDLING_DPI: AtomicBool = AtomicBool::new(false);
/// the window's current DPI (96 = 100%)
static DPI: AtomicU32 = AtomicU32::new(96);

/// Subclass the main window (once), move it to `saved_pos_px` (the outer
/// position in physical pixels) and fit it inside the monitor it lands on.
pub fn install(frame: &eframe::Frame, native_ppp: f32, saved_pos_px: Option<Pos2>) {
    if ORIG_PROC.load(Relaxed) != 0 {
        return;
    }
    let Ok(handle) = frame.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(h) = handle.as_raw() else {
        return;
    };
    let hwnd: HWND = h.hwnd.get();
    DPI.store((native_ppp * 96.0).round() as u32, Relaxed);
    unsafe {
        let ours: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT = wndproc;
        let orig = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, ours as usize as isize);
        ORIG_PROC.store(orig, Relaxed);
        if IsZoomed(hwnd) != 0 || IsIconic(hwnd) != 0 {
            return;
        }
        if let Some(p) = saved_pos_px {
            SetWindowPos(hwnd, 0, p.x.round() as i32, p.y.round() as i32, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
        }
        contain(hwnd);
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_ENTERSIZEMOVE => {
            IN_SIZE_MOVE.store(true, Relaxed);
            SIZING.store(false, Relaxed);
            SCALE_CHANGED_IN_MOVE.store(false, Relaxed);
        }
        WM_SIZING => SIZING.store(true, Relaxed),
        WM_EXITSIZEMOVE => {
            let r = call_orig(hwnd, msg, wparam, lparam);
            IN_SIZE_MOVE.store(false, Relaxed);
            // an edge resize ends where the user put it
            if SCALE_CHANGED_IN_MOVE.swap(false, Relaxed) && !SIZING.load(Relaxed) {
                contain(hwnd);
            }
            return r;
        }
        WM_DPICHANGED if !HANDLING_DPI.swap(true, Relaxed) => {
            let r = on_dpi_changed(hwnd, wparam, lparam);
            HANDLING_DPI.store(false, Relaxed);
            return r;
        }
        _ => {}
    }
    call_orig(hwnd, msg, wparam, lparam)
}

unsafe fn call_orig(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let orig = ORIG_PROC.load(Relaxed);
    if orig == 0 {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    let orig: WNDPROC = std::mem::transmute::<isize, WNDPROC>(orig);
    CallWindowProcW(orig, hwnd, msg, wparam, lparam)
}

unsafe fn on_dpi_changed(hwnd: HWND, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let new_dpi = (wparam & 0xFFFF) as u32;
    let old_dpi = DPI.swap(new_dpi, Relaxed).max(1);
    let before = window_rect(hwnd);
    // the monitor the new scale is for (the one the window is mostly on)
    let target = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
    let mut cursor = POINT { x: 0, y: 0 };
    GetCursorPos(&mut cursor);
    // winit: the new scale factor, its resize and nudge
    let r = call_orig(hwnd, WM_DPICHANGED, wparam, lparam);
    if IsZoomed(hwnd) != 0 || IsIconic(hwnd) != 0 {
        return r;
    }
    if !IN_SIZE_MOVE.load(Relaxed) {
        contain(hwnd);
        return r;
    }
    SCALE_CHANGED_IN_MOVE.store(true, Relaxed);
    if SIZING.load(Relaxed) {
        set_window_rect(hwnd, before);
        return r;
    }
    // title-bar drag: winit's size (capped), placed around the cursor
    let (window, visible) = frame_rects(hwnd);
    let work = work_area(target);
    let w = fit_len(width(visible), width(work));
    let h = fit_len(height(visible), height(work));
    let fx = (cursor.x - before.left) as f64 / width(before).max(1) as f64;
    let dy = (cursor.y - before.top) as f64 * new_dpi as f64 / old_dpi as f64;
    let left = cursor.x - (fx * w as f64).round() as i32;
    let top = cursor.y - dy.round() as i32;
    let mut want = RECT { left, top, right: left + w, bottom: top + h };
    // off `target` the scale would flip straight back
    if MonitorFromRect(&want, MONITOR_DEFAULTTONEAREST) != target {
        want = contained(want, work);
    }
    set_window_rect(hwnd, with_borders(want, window, visible));
    r
}

/// Fit `hwnd` inside the work area of the monitor it is mostly on.
unsafe fn contain(hwnd: HWND) {
    if IsZoomed(hwnd) != 0 || IsIconic(hwnd) != 0 {
        return;
    }
    let (window, visible) = frame_rects(hwnd);
    let work = work_area(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST));
    let want = contained(visible, work);
    if !same_rect(&want, &visible) {
        set_window_rect(hwnd, with_borders(want, window, visible));
    }
}

/// `r` shrunk (see `fit_len`) and slid to lie inside `work`.
fn contained(r: RECT, work: RECT) -> RECT {
    let w = fit_len(width(r), width(work));
    let h = fit_len(height(r), height(work));
    let left = r.left.min(work.right - w).max(work.left);
    let top = r.top.min(work.bottom - h).max(work.top);
    RECT { left, top, right: left + w, bottom: top + h }
}

/// A side longer than the space for it shrinks to 90% of that space.
fn fit_len(len: i32, avail: i32) -> i32 {
    if len > avail {
        avail * 9 / 10
    } else {
        len
    }
}

/// The window rect and its visible part: Windows 10+ adds invisible resize
/// borders to the window rect, which should not count against the screen.
unsafe fn frame_rects(hwnd: HWND) -> (RECT, RECT) {
    let window = window_rect(hwnd);
    let mut visible = window;
    let ok = DwmGetWindowAttribute(hwnd, DWMWA_EXTENDED_FRAME_BOUNDS as u32, &mut visible as *mut RECT as *mut _, std::mem::size_of::<RECT>() as u32) == 0;
    if !ok || width(visible) <= 0 || height(visible) <= 0 {
        visible = window;
    }
    (window, visible)
}

/// The window rect whose visible part is `want`, with the borders `window`
/// currently has around `visible`.
fn with_borders(want: RECT, window: RECT, visible: RECT) -> RECT {
    RECT {
        left: want.left - (visible.left - window.left),
        top: want.top - (visible.top - window.top),
        right: want.right + (window.right - visible.right),
        bottom: want.bottom + (window.bottom - visible.bottom),
    }
}

unsafe fn window_rect(hwnd: HWND) -> RECT {
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    GetWindowRect(hwnd, &mut r);
    r
}

unsafe fn work_area(monitor: HMONITOR) -> RECT {
    let mut info: MONITORINFO = std::mem::zeroed();
    info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    GetMonitorInfoW(monitor, &mut info);
    info.rcWork
}

unsafe fn set_window_rect(hwnd: HWND, r: RECT) {
    SetWindowPos(hwnd, 0, r.left, r.top, width(r), height(r), SWP_NOZORDER | SWP_NOACTIVATE);
}

fn width(r: RECT) -> i32 {
    r.right - r.left
}

fn height(r: RECT) -> i32 {
    r.bottom - r.top
}

fn same_rect(a: &RECT, b: &RECT) -> bool {
    (a.left, a.top, a.right, a.bottom) == (b.left, b.top, b.right, b.bottom)
}
