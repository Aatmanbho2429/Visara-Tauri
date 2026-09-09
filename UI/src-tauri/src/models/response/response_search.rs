// Search response payloads — moved here from services::search so the IPC
// shape lives in models/ per models.md; services::search keeps building
// these, it just imports them from here now.
use serde::Serialize;

#[derive(Debug, Serialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct MatchRegion {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub rank: usize,
    pub path: String,
    pub name: String,
    // Stage-1 combined score (Gabor rose + Gram-matrix), scaled 0-100.
    pub similarity: f32,
    pub pattern_match: f32,
    pub color_match: f32,
    pub folder: String,
    // True once SIFT/RANSAC has actually proven the query sits inside this
    // file — a direct geometric fact, not a similarity threshold.
    pub verified: bool,
    // True when `verified` AND the matched region is a small piece of this
    // file rather than nearly the whole frame — i.e. genuinely "found
    // inside a bigger design", not just "this is basically the same image".
    pub partial: bool,
    pub match_region: MatchRegion,
    // SIFT inlier count backing `verified` — the UI's confidence badge
    // (e.g. "embedded · 103 pts"). Zero when not verified.
    pub match_points: u32,
    // True when this was only provable against a horizontally-flipped
    // query — a book-matched or mirrored copy of the reference rather than
    // a straight one. Always false unless `verified`.
    pub mirrored: bool,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FailedFile {
    pub file: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseSidecarStatus {
    pub healthy: bool,
    pub ready: bool,
    // Broken out so the UI can tell "sidecar still booting" apart from
    // "waiting on the licensed model" — minutes apart on a cold start.
    pub embed_ready: bool,
}

// Payload for the `search_progress` event.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchProgressSnapshot {
    pub active: bool,
    pub phase: String,
    pub done: usize,
    pub total: usize,
    pub current: String,
    pub percent: f32,
    pub eta_sec: i64,
    pub errors: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseSearchProgress {
    pub progress: SearchProgressSnapshot,
}

// Payload for the `search_complete` event.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseSearchComplete {
    pub done: bool,
    pub results: Vec<SearchResult>,
    pub failed_files: Vec<FailedFile>,
}
