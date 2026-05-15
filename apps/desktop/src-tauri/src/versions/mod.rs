use crate::error::{AppError, AppResult};
use crate::http;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub(crate) const SUPPORTED_URL: &str = "https://cdn.flarial.xyz/launcher/Supported.json";
const GDK_URLS: &str = "https://cdn.jsdelivr.net/gh/MinecraftBedrockArchiver/GdkLinks@latest/urls.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupportedVersion {
    pub version: String,
    pub supported: bool,
}

pub async fn list_supported() -> AppResult<Vec<SupportedVersion>> {
    let map: BTreeMap<String, bool> = http::client()
        .get(SUPPORTED_URL)
        .send()
        .await?
        .json()
        .await?;
    let mut out: Vec<SupportedVersion> = map
        .into_iter()
        .map(|(version, supported)| SupportedVersion { version, supported })
        .collect();
    out.sort_by(|a, b| numeric_cmp(&b.version, &a.version));
    Ok(out)
}

pub async fn install(version: &str) -> AppResult<()> {
    #[derive(Deserialize)]
    struct Urls {
        release: BTreeMap<String, Vec<String>>,
    }

    let urls: Urls = http::client().get(GDK_URLS).send().await?.json().await?;
    let candidates: Vec<String> = urls
        .release
        .into_iter()
        .filter(|(k, _)| k.starts_with(&format!("{version}.")) || k == version)
        .flat_map(|(_, v)| v.into_iter())
        .collect();

    if candidates.is_empty() {
        return Err(AppError::InvalidInput(format!(
            "no GDK download URLs available for {version}"
        )));
    }

    let client = http::client();
    let mut last_err: Option<String> = None;
    let mut chosen: Option<String> = None;
    for uri in &candidates {
        match client.head(uri).send().await {
            Ok(r) if r.status().is_success() => {
                chosen = Some(uri.clone());
                break;
            }
            Ok(r) => last_err = Some(format!("{uri}: HTTP {}", r.status())),
            Err(e) => last_err = Some(format!("{uri}: {e}")),
        }
    }
    let uri = chosen.ok_or_else(|| {
        AppError::Network(last_err.unwrap_or_else(|| "no GDK URI reachable".into()))
    })?;

    #[cfg(target_os = "windows")]
    {
        crate::packages::install_msix(&uri).await?;
        return Ok(());
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = uri;
        Err(AppError::Other("install only on Windows".into()))
    }
}

fn numeric_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |s: &str| -> Vec<u32> {
        s.split('.').filter_map(|p| p.parse().ok()).collect()
    };
    parse(a).cmp(&parse(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_descending() {
        let mut v = vec!["1.20.0", "1.21.0", "1.20.99"];
        v.sort_by(|a, b| numeric_cmp(b, a));
        assert_eq!(v, vec!["1.21.0", "1.20.99", "1.20.0"]);
    }
}
