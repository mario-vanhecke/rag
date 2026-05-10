//! rag's schema migrations. The runner itself lives in `vault-core`.

use crate::error::Result;
use rusqlite::Connection;
use vault_core::Migration;

pub const SCHEMA_VERSION: u32 = 2;

const MIGRATION_001: &str = include_str!("migrations/001_initial.sql");
const MIGRATION_002: &str = include_str!("migrations/002_drop_chunks_unique_idx.sql");

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        sql: MIGRATION_001,
    },
    Migration {
        version: 2,
        sql: MIGRATION_002,
    },
];

/// Apply all pending rag migrations. Forwards to `vault_core::apply_pending`
/// with rag's specific migration list.
pub fn apply_pending(conn: &mut Connection) -> Result<Vec<u32>> {
    Ok(vault_core::apply_pending(conn, MIGRATIONS)?)
}

pub fn current_version(conn: &Connection) -> Result<u32> {
    Ok(vault_core::current_version(conn)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use vault_core::open_in_memory;

    #[test]
    fn migrations_apply_cleanly() {
        let mut conn = open_in_memory().unwrap();
        let applied = apply_pending(&mut conn).unwrap();
        assert_eq!(applied, vec![1, 2]);
        assert_eq!(current_version(&conn).unwrap(), 2);

        // Idempotent
        let applied2 = apply_pending(&mut conn).unwrap();
        assert!(applied2.is_empty());
    }

    #[test]
    fn migration_002_drops_unique_index() {
        let mut conn = open_in_memory().unwrap();
        apply_pending(&mut conn).unwrap();
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE name = 'idx_chunks_file_hash'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false);
        assert!(
            !exists,
            "idx_chunks_file_hash should be gone after migration 002"
        );
    }

    #[test]
    fn migration_002_runs_against_v1_vault() {
        let mut conn = open_in_memory().unwrap();
        // Apply only migration 001 by hand (simulating a v0.1.0 vault).
        conn.execute_batch(MIGRATION_001).unwrap();
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (1, 0)",
            [],
        )
        .unwrap();
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE name = 'idx_chunks_file_hash'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false);
        assert!(exists, "v1 vaults should have idx_chunks_file_hash");

        let applied = apply_pending(&mut conn).unwrap();
        assert_eq!(applied, vec![2]);
        let still_there: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE name = 'idx_chunks_file_hash'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false);
        assert!(!still_there, "v1 → v2 migration should drop the index");
    }

    #[test]
    fn schema_has_expected_tables() {
        let mut conn = open_in_memory().unwrap();
        apply_pending(&mut conn).unwrap();
        for tbl in &[
            "vault_meta",
            "settings",
            "files",
            "chunks",
            "chunk_vectors",
            "chunk_fts",
        ] {
            let exists: bool = conn
                .query_row(
                    "SELECT 1 FROM sqlite_master WHERE name = ?1",
                    params![tbl],
                    |_| Ok(true),
                )
                .unwrap_or(false);
            assert!(exists, "missing table {tbl}");
        }
    }
}
