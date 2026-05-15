//! Layer 2 keystone test: prove the injection mechanism works end-to-end against
//! a sacrificial process — without involving Minecraft.
//!
//! Spawns `sacrificial-target.exe`, injects `flarial_canary.dll`, polls
//! `%TEMP%` for the marker file the canary writes on `DLL_PROCESS_ATTACH`,
//! then kills the target.

#![cfg(target_os = "windows")]

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use flarial_launcher_lib::minecraft::inject::inject;
use flarial_launcher_lib::minecraft::library::is_loadable;

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
    }
}

fn workspace_target_dir() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    while !p.join("Cargo.toml").exists() || !p.join("crates").exists() {
        if !p.pop() {
            panic!("cannot locate workspace root");
        }
    }
    p.join("target")
}

fn fixture_path(profile: &str, name: &str) -> PathBuf {
    workspace_target_dir().join(profile).join(name)
}

fn build_fixtures() {
    let status = Command::new("cargo")
        .args([
            "build",
            "-p",
            "canary-dll",
            "-p",
            "sacrificial-target",
            "--release",
        ])
        .status()
        .expect("cargo must run");
    assert!(status.success(), "fixture build failed");
}

fn temp_dir() -> PathBuf {
    std::env::var_os("TEMP")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
}

fn marker_files_for(pid: u32) -> Vec<PathBuf> {
    let prefix = format!("flarial-canary-{pid}-");
    std::fs::read_dir(temp_dir())
        .map(|it| {
            it.flatten()
                .filter(|e| {
                    e.file_name()
                        .to_string_lossy()
                        .starts_with(&prefix)
                })
                .map(|e| e.path())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn canary_dll_validates_as_loadable() {
    build_fixtures();
    let dll = fixture_path("release", "flarial_canary.dll");
    assert!(dll.exists(), "fixture missing: {}", dll.display());
    assert!(is_loadable(&dll), "canary DLL should validate");
}

#[test]
fn inject_canary_into_sacrificial_target() {
    build_fixtures();
    let dll = fixture_path("release", "flarial_canary.dll");
    let target = fixture_path("release", "sacrificial-target.exe");
    assert!(dll.exists(), "missing: {}", dll.display());
    assert!(target.exists(), "missing: {}", target.display());

    let child = Command::new(&target)
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn sacrificial-target");
    let pid = child.id();
    let mut guard = ChildGuard(child);

    // Cleanup any stale marker from a prior run.
    for p in marker_files_for(pid) {
        let _ = std::fs::remove_file(p);
    }

    // Give the target a moment to fully initialize before injecting.
    std::thread::sleep(Duration::from_millis(150));

    let result = inject(pid, &dll).expect("inject should succeed");
    assert_eq!(result, pid);

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut markers = vec![];
    while Instant::now() < deadline {
        markers = marker_files_for(pid);
        if !markers.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(
        !markers.is_empty(),
        "canary did not write marker file for pid {pid}"
    );

    // Cleanup
    for p in &markers {
        let _ = std::fs::remove_file(p);
    }
    let _ = guard.0.try_wait();
}

#[test]
fn inject_invalid_pid_returns_error() {
    build_fixtures();
    let dll = fixture_path("release", "flarial_canary.dll");
    let result = inject(0xDEAD_BEEF, &dll);
    assert!(result.is_err(), "expected error for bogus pid");
}
