use thiserror::Error;

#[derive(Debug, Error)]
pub enum PictoriaError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Image error: {0}")]
    Image(#[from] image::ImageError),

    #[error("Inference error: {0}")]
    Ort(#[from] ort::Error),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Model decryption failed - invalid or expired key")]
    Decryption,

    #[error("No saved session - please login")]
    NoSession,

    #[error("Session expired - {0}")]
    SessionExpired(String),

    #[error("AI model not loaded - please restart and login again")]
    ModelNotReady,

    #[error("Search already in progress")]
    SearchBusy,

    #[error("{0}")]
    Fatal(String),
}

impl PictoriaError {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "success": false,
            "message": self.to_string(),
            "data":    null
        })
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
