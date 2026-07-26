use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ed25519_dalek::{Signer, SigningKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

const BASE_URL: &str = "https://api.flarial.xyz";
const EVENTS_PATH: &str = "/api/v2/events";

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredIdentity {
    installation_id: String,
    secret_key: String,
}

struct Identity {
    installation_id: String,
    signing_key: SigningKey,
}

pub fn fire_launch_event(legacy_install_id: String) {
    tokio::spawn(async move {
        if let Err(error) = send_launch_event(legacy_install_id).await {
            tracing::debug!(%error, "API v2 launch telemetry failed");
        }
    });
}

async fn send_launch_event(legacy_install_id: String) -> anyhow::Result<()> {
    let identity = load_or_register().await?;
    let occurred_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis() as u64;
    let body = serde_json::json!({
        "batchId": random_token(),
        "events": [{
            "type": "launcher_launch",
            "occurredAtMs": occurred_at_ms,
            "properties": {
                "legacyInstallId": legacy_install_id,
                "version": env!("CARGO_PKG_VERSION")
            }
        }]
    })
    .to_string();
    let timestamp = (occurred_at_ms / 1000).to_string();
    let nonce = random_token();
    let digest = Sha256::digest(body.as_bytes());
    let canonical = format!(
        "POST\n{EVENTS_PATH}\n{timestamp}\n{nonce}\n{}",
        URL_SAFE_NO_PAD.encode(digest)
    );
    let signature = identity.signing_key.sign(canonical.as_bytes());
    let response = crate::http::client()
        .post(format!("{BASE_URL}{EVENTS_PATH}"))
        .header("User-Agent", "Flarial-Launcher/2.0")
        .header("X-Flarial-Installation", identity.installation_id)
        .header("X-Flarial-Timestamp", timestamp)
        .header("X-Flarial-Nonce", nonce)
        .header(
            "X-Flarial-Signature",
            URL_SAFE_NO_PAD.encode(signature.to_bytes()),
        )
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await?;
    anyhow::ensure!(
        response.status() == reqwest::StatusCode::ACCEPTED,
        "event endpoint returned {}",
        response.status()
    );
    Ok(())
}

async fn load_or_register() -> anyhow::Result<Identity> {
    let path = identity_path()?;
    if let Ok(saved) = tokio::fs::read(&path).await {
        if let Ok(stored) = serde_json::from_slice::<StoredIdentity>(&saved) {
            if let Ok(secret) = URL_SAFE_NO_PAD.decode(stored.secret_key) {
                if let Ok(bytes) = <[u8; 32]>::try_from(secret) {
                    return Ok(Identity {
                        installation_id: stored.installation_id,
                        signing_key: SigningKey::from_bytes(&bytes),
                    });
                }
            }
        }
    }

    let signing_key = SigningKey::from_bytes(&rand::random());
    let request = serde_json::json!({
        "publicKey": URL_SAFE_NO_PAD.encode(signing_key.verifying_key().to_bytes()),
        "platform": "launcher",
        "version": env!("CARGO_PKG_VERSION")
    });
    let response = crate::http::client()
        .post(format!("{BASE_URL}/api/v2/installations"))
        .header("User-Agent", "Flarial-Launcher/2.0")
        .json(&request)
        .send()
        .await?;
    anyhow::ensure!(
        response.status() == reqwest::StatusCode::CREATED,
        "installation endpoint returned {}",
        response.status()
    );
    let response_body: serde_json::Value = response.json().await?;
    let installation_id = response_body["installationId"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("installation response omitted installationId"))?
        .to_owned();
    let stored = StoredIdentity {
        installation_id: installation_id.clone(),
        secret_key: URL_SAFE_NO_PAD.encode(signing_key.to_bytes()),
    };
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let temporary = path.with_extension("json.tmp");
    tokio::fs::write(&temporary, serde_json::to_vec(&stored)?).await?;
    tokio::fs::rename(temporary, path).await?;
    Ok(Identity {
        installation_id,
        signing_key,
    })
}

fn identity_path() -> anyhow::Result<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA")
        .ok_or_else(|| anyhow::anyhow!("LOCALAPPDATA is unavailable"))?;
    Ok(PathBuf::from(local)
        .join("Flarial")
        .join("Launcher")
        .join("api-v2.json"))
}

fn random_token() -> String {
    URL_SAFE_NO_PAD.encode(rand::random::<[u8; 18]>())
}
