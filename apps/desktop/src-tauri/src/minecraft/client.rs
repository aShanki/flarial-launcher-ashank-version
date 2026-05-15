//! Flarial client DLL: hash check + download.
//! BEHAVIOR_SPEC §5.

use crate::error::{AppError, AppResult};
use crate::http;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

const HASHES_URL: &str = "https://cdn.flarial.xyz/dll_hashes.json";
const DLL_URL: &str = "https://cdn.flarial.xyz/dll/latest.dll";
const FILENAME: &str = "Flarial.Client.Release.dll";

#[derive(Debug, Deserialize)]
struct Hashes {
    #[serde(rename = "Release")]
    release: String,
}

pub fn dll_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(FILENAME)))
        .unwrap_or_else(|| PathBuf::from(FILENAME))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let out = h.finalize();
    out.iter().map(|b| format!("{b:02X}")).collect()
}

pub async fn ensure_up_to_date() -> AppResult<PathBuf> {
    let path = dll_path();

    let local_hash = match tokio::fs::read(&path).await {
        Ok(b) => sha256_hex(&b),
        Err(_) => String::new(),
    };

    let remote: Hashes = http::client().get(HASHES_URL).send().await?.json().await?;
    if !local_hash.is_empty() && local_hash.eq_ignore_ascii_case(&remote.release) {
        return Ok(path);
    }

    if path.exists() {
        if let Err(e) = tokio::fs::remove_file(&path).await {
            return Err(AppError::Io(format!(
                "cannot replace existing client DLL (held by another process?): {e}"
            )));
        }
    }

    let bytes = http::client().get(DLL_URL).send().await?.bytes().await?;
    tokio::fs::write(&path, &bytes).await?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_known_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD"
        );
    }
}
