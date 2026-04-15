use thiserror::Error;

#[derive(Error, Debug)]
pub enum MelcloudError {
    #[error("HTTP request error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON parse/serialize error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Protocol error: {0}")]
    Protocol(String),
    #[error("Authentication failed: {0}")]
    Auth(String),
    #[error("Missing credentials. Provide --email and --password, or environment variables.")]
    MissingCredentials,
    #[error("No ATA devices found")]
    NoDevices,
    #[error("Bound device not found: {0}")]
    DeviceNotFound(String),
    #[error("Request rejected by server: {0}")]
    Rejected(String),
    #[error("Invalid payload value: {0}")]
    InvalidPayload(String),
}

pub type Result<T> = std::result::Result<T, MelcloudError>;
