use crate::error::{AppError, AppResult};

pub async fn launch() -> AppResult<u32> {
    let info = crate::packages::query_minecraft().await?;
    if !info.installed {
        return Err(AppError::NotInstalled);
    }
    if !info.gaming_services_installed {
        return Err(AppError::GamingServicesMissing);
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(Some((hwnd, pid))) = crate::minecraft::discovery::find_minecraft_window() {
            if crate::minecraft::discovery::window_is_visible(hwnd) {
                crate::minecraft::discovery::switch_to_window(hwnd);
                return Ok(pid);
            }
        }
        let installed_path = info
            .installed_path
            .ok_or_else(|| AppError::LaunchFailed("no installed path".into()))?;
        activate_via_powershell(&installed_path).await?;
        for _ in 0..100 {
            if let Some(pid) = crate::minecraft::discovery::find_minecraft_pid()? {
                return Ok(pid);
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        Err(AppError::LaunchFailed("Minecraft did not appear".into()))
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err(AppError::LaunchFailed("non-Windows target".into()))
    }
}

#[cfg(target_os = "windows")]
async fn activate_via_powershell(installed_path: &str) -> AppResult<()> {
    let exe = std::path::Path::new(installed_path).join("Minecraft.Windows.exe");
    let script = format!(
        "Invoke-CommandInDesktopPackage \
           -AppId 'Game' \
           -Command '{}' \
           -PackageFamilyName '{}'",
        exe.display(),
        crate::packages::MINECRAFT_FAMILY,
    );
    let status = tokio::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .status()
        .await?;
    if !status.success() {
        return Err(AppError::LaunchFailed(format!(
            "powershell exited with {status}"
        )));
    }
    Ok(())
}
