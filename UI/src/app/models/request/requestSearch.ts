// Search request payloads — mirrors UI/src-tauri/src/models/request/request_search.rs.

// `scopePaths` empty or omitted → search every watched folder in the Library.
export interface requestStartSearch {
  imagePath: string;
  scopePaths: string[];
  topK: number;
}
