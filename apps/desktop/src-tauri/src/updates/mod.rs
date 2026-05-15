use crate::error::AppResult;
use crate::http;
use serde::Deserialize;

const VERSION_URL: &str = "https://cdn.flarial.xyz/launcher/launcherVersion.txt";

#[derive(Debug, Deserialize)]
struct VersionResponse {
    version: String,
}

pub async fn check_launcher() -> AppResult<Option<String>> {
    let local = env!("CARGO_PKG_VERSION");
    let resp: VersionResponse = http::client()
        .get(VERSION_URL)
        .send()
        .await?
        .json()
        .await?;
    if resp.version == local {
        Ok(None)
    } else {
        Ok(Some(resp.version))
    }
}
