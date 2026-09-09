// Browse-grid response payloads — mirrors UI/src-tauri/src/models/response/response_browse.rs.

// One entry (folder or image) in the Browse view.
export interface BrowseEntry {
  name: string;
  path: string;
}

export interface responseBrowseDirectory {
  current: string;
  breadcrumb: BrowseEntry[];
  folders: BrowseEntry[];
  images: BrowseEntry[];
}

export interface responseGetThumbnail {
  path: string;
}
