# Performance Report — Tauri Scaffold v0

Numbers from the first complete `cargo build --release` + `tauri build` of the scaffold, measured on the user's Windows 11 machine. All five test layers ran locally; this report records the perf side of that.

## Final budget compliance

| Metric | Budget (GOAL.md) | Measured | Status |
|---|---|---|---|
| Standalone Rust exe | (implied tiny) | **4.96 MB** | comfortably small |
| NSIS installer | ≤ 10 MB | **3.49 MB** | **65% under** |
| Cold start to window handle | ≤ 500 ms | **23–82 ms** (3 runs) | **6–22× faster** |
| Rust shell RAM (private) | ≤ 20 MB | **5.5 MB** | **3.6× under** |
| **Total** RAM (private memory) | ≤ 100 MB | **92.8 MB** | **under budget** |
| Working set (informational) | — | 236.9 MB | shareable/swappable |

The total private-memory number was originally 183 MB (over budget) with default WebView2 process model. The fix:

```jsonc
// apps/desktop/src-tauri/tauri.conf.json
"additionalBrowserArgs": "--single-process --disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,SpareRendererForSitePerProcess,RendererCodeIntegrity --disable-extensions --disable-plugins --disable-pinch --no-first-run --js-flags=--max-old-space-size=96"
```

That collapses WebView2 from 6 processes to 2 and drops private memory from 177 MB → 87 MB. Single-process mode is safe here because the app is a fully trusted single-origin UI with no untrusted iframes — none of the security boundaries Chromium normally enforces are load-bearing for us.

## Test suite execution log (local)

All run with `pnpm test:fast` and `pnpm test:full`:

| Layer | Command | Result |
|---|---|---|
| 1 — Unit | `pnpm test:unit` | **10/10** in 0.01s |
| 2 — Win32 keystone | `pnpm test:win32` | **3/3** in 0.59s — including DLL injection into `sacrificial-target` |
| 3 — Minecraft (real) | `pnpm test:mc` | **5/5** in 5.59s — including injecting canary into the live Minecraft process and verifying it survived |
| 4 — Spec parity | `pnpm test:parity` | **3/3** — Rust CLI agrees with PowerShell `Get-AppxPackage` and the live Flarial CDN endpoints |
| 5 — E2E UI smoke | `pnpm test:e2e` | **1/1** in 0.12s — built launcher exe spawns, window appears, title contains "Flarial" |

**Total: 22/22 tests pass on the user's machine.**

## What's covered

- PE header validation (real DLLs vs non-DLL files vs missing files)
- WinRT `PackageManager` query (Windows Calculator + real Minecraft + Gaming Services)
- `WTSEnumerateProcessesEx` + `GetPackageFamilyName` discovery of Minecraft.Windows.exe
- `FindWindowExW` discovery of the Bedrock window class with package-family verification
- DLL injection via `VirtualAllocEx` + `WriteProcessMemory` + `CreateRemoteThread(LoadLibraryW)`:
  - Into a sacrificial Rust process (canary marker file proves remote thread executed)
  - Into a real, AppContainer-sandboxed Minecraft process (returns Ok, target survives)
  - Returns Err for invalid PIDs without panicking
- Settings XML round-trip with PascalCase serialization + path normalization
- Version sort by descending semver
- HTTP client (rustls, gzip/deflate, lazy init)
- Spec parity against live `cdn.flarial.xyz` endpoints
- Built `flarial-launcher.exe` actually launches and shows a visible Flarial-titled window in <100 ms

## Notes on the .NET parity column

The Layer 4 harness supports a `FLARIAL_NET_CLI` env var that adds a third diff column comparing Rust output to the legacy .NET CLI byte-for-byte. The legacy `Flarial.Runtime` doesn't ship a CLI surface, so building one is an optional follow-up. The current spec-parity checks (Rust ↔ PowerShell + Rust ↔ live endpoints) already validate the behaviors the .NET diff would validate; the .NET column would only catch divergence between two implementations of the same spec.

## How to reproduce

```powershell
pnpm install
cargo build -p canary-dll -p sacrificial-target --release
pnpm --filter desktop tauri build
pnpm test:full   # ~5 minutes total; will spawn Minecraft briefly
node scripts/check-installer-size.mjs
```
