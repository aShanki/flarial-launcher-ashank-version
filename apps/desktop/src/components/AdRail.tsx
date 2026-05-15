/**
 * Reserves the right-edge gutter for the native child WebView2 that the Rust
 * side mounts onto the main window. The native webview is positioned by
 * `attach_ad_webview()` in `src-tauri/src/lib.rs` and floats above this
 * placeholder; we just need to keep main-content layout from spilling under it.
 */

const RAIL_W = 232;

export function AdRail() {
  return (
    <aside
      className="relative shrink-0"
      style={{ width: RAIL_W }}
      aria-hidden
    />
  );
}
