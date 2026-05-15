use crate::error::AppResult;
use crate::packages::MinecraftPackageInfo;
use crate::promotions::Promotion;
use crate::settings::AppSettings;
use crate::versions::SupportedVersion;

#[tauri::command]
pub async fn get_minecraft_package() -> AppResult<MinecraftPackageInfo> {
    crate::packages::query_minecraft().await
}

#[tauri::command]
pub async fn launch_minecraft() -> AppResult<u32> {
    use crate::error::AppError;
    use crate::minecraft::{client, inject, launch, library};

    let settings = crate::settings::load().await?;
    let dll_path = if settings.use_custom_dll && !settings.custom_dll_path.is_empty() {
        std::path::PathBuf::from(&settings.custom_dll_path)
    } else {
        client::ensure_up_to_date().await?
    };

    if !library::is_loadable(&dll_path) {
        return Err(AppError::InvalidInput(format!(
            "DLL is not a valid Windows DLL: {}",
            dll_path.display()
        )));
    }

    let pid = launch::launch().await?;
    wait_for_window(pid).await;
    inject::inject(pid, &dll_path)?;

    crate::telemetry::fire_launch_event(install_id());
    Ok(pid)
}

async fn wait_for_window(pid: u32) {
    #[cfg(target_os = "windows")]
    {
        use crate::minecraft::discovery;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while std::time::Instant::now() < deadline {
            let ready = {
                match discovery::find_minecraft_window() {
                    Ok(Some((hwnd, found_pid))) => {
                        found_pid == pid && discovery::window_is_visible(hwnd)
                    }
                    _ => false,
                }
            };
            if ready {
                tokio::time::sleep(std::time::Duration::from_millis(400)).await;
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }
    #[cfg(not(target_os = "windows"))]
    let _ = pid;
}

fn install_id() -> String {
    #[cfg(target_os = "windows")]
    {
        use windows::System::Profile::SystemIdentification;
        if let Ok(info) = SystemIdentification::GetSystemIdForPublisher() {
            if let Ok(buf) = info.Id() {
                if let Ok(reader) = windows::Storage::Streams::DataReader::FromBuffer(&buf) {
                    let len = buf.Length().unwrap_or(0) as usize;
                    let mut bytes = vec![0u8; len];
                    if reader.ReadBytes(&mut bytes).is_ok() {
                        return bytes.iter().map(|b| format!("{b:02X}")).collect();
                    }
                }
            }
        }
    }
    String::new()
}

#[tauri::command]
pub async fn list_versions() -> AppResult<Vec<SupportedVersion>> {
    crate::versions::list_supported().await
}

#[tauri::command]
pub async fn install_version(version: String) -> AppResult<()> {
    crate::versions::install(&version).await
}

#[tauri::command]
pub async fn list_promotions() -> AppResult<Vec<Promotion>> {
    crate::promotions::list().await
}

#[tauri::command]
pub async fn check_launcher_update() -> AppResult<Option<String>> {
    crate::updates::check_launcher().await
}

#[tauri::command]
pub async fn load_settings() -> AppResult<AppSettings> {
    crate::settings::load().await
}

#[tauri::command]
pub async fn save_settings(settings: AppSettings) -> AppResult<()> {
    crate::settings::save(&settings).await
}

#[tauri::command]
pub async fn attach_ad_webview(app: tauri::AppHandle) -> Result<(), String> {
    let (tx, rx) = tokio::sync::oneshot::channel::<Result<(), String>>();
    let app_for_main = app.clone();
    app.run_on_main_thread(move || {
        let result = crate::attach_ad_webview_impl(&app_for_main).map_err(|e| e.to_string());
        let _ = tx.send(result);
    })
    .map_err(|e| e.to_string())?;
    rx.await.map_err(|e| e.to_string())?
}
