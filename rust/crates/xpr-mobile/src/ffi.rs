//! The C ABI (`include/xpr_mobile.h`): JSON in, JSON out. Panics are caught
//! at the boundary and reported as errors so a bug in one command cannot
//! take the app down.

use std::ffi::{c_char, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Mutex;

use serde_json::{json, Value};

use crate::session::Session;

static SESSION: Mutex<Option<Session>> = Mutex::new(None);

fn to_c(v: Value) -> *mut c_char {
    let s = serde_json::to_string(&v).unwrap_or_else(|_| "{\"err\":\"serialisation failed\"}".into());
    CString::new(s).unwrap_or_else(|_| CString::new("{\"err\":\"nul in output\"}").unwrap()).into_raw()
}

fn read(p: *const c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
}

fn envelope(r: Result<Value, String>) -> Value {
    match r {
        Ok(v) => json!({ "ok": v }),
        Err(e) => json!({ "err": e }),
    }
}

fn panic_text(e: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = e.downcast_ref::<&str>() {
        format!("internal error: {}", s)
    } else if let Some(s) = e.downcast_ref::<String>() {
        format!("internal error: {}", s)
    } else {
        "internal error".into()
    }
}

#[no_mangle]
pub extern "C" fn xpr_init(config_dir: *const c_char, data_dir: *const c_char) -> *mut c_char {
    let (c, d) = (read(config_dir), read(data_dir));
    let r = catch_unwind(|| Session::new(c.into(), d.into()));
    let r = match r {
        Ok(Ok(s)) => {
            let mut g = SESSION.lock().unwrap_or_else(|e| e.into_inner());
            *g = Some(s);
            Ok(json!(true))
        }
        Ok(Err(e)) => Err(e),
        Err(e) => Err(panic_text(e)),
    };
    to_c(envelope(r))
}

fn with_session<T>(f: impl FnOnce(&mut Session) -> Result<T, String>) -> Result<T, String> {
    let mut g = SESSION.lock().unwrap_or_else(|e| e.into_inner());
    let s = g.as_mut().ok_or_else(|| "session not started".to_string())?;
    match catch_unwind(AssertUnwindSafe(|| f(s))) {
        Ok(r) => r,
        Err(e) => Err(panic_text(e)),
    }
}

#[no_mangle]
pub extern "C" fn xpr_call(method: *const c_char, params_json: *const c_char) -> *mut c_char {
    let method = read(method);
    let raw = read(params_json);
    let params: Value = if raw.trim().is_empty() { json!({}) } else { serde_json::from_str(&raw).unwrap_or(json!({})) };
    let r = with_session(|s| s.dispatch(&method, &params));
    if let Err(e) = &r {
        log::warn!("{} failed: {}", method, e);
    }
    to_c(envelope(r))
}

#[no_mangle]
pub extern "C" fn xpr_png(method: *const c_char, params_json: *const c_char, out: *mut *mut u8) -> usize {
    let method = read(method);
    let raw = read(params_json);
    let params: Value = serde_json::from_str(&raw).unwrap_or(json!({}));
    match with_session(|s| s.png(&method, &params)) {
        Ok(bytes) if !bytes.is_empty() && !out.is_null() => {
            let mut b = bytes.into_boxed_slice();
            let len = b.len();
            let p = b.as_mut_ptr();
            std::mem::forget(b);
            unsafe { *out = p };
            len
        }
        Ok(_) => 0,
        Err(e) => {
            log::warn!("{} failed: {}", method, e);
            0
        }
    }
}

#[no_mangle]
pub extern "C" fn xpr_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe { drop(CString::from_raw(s)) };
    }
}

#[no_mangle]
pub extern "C" fn xpr_free_bytes(p: *mut u8, len: usize) {
    if !p.is_null() {
        unsafe { drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(p, len))) };
    }
}
