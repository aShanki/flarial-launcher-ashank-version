use crate::http;
use serde::Serialize;

const ENDPOINT: &str = "https://api.flarial.xyz/launcher/events/launch";

#[derive(Serialize)]
struct LaunchEvent {
    timestamp: String,
    #[serde(rename = "installId")]
    install_id: String,
}

pub fn fire_launch_event(install_id: String) {
    tokio::spawn(async move {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis().to_string())
            .unwrap_or_default();
        let nonce = uuid::Uuid::new_v4().to_string();
        let body = LaunchEvent {
            timestamp: now.clone(),
            install_id: install_id.clone(),
        };
        let _ = http::client()
            .post(ENDPOINT)
            .header("User-Agent", "Samsung Smart Fridge")
            .header("X-Flarial.Timestamp", now)
            .header("X-Flarial-InstallId", install_id)
            .header("X-Flarial-Nonce", nonce)
            .json(&body)
            .send()
            .await;
    });
}
