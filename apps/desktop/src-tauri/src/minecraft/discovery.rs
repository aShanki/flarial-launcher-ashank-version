#[cfg(target_os = "windows")]
mod imp {
    use crate::error::AppResult;
    use crate::packages::MINECRAFT_FAMILY;
    use windows::core::PWSTR;
    use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND};
    use windows::Win32::Storage::Packaging::Appx::GetPackageFamilyName;
    use windows::Win32::System::RemoteDesktop::{
        WTSEnumerateProcessesExW, WTSFreeMemoryExW, WTSTypeProcessInfoLevel0,
        WTS_CURRENT_SERVER_HANDLE, WTS_PROCESS_INFOW,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowExW, GetWindowThreadProcessId, IsWindowVisible, SwitchToThisWindow,
    };

    pub fn process_package_family(handle: HANDLE) -> Option<String> {
        let mut len: u32 = 0;
        unsafe {
            let _ = GetPackageFamilyName(handle, &mut len, Some(PWSTR::null()));
        }
        if len == 0 {
            return None;
        }
        let mut buf = vec![0u16; len as usize];
        let rc = unsafe {
            GetPackageFamilyName(handle, &mut len, Some(PWSTR(buf.as_mut_ptr())))
        };
        if rc.is_err() {
            return None;
        }
        let nul = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Some(String::from_utf16_lossy(&buf[..nul]))
    }

    pub fn find_minecraft_pid() -> AppResult<Option<u32>> {
        const WTS_CURRENT_SESSION: u32 = u32::MAX;
        unsafe {
            let mut count: u32 = 0;
            let mut info_ptr: *mut WTS_PROCESS_INFOW = std::ptr::null_mut();
            let mut level: u32 = 0;
            WTSEnumerateProcessesExW(
                Some(WTS_CURRENT_SERVER_HANDLE),
                &mut level,
                WTS_CURRENT_SESSION,
                &mut info_ptr as *mut _ as *mut *mut u16 as *mut _,
                &mut count,
            )?;

            let slice = std::slice::from_raw_parts(info_ptr, count as usize);
            let mut found = None;
            for p in slice {
                let name = wide_to_string(p.pProcessName.0);
                if !name.eq_ignore_ascii_case(super::super::PROCESS_NAME) {
                    continue;
                }
                let Ok(handle) = OpenProcess(
                    PROCESS_QUERY_LIMITED_INFORMATION,
                    false,
                    p.ProcessId,
                ) else {
                    continue;
                };
                let family = process_package_family(handle);
                let _ = CloseHandle(handle);
                if matches!(family, Some(f) if f.eq_ignore_ascii_case(MINECRAFT_FAMILY)) {
                    found = Some(p.ProcessId);
                    break;
                }
            }
            let _ = WTSFreeMemoryExW(
                WTSTypeProcessInfoLevel0,
                info_ptr as *mut _,
                count,
            );
            Ok(found)
        }
    }

    pub fn find_minecraft_window() -> AppResult<Option<(HWND, u32)>> {
        unsafe {
            let class = windows::core::w!("Bedrock");
            let mut prev: Option<HWND> = None;
            loop {
                let hwnd = match FindWindowExW(None, prev, class, None) {
                    Ok(h) if !h.0.is_null() => h,
                    _ => return Ok(None),
                };
                let mut pid: u32 = 0;
                GetWindowThreadProcessId(hwnd, Some(&mut pid));
                if let Ok(handle) =
                    OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
                {
                    let family = process_package_family(handle);
                    let _ = CloseHandle(handle);
                    if matches!(family, Some(f) if f.eq_ignore_ascii_case(MINECRAFT_FAMILY)) {
                        return Ok(Some((hwnd, pid)));
                    }
                }
                prev = Some(hwnd);
            }
        }
    }

    pub fn window_is_visible(hwnd: HWND) -> bool {
        unsafe { IsWindowVisible(hwnd).as_bool() }
    }

    pub fn switch_to_window(hwnd: HWND) {
        unsafe { SwitchToThisWindow(hwnd, true) };
    }

    fn wide_to_string(p: *const u16) -> String {
        if p.is_null() {
            return String::new();
        }
        unsafe {
            let mut len = 0;
            while *p.add(len) != 0 {
                len += 1;
            }
            String::from_utf16_lossy(std::slice::from_raw_parts(p, len))
        }
    }
}

#[cfg(target_os = "windows")]
pub use imp::*;
