use crate::error::AppResult;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const SETTINGS_FILENAME: &str = "Flarial.Launcher.xml";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AppSettings {
    pub automatic_updates: bool,
    pub use_custom_dll: bool,
    pub custom_dll_path: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            automatic_updates: true,
            use_custom_dll: false,
            custom_dll_path: String::new(),
        }
    }
}

fn settings_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(SETTINGS_FILENAME)))
        .unwrap_or_else(|| PathBuf::from(SETTINGS_FILENAME))
}

pub async fn load() -> AppResult<AppSettings> {
    let path = settings_path();
    let bytes = match tokio::fs::read(&path).await {
        Ok(b) => b,
        Err(_) => return Ok(AppSettings::default()),
    };
    let text = String::from_utf8_lossy(&bytes);
    let mut settings: AppSettings =
        quick_xml::de::from_str(&text).unwrap_or_default();
    settings.custom_dll_path = normalize_path(&settings.custom_dll_path);
    Ok(settings)
}

pub async fn save(settings: &AppSettings) -> AppResult<()> {
    let xml = quick_xml::se::to_string_with_root("AppSettings", settings)
        .map_err(|e| crate::AppError::Other(e.to_string()))?;
    let body = format!(r#"<?xml version="1.0"?>{xml}"#);
    tokio::fs::write(settings_path(), body).await?;
    Ok(())
}

fn normalize_path(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    std::path::Path::new(trimmed)
        .canonicalize()
        .ok()
        .and_then(|p| p.to_str().map(String::from))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec() {
        let s = AppSettings::default();
        assert!(s.automatic_updates);
        assert!(!s.use_custom_dll);
        assert!(s.custom_dll_path.is_empty());
    }

    #[test]
    fn normalize_drops_invalid_path() {
        assert_eq!(normalize_path("  \\\\?\\not\\a\\real\\thing\\ever\\xyzzy"), "");
        assert_eq!(normalize_path(""), "");
        assert_eq!(normalize_path("   "), "");
    }

    #[test]
    fn round_trip_serializes_pascal_case() {
        let s = AppSettings {
            automatic_updates: false,
            use_custom_dll: true,
            custom_dll_path: "C:\\Windows".into(),
        };
        let xml = quick_xml::se::to_string_with_root("AppSettings", &s).unwrap();
        assert!(xml.contains("<AutomaticUpdates>false"), "xml was: {xml}");
        assert!(xml.contains("<UseCustomDll>true"), "xml was: {xml}");
    }
}
