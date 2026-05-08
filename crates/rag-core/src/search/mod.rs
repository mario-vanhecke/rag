pub mod keyword;
pub mod rrf;
pub mod vector;

use crate::embed::Embedder;
use crate::error::Result;
use crate::vault::Vault;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub query: String,
    pub k: u32,
    pub filter: Option<String>,
    pub mode: SearchMode,
    pub threshold: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    Hybrid,
    VectorOnly,
    KeywordOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hit {
    pub chunk_id: String,
    pub file_path: String,
    pub ordinal: u32,
    pub heading_path: Option<String>,
    pub page_number: Option<u32>,
    pub content: String,
    pub score: f32,
    pub vector_rank: Option<u32>,
    pub keyword_rank: Option<u32>,
}

pub fn search(vault: &Vault, embedder: &dyn Embedder, query: &SearchQuery) -> Result<Vec<Hit>> {
    let n = (query.k * 2).max(query.k);
    let pool: Vec<rrf::Candidate> = match query.mode {
        SearchMode::Hybrid => {
            let mut candidates = Vec::new();
            let q_vec = embedder.embed_batch(&[query.query.as_str()])?.pop();
            if let Some(v) = q_vec {
                let vr = vector::query(vault, &v, n, query.filter.as_deref())?;
                for (i, h) in vr.iter().enumerate() {
                    candidates.push(rrf::Candidate {
                        chunk_id: h.chunk_id.clone(),
                        file_path: h.file_path.clone(),
                        ordinal: h.ordinal,
                        heading_path: h.heading_path.clone(),
                        page_number: h.page_number,
                        content: h.content.clone(),
                        vector_rank: Some((i + 1) as u32),
                        keyword_rank: None,
                    });
                }
            }
            let kr = keyword::query(vault, &query.query, n, query.filter.as_deref())?;
            for (i, h) in kr.iter().enumerate() {
                if let Some(existing) = candidates.iter_mut().find(|c| c.chunk_id == h.chunk_id) {
                    existing.keyword_rank = Some((i + 1) as u32);
                } else {
                    candidates.push(rrf::Candidate {
                        chunk_id: h.chunk_id.clone(),
                        file_path: h.file_path.clone(),
                        ordinal: h.ordinal,
                        heading_path: h.heading_path.clone(),
                        page_number: h.page_number,
                        content: h.content.clone(),
                        vector_rank: None,
                        keyword_rank: Some((i + 1) as u32),
                    });
                }
            }
            candidates
        }
        SearchMode::VectorOnly => {
            let q_vec = embedder
                .embed_batch(&[query.query.as_str()])?
                .pop()
                .unwrap_or_default();
            let vr = vector::query(vault, &q_vec, query.k, query.filter.as_deref())?;
            vr.into_iter()
                .enumerate()
                .map(|(i, h)| rrf::Candidate {
                    chunk_id: h.chunk_id,
                    file_path: h.file_path,
                    ordinal: h.ordinal,
                    heading_path: h.heading_path,
                    page_number: h.page_number,
                    content: h.content,
                    vector_rank: Some((i + 1) as u32),
                    keyword_rank: None,
                })
                .collect()
        }
        SearchMode::KeywordOnly => {
            let kr = keyword::query(vault, &query.query, query.k, query.filter.as_deref())?;
            kr.into_iter()
                .enumerate()
                .map(|(i, h)| rrf::Candidate {
                    chunk_id: h.chunk_id,
                    file_path: h.file_path,
                    ordinal: h.ordinal,
                    heading_path: h.heading_path,
                    page_number: h.page_number,
                    content: h.content,
                    vector_rank: None,
                    keyword_rank: Some((i + 1) as u32),
                })
                .collect()
        }
    };

    let rrf_const = vault.config.retrieval.rrf_constant;
    let mut hits = rrf::fuse(pool, rrf_const);

    if let Some(t) = query.threshold {
        hits.retain(|h| h.score >= t);
    }
    hits.truncate(query.k as usize);
    Ok(hits)
}
