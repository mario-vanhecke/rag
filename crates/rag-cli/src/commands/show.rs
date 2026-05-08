use crate::cli::ShowCmd;
use crate::commands::open_vault;
use crate::output::emit_json;
use rag_core::registry;
use rag_core::rusqlite::{self, params};
use serde_json::json;
use std::path::Path;

pub fn run(cmd: ShowCmd, json: bool, vault_arg: Option<&Path>) -> anyhow::Result<i32> {
    let vault = open_vault(vault_arg)?;
    // First try chunk lookup by ID.
    let chunk: Option<(String, i64, i64, String, String, Option<String>, Option<i64>, i64)> = vault
        .conn
        .query_row(
            "SELECT id, file_id, ordinal, content, content_hash, heading_path, page_number, token_count
             FROM chunks WHERE id = ?1",
            params![cmd.target],
            |r| Ok((
                r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?,
                r.get(5)?, r.get(6)?, r.get(7)?,
            )),
        )
        .ok();

    if let Some((id, file_id, ordinal, content, _hash, heading_path, page_number, token_count)) =
        chunk
    {
        // Look up file path
        let file_path: String = vault.conn.query_row(
            "SELECT path FROM files WHERE id = ?1",
            params![file_id],
            |r| r.get(0),
        )?;
        if json {
            emit_json(&json!({
                "chunk_id": id,
                "file": {"path": file_path, "status": "indexed"},
                "ordinal": ordinal,
                "heading_path": heading_path,
                "page_number": page_number,
                "token_count": token_count,
                "content": content,
            }))?;
        } else {
            println!("Chunk {}", id);
            println!("File:    {}", file_path);
            println!("Ordinal: {}", ordinal);
            if let Some(h) = heading_path {
                println!("Heading: {}", h);
            }
            if let Some(p) = page_number {
                println!("Page:    {}", p);
            }
            println!("---");
            println!("{}", content);
        }
        return Ok(0);
    }

    // Treat as a path.
    let row = registry::find_by_path(&vault.conn, &cmd.target)?;
    let row = match row {
        Some(r) => r,
        None => anyhow::bail!("not found: no chunk or file matches {}", cmd.target),
    };
    // Also enumerate chunks for this file
    let mut stmt = vault.conn.prepare(
        "SELECT id, ordinal, heading_path, page_number, token_count
         FROM chunks WHERE file_id = ?1 ORDER BY ordinal",
    )?;
    let chunks: Vec<_> = stmt
        .query_map(params![row.id], |r| {
            Ok(json!({
                "chunk_id": r.get::<_, String>(0)?,
                "ordinal": r.get::<_, i64>(1)?,
                "heading_path": r.get::<_, Option<String>>(2)?,
                "page_number": r.get::<_, Option<i64>>(3)?,
                "token_count": r.get::<_, i64>(4)?,
            }))
        })?
        .collect::<rusqlite::Result<_>>()?;

    if json {
        emit_json(&json!({
            "path": row.path,
            "status": row.status.as_str(),
            "added_at": row.added_at,
            "last_indexed": row.last_indexed,
            "chunk_count": chunks.len(),
            "chunks": chunks,
        }))?;
    } else {
        println!("File: {}", row.path);
        println!("Status: {}", row.status.as_str());
        println!("Chunks: {}", chunks.len());
        for c in &chunks {
            println!(
                "  {} #{:<3} {}",
                c["chunk_id"].as_str().unwrap_or(""),
                c["ordinal"].as_i64().unwrap_or(0),
                c["heading_path"].as_str().unwrap_or("")
            );
        }
    }
    Ok(0)
}
