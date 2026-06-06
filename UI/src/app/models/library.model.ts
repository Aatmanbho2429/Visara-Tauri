/** Status reported by the Rust backend for a watched folder. */
export type WatchedFolderStatus =
  | 'watching'   // OS subscription active, idle
  | 'indexing'   // catching up on changes / initial scan
  | 'paused'     // user-suspended; no events processed
  | 'error'      // last sync failed
  | 'missing';   // folder path no longer exists on disk

export interface WatchedFolder {
  id:            number;
  path:          string;
  status:        WatchedFolderStatus;
  added_at:      number;
  last_event_at: number;
  image_count:   number;
}

export interface ListFoldersData {
  folders: WatchedFolder[];
}

export interface LibraryStats {
  watched_folder_count: number;
  total_indexed_files:  number;
}

/** One node in a watched folder's subfolder tree.
 *  `direct` = images stored directly in this folder;
 *  `total`  = images in this folder and all descendants. */
export interface FolderTreeNode {
  name:     string;
  rel:      string;
  direct:   number;
  total:    number;
  children: FolderTreeNode[];
}

export interface FolderTreeData {
  tree: FolderTreeNode;
}
