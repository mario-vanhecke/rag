use crate::error::Result;
use rusqlite::{params, Connection};

pub const SCHEMA_VERSION: u32 = 1;

const MIGRATION_001: &str = include_str!("migrations/001_initial.sql");

pub struct Migration {
    pub version: u32,
    pub sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    sql: MIGRATION_001,
}];

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
        assert_eq!(applied, vec![1]);
        assert_eq!(current_version(&conn).unwrap(), 1);

        // Idempotent
        let applied2 = apply_pending(&mut conn).unwrap();
        assert!(applied2.is_empty());
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
