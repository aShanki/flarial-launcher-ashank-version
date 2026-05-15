# Flarial Launcher — Rewrite Goal Prompt

> Paste this entire file into any AI agent's context as the master mission for the rewrite. The agent should also load `BEHAVIOR_SPEC.md` and `TESTING.md`.

---

## Mission

Re-implement the **Flarial Launcher for Minecraft Bedrock** as a tiny, fast Tauri 2 desktop app on Windows.

Two non-negotiable goals:

1. **Be small and lean.** Installer ≤ 10 MB. Idle RAM ≤ 100 MB. Cold start ≤ 500 ms. The user explicitly chose Tauri over Electron because every megabyte and every megabyte/second matters.
2. **Be writable by AI agents.** UI is React 19 + Vite + Tailwind 4 + shadcn/ui so any LLM can produce a screen without designer hand-holding. Backend is idiomatic Rust with a thin Tauri command surface — also LLM-friendly.

The existing .NET implementation (under `src/Flarial.Launcher/**` and `src/Flarial.Runtime/**`) is the **behavioral reference**. Match its observable side effects exactly. See `BEHAVIOR_SPEC.md` for the full contract.

---

## Stack (locked)

| Layer | Choice |
|---|---|
| Shell | **Tauri 2** (Rust 1.80+) |
| Webview | **WebView2** (system, no Chromium bundle) |
| Frontend framework | **React 19** + **TypeScript 5** strict |
| Bundler | **Vite 6** |
| Styling | **Tailwind CSS 4** + **shadcn/ui** components (copy-in, no runtime dep) |
| State | **Zustand** for global, React Query for async server state |
| Tauri↔React bridge | `@tauri-apps/api` v2, **`tauri-specta`** for typed commands |
| Rust async runtime | **Tokio** (Tauri default) |
| Rust HTTP | **`reqwest`** with `rustls-tls` (no openssl bloat) |
| JSON | **`serde_json`** |
| XML (legacy settings migration) | **`quick-xml`** with serde |
| Win32 bindings | **`windows`** crate (official Microsoft) |
| AppX/MSIX | **`windows::Management::Deployment::PackageManager`** (WinRT) |
| File watcher | **`notify`** |
| Hashing | **`sha2`** |
| Logging | **`tracing`** + `tracing-subscriber` (release: file + ring buffer in memory) |
| Errors | **`thiserror`** for typed enums + `anyhow` only at top-level commands |
| Build target | `x86_64-pc-windows-msvc` only (Windows-only product) |

**Anti-stack — do not pull in:**
- Electron, Neutralino, Wails (decision is Tauri)
- Material UI, Ant Design, Chakra (Tailwind+shadcn already cover this; bloats bundle)
- Redux, MobX (overkill — Zustand is enough)
- Any web framework that ships its own router runtime > 5 KB gzipped (we'll use a tiny custom router or `wouter`)
- OpenSSL (use rustls)
- `napi`, `neon`, FFI bridges other than Tauri's IPC

---

## Repo Layout (target)

```
launcher/
├─ apps/
│  └─ desktop/                 # Tauri app
│     ├─ src/                  # React frontend
│     │  ├─ pages/
│     │  ├─ components/
│     │  ├─ stores/            # Zustand
│     │  └─ ipc/               # Generated tauri-specta bindings
│     ├─ src-tauri/
│     │  ├─ src/
│     │  │  ├─ main.rs
│     │  │  ├─ commands/       # #[tauri::command] surface
│     │  │  ├─ minecraft/      # Game discovery, launch, injection
│     │  │  ├─ packages/       # AppX/MSIX (PackageManager)
│     │  │  ├─ updates/        # Self-update + client DLL update
│     │  │  ├─ settings/       # Persistence + legacy XML migration
│     │  │  ├─ http/           # reqwest wrapper
│     │  │  └─ telemetry/      # Launch analytics
│     │  ├─ tauri.conf.json
│     │  └─ Cargo.toml
│     └─ package.json
├─ crates/
│  ├─ canary-dll/              # Test fixture: tiny DLL that records its load
│  └─ sacrificial-target/      # Test fixture: idle process for injection tests
├─ tests/
│  ├─ rust/                    # cargo tests (unit + integration)
│  ├─ e2e/                     # WebDriver-based UI smoke
│  └─ parity/                  # .NET vs Rust diff harness
├─ docs/
│  └─ rewrite/
│     ├─ BEHAVIOR_SPEC.md      # binding contract
│     ├─ GOAL.md               # this file
│     └─ TESTING.md            # test framework spec
└─ src/                        # legacy .NET (kept until parity reached)
```

The legacy `src/` tree stays in the repo until the Rust port passes every parity test in `TESTING.md`. **Do not delete it preemptively.** Once parity is reached, move it to `legacy/` and remove on the release commit.

---

## Frontend Conventions (for AI agents producing UI)

- **Component file = one default export.** No barrel files (`index.ts` re-exports).
- **Props interfaces are exported** so siblings can compose.
- **Tailwind classes only.** No CSS modules, no styled-components.
- **shadcn/ui** is the visual baseline. Copy components into `src/components/ui/` (don't add a runtime dependency); style overrides via the `cn()` helper.
- **Icons:** `lucide-react`.
- **Animation:** `framer-motion` only when shadcn primitives don't suffice.
- **Routing:** start without one; add `wouter` if the screen count exceeds 4.
- **Forms:** native `<form>` + React 19 `useFormState`. Pull in `react-hook-form` only if a screen has 5+ fields with cross-validation.
- **Data fetching from Rust:** use the typed wrapper generated by `tauri-specta`, e.g.
  ```ts
  import { commands } from "@/ipc/bindings";
  const { data: pkg } = useQuery({ queryKey: ["mc"], queryFn: commands.getMinecraftPackage });
  ```
  **Never** call `invoke("...")` with a raw string — it bypasses type safety and the agent will misspell it.

---

## Backend Conventions (for AI agents producing Rust)

- **One module per spec section** (`minecraft`, `packages`, `updates`, etc.).
- **Public surface = `#[tauri::command]` functions in `commands/`.** Internal modules expose plain Rust APIs to those commands; no `#[tauri::command]` deeper than the `commands/` directory.
- **Errors:** every command returns `Result<T, AppError>` where `AppError` is a `thiserror` enum that **serializes to a stable JSON shape** the frontend can switch on:
  ```rust
  #[derive(thiserror::Error, Debug, serde::Serialize)]
  #[serde(tag = "kind", content = "detail")]
  pub enum AppError {
      #[error("Minecraft is not installed")] NotInstalled,
      #[error("Gaming Services missing")]    GamingServicesMissing,
      #[error("Injection failed: {0}")]      InjectionFailed(String),
      #[error("Network error: {0}")]         Network(String),
      #[error(transparent)]                  Other(#[from] anyhow::Error),
  }
  ```
- **No `unwrap()` / `expect()` in non-test code.** Propagate with `?`. The only exception is `Mutex::lock().unwrap()` — poisoning is a real bug, panic is correct.
- **No `unsafe` outside `minecraft/inject.rs` and `packages/native.rs`.** Both must have a `// SAFETY:` comment for every `unsafe` block citing the Win32 doc requirement satisfied.
- **No comments** unless explaining a non-obvious Win32 quirk or hidden invariant. Don't restate what `windows-rs` types already make clear.
- **Owned strings everywhere at the IPC boundary.** No `&str` in `#[tauri::command]` signatures.
- **Logging:** every command emits one `info!` at entry with structured fields and one `error!` on failure. Use `tracing::instrument`.

---

## Performance Budget

| Metric | Budget | Measurement |
|---|---|---|
| Installer (NSIS) | ≤ 10 MB | `tauri build` artifact size |
| Cold start to first frame | ≤ 500 ms | wall clock on a 2024-era laptop |
| Rust shell idle RAM (private memory) | ≤ 20 MB | `Get-Process` after 5 min idle |
| **Total** idle RAM (private memory, incl. WebView2 children) | ≤ 100 MB | as above |
| Time to "Launch" button responsive | ≤ 1 s from app open | manual + perf trace |
| Injection latency (process found → DLL loaded) | ≤ 200 ms | tracing span timing |

> Hitting the 100 MB total requires running WebView2 in a tightened mode. We pass these args to the webview in `tauri.conf.json` → `app.windows[].additionalBrowserArgs`:
>
> `--single-process --disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,SpareRendererForSitePerProcess,RendererCodeIntegrity --disable-extensions --disable-plugins --disable-pinch --no-first-run --js-flags=--max-old-space-size=96`
>
> Without these flags WebView2 spawns ~6 child processes and consumes ~180 MB private memory; with them it runs as 2 processes at ~90 MB. Single-process mode is safe here because the launcher is a fully trusted single-origin app — there is no third-party content sandbox to preserve.

Add a local script `pnpm size:check` that runs after `tauri build` and **errors out if the installer exceeds 12 MB** (10 MB target + 20% headroom). Run it manually before declaring a feature done. (Already implemented — see `scripts/check-installer-size.mjs`.)

---

## Non-Goals

- **Cross-platform.** Windows only. Don't add macOS/Linux build targets. Don't write `cfg(unix)` branches "for the future."
- **Plugin system.** Out of scope for v1.
- **In-app login / accounts.** Use the existing telemetry-only model.
- **Custom theming UI.** Light/dark only, follow OS preference.

---

## Definition of Done (per feature)

A backend feature is **done** when:

1. The Rust implementation matches the row in `BEHAVIOR_SPEC.md`.
2. Every test layer applicable to it passes (see `TESTING.md`):
   - Unit tests for pure logic
   - Integration tests for Win32/WinRT
   - Parity test against the .NET reference (where one exists)
3. `cargo clippy --all-targets -- -D warnings` is clean.
4. `pnpm typecheck` is clean.
5. The change keeps the installer under budget.

A frontend feature is **done** when:

1. It uses only allowed deps (see Stack table).
2. It passes Playwright smoke for the happy path.
3. It renders correctly in light + dark.
4. It has no `console.log` / no `any` / no `@ts-expect-error` without a comment citing the bug it works around.

---

## How to Work With This Codebase as an Agent

1. **Read `BEHAVIOR_SPEC.md` first.** Don't infer behavior from the .NET source unless the spec is silent — and if it is, update the spec first, *then* implement.
2. **Pick the smallest unit of work that crosses the IPC boundary** (one `#[tauri::command]` + its UI consumer) and ship that end-to-end before moving on.
3. **Write the parity test before the implementation** when one is feasible (`TESTING.md` shows how).
4. **Never skip the canary-DLL injection test.** It's the one test that proves the most dangerous code path actually works without launching the real game.
5. When the .NET reference and the spec disagree, **trust the .NET reference** (it's running in production) and patch the spec.
