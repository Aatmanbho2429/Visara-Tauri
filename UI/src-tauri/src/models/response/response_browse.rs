// Browse-grid response payloads.
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowseEntry {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseBrowseDirectory {
    pub current: String,
    pub breadcrumb: Vec<BrowseEntry>,
    pub folders: Vec<BrowseEntry>,
    pub images: Vec<BrowseEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseGetThumbnail {
    pub path: String,
}
