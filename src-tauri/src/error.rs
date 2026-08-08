use reqwest::StatusCode;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("keychain error: {0}")]
    Keychain(#[from] keyring::Error),
    #[error("unsupported provider: {0}")]
    UnsupportedProvider(String),
    #[error("missing environment variable: {0}")]
    MissingEnv(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("auth error: {0}")]
    Auth(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("app data directory unavailable")]
    AppDataDirUnavailable,
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type CommandResult<T> = Result<T, AppError>;

pub fn response_error_message(service: &str, status: StatusCode, body: &str) -> String {
    let fallback = format!("{service} request failed with HTTP {status}");
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return if body.trim().is_empty() {
            fallback
        } else {
            format!("{fallback}: {}", body.trim())
        };
    };

    value
        .pointer("/error/message")
        .or_else(|| value.pointer("/message"))
        .or_else(|| value.pointer("/error"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or(fallback)
}
