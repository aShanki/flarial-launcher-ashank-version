use std::path::Path;

#[cfg(target_os = "windows")]
pub fn is_loadable(path: &Path) -> bool {
    use windows::core::HSTRING;
    use windows::Win32::Foundation::{FreeLibrary, HMODULE};
    use windows::Win32::System::LibraryLoader::{
        LoadLibraryExW, DONT_RESOLVE_DLL_REFERENCES,
    };

    if !path.exists() {
        return false;
    }
    let wide = HSTRING::from(path.as_os_str());
    let module: HMODULE = match unsafe {
        LoadLibraryExW(&wide, None, DONT_RESOLVE_DLL_REFERENCES)
    } {
        Ok(m) => m,
        Err(_) => return false,
    };

    let result = unsafe { inspect_pe(module.0 as *const u8) };
    let _ = unsafe { FreeLibrary(module) };
    result
}

#[cfg(target_os = "windows")]
unsafe fn inspect_pe(base: *const u8) -> bool {
    const IMAGE_DOS_SIGNATURE: u16 = 0x5A4D;
    const IMAGE_NT_SIGNATURE: u32 = 0x00004550;
    const IMAGE_FILE_DLL: u16 = 0x2000;

    let dos_sig = *(base as *const u16);
    if dos_sig != IMAGE_DOS_SIGNATURE {
        return false;
    }
    let e_lfanew = *(base.add(0x3C) as *const i32) as isize;
    let nt = base.offset(e_lfanew);
    let nt_sig = *(nt as *const u32);
    if nt_sig != IMAGE_NT_SIGNATURE {
        return false;
    }
    let characteristics = *(nt.add(4 + 18) as *const u16);
    (characteristics & IMAGE_FILE_DLL) != 0
}

#[cfg(not(target_os = "windows"))]
pub fn is_loadable(_: &Path) -> bool {
    false
}

#[cfg(test)]
#[cfg(target_os = "windows")]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn user32_is_loadable() {
        let p = PathBuf::from(r"C:\Windows\System32\user32.dll");
        assert!(is_loadable(&p));
    }

    #[test]
    fn missing_file_is_not_loadable() {
        let p = PathBuf::from(r"C:\Windows\System32\definitely_not_a_real_file.dll");
        assert!(!is_loadable(&p));
    }

    #[test]
    fn exe_is_not_loadable_as_dll() {
        let p = PathBuf::from(r"C:\Windows\System32\notepad.exe");
        assert!(!is_loadable(&p));
    }
}
