use crate::models::response::ApiResponse;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PictoriaError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Image error: {0}")]
    Image(#[from] image::ImageError),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Network error: {0}")]
    Network(String),

    #[error("No saved session - please login")]
    NoSession,

    #[error("Session expired - {0}")]
    SessionExpired(String),

    #[error("Design-match engine not ready yet - please wait a moment and try again")]
    ModelNotReady,

    #[error("Search already in progress")]
    SearchBusy,

    #[error("{0}")]
    Fatal(String),
}

impl PictoriaError {
    // Error-range statusCode for the ApiResponse envelope — see
    // .claude/rules/api-response-format.md for the mapping.
    pub fn status_code(&self) -> u16 {
        match self {
            PictoriaError::NoSession | PictoriaError::SessionExpired(_) => 401,
            PictoriaError::Image(_) => 422,
            PictoriaError::SearchBusy => 409,
            PictoriaError::Network(_) => 503,
            PictoriaError::ModelNotReady => 503,
            PictoriaError::Io(_) | PictoriaError::Database(_) | PictoriaError::Fatal(_) => 500,
        }
    }

    // Builds the typed envelope directly, so callers don't hand-roll
    // `ApiResponse::err(e.status_code(), e.to_string())` at every call site.
    pub fn to_response<T>(&self) -> ApiResponse<T> {
        ApiResponse::err(self.status_code(), self.to_string())
    }
}

impl From<reqwest::Error> for PictoriaError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_connect() || e.is_timeout() {
            PictoriaError::Network(
                "No internet connection. Please connect and try again.".into(),
            )
        } else {
            PictoriaError::Network(e.to_string())
        }
    }
}

impl From<anyhow::Error> for PictoriaError {
    fn from(e: anyhow::Error) -> Self {
        PictoriaError::Fatal(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, PictoriaError>;
