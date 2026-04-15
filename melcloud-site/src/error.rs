use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum SiteError {
    #[error("{0}")]
    Protocol(String),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("yaml error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("cli error: {0}")]
    Cli(String),
    #[error("cli timeout: {0}")]
    CliTimeout(String),
    #[error("write not confirmed: {0}")]
    WriteNotConfirmed(String),
}

impl SiteError {
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::Protocol(_) => "protocol",
            Self::Io(_) => "io",
            Self::Json(_) => "json",
            Self::Yaml(_) => "yaml",
            Self::Cli(_) => "cli",
            Self::CliTimeout(_) => "cli_timeout",
            Self::WriteNotConfirmed(_) => "write_not_confirmed",
        }
    }

    pub(crate) fn user_message(&self) -> String {
        match self {
            Self::Protocol(message) => message.clone(),
            Self::Cli(_) => "MELCloud command failed.".to_string(),
            Self::CliTimeout(_) => "MELCloud command timed out.".to_string(),
            Self::WriteNotConfirmed(_) => "Device update was not confirmed.".to_string(),
            Self::Io(_) => "Server file operation failed.".to_string(),
            Self::Json(_) | Self::Yaml(_) => "Server data file is invalid.".to_string(),
        }
    }
}

pub(crate) type Result<T> = std::result::Result<T, SiteError>;
