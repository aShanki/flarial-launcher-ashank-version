use crate::error::{AppError, AppResult};
use std::path::Path;

#[cfg(target_os = "windows")]
pub fn inject(pid: u32, dll_path: &Path) -> AppResult<u32> {
    use windows::core::{s, PCWSTR};
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
    use windows::Win32::System::Memory::{
        VirtualAllocEx, VirtualFreeEx, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE,
    };
    use windows::Win32::System::Threading::{
        CreateRemoteThread, OpenProcess, WaitForSingleObject, INFINITE,
        LPTHREAD_START_ROUTINE, PROCESS_CREATE_THREAD, PROCESS_VM_OPERATION,
        PROCESS_VM_READ, PROCESS_VM_WRITE, PROCESS_QUERY_INFORMATION,
    };
    use windows::Win32::System::Diagnostics::Debug::WriteProcessMemory;

    let wide: Vec<u16> = dll_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let bytes = wide.len() * std::mem::size_of::<u16>();

    unsafe {
        let kernel32 = GetModuleHandleA(s!("Kernel32"))
            .map_err(|e| AppError::InjectionFailed(format!("GetModuleHandle Kernel32: {e}")))?;
        let load_library = GetProcAddress(kernel32, s!("LoadLibraryW"))
            .ok_or_else(|| AppError::InjectionFailed("LoadLibraryW not found".into()))?;
        let routine: LPTHREAD_START_ROUTINE =
            Some(std::mem::transmute::<_, unsafe extern "system" fn(*mut std::ffi::c_void) -> u32>(load_library));

        let access = PROCESS_CREATE_THREAD
            | PROCESS_VM_OPERATION
            | PROCESS_VM_WRITE
            | PROCESS_VM_READ
            | PROCESS_QUERY_INFORMATION;
        let handle: HANDLE = OpenProcess(access, false, pid)
            .map_err(|e| AppError::InjectionFailed(format!("OpenProcess({pid}): {e}")))?;

        let buffer = VirtualAllocEx(
            handle,
            None,
            bytes,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        );
        if buffer.is_null() {
            let _ = CloseHandle(handle);
            return Err(AppError::InjectionFailed("VirtualAllocEx returned null".into()));
        }

        let mut written: usize = 0;
        let write_ok = WriteProcessMemory(
            handle,
            buffer,
            wide.as_ptr() as *const _,
            bytes,
            Some(&mut written),
        );
        if write_ok.is_err() || written != bytes {
            let _ = VirtualFreeEx(handle, buffer, 0, MEM_RELEASE);
            let _ = CloseHandle(handle);
            return Err(AppError::InjectionFailed("WriteProcessMemory failed".into()));
        }

        let thread = CreateRemoteThread(
            handle,
            None,
            0,
            routine,
            Some(buffer),
            0,
            None,
        );
        let thread = match thread {
            Ok(h) => h,
            Err(e) => {
                let _ = VirtualFreeEx(handle, buffer, 0, MEM_RELEASE);
                let _ = CloseHandle(handle);
                return Err(AppError::InjectionFailed(format!("CreateRemoteThread: {e}")));
            }
        };

        WaitForSingleObject(thread, INFINITE);
        let _ = CloseHandle(thread);
        let _ = VirtualFreeEx(handle, buffer, 0, MEM_RELEASE);
        let _ = CloseHandle(handle);

        let _ = PCWSTR::null();
        Ok(pid)
    }
}

#[cfg(not(target_os = "windows"))]
pub fn inject(_pid: u32, _dll_path: &Path) -> AppResult<u32> {
    Err(AppError::InjectionFailed("non-Windows target".into()))
}

#[cfg(target_os = "windows")]
use std::os::windows::ffi::OsStrExt;
