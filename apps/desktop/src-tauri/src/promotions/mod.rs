use crate::error::AppResult;
use crate::http;
use serde::{Deserialize, Serialize};

const PROMOTIONS_URL: &str = "https://cdn.flarial.xyz/launcher/Promotions.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Promotion {
    #[serde(rename = "Image")]
    pub image: String,
    #[serde(rename = "Uri")]
    pub uri: String,
}

pub async fn list() -> AppResult<Vec<Promotion>> {
    let promos: Vec<Promotion> = http::client()
        .get(PROMOTIONS_URL)
        .send()
        .await?
        .json()
        .await?;
    Ok(promos)
}
