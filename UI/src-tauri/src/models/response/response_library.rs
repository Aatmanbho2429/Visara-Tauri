// Library / watched-folder response payloads. Wraps the `core::database`
// row types directly rather than duplicating their fields — those structs
// already carry `#[serde(rename_all = "camelCase")]` for the same wire format.
use crate::core::database::{ClientFileRow, DirEntry, FolderTreeNode, WatchedFolder};
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseLibraryCrumb {
    pub name: String,
    pub rel: String,
}

// Status tallies for the images directly in one directory; drives the filter chips.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseLibraryDirCounts {
    pub loaded: usize,
    pub failed: usize,
    pub pending: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseLibraryDir {
    pub root: String,
    pub rel: String,
    pub breadcrumb: Vec<ResponseLibraryCrumb>,
    pub folders: Vec<DirEntry>,
    pub images: Vec<ClientFileRow>,
    pub counts: ResponseLibraryDirCounts,
    pub folder_status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseLibraryNotLoaded {
    pub root: String,
    pub images: Vec<ClientFileRow>,
}
