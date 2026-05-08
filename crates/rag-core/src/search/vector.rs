use crate::error::Result;
use crate::index::pipeline::floats_to_bytes;
use crate::vault::Vault;
use rusqlite::params;

#[derive(Debug, Clone)]
pub struct RawHit {
    pub chunk_id: String,
    pub file_path: String,
    pub ordinal: u32,
    pub heading_path: Option<String>,
    pub page_number: Option<u32>,
    pub content: String,
    pub distance: f32,
}

pub fn query(
    vault: &Vault,
    embedding: &[f32],
    k: u32,
    glob_filter: Option<&str>,
) -> Result<Vec<RawHit>> {
    let bytes = floats_to_bytes(embedding);
    let mut stmt = vault.conn.prepare(
        "SELECT v.chunk_id, c.ordinal, c.heading_path, c.page_number, c.content, f.path, v.distance
         FROM chunk_vectors v
         JOIN chunks c ON c.id = v.chunk_id
         JOIN files f ON f.id = c.file_id
         WHERE v.embedding MATCH ?1
           AND k = ?2
         ORDER BY v.distance",
    )?;
    let rows = stmt.query_map(params![bytes, k as i64], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)? as u32,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, Option<i64>>(3)?.map(|p| p as u32),
            r.get::<_, String>(4)?,
            r.get::<_, String>(5)?,
            r.get::<_, f32>(6)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (chunk_id, ordinal, heading_path, page_number, content, file_path, distance) = row?;
        if let Some(g) = glob_filter {
            if !match_glob(g, &file_path) {
                continue;
            }
        }
        out.push(RawHit {
            chunk_id,
            file_path,
            ordinal,
            heading_path,
            page_number,
            content,
            distance,
        });
    }
    Ok(out)
}

fn match_glob(pat: &str, path: &str) -> bool {
    match glob::Pattern::new(pat) {
        Ok(p) => p.matches(path),
        Err(_) => false,
    }
}
