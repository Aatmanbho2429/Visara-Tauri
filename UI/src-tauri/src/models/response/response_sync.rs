// Library-sync event payloads (the `library_sync_*` event stream) plus the
// `FileError` type moved here from services::sync, per models.md.
use crate::core::progress::ProgressSnapshot;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileError {
    pub file: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseLibrarySyncStarted {
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseLibrarySyncProgress {
    pub path: String,
    pub progress: ProgressSnapshot,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseLibrarySyncComplete {
    pub path: String,
    pub errors: usize,
    pub failed: Vec<FileError>,
    pub image_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseLibrarySyncError {
    pub path: String,
    pub message: String,
}
