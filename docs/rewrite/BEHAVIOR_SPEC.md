# Flarial Launcher — Backend Behavioral Contract

This document is the **binding contract** for the Tauri/Rust rewrite of the Flarial Launcher. Every behavior listed here must be preserved by the Rust backend. The UI may change freely; the side effects on the user's machine must not.

Source of truth: the .NET implementation at `src/Flarial.Runtime/**` and `src/Flarial.Launcher/Management/**` on `main` (commit `438c3b9` or later).

---

## 1. Package Discovery

### 1.1 Minecraft package
- **API:** `Windows.Management.Deployment.PackageManager.FindPackagesForUser(string.Empty, "Microsoft.MinecraftUWP_8wekyb3d8bbwe")`
- **Output:**
  - `Minecraft.IsInstalled : bool` — package found
  - `Minecraft.IsPackaged  : bool` — `Package.SignatureKind == Store`
  - `Minecraft.Version     : string` — derived from `Package.Id.Version` as `"{major}.{minor}.{build/100}"`
- **Errors:** never throws; returns `null`/`false` on failure
- **Rust:** `windows::Management::Deployment::PackageManager`

### 1.2 Gaming Services package
- **API:** same `FindPackagesForUser` with `"Microsoft.GamingServices_8wekyb3d8bbwe"`
- **Output:** `Minecraft.IsGamingServicesInstalled : bool`
- If missing: launch must fail with a recognizable error so UI can show `GamingServicesMissingDialog`

---

## 2. Process & Window Enumeration

### 2.1 Find Minecraft process
- Enumerate via `WTSEnumerateProcessesEx(WTS_CURRENT_SERVER_HANDLE, WTSTypeProcessInfoLevel0, WTS_CURRENT_SESSION)`
- Filter by name `"Minecraft.Windows.exe"` (ordinal, case-insensitive)
- Open survivors with `PROCESS_QUERY_LIMITED_INFORMATION`
- Verify ownership via `GetPackageFamilyName(handle, ...)` matches `Microsoft.MinecraftUWP_8wekyb3d8bbwe`
- Free with `WTSFreeMemoryEx`
- **Returns:** `Option<u32>` (PID)

### 2.2 Find Bedrock window
- `FindWindowExW(NULL, prev, "Bedrock", NULL)` in a loop
- For each hit: `GetWindowThreadProcessId` → open process → verify package family
- **Returns:** `Option<NativeWindow { hwnd, pid }>`
- **Derived:** `is_visible()` via `IsWindowVisible`, `switch()` via `SwitchToThisWindow(hwnd, true)`

---

## 3. Game Launch (`MinecraftGDK.LaunchAsync`)

Preconditions (in order, fail fast):
1. `IsInstalled` else `ERROR_INSTALL_PACKAGE_NOT_FOUND`
2. `IsGamingServicesInstalled` else `ERROR_INSTALL_PREREQUISITE_FAILED`

### 3.1 Window-reuse short circuit
If a Bedrock window already exists and is visible, call `switch()` and return its PID. **Do not spawn another instance.**

### 3.2 Activation
Spawn PowerShell, load `Appx` module, run:
```powershell
Invoke-CommandInDesktopPackage `
  -AppId "Game" `
  -Command "<Minecraft.Package.InstalledPath>\Minecraft.Windows.exe" `
  -PackageFamilyName "Microsoft.MinecraftUWP_8wekyb3d8bbwe"
```
Runspace settings: `ExecutionPolicy=Bypass`, `ThreadOptions=UseCurrentThread`. Poll `find_minecraft_pid()` until non-null, then return PID.

> No native Win32 substitute for this exists — `Invoke-CommandInDesktopPackage` is required to set up the AppContainer correctly. The Rust port must shell out to `powershell.exe`. (Investigate `IDesktopAppXActivator` COM interface as a possible native replacement; if it works, prefer it.)

### 3.3 Synchronization

**Store (`IsPackaged = true`):**
- Open process with `PROCESS_SYNCHRONIZE`
- Create manual-reset event
- Watch `%AppData%\Minecraft Bedrock\Users\` for **deletion** of any file matching `*menu_load_lock`
- On delete: `SetEvent`
- `WaitForMultipleObjects([event, processHandle], false, INFINITE)`
  - Event first → return PID
  - Process exited first → return `None`

**Sideloaded (`IsPackaged = false`):**
- Poll: `process.wait(1ms)` until `find_window()` returns `Some`
- Poll: `process.wait(1ms)` until `window.is_visible()` is true
- Return PID

---

## 4. DLL Injection (`Injector.Launch`)

### 4.1 DLL validation (`Library.IsLoadable`)
- `LoadLibraryExW(path, NULL, DONT_RESOLVE_DLL_REFERENCES)`
- Read `IMAGE_DOS_HEADER` at module base
- Read `IMAGE_NT_HEADERS64` at `base + dos.e_lfanew`
- Return `(nt.FileHeader.Characteristics & IMAGE_FILE_DLL) != 0`
- `FreeLibrary`
- **Rust:** `windows::Win32::System::LibraryLoader::*` + manual PE parse (or `goblin`)

### 4.2 Remote-thread injection
1. `OpenProcess(PROCESS_ALL_ACCESS, false, pid)` — actually only needs `CREATE_THREAD | VM_OPERATION | VM_WRITE | VM_READ`; reduce surface
2. `VirtualAllocEx(handle, NULL, (path.len()+1)*2, MEM_COMMIT|MEM_RESERVE, PAGE_READWRITE)` for UTF-16 path
3. `WriteProcessMemory(handle, buffer, path_utf16, len, NULL)`
4. Resolve `LoadLibraryW` once at startup: `GetProcAddress(GetModuleHandleW("kernel32"), "LoadLibraryW")`
5. `CreateRemoteThread(handle, NULL, 0, LoadLibraryW, buffer, 0, NULL)`
6. `WaitForSingleObject(thread, INFINITE)`
7. Cleanup: `CloseHandle(thread)`, `VirtualFreeEx(handle, buffer, 0, MEM_RELEASE)`, `CloseHandle(handle)`
- Failure at any step: return `None` (silent)
- **Returns:** `Option<u32>` (the same PID, signaling success)

> The DLL is loaded into Minecraft's process and immediately begins executing its `DllMain`. From that point the launcher's job for this session is done.

---

## 5. Client DLL Update Flow

- Local hash: SHA-256 of `Flarial.Client.Release.dll` next to launcher (empty if missing). Use a `Mutex` to prevent concurrent reads.
- Remote hashes: `GET https://cdn.flarial.xyz/dll_hashes.json` → `{"Release": "<hex>", "Debug": "<hex>"}`. Compare uppercase, case-insensitive.
- If equal → no-op, return `true`.
- If different → delete local file (returns `false` if file is locked, e.g. running game holds it). Then `GET https://cdn.flarial.xyz/dll/latest.dll`, stream to disk with `(downloaded/total)*100` progress callback.

---

## 6. Version Management

### 6.1 Supported versions
- `GET https://cdn.flarial.xyz/launcher/Supported.json` → `{"1.20.0": true, "1.21.0": true, ...}`
- Build a `BTreeMap<NumericVersion, VersionEntry>` keyed by version, sorted **descending** (latest first)
- `PreferredVersion` = first supported entry

### 6.2 GDK version metadata
- `GET https://cdn.jsdelivr.net/gh/MinecraftBedrockArchiver/GdkLinks@latest/urls.json` → `{"release": {"1.20.0.12345": ["url1", "url2", ...]}}`
- `GET https://cdn.flarial.xyz/launcher/gamelaunchhelper.dll` (binary, kept in memory)
- For each key in `release`, strip the trailing `.<build>` and look up `1.20.0` in the registry; attach a `GDKVersionItem(version, uris, gameLaunchHelper)`.

### 6.3 Lazy URI resolution
- On install: race all URIs in the array via concurrent `HEAD` (or small `GET`) requests; first 2xx wins; cancel the rest.
- All fail → throw `InvalidOperationException` equivalent (Rust: return an `Err` enum).

### 6.4 Install MSIX
- `Windows.Management.Deployment.PackageManager.AddPackageAsync(uri, null, ForceApplicationShutdown | ForceUpdateFromAnyVersion)`
- Wire up `Progress += pct → callback`, `Completed += SetEvent`
- Block on the event
- On error: surface `operation.ErrorCode`
- After success: write `gamelaunchhelper.dll` (from §6.2) into `Minecraft.Package.InstalledPath`

---

## 7. Launcher Self-Update

### 7.1 Connectivity
`GET https://cdn.flarial.xyz/202.txt` → 2xx means online.

### 7.2 Version check
`GET https://cdn.flarial.xyz/launcher/launcherVersion.txt` → `{"version":"1.2.3"}`. If `remote != local` (assembly version), update available.

### 7.3 Replace + restart
1. Download `https://cdn.flarial.xyz/launcher/Flarial.Launcher.exe` to `%TEMP%\<rand>.exe` with progress.
2. Write `%TEMP%\<rand>.cmd`:
   ```bat
   chcp 65001
   :_
   move /y "<downloaded>" "<current_exe>"
   if not %errorlevel%==0 goto _
   del "%~f0"
   ```
3. Spawn: `cmd.exe /e:on /f:off /v:off /d /c call "<script>" & "<cmd>" /c start "" "<current_exe>"` with `CreateNoWindow=true`.
4. `Environment.Exit(0)`.

---

## 8. Telemetry (`LaunchAnalytics`)

Only fires **after successful injection**. Fire-and-forget; never block launch; never retry.

- `POST https://api.flarial.xyz/launcher/events/launch`
- Headers:
  - `User-Agent: Samsung Smart Fridge`
  - `X-Flarial.Timestamp: <unix_millis>`
  - `X-Flarial-InstallId: <SystemIdentification.GetSystemIdForPublisher().Id>`
  - `X-Flarial-Nonce: <new GUID>`
- Body: `{"timestamp":"<millis>","installId":"<id>"}`

---

## 9. Settings Persistence

File: `Flarial.Launcher.xml` next to the executable. `DataContractSerializer` with indented output.

```xml
<AppSettings xmlns:i="http://www.w3.org/2001/XMLSchema-instance">
  <AutomaticUpdates>true</AutomaticUpdates>
  <UseCustomDll>false</UseCustomDll>
  <CustomDllPath></CustomDllPath>
</AppSettings>
```

Defaults: `AutomaticUpdates=true`, `UseCustomDll=false`, `CustomDllPath=""`. On load, normalize via `Path.GetFullPath(trim(CustomDllPath))`; on failure reset to `""`. On parse failure of the whole file, return defaults.

> The Rust port may switch to `flarial-launcher.toml` or JSON, but only after writing a one-time migration that reads the legacy XML if present.

---

## 10. Network Endpoints (full list)

| Method | URL | Purpose |
|---|---|---|
| GET | `https://cdn.flarial.xyz/202.txt` | connectivity check |
| GET | `https://cdn.flarial.xyz/launcher/launcherVersion.txt` | launcher version |
| GET | `https://cdn.flarial.xyz/launcher/Flarial.Launcher.exe` | download new launcher |
| GET | `https://cdn.flarial.xyz/launcher/Supported.json` | supported MC versions |
| GET | `https://cdn.flarial.xyz/launcher/Promotions.json` | promo carousel |
| GET | `https://cdn.flarial.xyz/dll_hashes.json` | client DLL hashes |
| GET | `https://cdn.flarial.xyz/dll/latest.dll` | client DLL binary |
| GET | `https://cdn.flarial.xyz/launcher/gamelaunchhelper.dll` | GDK helper |
| GET | `https://cdn.jsdelivr.net/gh/MinecraftBedrockArchiver/GdkLinks@latest/urls.json` | GDK MSIX URL map |
| POST | `https://api.flarial.xyz/launcher/events/launch` | launch telemetry |

HTTP client: enable gzip/deflate, follow redirects, ~30s timeout. UA `Samsung Smart Fridge` only on the telemetry endpoint; everything else uses default.

---

## 11. Hard Invariants

1. **Never inject into a process that is not the right Minecraft package.** Always verify package family name *after* opening the handle.
2. **Crash-tolerant.** If injection fails, the launcher must continue running and surface a dialog, not panic.
3. **AppContainer-aware.** Treat Minecraft as sandboxed; do not assume normal file/registry paths inside the game.
4. **PowerShell activation is required for Store version launch.** Native `CreateProcessAsUser` will not produce the same activation.
5. **`menu_load_lock` watcher is the canonical "game ready" signal for Store builds.** Polling the window is the fallback for sideloaded builds only.
6. **`LoadLibraryW` is hardcoded as the remote thread routine.** DLL path is null-terminated UTF-16LE in remote memory.
7. **Silent failures with `Option::None`/`bool=false` are intentional.** The UI layer turns absence into the appropriate dialog.
8. **Version sort is `(major DESC, minor DESC, build DESC)`.** `1.21.0 > 1.20.99`.
9. **DLL hash compare is case-insensitive.**
10. **Telemetry never blocks gameplay.**
