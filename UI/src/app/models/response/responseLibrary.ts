// Library / watched-folder response payloads — mirrors
// UI/src-tauri/src/models/response/response_library.rs.

// Status reported by the Rust backend for a watched folder.
export type WatchedFolderStatus =
  | 'watching'   // OS subscription active, idle
  | 'indexing'   // catching up on changes / initial scan
  | 'paused'     // user-suspended; no events processed
  | 'error'      // last sync failed
  | 'missing';   // folder path no longer exists on disk

export interface WatchedFolder {
  id: number;
  path: string;
  status: WatchedFolderStatus;
  addedAt: number;
  lastEventAt: number;
  imageCount: number;
  // Images on disk that made it into the index / images on disk in total.
  loadedCount: number;
  totalCount: number;
}

export interface responseListFolders {
  folders: WatchedFolder[];
}

export interface responseLibraryStats {
  watchedFolderCount: number;
  totalIndexedFiles: number;
}

// One node in a watched folder's subfolder tree.
// `direct` = images stored directly in this folder;
// `total`  = images in this folder and all descendants;
// the `*Loaded` pair counts only those that were indexed.
export interface FolderTreeNode {
  name: string;
  rel: string;
  direct: number;
  directLoaded: number;
  total: number;
  totalLoaded: number;
  children: FolderTreeNode[];
}

export interface responseFolderTree {
  tree: FolderTreeNode;
}

// Load state of one image on disk: indexed, tried and failed, or not tried yet.
export type ClientFileStatus = 'loaded' | 'failed' | 'pending';

export interface ClientFileRow {
  name: string;
  path: string;
  // Parent directory relative to the watched root, '/'-joined; '' = root.
  rel: string;
  status: ClientFileStatus;
  errorCode?: string;
  errorReason?: string;
}

export interface LibraryDirEntry {
  name: string;
  rel: string;
  loaded: number;
  total: number;
}

export interface responseLibraryCrumb {
  name: string;
  rel: string;
}

export interface responseLibraryDirCounts {
  loaded: number;
  failed: number;
  pending: number;
  total: number;
}

export interface responseLibraryDir {
  root: string;
  rel: string;
  breadcrumb: responseLibraryCrumb[];
  folders: LibraryDirEntry[];
  images: ClientFileRow[];
  counts: responseLibraryDirCounts;
  folderStatus: WatchedFolderStatus;
}

export interface responseLibraryNotLoaded {
  root: string;
  images: ClientFileRow[];
}
