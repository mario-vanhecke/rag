use crate::error::Result;
use crate::vault::Vault;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfoReport {
    pub path: String,
    pub vault_id: String,
    pub name: String,
    pub created_at: i64,
    pub schema_version: u32,
    pub tool_version: String,
    pub embedding: EmbeddingInfo,
    pub chunking: ChunkingInfo,
    pub counts: CountsBlock,
    pub size_bytes: u64,
    pub last_indexed_at: Option<i64>,
    pub last_added_at: Option<i64>,
    pub checks: Option<ChecksBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingInfo {
    pub model: String,
    pub dimension: u32,
    pub device: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkingInfo {
    pub target_tokens: u32,
    pub max_tokens: u32,
    pub overlap_tokens: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountsBlock {
    pub registered: u32,
    pub indexed: u32,
    pub chunks: u32,
    pub vectors: u32,
    pub fts_rows: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecksBlock {
    pub vectors_match_chunks: bool,
    pub fts_matches_chunks: bool,
    pub chunks_have_indexed_files: bool,
}

pub fn compute(vault: &Vault, run_checks: bool) -> Result<InfoReport> {
    let (vault_id, created_at, tool_version) = vault.meta()?;
    let schema_version = vault.schema_version()?;
    let counts = CountsBlock {
        registered: scalar(vault, "SELECT COUNT(*) FROM files")?,
        indexed: scalar(vault, "SELECT COUNT(*) FROM files WHERE status='indexed'")?,
        chunks: scalar(vault, "SELECT COUNT(*) FROM chunks")?,
        vectors: scalar(vault, "SELECT COUNT(*) FROM chunk_vectors")?,
        fts_rows: scalar(vault, "SELECT COUNT(*) FROM chunk_fts")?,
    };
    let last_indexed_at: Option<i64> = vault
        .conn
        .query_row("SELECT MAX(last_indexed) FROM files", [], |r| r.get(0))
        .unwrap_or(None);
    let last_added_at: Option<i64> = vault
        .conn
        .query_row("SELECT MAX(added_at) FROM files", [], |r| r.get(0))
        .unwrap_or(None);

    let size_bytes = std::fs::metadata(&vault.db_path)
        .map(|m| m.len())
        .unwrap_or(0);

    let checks = if run_checks {
        let v_eq_c = counts.vectors == counts.chunks;
        let f_eq_c = counts.fts_rows == counts.chunks;
        let orphan: i64 = vault.conn.query_row(
            "SELECT COUNT(*) FROM chunks c
             LEFT JOIN files f ON f.id = c.file_id
             WHERE f.id IS NULL OR f.status != 'indexed'",
            [],
            |r| r.get(0),
        )?;
        Some(ChecksBlock {
            vectors_match_chunks: v_eq_c,
            fts_matches_chunks: f_eq_c,
            chunks_have_indexed_files: orphan == 0,
        })
    } else {
        None
    };

    let name = if vault.config.vault_name.is_empty() {
        vault
            .root
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default()
    } else {
        vault.config.vault_name.clone()
    };

    Ok(InfoReport {
        path: vault.root.to_string_lossy().to_string(),
        vault_id,
        name,
        created_at,
        schema_version,
        tool_version,
        embedding: EmbeddingInfo {
            model: vault.config.embedding.model.clone(),
            dimension: vault.config.embedding.dimension,
            device: vault.config.embedding.device.as_str().to_string(),
        },
        chunking: ChunkingInfo {
            target_tokens: vault.config.chunking.target_tokens,
            max_tokens: vault.config.chunking.max_tokens,
            overlap_tokens: vault.config.chunking.overlap_tokens,
        },
        counts,
        size_bytes,
        last_indexed_at,
        last_added_at,
        checks,
    })
}

fn scalar(vault: &Vault, sql: &str) -> Result<u32> {
    let n: i64 = vault.conn.query_row(sql, [], |r| r.get(0))?;
    Ok(n as u32)
}
