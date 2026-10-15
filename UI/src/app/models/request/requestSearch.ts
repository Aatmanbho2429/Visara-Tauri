// Search request payloads — mirrors UI/src-tauri/src/models/request/request_search.rs.

// `scopePaths` empty or omitted → search every watched folder in the Library.
export interface requestStartSearch {
  imagePath: string;
  scopePaths: string[];
  topK: number;
  // Deep tier's cosine floor; omitted means the backend default (0.4).
  deepMinSim?: number;
}
