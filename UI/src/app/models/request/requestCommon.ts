// Shared request shapes reused by several commands that take nothing but a
// path or a path list — mirrors UI/src-tauri/src/models/request/request_common.rs.

export interface requestPath {
  path: string;
}

export interface requestPaths {
  paths: string[];
}
