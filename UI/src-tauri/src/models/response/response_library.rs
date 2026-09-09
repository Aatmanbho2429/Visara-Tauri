// Library / watched-folder response payloads. Wraps the `core::database`
// row types directly rather than duplicating their fields — those structs
// already carry `#[serde(rename_all = "camelCase")]` for the same wire format.
use crate::core::database::{FolderTreeNode, WatchedFolder};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseListFolders {
    pub folders: Vec<WatchedFolder>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseLibraryStats {
    pub watched_folder_count: usize,
    pub total_indexed_files: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseFolderTree {
    pub tree: FolderTreeNode,
}
