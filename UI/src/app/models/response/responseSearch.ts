// Search response payloads — mirrors UI/src-tauri/src/models/response/response_search.rs.

// Where in a result image the query design was found, as fractions of its
// width and height.
export interface MatchRegion {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface SearchResult {
  rank: number;
  path: string;
  name: string;
  similarity: number;
  patternMatch: number;
  colorMatch: number;
  folder: string;
  // True once SIFT/RANSAC has geometrically proven the query sits inside
  // this file — a direct fact, not a similarity threshold.
  verified: boolean;
  // True when `verified` AND the matched region is a small piece of this
  // file rather than nearly the whole frame — genuinely "found inside a
  // bigger design", not just "this is basically the same image".
  partial: boolean;
  matchRegion: MatchRegion;
  // SIFT inlier count backing `verified` (0 when not verified).
  matchPoints: number;
  // True when the design was only provable against a horizontally-flipped
  // reference — a mirrored or book-matched copy, not a straight one.
  // Always false unless `verified`.
  mirrored: boolean;
}

export interface FailedFile {
  file: string;
  reason: string;
}

export interface responseSidecarStatus {
  healthy: boolean;
  ready: boolean;
  embedReady: boolean;
}

export interface SearchProgressSnapshot {
  active: boolean;
  phase: string;
  done: number;
  total: number;
  current: string;
  percent: number;
  etaSec: number;
  errors: number;
}

export interface responseSearchProgress {
  progress: SearchProgressSnapshot;
}

export interface responseSearchComplete {
  done: boolean;
  results: SearchResult[];
  failedFiles: FailedFile[];
}
