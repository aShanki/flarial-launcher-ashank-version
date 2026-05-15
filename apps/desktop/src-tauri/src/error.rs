use serde::Serialize;

#[derive(Debug, thiserror::Error, Serialize)]
#[serde(tag = "kind", content = "detail")]
pub enum AppError {
    #[error("Minecraft is not installed")]
    NotInstalled,
    #[error("Gaming Services package is missing")]
    GamingServicesMissing,
    #[error("Injection failed: {0}")]
    InjectionFailed(String),
    #[error("Launch failed: {0}")]
    LaunchFailed(String),
    #[error("Network error: {0}")]
    Network(String),
    #[error("IO error: {0}")]
    Io(String),
    #[error("Win32 error: {0}")]
    Win32(String),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Other: {0}")]
    Other(String),
}

impl From<anyhow::Error> for AppError {
    fn from(value: anyhow::Error) -> Self {
        AppError::Other(value.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        AppError::Io(value.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(value: reqwest::Error) -> Self {
        AppError::Network(value.to_string())
    }
}

#[cfg(target_os = "windows")]
impl From<windows::core::Error> for AppError {
    fn from(value: windows::core::Error) -> Self {
        AppError::Win32(value.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
