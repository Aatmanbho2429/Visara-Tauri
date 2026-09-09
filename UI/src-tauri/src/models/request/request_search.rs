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
}
