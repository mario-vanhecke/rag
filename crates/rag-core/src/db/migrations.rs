use crate::error::Result;
use rusqlite::{params, Connection};

pub const SCHEMA_VERSION: u32 = 2;

const MIGRATION_001: &str = include_str!("migrations/001_initial.sql");
const MIGRATION_002: &str = include_str!("migrations/002_drop_chunks_unique_idx.sql");

pub struct Migration {
    pub version: u32,
    pub sql: &'static str,
}

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

/// Apply all pending migrations. Returns the list of versions applied.
pub fn apply_pending(conn: &mut Connection) -> Result<Vec<u32>> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version    INTEGER PRIMARY KEY,
            applied_at INTEGER NOT NULL
         );",
    )?;

    let applied: std::collections::HashSet<u32> = conn
        .prepare("SELECT version FROM schema_migrations")?
        .query_map([], |r| r.get::<_, u32>(0))?
        .collect::<std::result::Result<_, _>>()?;

    let now = chrono::Utc::now().timestamp_millis();

    let mut newly_applied = Vec::new();
    for m in MIGRATIONS {
        if applied.contains(&m.version) {
            continue;
        }
        let tx = conn.transaction()?;
        tx.execute_batch(m.sql)?;
        // Each migration's body inserts its own row, but use INSERT OR IGNORE
        // here as a belt-and-suspenders guarantee.
        tx.execute(
            "INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
            params![m.version, now],
        )?;
        tx.commit()?;
        newly_applied.push(m.version);
    }

    Ok(newly_applied)
}

pub fn current_version(conn: &Connection) -> Result<u32> {
    let v: Option<u32> = conn
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |r| {
            r.get(0)
        })
        .ok();
    Ok(v.unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::open_in_memory;

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
        // Simulate a vault created at schema_version=1 (with the old unique
        // index) and verify migration 002 applies cleanly.
        let mut conn = open_in_memory().unwrap();
        // Apply only migration 001 by hand.
        conn.execute_batch(MIGRATION_001).unwrap();
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (1, 0)",
            [],
        )
        .unwrap();
        // Confirm the old index is present.
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE name = 'idx_chunks_file_hash'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false);
        assert!(exists, "v1 vaults should have idx_chunks_file_hash");

        // Now run apply_pending — should bring it to v2.
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
