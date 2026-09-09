// Library / watched-folder request payloads.
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestRemoveFolder {
    pub path: String,
    pub purge: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestSetPaused {
    pub path: String,
    pub paused: bool,
}
