use crate::cli::SearchCmd;
use crate::commands::open_vault;
use crate::output::emit_json;
use rag_core::embed::CandleEmbedder;
use rag_core::search::{search, SearchMode, SearchQuery};
use serde_json::json;
use std::path::Path;

pub fn run(cmd: SearchCmd, json: bool, vault_arg: Option<&Path>) -> anyhow::Result<i32> {
    let vault = open_vault(vault_arg)?;
    let k = cmd.k.unwrap_or(vault.config.retrieval.default_k);
    let mode = if cmd.vector_only {
        SearchMode::VectorOnly
    } else if cmd.keyword_only {
        SearchMode::KeywordOnly
    } else {
        SearchMode::Hybrid
    };

    // Loading the embedder downloads the model on first run; only do it if we
    // actually need vector retrieval.
    let need_embedder = !matches!(mode, SearchMode::KeywordOnly);
    let embedder = if need_embedder {
        Some(CandleEmbedder::load(
            &vault.config.embedding.model,
            vault.config.embedding.device.clone(),
            &vault.models_dir(),
            vault.config.embedding.batch_size,
            None,
        )?)
    } else {
        None
    };

    let q = SearchQuery {
        query: cmd.query.clone(),
        k,
        filter: cmd.filter,
        mode,
        threshold: cmd.threshold,
    };

    // Provide a no-op embedder for keyword-only mode.
    struct NullEmbedder;
    impl rag_core::embed::Embedder for NullEmbedder {
        fn dimension(&self) -> u32 {
            1024
        }
        fn model_id(&self) -> &str {
            "null"
        }
        fn embed_batch(&self, _: &[&str]) -> rag_core::Result<Vec<Vec<f32>>> {
            Ok(Vec::new())
        }
    }

    let hits = if let Some(e) = &embedder {
        search(&vault, e, &q)?
    } else {
        let null = NullEmbedder;
        search(&vault, &null, &q)?
    };

    if json {
        emit_json(&json!({
            "query": cmd.query,
            "k": k,
            "results": hits,
        }))?;
    } else {
        for (i, h) in hits.iter().enumerate() {
            let heading = h.heading_path.as_deref().unwrap_or("");
            let preview = h.content.chars().take(200).collect::<String>();
            println!("{}. [{:.3}] {}", i + 1, h.score, h.file_path);
            if !heading.is_empty() {
                println!("   {}", heading);
            }
            println!("   {}", preview);
            println!();
        }
        if hits.is_empty() {
            println!("(no results)");
        }
    }
    Ok(0)
}
