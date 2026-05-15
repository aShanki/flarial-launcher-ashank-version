//! Test-only DLL that records its load into `%TEMP%\flarial-canary-{pid}-{nanos}.txt`.
//! Used by the launcher's injection tests to prove our remote-thread mechanism
//! actually executes code in the target process.

#![cfg(target_os = "windows")]

use std::ffi::c_void;
use std::io::Write;

#[no_mangle]
#[allow(non_snake_case, unused_variables)]
pub extern "system" fn DllMain(
    _module: *mut c_void,
    reason: u32,
    _reserved: *mut c_void,
) -> i32 {
    const DLL_PROCESS_ATTACH: u32 = 1;
    if reason == DLL_PROCESS_ATTACH {
        let _ = write_marker();
    }
    1
}

fn write_marker() -> std::io::Result<()> {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::var_os("TEMP")
        .or_else(|| std::env::var_os("TMP"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let path = dir.join(format!("flarial-canary-{pid}-{nanos}.txt"));
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "LOADED pid={pid} nanos={nanos}")?;
    Ok(())
}
