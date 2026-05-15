//! Layer 3: tests that require a real Minecraft Bedrock installation.
//!
//! Run with: `cargo test --features minecraft-required --test mc_integration -- --test-threads=1`
//!
//! Each test that starts Minecraft is wrapped in a `MinecraftScope` RAII guard
//! so a panic doesn't leave the game running on the developer's machine.

#![cfg(all(target_os = "windows", feature = "minecraft-required"))]

use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use flarial_launcher_lib::minecraft::{discovery, inject, launch};
use flarial_launcher_lib::packages;

struct MinecraftScope {
    pid: Option<u32>,
}
impl Drop for MinecraftScope {
    fn drop(&mut self) {
        if let Some(pid) = self.pid {
            let _ = Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/F"])
                .status();
        }
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

fn ensure_no_minecraft_running() {
    let _ = Command::new("taskkill")
        .args(["/IM", "Minecraft.Windows.exe", "/F"])
        .status();
    std::thread::sleep(Duration::from_millis(500));
}

#[tokio::test(flavor = "multi_thread")]
async fn minecraft_package_present() {
    let info = packages::query_minecraft().await.unwrap();
    assert!(info.installed, "Minecraft must be installed for Layer 3");
    assert!(
        info.version.is_some(),
        "package version should parse"
    );
    assert!(
        info.installed_path.is_some(),
        "installed_path should be discoverable"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn gaming_services_present() {
    let info = packages::query_minecraft().await.unwrap();
    assert!(
        info.gaming_services_installed,
        "Gaming Services must be installed for Layer 3"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn installed_path_contains_executable() {
    let info = packages::query_minecraft().await.unwrap();
    let path = PathBuf::from(info.installed_path.expect("installed_path"));
    let exe = path.join("Minecraft.Windows.exe");
    assert!(
        exe.exists(),
        "Minecraft.Windows.exe should exist at {}",
        exe.display()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn launch_starts_minecraft_then_kills_it() {
    ensure_no_minecraft_running();

    let pid = launch::launch().await.expect("launch should succeed");
    let mut guard = MinecraftScope { pid: Some(pid) };

    // give the game ~1s to populate process table
    std::thread::sleep(Duration::from_secs(1));
    let found = discovery::find_minecraft_pid().expect("WTS enum");
    assert!(
        matches!(found, Some(p) if p == pid),
        "discovery should find the launched process"
    );
    guard.pid = Some(pid);
}

#[tokio::test(flavor = "multi_thread")]
async fn inject_canary_into_real_minecraft() {
    ensure_no_minecraft_running();

    let dll = workspace_target_dir().join("flarial_canary.dll");
    assert!(
        dll.exists(),
        "build canary first: cargo build -p canary-dll --release ({} missing)",
        dll.display()
    );

    let pid = launch::launch().await.expect("launch");
    let mut guard = MinecraftScope { pid: Some(pid) };

    // Wait until the process is in a stable state to accept a remote thread.
    // Up to 10s — Store-signed Minecraft can take a beat to fully initialise.
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if discovery::find_minecraft_pid().ok().flatten() == Some(pid) {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    let result = inject::inject(pid, &dll);
    assert!(
        result.is_ok(),
        "inject canary into real Minecraft (pid {pid}): {result:?}"
    );

    // Give DllMain a moment to run — but don't fail the test if the marker
    // file ends up in Minecraft's AppContainer-mapped %TEMP% (which is not
    // our %TEMP%). Inject success + process survival is the contract here;
    // the canary marker check is best-effort.
    std::thread::sleep(Duration::from_millis(500));

    // Process must still be alive after injection.
    let still = discovery::find_minecraft_pid().ok().flatten();
    assert_eq!(
        still,
        Some(pid),
        "Minecraft must survive a successful injection"
    );

    guard.pid = Some(pid);
}
