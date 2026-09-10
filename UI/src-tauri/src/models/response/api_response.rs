// Uniform envelope every command/event response carries across the IPC
// boundary — see .claude/rules/api-response-format.md. `data` is optional so
// error responses don't need a caller to invent a placeholder payload.
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiResponse<T> {
    pub status_code: u16,
    pub message: String,
    pub data: Option<T>,
    // Echoes the client-supplied correlation id (see ZoneWrapperService on the
    // Angular side) so a command fired concurrently more than once — the
    // Browse grid's thumbnail requests, for instance — can match each
    // response back to the call that asked for it instead of the first
    // `<command>_response` event racing every pending caller.
    pub request_id: Option<String>,
}

impl<T> ApiResponse<T> {
    // 200 OK with a payload.
    pub fn ok(data: T) -> Self {
        Self { status_code: 200, message: "ok".to_string(), data: Some(data), request_id: None }
    }

    // 200 OK with a payload and a custom message (e.g. "Folder added — …").
    pub fn ok_with_message(message: impl Into<String>, data: T) -> Self {
        Self { status_code: 200, message: message.into(), data: Some(data), request_id: None }
    }

    // Attaches the caller's correlation id; chainable after ok()/err().
    pub fn with_request_id(mut self, request_id: Option<String>) -> Self {
        self.request_id = request_id;
        self
    }
}

impl ApiResponse<()> {
    // 200 OK with no payload — most mutation commands (add folder, set tag, …).
    pub fn ok_empty(message: impl Into<String>) -> Self {
        Self { status_code: 200, message: message.into(), data: Some(()), request_id: None }
    }
}

impl<T> ApiResponse<T> {
    // Error response: an error-range statusCode and a message, no data.
    pub fn err(status_code: u16, message: impl Into<String>) -> Self {
        Self { status_code, message: message.into(), data: None, request_id: None }
    }
}
