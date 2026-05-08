pub mod add;
pub mod prune;
pub mod rm;
pub mod status;

pub use add::{add_paths, AddOptions, AddReport};
pub use prune::{prune, PruneOptions, PruneReport};
pub use rm::{remove_paths, RmReport};
pub use status::FileStatus;

use crate::error::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRow {
    pub id: i64,
    pub path: String,
    pub added_at: i64,
    pub status: FileStatus,
    pub status_detail: Option<String>,
    pub status_note: Option<String>,
    pub last_mtime: Option<i64>,
    pub last_size: Option<i64>,
    pub last_hash: Option<String>,
    pub last_indexed: Option<i64>,
    pub attempts: i64,
    pub last_attempt: Option<i64>,
}

pub fn list_all(conn: &Connection) -> Result<Vec<FileRow>> {
    list_filtered(conn, None)
}

pub fn list_filtered(conn: &Connection, status: Option<FileStatus>) -> Result<Vec<FileRow>> {
    let mut stmt = if status.is_some() {
        conn.prepare(
            "SELECT id, path, added_at, status, status_detail, status_note,
                    last_mtime, last_size, last_hash, last_indexed, attempts, last_attempt
             FROM files WHERE status = ?1 ORDER BY path",
        )?
    } else {
        conn.prepare(
            "SELECT id, path, added_at, status, status_detail, status_note,
                    last_mtime, last_size, last_hash, last_indexed, attempts, last_attempt
             FROM files ORDER BY path",
        )?
    };
    let map = |r: &rusqlite::Row<'_>| -> rusqlite::Result<FileRow> {
        let status_str: String = r.get(3)?;
        Ok(FileRow {
            id: r.get(0)?,
            path: r.get(1)?,
            added_at: r.get(2)?,
            status: FileStatus::from_str(&status_str).unwrap_or(FileStatus::Pending),
            status_detail: r.get(4)?,
            status_note: r.get(5)?,
            last_mtime: r.get(6)?,
            last_size: r.get(7)?,
            last_hash: r.get(8)?,
            last_indexed: r.get(9)?,
            attempts: r.get(10)?,
            last_attempt: r.get(11)?,
        })
    };
    let rows: Vec<FileRow> = if let Some(s) = status {
        stmt.query_map(params![s.as_str()], map)?
            .collect::<std::result::Result<_, _>>()?
    } else {
        stmt.query_map([], map)?
            .collect::<std::result::Result<_, _>>()?
    };
    Ok(rows)
}

pub fn count_by_status(conn: &Connection) -> Result<Vec<(FileStatus, i64)>> {
    let mut stmt = conn.prepare("SELECT status, COUNT(*) FROM files GROUP BY status")?;
    let rows = stmt.query_map([], |r| {
        let s: String = r.get(0)?;
        let n: i64 = r.get(1)?;
        Ok((s, n))
    })?;
    let mut out = Vec::new();
    for r in rows {
        let (s, n) = r?;
        if let Ok(st) = FileStatus::from_str(&s) {
            out.push((st, n));
        }
    }
    Ok(out)
}

pub fn find_by_path(conn: &Connection, path: &str) -> Result<Option<FileRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, path, added_at, status, status_detail, status_note,
                last_mtime, last_size, last_hash, last_indexed, attempts, last_attempt
         FROM files WHERE path = ?1",
    )?;
    let mut rows = stmt.query(params![path])?;
    if let Some(r) = rows.next()? {
        let status_str: String = r.get(3)?;
        Ok(Some(FileRow {
            id: r.get(0)?,
            path: r.get(1)?,
            added_at: r.get(2)?,
            status: FileStatus::from_str(&status_str).unwrap_or(FileStatus::Pending),
            status_detail: r.get(4)?,
            status_note: r.get(5)?,
            last_mtime: r.get(6)?,
            last_size: r.get(7)?,
            last_hash: r.get(8)?,
            last_indexed: r.get(9)?,
            attempts: r.get(10)?,
            last_attempt: r.get(11)?,
        }))
    } else {
        Ok(None)
    }
}
