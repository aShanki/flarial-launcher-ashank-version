# Flarial Launcher — Testing Framework

The rewrite must prove, automatically, that **the Rust backend behaves identically to the .NET reference**, with special attention to Minecraft launch and DLL injection. This document defines the five test layers, the fixtures we build, and what runs in CI.

---

## The Five Layers

All layers run **locally on your Windows machine**. No CI, no remote runners, no GitHub Actions. One command per layer; one top-level `pnpm test:all` runs the lot.

| # | Layer | Requires Minecraft installed? | Speed |
|---|---|---|---|
| 1 | **Unit** (Rust) | no | < 5 s |
| 2 | **Win32 integration** (sacrificial process) | no | < 30 s |
| 3 | **Minecraft integration** (real game) | yes | minutes |
| 4 | **.NET ↔ Rust parity diff** | yes | minutes |
| 5 | **End-to-end UI smoke** (Playwright + tauri-driver, mocked backend) | no | < 2 min |

Each layer is described below with concrete commands, fixtures, and pass/fail criteria.

---

## Layer 1 — Unit Tests (Rust)

**Scope:** pure-logic functions with no I/O. Examples:

- `NumericVersion` parsing + ordering
- `VersionItemComparer` sort
- `Supported.json` deserialization
- `dll_hashes.json` deserialization + case-insensitive compare
- Settings XML parse + round-trip + path normalization
- Settings legacy-XML → new-format migration

**Convention:**

```rust
// crates/.../src/version.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_descending() {
        let mut v = vec!["1.20.0", "1.21.0", "1.20.99"]
            .into_iter().map(NumericVersion::parse).collect::<Vec<_>>();
        v.sort();
        assert_eq!(v.iter().map(|x| x.to_string()).collect::<Vec<_>>(),
                   vec!["1.21.0", "1.20.99", "1.20.0"]);
    }
}
```

**Run:** `cargo test --workspace --lib`

**Pass criterion:** all green, < 5 s wall clock.

---

## Layer 2 — Win32 Integration (no Minecraft required)

These tests exercise the **real Win32 / WinRT APIs** but against safe targets, not Minecraft. They prove the wiring works on a fresh CI runner.

### 2.1 Test fixtures we build

#### `crates/sacrificial-target` — idle process for injection
A minimal Rust binary:
```rust
fn main() {
    println!("sacrificial-target ready, pid={}", std::process::id());
    std::thread::park();
}
```
Used as a stand-in process for: enumeration, window discovery (it has no window — that branch is exercised differently), handle-open, and DLL injection.

#### `crates/canary-dll` — proof-of-load DLL
Cargo `crate-type = ["cdylib"]`. On `DLL_PROCESS_ATTACH`:
- Reads its host process PID.
- Writes `%TEMP%\flarial-canary-{pid}-{nanos}.txt` containing `LOADED <pid> <iso8601>\n`.
- Returns `TRUE`.

This DLL has **no dependencies**, no allocator quirks, and no UI. It exists to answer one question: *"did our injection actually run code in the target?"*

Build once with `cargo build -p canary-dll --release`; tests reference the built artifact path via `env!("CARGO_BIN_FILE_CANARY_DLL")` or a workspace constant.

### 2.2 What we test at this layer

| Test | What it proves |
|---|---|
| `enumerate_processes_finds_self` | `WTSEnumerateProcessesEx` wrapper returns at least the test process |
| `enumerate_processes_filters_by_name` | spawn `notepad.exe`, find it, kill it |
| `package_family_lookup_for_self_returns_none` | `GetPackageFamilyName` against an unpackaged process returns the expected error code |
| `find_window_returns_none_for_unknown_class` | `FindWindowExW("NotARealClassXYZ")` is `None` |
| `loadable_validates_real_dll` | `Library::is_loadable("C:\\Windows\\System32\\user32.dll")` → true |
| `loadable_rejects_non_dll` | `Library::is_loadable(<the launcher exe>)` → false |
| `loadable_rejects_missing_file` | non-existent path → false, no panic |
| **`inject_canary_into_sacrificial_target`** | spawn `sacrificial-target`, inject `canary-dll`, poll for `%TEMP%\flarial-canary-{pid}-*.txt`, kill target — **this is the keystone test for injection** |
| `inject_double_does_not_crash_target` | inject canary twice, target still alive |
| `inject_invalid_pid_returns_none` | inject into PID `0xDEADBEEF` returns `None`, no panic |
| `appx_query_known_package` | query `Microsoft.WindowsCalculator_8wekyb3d8bbwe` (always present on Win10/11) |
| `http_client_follows_redirect` | hit a `httpbin`-style endpoint via `wiremock` and assert redirect count |
| `settings_round_trip` | write defaults, read back, deep equal |
| `legacy_xml_migration_drops_invalid_path` | feed an XML with a malformed path, expect `CustomDllPath = ""` |

**Run:** `cargo test --workspace --tests`

**Pass criterion:** all green on your Windows machine with no extra software. The keystone is `inject_canary_into_sacrificial_target` — if that fails, **stop and fix before doing anything else**. A broken injection mechanism makes every other test irrelevant.

---

## Layer 3 — Minecraft Integration (real game required)

Gated by Cargo feature `minecraft-required` so casual contributors aren't forced to install Bedrock:

```toml
# tests/rust/Cargo.toml
[features]
minecraft-required = []
```

```rust
#[cfg(feature = "minecraft-required")]
#[test]
fn finds_minecraft_package() { ... }
```

Run via `cargo test --features minecraft-required -- --test-threads=1`. Sequential because most of these tests want sole control of the Minecraft process.

### 3.1 Tests

| Test | What it proves |
|---|---|
| `minecraft_package_present` | `find_packages("Microsoft.MinecraftUWP_8wekyb3d8bbwe")` returns one entry; version string parses |
| `gaming_services_present` | same for `Microsoft.GamingServices_8wekyb3d8bbwe` |
| `installed_path_exists` | `Package.InstalledPath\Minecraft.Windows.exe` is a file |
| `launch_starts_minecraft_then_kills_it` | call `launch()`, get PID, assert process exists, **immediately kill it**; do not wait for Bedrock window unless `MC_FULL_LAUNCH=1` env is set |
| `launch_full_window` *(opt-in via env)* | `launch()` returns PID, Bedrock window appears, becomes visible, then test kills the process |
| `window_finder_after_launch` | with game running, `find_window()` returns a `NativeWindow` whose `pid` matches |
| **`inject_canary_into_real_minecraft`** | start MC, inject `canary-dll` (NOT the real client), assert canary file appears in `%TEMP%`, MC still running, then kill — **proves end-to-end injection works against the real AppContainer'd target** |
| `menu_load_lock_watcher_fires` | start MC, watch `%AppData%\Minecraft Bedrock\Users\`, assert a deletion event eventually arrives within 30 s |
| `appx_install_progress_callback_fires` | (skip if no test MSIX available; otherwise install a known-tiny MSIX and assert progress callbacks appear with monotonic 0..100) |

The injection test uses `canary-dll` deliberately. It would be wrong — and slow — to test injection by loading the real Flarial client and waiting for it to do something visible in the game. The canary test isolates the **injection mechanism** from the client behavior.

### 3.2 Cleanup harness

Every Layer-3 test must run inside a `MinecraftScope` RAII guard:

```rust
struct MinecraftScope { pid: Option<u32> }
impl Drop for MinecraftScope {
    fn drop(&mut self) {
        if let Some(pid) = self.pid {
            unsafe { TerminateProcess(open(pid), 1); }
        }
    }
}
```

If a test panics, the guard kills the leaked Minecraft process so your machine doesn't accumulate orphaned game instances between test runs.

---

## Layer 4 — .NET ↔ Rust Parity Diff

The point of this layer: prove that *for the same input scenario*, the new Rust backend produces the same observable side effects as the old .NET backend.

### 4.1 Architecture

We expose a tiny **CLI wrapper** around each backend:

```
flarial-cli-net    (existing project, new --json flag)
flarial-cli-rust   (new, lives under apps/desktop/src-tauri/bin/)
```

Both expose the same subcommands:

```
flarial-cli {net|rust} package query                # JSON of {installed, packaged, version, installed_path}
flarial-cli {net|rust} package query --gdk          # GDK version + supported list
flarial-cli {net|rust} versions list                # supported.json + GDK URLs
flarial-cli {net|rust} updates check                # launcher version delta
flarial-cli {net|rust} dll check                    # local hash + remote hash + needs_download
flarial-cli {net|rust} launch --no-inject --kill    # launch MC, return PID, kill it
flarial-cli {net|rust} launch --inject canary       # full path: launch + inject canary + report timing
```

### 4.2 The diff harness

A `tests/parity/run.ps1` (or `cargo run -p parity-runner`) script that, for each scenario:

1. Snapshots the world: HTTP traffic captured by **`mitmproxy --mode reverse`** intercepting `*.flarial.xyz` (or `wiremock` if we want fully offline), file-system snapshot of `%LOCALAPPDATA%\Flarial`, snapshot of running processes named `Minecraft.Windows.exe`.
2. Runs `flarial-cli-net <subcommand>`, captures stdout JSON + side effects.
3. Restores the snapshot.
4. Runs `flarial-cli-rust <subcommand>`, captures the same.
5. Diffs:
   - **stdout JSON** with `serde_json::Value` deep-equals (ignoring fields explicitly listed in a per-scenario `ignore` set, e.g. timestamps, install IDs)
   - **HTTP traffic**: same set of method+host+path requests, same set of headers (after stripping `User-Agent`, `X-Flarial-Nonce`, `X-Flarial-Timestamp` from comparison)
   - **file-system delta**: same files created/modified
   - **process delta**: both spawned (or both didn't spawn) `Minecraft.Windows.exe`

### 4.3 Scenarios

`tests/parity/scenarios/*.json`:

```json
{
  "name": "package-query-installed",
  "subcommand": ["package", "query"],
  "ignore_json_fields": ["installed_path"],
  "expect_http": [
    {"method": "GET", "host": "cdn.flarial.xyz", "path": "/launcher/Supported.json"}
  ],
  "expect_files_created": [],
  "expect_minecraft_spawned": false
}
```

Initial set:
- `package-query-installed`
- `package-query-not-installed` (run after temporarily renaming MC package — *manual scenario only*)
- `versions-list`
- `updates-check-up-to-date` (mock launcher version response to match local)
- `updates-check-needs-update` (mock launcher version response to be ahead)
- `dll-check-already-current`
- `dll-check-needs-download`
- `launch-no-inject-then-kill`
- `launch-inject-canary` (Layer-3 + parity gold standard — this is the test that catches subtle Win32 differences)

### 4.4 Pass criterion

All scenarios diff-clean. Any genuine intentional behavior change must:
1. Add itself to the scenario's `ignore_*` list with a comment explaining why, **or**
2. Update the .NET CLI wrapper to match (preferred when the new behavior is correct), **or**
3. Update `BEHAVIOR_SPEC.md` and the scenario file.

---

## Layer 5 — End-to-End UI Smoke (Playwright + tauri-driver)

Drives the actual built `.exe` via WebDriver. Backend commands are mocked at the Rust level via a `--test-mode` flag that swaps `commands::*` for in-memory fakes.

```
pnpm test:e2e
```

Stack: `playwright` + `tauri-driver` + `webdriverio`. (Playwright doesn't natively speak WebDriver Classic, so we use `webdriverio` directly; Playwright is installed only to share its browser-management ergonomics.)

### Smoke tests

- App launches, splash screen disappears, home screen visible
- "Launch" button is disabled when fake `getMinecraftPackage` returns `not installed`
- "Launch" button enabled when package is installed and supported
- Click "Launch" → loading state → success state when fake `launch` resolves
- Click "Launch" → error dialog matches `AppError.kind` from fake `launch` rejection
- Settings page round-trip: toggle "Use custom DLL", restart app (in-test), value persisted

These tests **do not** start Minecraft. The whole point is to validate UI logic against the typed IPC surface.

---

## Local Test Runner

Single entrypoint that wraps every layer. Implemented as a top-level `package.json` script so you don't have to remember six commands:

```json
{
  "scripts": {
    "test:unit":     "cargo test --workspace --lib",
    "test:win32":    "cargo test --workspace --tests",
    "test:mc":       "cargo test --features minecraft-required -- --test-threads=1",
    "test:parity":   "node tests/parity/run.mjs",
    "test:e2e":      "wdio run tests/e2e/wdio.conf.ts",
    "test:fast":     "pnpm test:unit && pnpm test:win32 && pnpm test:e2e",
    "test:full":     "pnpm test:fast && pnpm test:mc && pnpm test:parity",
    "size:check":    "node scripts/check-installer-size.mjs"
  }
}
```

Two daily workflows:

- **`pnpm test:fast`** — runs in under 3 minutes, doesn't touch Minecraft. Run this on every save / before every commit.
- **`pnpm test:full`** — full parity check. Run before declaring a feature done. Will launch and kill Minecraft several times; close any session you care about first.

Both write a Markdown summary to `target/test-report.md` listing pass/fail per scenario so you can scan it without rereading raw cargo output.

---

## Test-First Workflow for the Rewrite

For each row in `BEHAVIOR_SPEC.md`:

1. Write the parity scenario JSON (Layer 4) and the Layer-2 integration test stub.
2. Run them — they should fail because the Rust implementation doesn't exist yet.
3. Implement the Rust function.
4. `pnpm test:fast` — Layer 1, 2, 5 should pass.
5. When the feature touches Minecraft, run `pnpm test:full` once before calling it done.
6. Commit. (No CI; you are the CI.)

---

## Quick Reference

```powershell
# Most common — daily loop, no Minecraft involvement
pnpm test:fast

# Full check before declaring a feature done — will start/kill Minecraft
pnpm test:full

# Individual layers
pnpm test:unit          # Layer 1 — pure logic
pnpm test:win32         # Layer 2 — Win32 against sacrificial process
pnpm test:mc            # Layer 3 — real Minecraft
pnpm test:parity        # Layer 4 — .NET vs Rust diff
pnpm test:e2e           # Layer 5 — UI smoke with mocked backend

# Bundle-size sanity check
pnpm tauri build ; pnpm size:check
```

If a Minecraft test leaves an orphaned `Minecraft.Windows.exe` running (shouldn't happen — the `MinecraftScope` guard cleans up — but if it does):

```powershell
Get-Process Minecraft.Windows -ErrorAction SilentlyContinue | Stop-Process -Force
```
