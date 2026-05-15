use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{
    webview::PageLoadEvent, LogicalSize, Manager, PhysicalPosition, Runtime, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder, WindowEvent,
};
use tracing_subscriber::EnvFilter;

pub mod commands;
pub mod error;
pub mod http;
pub mod minecraft;
pub mod packages;
pub mod promotions;
pub mod settings;
pub mod telemetry;
pub mod updates;
pub mod versions;

pub use error::AppError;

const AD_URL: &str = "https://ecoursefree.com/CalcCourse.html";
const AD_LABEL: &str = "ad";
const AD_W: f64 = 200.0;
const AD_INSET_RIGHT: f64 = 16.0;
const AD_INSET_TOP: f64 = 20.0;
const AD_INSET_BOTTOM: f64 = 20.0;
const AD_MIN_H: f64 = 240.0;

// MUST match tauri.conf.json's main window `additionalBrowserArgs`. WebView2
// rejects creating a second controller against the same env with different
// browser args, surfacing as HRESULT 0x8007139F ("wrong state") from
// `CreateCoreWebView2Controller`.
const AD_BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,SpareRendererForSitePerProcess,RendererCodeIntegrity --disable-extensions --disable-plugins --disable-pinch --no-first-run --js-flags=--max-old-space-size=96";

// Ported from PR #74 on flarialmc/launcher (feat/adsense-side-banners).
// Runs in the proxy page's context BEFORE its own scripts. Tags slot 0 as a
// vertical 160x600 skyscraper so AdSense fills it as a side-banner. Hides
// the article + the other two slots.
const PRE_SCRIPT: &str = r#"
(function () {
  var slotIndex = 0;
  try {
    var s = document.createElement('style');
    s.id = '__flarial_pre_style';
    s.textContent = "html, body { background: #120e0f !important; color: transparent !important; margin: 0 !important; padding: 0 !important; overflow: hidden !important; height: 100% !important; width: 100% !important; }";
    (document.head || document.documentElement).appendChild(s);
  } catch (_) {}

  document.addEventListener('DOMContentLoaded', function () {
    try {
      var nodes = document.querySelectorAll('ins.adsbygoogle');
      nodes.forEach(function (ins, i) {
        if (i === slotIndex) {
          ins.setAttribute('data-ad-format', 'vertical');
          ins.setAttribute('data-full-width-responsive', 'false');
          ins.style.display = 'inline-block';
          ins.style.width = '160px';
          ins.style.height = '600px';
        } else {
          ins.setAttribute('data-adsbygoogle-status', 'done');
          ins.style.display = 'none';
        }
      });
    } catch (_) {}
  });
})();
"#;

// Runs after the page (including AdSense) has loaded. Lifts the chosen slot
// out of the article DOM and overlays it full-viewport on a black backdrop.
const POST_SCRIPT: &str = r#"
(function () {
  var slotIndex = 0;
  var s = document.createElement('style');
  s.textContent = "body > *:not(.__flarial_ad) { display: none !important; } body .__flarial_ad { display: flex !important; align-items: center !important; justify-content: center !important; width: 100% !important; height: 100vh !important; padding: 0 !important; margin: 0 !important; background: #120e0f !important; position: fixed !important; inset: 0 !important; } body .__flarial_ad ins.adsbygoogle, body .__flarial_ad ins.adsbygoogle iframe { display: inline-block !important; width: 160px !important; height: 600px !important; max-width: 100% !important; max-height: 100% !important; }";
  document.head.appendChild(s);
  var nodes = document.querySelectorAll('ins.adsbygoogle');
  var ins = nodes[slotIndex] || nodes[0];
  if (!ins) return;
  var wrap = document.createElement('div');
  wrap.className = '__flarial_ad';
  wrap.appendChild(ins);
  document.body.appendChild(wrap);
})();
"#;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .try_init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_minecraft_package,
            commands::launch_minecraft,
            commands::list_versions,
            commands::install_version,
            commands::list_promotions,
            commands::check_launcher_update,
            commands::load_settings,
            commands::save_settings,
            commands::attach_ad_webview,
        ])
        .setup(|app| {
            // Sync the ad child webview's geometry whenever the main window
            // moves or resizes. The actual attach is driven from React (see
            // `commands::attach_ad_webview`) so the main WebView2 is fully
            // alive before we add a second child — avoids HRESULT 0x8007139F.
            let main = app.get_window("main").ok_or("main window missing")?;
            let handle_for_events = app.handle().clone();
            main.clone().on_window_event(move |event| {
                if matches!(event, WindowEvent::Resized(_) | WindowEvent::Moved(_)) {
                    sync_ad_position(&handle_for_events);
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

pub fn attach_ad_webview_impl<R: Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<(), Box<dyn std::error::Error>> {
    let main = app.get_webview_window("main").ok_or("main window missing")?;

    // Idempotent: bail if a previous invocation already created the ad window.
    if app.get_webview_window(AD_LABEL).is_some() {
        return Ok(());
    }

    let url: tauri::Url = AD_URL.parse()?;
    let (initial_pos, initial_size) = compute_ad_geometry(&main)?;

    let posted = Arc::new(AtomicBool::new(false));
    let posted_for_cb = posted.clone();

    let builder = WebviewWindowBuilder::new(app, AD_LABEL, WebviewUrl::External(url))
        .title("Flarial Ad")
        .inner_size(AD_W, initial_size.height)
        .decorations(false)
        .resizable(false)
        .skip_taskbar(true)
        .focused(false)
        .visible(false) // show after we've positioned it to avoid a flash
        .additional_browser_args(AD_BROWSER_ARGS)
        .initialization_script(PRE_SCRIPT)
        .on_page_load(move |webview, payload| {
            if matches!(payload.event(), PageLoadEvent::Finished)
                && !posted_for_cb.swap(true, Ordering::SeqCst)
            {
                let wv = webview.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
                    let _ = wv.eval(POST_SCRIPT);
                });
            }
        });

    // Parent the ad window to main on Windows so it stays z-ordered above main
    // without joining the alt-tab list and dies when main closes.
    #[cfg(target_os = "windows")]
    let builder = builder.owner(&main)?;

    let ad = builder.build()?;
    let _ = ad.set_size(initial_size);
    let _ = ad.set_position(initial_pos);

    // Win11 rounds borderless windows by default; force hard corners so the
    // ad sits flush with the launcher gutter instead of looking like a card.
    #[cfg(target_os = "windows")]
    disable_window_corner_rounding(&ad);

    let _ = ad.show();
    tracing::info!("ad webview created");
    Ok(())
}

fn compute_ad_geometry<R: Runtime>(
    main: &WebviewWindow<R>,
) -> Result<(PhysicalPosition<i32>, LogicalSize<f64>), Box<dyn std::error::Error>> {
    // `inner_position` is the top-left of the client area in physical pixels —
    // already accounts for window chrome on every platform, so we avoid the
    // outer-vs-inner heuristic that was misplacing the ad outside the window.
    let scale = main.scale_factor().unwrap_or(1.0);
    let inner_pos = main.inner_position()?;
    let inner_size = main.inner_size()?;

    let inner_h_logical = inner_size.height as f64 / scale;
    let ad_h = (inner_h_logical - AD_INSET_TOP - AD_INSET_BOTTOM).max(AD_MIN_H);

    let ad_w_phys = (AD_W * scale).round() as i32;
    let inset_r_phys = (AD_INSET_RIGHT * scale).round() as i32;
    let inset_t_phys = (AD_INSET_TOP * scale).round() as i32;

    let x = inner_pos.x + (inner_size.width as i32) - ad_w_phys - inset_r_phys;
    let y = inner_pos.y + inset_t_phys;

    Ok((PhysicalPosition::new(x, y), LogicalSize::new(AD_W, ad_h)))
}

fn sync_ad_position<R: Runtime>(app: &tauri::AppHandle<R>) {
    let Some(main) = app.get_webview_window("main") else { return };
    let Some(ad) = app.get_webview_window(AD_LABEL) else { return };
    let Ok((pos, size)) = compute_ad_geometry(&main) else { return };
    let _ = ad.set_size(size);
    let _ = ad.set_position(pos);
}

#[cfg(target_os = "windows")]
fn disable_window_corner_rounding<R: Runtime>(ad: &WebviewWindow<R>) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND,
    };
    let Ok(raw) = ad.hwnd() else { return };
    let hwnd = HWND(raw.0 as *mut _);
    let pref = DWMWCP_DONOTROUND;
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&pref as *const _) as *const _,
            std::mem::size_of_val(&pref) as u32,
        );
    }
}
