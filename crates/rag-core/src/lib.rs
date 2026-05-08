//! `rag-core` — library for indexing and searching a vault of documents.
//!
//! This crate is the engine. The `rag` CLI binary in `rag-cli` is a thin frontend
//! over the public API exposed here.

#![allow(clippy::should_implement_trait)] // FileStatus::from_str predates std::str::FromStr trait usage
#![allow(clippy::type_complexity)] // progress callback signature is intentionally explicit
#![allow(clippy::verbose_file_reads)] // OpenOptions for the index lock file does not need .truncate()

pub mod chunk;
pub mod config;
pub mod db;
pub mod embed;
pub mod error;
pub mod extract;
pub mod ignore;
pub mod index;
pub mod info;
pub mod registry;
pub mod search;
pub mod status;
pub mod vault;

pub use error::{Error, Result};
pub use vault::Vault;

// Re-export rusqlite so consumers (rag-cli) can use the same version without
// pinning it independently in their Cargo.toml.
pub use rusqlite;
