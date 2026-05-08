use crate::error::Result;
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
    pub bm25: f32,
}

pub fn query(vault: &Vault, text: &str, k: u32, glob_filter: Option<&str>) -> Result<Vec<RawHit>> {
    let safe_query = sanitize_for_fts(text);
    if safe_query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut stmt = vault.conn.prepare(
        "SELECT fts.chunk_id, c.ordinal, c.heading_path, c.page_number, c.content, f.path,
                bm25(chunk_fts) AS score
         FROM chunk_fts AS fts
         JOIN chunks c ON c.id = fts.chunk_id
         JOIN files f ON f.id = c.file_id
         WHERE chunk_fts MATCH ?1
         ORDER BY score
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![safe_query, k as i64], |r| {
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
        let (chunk_id, ordinal, heading_path, page_number, content, file_path, bm25) = row?;
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
            bm25,
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

/// FTS5 has a syntax of its own — quoting each term as a phrase is a robust way
/// to handle arbitrary user input.
fn sanitize_for_fts(text: &str) -> String {
    let mut out = String::new();
    for tok in text.split_whitespace() {
        let cleaned: String = tok
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if cleaned.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push('"');
        out.push_str(&cleaned);
        out.push('"');
    }
    out
}
