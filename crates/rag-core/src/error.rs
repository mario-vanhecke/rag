use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    /// Anything that flows up from `vault-core` (io, sqlite, lock contention,
    /// no-vault, schema mismatch, invalid path, …) lands here. Callers that
    /// need to discriminate can match on the inner `vault_core::Error`.
    #[error(transparent)]
    Vault(#[from] vault_core::Error),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    #[error("config: {0}")]
    Config(String),

    #[error("path not in registry: {0}")]
    PathNotInRegistry(String),

    #[error("extractor: {0}")]
    Extractor(String),

    #[error("subprocess: {0}")]
    Subprocess(String),

    #[error("embedder: {0}")]
    Embedder(String),

    #[error("invariant violation: {0}")]
    Invariant(String),

    #[error("{0}")]
    Other(String),
}

impl Error {
    pub fn other<S: Into<String>>(s: S) -> Self {
        Self::Other(s.into())
    }
    pub fn config<S: Into<String>>(s: S) -> Self {
        Self::Config(s.into())
    }
    pub fn embedder<S: Into<String>>(s: S) -> Self {
        Self::Embedder(s.into())
    }
    pub fn extractor<S: Into<String>>(s: S) -> Self {
        Self::Extractor(s.into())
    }
}
