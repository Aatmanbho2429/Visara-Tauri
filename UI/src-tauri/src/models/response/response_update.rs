// Auto-updater event payloads (`update_available`, `update_progress`).
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseUpdateAvailable {
    pub version: String,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseUpdateProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
}
