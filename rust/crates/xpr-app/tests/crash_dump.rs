//! `crash_dump`: an access violation leaves a minidump in the folder given.
#![cfg(windows)]

use std::process::Command;

const CHILD_ENV: &str = "XPR_CRASH_DUMP_TEST_DIR";

#[test]
fn an_access_violation_writes_a_dump() {
    if let Ok(dir) = std::env::var(CHILD_ENV) {
        // the child: crash for real, without a Windows Error Reporting dialog
        unsafe { windows_sys::Win32::System::Diagnostics::Debug::SetErrorMode(windows_sys::Win32::System::Diagnostics::Debug::SEM_NOGPFAULTERRORBOX) };
        xpr_app::crash_dump::install(std::path::Path::new(&dir));
        unsafe { std::ptr::write_volatile(8 as *mut u64, 1) };
        unreachable!();
    }
    let dir = std::env::temp_dir().join(format!("xpr_crash_dump_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .args(["an_access_violation_writes_a_dump", "--exact", "--nocapture"])
        .env(CHILD_ENV, &dir)
        .status()
        .unwrap();
    assert!(!status.success());
    let dumps: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "dmp")).collect();
    assert_eq!(dumps.len(), 1, "{dumps:?}");
    let bytes = std::fs::read(&dumps[0]).unwrap();
    assert!(bytes.starts_with(b"MDMP"), "not a minidump");
    let _ = std::fs::remove_dir_all(&dir);
}
