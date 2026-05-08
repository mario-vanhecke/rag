use super::Hit;

#[derive(Debug, Clone)]
pub struct Candidate {
    pub chunk_id: String,
    pub file_path: String,
    pub ordinal: u32,
    pub heading_path: Option<String>,
    pub page_number: Option<u32>,
    pub content: String,
    pub vector_rank: Option<u32>,
    pub keyword_rank: Option<u32>,
}

pub fn fuse(candidates: Vec<Candidate>, rrf_constant: u32) -> Vec<Hit> {
    let k = rrf_constant as f32;
    let mut hits: Vec<Hit> = candidates
        .into_iter()
        .map(|c| {
            let mut score = 0.0_f32;
            if let Some(r) = c.vector_rank {
                score += 1.0 / (k + r as f32);
            }
            if let Some(r) = c.keyword_rank {
                score += 1.0 / (k + r as f32);
            }
            Hit {
                chunk_id: c.chunk_id,
                file_path: c.file_path,
                ordinal: c.ordinal,
                heading_path: c.heading_path,
                page_number: c.page_number,
                content: c.content,
                score,
                vector_rank: c.vector_rank,
                keyword_rank: c.keyword_rank,
            }
        })
        .collect();
    hits.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fusion_prefers_double_match() {
        let cs = vec![
            Candidate {
                chunk_id: "a".into(),
                file_path: "a".into(),
                ordinal: 0,
                heading_path: None,
                page_number: None,
                content: String::new(),
                vector_rank: Some(1),
                keyword_rank: Some(1),
            },
            Candidate {
                chunk_id: "b".into(),
                file_path: "b".into(),
                ordinal: 0,
                heading_path: None,
                page_number: None,
                content: String::new(),
                vector_rank: Some(2),
                keyword_rank: None,
            },
        ];
        let h = fuse(cs, 60);
        assert_eq!(h[0].chunk_id, "a");
    }
}
