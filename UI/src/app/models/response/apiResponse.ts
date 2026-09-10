// Uniform envelope every command/event response carries — see
// .claude/rules/api-response-format.md. Mirrors Rust's ApiResponse<T>
// (UI/src-tauri/src/models/response/api_response.rs) field-for-field.
export interface apiResponse<T> {
  statusCode: number;
  message:    string;
  data:       T | null;
  requestId?: string;
}

// Thrown by ZoneWrapperService.invoke() when statusCode falls outside 2xx —
// callers catch this instead of checking `.success` on every response.
export interface ApiError {
  statusCode: number;
  message:    string;
}
