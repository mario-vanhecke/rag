use crate::cli::InfoCmd;
use crate::commands::open_vault;
use crate::output::emit_json;
use rag_core::info::compute;
use std::path::Path;

pub fn run(cmd: InfoCmd, json: bool, vault_arg: Option<&Path>) -> anyhow::Result<i32> {
    let vault = open_vault(vault_arg)?;
    let report = compute(&vault, cmd.check)?;

    if json {
        emit_json(&report)?;
    } else {
        println!("Path:           {}", report.path);
        println!("Vault ID:       {}", report.vault_id);
        println!("Name:           {}", report.name);
        println!("Schema:         {}", report.schema_version);
        println!("Tool version:   {}", report.tool_version);
        println!();
        println!(
            "Embedding:      {} ({} dim, {})",
            report.embedding.model, report.embedding.dimension, report.embedding.device
        );
        println!(
            "Chunking:       target={} max={} overlap={}",
            report.chunking.target_tokens,
            report.chunking.max_tokens,
            report.chunking.overlap_tokens
        );
        println!();
        let c = &report.counts;
        println!(
            "Counts: registered={} indexed={} chunks={} vectors={} fts={}",
            c.registered, c.indexed, c.chunks, c.vectors, c.fts_rows
        );
        println!("Database size:  {} bytes", report.size_bytes);
        if let Some(checks) = &report.checks {
            println!();
            println!("Checks:");
            println!(
                "  vectors_match_chunks:        {}",
                checks.vectors_match_chunks
            );
            println!(
                "  fts_matches_chunks:          {}",
                checks.fts_matches_chunks
            );
            println!(
                "  chunks_have_indexed_files:   {}",
                checks.chunks_have_indexed_files
            );
            if !checks.vectors_match_chunks
                || !checks.fts_matches_chunks
                || !checks.chunks_have_indexed_files
            {
                return Ok(crate::exit_codes::VAULT_CORRUPTION);
            }
        }
    }
    Ok(0)
}
