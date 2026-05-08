use crate::chunk::Chunk;
use crate::error::Result;
use crate::registry::{FileRow, FileStatus};
use crate::vault::Vault;
use rusqlite::params;

/// Atomic write of a status-only update. If `delete_chunks` is true, the
/// transaction also drops chunks for this file before changing the status. The
/// consistency invariant requires that any transition out of `indexed` drops
/// chunks in the same transaction; pass `true` whenever the previous status
/// was `indexed` (regardless of whether chunks "should" exist).
pub fn write_status_only(
    vault: &Vault,
    row: &FileRow,
    status: FileStatus,
    detail: Option<&str>,
    note: Option<&str>,
    delete_chunks: bool,
) -> Result<()> {
    let tx = vault.conn.unchecked_transaction()?;
    if delete_chunks {
        tx.execute("DELETE FROM chunks WHERE file_id = ?1", params![row.id])?;
    }
    let now = chrono::Utc::now().timestamp_millis();
    let attempts = if matches!(status, FileStatus::Failed) {
        row.attempts + 1
    } else {
        0
    };
    tx.execute(
        "UPDATE files SET
            status = ?1,
            status_detail = ?2,
            status_note = ?3,
            attempts = ?4,
            last_attempt = ?5
         WHERE id = ?6",
        params![status.as_str(), detail, note, attempts, now, row.id],
    )?;
    tx.commit()?;
    Ok(())
}

/// Atomic write of a fully-indexed file: replace chunks + update files row.
pub fn write_indexed_content(
    vault: &Vault,
    row: &FileRow,
    chunks: &[Chunk],
    embeddings: &[Vec<f32>],
    mtime_ms: Option<i64>,
    size: i64,
    content_hash: &str,
) -> Result<()> {
    assert_eq!(chunks.len(), embeddings.len());
    let now = chrono::Utc::now().timestamp_millis();
    let tx = vault.conn.unchecked_transaction()?;

    tx.execute("DELETE FROM chunks WHERE file_id = ?1", params![row.id])?;

    {
        let mut stmt_chunk = tx.prepare(
            "INSERT INTO chunks
                (id, file_id, ordinal, content, content_hash,
                 heading_path, page_number, token_count, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )?;
        let mut stmt_vec =
            tx.prepare("INSERT INTO chunk_vectors (chunk_id, embedding) VALUES (?1, ?2)")?;
        let mut stmt_fts = tx.prepare(
            "INSERT INTO chunk_fts (chunk_id, content, heading_path) VALUES (?1, ?2, ?3)",
        )?;

        for (chunk, embedding) in chunks.iter().zip(embeddings.iter()) {
            let id = uuid::Uuid::now_v7().to_string();
            stmt_chunk.execute(params![
                id,
                row.id,
                chunk.ordinal as i64,
                chunk.content,
                chunk.content_hash,
                chunk.heading_path,
                chunk.page_number.map(|p| p as i64),
                chunk.token_count as i64,
                now,
            ])?;
            // sqlite-vec accepts a BLOB of f32 little-endian.
            let bytes = floats_to_bytes(embedding);
            stmt_vec.execute(params![id, bytes])?;
            stmt_fts.execute(params![
                id,
                chunk.content,
                chunk.heading_path.clone().unwrap_or_default()
            ])?;
        }
    }

    tx.execute(
        "UPDATE files SET
            status = 'indexed',
            status_detail = NULL,
            status_note = NULL,
            last_mtime = ?1,
            last_size = ?2,
            last_hash = ?3,
            last_indexed = ?4,
            attempts = 0,
            last_attempt = ?4
         WHERE id = ?5",
        params![mtime_ms, size, content_hash, now, row.id],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn delete_chunks_for(vault: &Vault, file_id: i64) -> Result<usize> {
    let n = vault
        .conn
        .execute("DELETE FROM chunks WHERE file_id = ?1", params![file_id])?;
    Ok(n)
}

pub fn floats_to_bytes(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for f in v {
        out.extend_from_slice(&f.to_le_bytes());
    }
    out
}
