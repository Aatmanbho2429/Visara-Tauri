// Search request payloads.
use serde::Deserialize;

// `scope_paths` empty or omitted → search every watched folder in the Library.
// Otherwise the search is restricted to the provided folders.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestStartSearch {
    pub image_path: String,
    pub scope_paths: Option<Vec<String>>,
    pub top_k: usize,
    // Deep tier's cosine floor; clamped by `search_start`, absent means the default.
    pub deep_min_sim: Option<f32>,
    // Verify every file in scope with no floor, cap or budget; absent means off.
    pub full_db: Option<bool>,
}
