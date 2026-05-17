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
