use crate::cli::IndexCmd;
use crate::commands::open_vault;
use crate::output::emit_json;
use indicatif::{ProgressBar, ProgressStyle};
use rag_core::embed::CandleEmbedder;
use rag_core::extract::ExtractorRegistry;
use rag_core::index::{run_index, IndexOptions};
use std::path::Path;
use std::time::Duration;

pub fn run(cmd: IndexCmd, json: bool, vault_arg: Option<&Path>) -> anyhow::Result<i32> {
    let mut vault = open_vault(vault_arg)?;
    let extractors = ExtractorRegistry::standard();

    // Count work to decide whether to show progress.
    let pending: i64 = vault
        .conn
        .query_row(
            "SELECT COUNT(*) FROM files WHERE status IN ('pending','indexed','missing','failed','needs_ocr','unsupported','excluded','too_large')",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    if !json && pending > 0 {
        println!("Loading embedder ({})...", vault.config.embedding.model);
    }

    let cache_dir = vault.models_dir();
    let model_id = vault.config.embedding.model.clone();
    let device = vault.config.embedding.device.clone();
    let batch = vault.config.embedding.batch_size;
    let embedder = CandleEmbedder::load(
        &model_id,
        device,
        &cache_dir,
        batch,
        if json {
            None
        } else {
            Some(&|m: &str| eprintln!("{m}"))
        },
    )?;

    let opts = IndexOptions {
        force: cmd.force,
        retry_failed: cmd.retry_failed,
        paths: if cmd.paths.is_empty() {
            None
        } else {
            Some(cmd.paths)
        },
        no_wait: cmd.no_wait,
        wait_seconds: Some(cmd.wait),
    };

    let pb = if !json {
        let bar = ProgressBar::new(pending as u64);
        bar.set_style(
            ProgressStyle::with_template("[{bar:40.cyan/blue}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("##-"),
        );
        bar.enable_steady_tick(Duration::from_millis(120));
        Some(bar)
    } else {
        None
    };

    let progress_cb: Option<Box<dyn Fn(usize, usize, &str)>> = pb.as_ref().map(|bar| {
        let bar = bar.clone();
        Box::new(move |i: usize, total: usize, path: &str| {
            bar.set_length(total as u64);
            bar.set_position(i as u64);
            bar.set_message(path.to_string());
        }) as Box<dyn Fn(usize, usize, &str)>
    });

    let report = run_index(
        &mut vault,
        &embedder,
        &extractors,
        &opts,
        progress_cb
            .as_deref()
            .map(|b| b as &dyn Fn(usize, usize, &str)),
    )?;

    if let Some(bar) = pb {
        bar.finish_and_clear();
    }

    if json {
        emit_json(&report)?;
    } else {
        let s = &report.summary;
        println!(
            "Indexed: {} | Skipped: {} | Failed: {}",
            s.indexed, s.skipped, s.failed
        );
        println!(
            "Missing: {} | NeedsOcr: {} | Unsupported: {} | Excluded: {} | TooLarge: {}",
            s.missing, s.needs_ocr, s.unsupported, s.excluded, s.too_large
        );
        println!("Total time: {} ms", report.duration_ms);
    }
    Ok(0)
}
