# Context — AdSense side-banner work

## Project shape

Tauri 2 + React 19 + Tailwind 4 launcher. Sources of truth:
- `docs/rewrite/GOAL.md` — mission + stack + budgets
- `docs/rewrite/BEHAVIOR_SPEC.md` — backend contract ported from the .NET launcher
- `docs/rewrite/TESTING.md` — 5-layer local test runner
- `docs/rewrite/PERF_REPORT.md` — last measured perf numbers

Branch: `tauri-rewrite` on `fork` remote (`aShanki/flarial-launcher-ashank-version`). Upstream is `flarialmc/launcher` as `origin`.

## What works already

- Launcher boots, status badge, launch button injects the Flarial client DLL into Minecraft (verified end-to-end against real MC).
- React frontend: Home / Versions / Settings, directional page transitions via framer-motion, autosave on settings, Space Grotesk + Inter via `@fontsource/*`.
- Litebyte sponsor PNG overlay at top of hero, fetched live from `cdn.flarial.xyz/launcher/Promotions.json` (same endpoint the legacy launcher uses).
- Dev mode: `pnpm --filter desktop tauri dev` runs Vite + Tauri together with HMR on the React side. Rust changes trigger a full rebuild + restart.
- Frontend can be inspected by navigating Chrome to `http://localhost:1420` — `read_network_requests`, `read_console_messages`, `javascript_tool`, screenshots all work via the Chrome MCP. Use this instead of asking the user.

## The side-banner ad problem

Reference implementation is PR #74 on `flarialmc/launcher`, branch `feat/adsense-side-banners`, file `src/Flarial.Launcher/Controls/BannerAdsView.cs`. Uses a WPF `Microsoft.Web.WebView2.Wpf.WebView2` control inside the main launcher window.

- Loads `https://ecoursefree.com/CalcCourse.html` AS THE TOP URL of a fresh WebView2 control (not an iframe — that's the key). AdSense fills the slot only when the page is the top document, because the publisher's anti-fraud check rejects unauthorized parent origins.
- Pre-document script (`AddScriptToExecuteOnDocumentCreatedAsync`) tags one `ins.adsbygoogle` slot as `data-ad-format="vertical"` 160×600 and hides the others, BEFORE AdSense's own scripts run.
- Post-load script (`ExecuteScriptAsync` on `NavigationCompleted`) lifts that slot into a `.__flarial_ad` overlay and hides everything else.

I've already ported both scripts as `PRE_SCRIPT` and `POST_SCRIPT` constants in `apps/desktop/src-tauri/src/lib.rs`, and written `attach_ad_webview()` that calls `main.add_child(WebviewBuilder::new(...).initialization_script(PRE).on_page_load(POST), pos, size)`. `tauri::webview::WebviewBuilder` requires `tauri = { features = ["unstable"] }` — already set in `apps/desktop/src-tauri/Cargo.toml`.

The React side reserves a 232px gutter via `apps/desktop/src/components/AdRail.tsx` (empty placeholder); the native webview is meant to float on top of it.

## What's blocking it

`tauri_runtime_wry` throws **HRESULT 0x8007139F** ("the group or resource is not in the correct state") on `add_child` when called from inside `setup()` or shortly after — the main webview's WebView2 environment isn't ready to host a second view. Worse: `add_child` returns `Ok(())` to Rust anyway. The child registers in Tauri's manager but its WebView2 instance is half-initialized. Later it corrupts the WebView2 env and the launcher process exits without a panic message.

Attempts so far:
- Defer with `tokio::sleep(600ms)` then `run_on_main_thread` — same error, process eventually dies.
- Retry with exponential backoff (1.5s, 3s, 6s, up to 8 attempts) — first attempt returns Ok, never retries because the swallowed error doesn't surface.
- Hook main webview's first `PageLoadEvent::Finished` and attach 800ms after that — current state of `lib.rs`. Untested.

## What to try

**First: frontend-driven attach.** Add a `attach_ad_webview` Tauri command, call it from React via `useEffect(() => { invoke("attach_ad_webview"); }, [])` after the main UI has mounted. React's first paint guarantees the main WebView2 is fully alive — that should sidestep the init race entirely. This is the most likely fix and the smallest change.

**If that still hits 0x8007139F:** fall back to a top-level `WebviewWindowBuilder` window glued to the main window's edge via `WindowEvent::Moved/Resized`. The earlier attempt at this also hit 0x8007139F but in a different code path; combined with the post-mount trigger it should work.

**Last resort:** drop the AdSense rail entirely and ship with the Litebyte PNG promo only (already working, paid placement, no compliance risk).

## How to run + verify

```powershell
pnpm --filter desktop tauri dev
```

Then in Chrome via MCP, navigate to `http://localhost:1420`. The hero panel + Litebyte logo + launch button render normally. The right-side gutter should fill with a vertical AdSense ad (or a 160×600 skyscraper) once `attach_ad_webview` succeeds. Watch the dev terminal for `ad webview attached` (success) or `ad webview failed: ...` (Rust-side error). For WebView2 errors, watch for `failed to create webview: WebView2 error: ... 0x8007139F` from `tauri_runtime_wry` — that's the silent failure mode.

## Definition of done

Real Google ad visible in the right gutter, survives `Window::set_position` / `set_size`, and the launcher doesn't die. Litebyte promo at top of hero is untouched.
