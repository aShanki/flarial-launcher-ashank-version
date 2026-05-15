import { invoke } from "@tauri-apps/api/core";

export type AppErrorKind =
  | "NotInstalled"
  | "GamingServicesMissing"
  | "InjectionFailed"
  | "LaunchFailed"
  | "Network"
  | "Io"
  | "Win32"
  | "InvalidInput"
  | "Other";

export type AppError = { kind: AppErrorKind; detail?: string };

export type MinecraftPackage = {
  installed: boolean;
  packaged: boolean;
  version: string | null;
  installed_path: string | null;
  gaming_services_installed: boolean;
};

export type SupportedVersion = { version: string; supported: boolean };

export type Promotion = { Image: string; Uri: string };

export type AppSettings = {
  AutomaticUpdates: boolean;
  UseCustomDll: boolean;
  CustomDllPath: string;
};

/**
 * Hand-written wrappers around the Rust commands. Stay in sync with
 * apps/desktop/src-tauri/src/commands/mod.rs. Switch to tauri-specta
 * code generation once the surface stabilizes.
 */
export const commands = {
  getMinecraftPackage: () => invoke<MinecraftPackage>("get_minecraft_package"),
  launchMinecraft: () => invoke<number>("launch_minecraft"),
  listVersions: () => invoke<SupportedVersion[]>("list_versions"),
  installVersion: (version: string) =>
    invoke<void>("install_version", { version }),
  listPromotions: () => invoke<Promotion[]>("list_promotions"),
  checkLauncherUpdate: () => invoke<string | null>("check_launcher_update"),
  loadSettings: () => invoke<AppSettings>("load_settings"),
  saveSettings: (settings: AppSettings) =>
    invoke<void>("save_settings", { settings }),
};
