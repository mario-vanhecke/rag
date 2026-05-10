//! Shared lifecycle plumbing for tools in this distribution.
//!
//! What lives here:
//!   - SQLite connection helpers (auto-loaded sqlite-vec extension, WAL,
//!     foreign keys, busy_timeout)
//!   - A generic migrations runner over a list of `Migration { version, sql }`
//!   - State-directory discovery (walk-up from cwd looking for `.<name>/`)
//!   - File-locking primitives (fs2-based; used by `rag index` / `md convert`)
//!   - A gitignore-style walker for adding files to a tool's registry
//!   - Path helpers (relativize, absolutize, forward-slash normalization)
//!
//! What does NOT live here:
//!   - Tool-specific schemas (rag's chunks/vectors/fts; md's outputs)
//!   - Tool-specific config (each tool defines its own typed Config on top
//!     of the generic `settings` table the migrations create)
//!   - Embedders, chunkers, search — those are rag's concern
//!   - Conversion output writing — that's md's concern

pub mod connection;
pub mod error;
pub mod ignore;
pub mod lock;
pub mod migrations;
pub mod path;

pub use connection::{open_connection, open_in_memory};
pub use error::{Error, Result};
pub use ignore::{VaultIgnore, BUILT_IN_DEFAULTS};
pub use lock::{acquire_lock, LockOptions};
pub use migrations::{apply_pending, current_version, Migration};

// Re-export rusqlite so consumers don't have to pin it separately.
pub use rusqlite;

// Re-export fs2's FileExt so callers can unlock the lock file we returned.
pub use fs2::FileExt;
