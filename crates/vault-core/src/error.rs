use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can come out of `vault-core`. Tool-specific error types
/// (`rag_core::Error`, `md_core::Error`) typically wrap this with
/// `#[error(transparent)] Vault(#[from] vault_core::Error)`.
#[derive(Debug, Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("no state directory '{name}' found in {start} or any parent directory")]
    NoState { name: String, start: PathBuf },

    #[error("state directory already exists at {path}")]
    StateExists { path: PathBuf },

    #[error("schema version mismatch: db={db}, expected={expected}")]
    SchemaMismatch { db: u32, expected: u32 },

    #[error("invalid path: {0}")]
    InvalidPath(String),

    #[error("lock contention: another process holds the vault lock")]
    LockContention,

    #[error("{0}")]
    Other(String),
}

impl Error {
    pub fn other<S: Into<String>>(s: S) -> Self {
        Self::Other(s.into())
    }
    pub fn invalid_path<S: Into<String>>(s: S) -> Self {
        Self::InvalidPath(s.into())
    }
}
