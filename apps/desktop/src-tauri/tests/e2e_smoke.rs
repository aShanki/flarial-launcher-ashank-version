//! Layer 5: end-to-end smoke test for the built launcher binary.
//!
//! Spawns the actual `flarial-launcher.exe`, waits for its main window to
//! appear, asserts it's visible and titled correctly, then kills it.
//! Doesn't require Minecraft. Doesn't require WebDriver tooling.
//!
//! Run via `cargo test --test e2e_smoke --release`.

#![cfg(target_os = "windows")]

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use windows::core::HSTRING;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Threading::GetProcessId;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
    IsWindowVisible,
};

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn workspace_target_dir() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    while !p.join("Cargo.toml").exists() || !p.join("crates").exists() {
        if !p.pop() {
            panic!("cannot locate workspace root");
        }
    }
    p.join("target").join("release")
}

fn launcher_exe() -> PathBuf {
    workspace_target_dir().join("flarial-launcher.exe")
}

fn ensure_launcher_built() {
    if launcher_exe().exists() {
        return;
    }
    let status = Command::new("cargo")
        .args([
            "build",
            "--release",
            "-p",
            "flarial-launcher",
            "--bin",
            "flarial-launcher",
        ])
        .status()
        .expect("cargo build");
    assert!(status.success(), "launcher build failed");
}

#[derive(Default)]
struct WindowSearch {
    target_pid: u32,
    found: Option<(HWND, String)>,
}

unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: windows::Win32::Foundation::LPARAM) -> windows_core::BOOL {
    let search = &mut *(lparam.0 as *mut WindowSearch);
    let mut pid: u32 = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid != search.target_pid {
        return true.into();
    }
    if !IsWindowVisible(hwnd).as_bool() {
        return true.into();
    }
    let len = GetWindowTextLengthW(hwnd);
    if len == 0 {
        return true.into();
    }
    let mut buf = vec![0u16; (len as usize) + 1];
    let copied = GetWindowTextW(hwnd, &mut buf);
    if copied <= 0 {
        return true.into();
    }
    let title = String::from_utf16_lossy(&buf[..copied as usize]);
    search.found = Some((hwnd, title));
    false.into()
}

fn find_visible_window_for(pid: u32) -> Option<(HWND, String)> {
    let mut search = WindowSearch { target_pid: pid, found: None };
    unsafe {
        let _ = EnumWindows(
            Some(enum_proc),
            windows::Win32::Foundation::LPARAM(&mut search as *mut _ as isize),
        );
    }
    search.found
}

#[test]
fn launcher_window_appears_with_correct_title() {
    ensure_launcher_built();

    let exe = launcher_exe();
    assert!(exe.exists(), "missing: {}", exe.display());

    let child = Command::new(&exe)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn launcher");
    let pid = child.id();
    let guard = ChildGuard(child);

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut window: Option<(HWND, String)> = None;
    while Instant::now() < deadline {
        if let Some(found) = find_visible_window_for(pid) {
            window = Some(found);
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    let (_hwnd, title) = window.expect("launcher window did not appear within 10s");
    assert!(
        title.contains("Flarial"),
        "window title should contain 'Flarial', got: {title:?}"
    );

    drop(guard);
    let _ = HSTRING::new(); // anchor windows crate use
    let _ = unsafe { GetProcessId(windows::Win32::Foundation::HANDLE::default()) };
}
