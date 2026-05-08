use crate::cli::StatusCmd;
use crate::commands::open_vault;
use crate::output::emit_json;
use rag_core::registry::FileStatus;
use rag_core::status::{compute, StatusOptions};
use std::path::Path;

pub fn run(cmd: StatusCmd, json: bool, vault_arg: Option<&Path>) -> anyhow::Result<i32> {
    let vault = open_vault(vault_arg)?;
    let opts = StatusOptions {
        filter: match cmd.filter.as_deref() {
            Some(s) => Some(FileStatus::from_str(s)?),
            None => None,
        },
        no_stat: cmd.no_stat,
        show_untracked: cmd.show_untracked,
    };
    let report = compute(&vault, &opts)?;

    if json {
        emit_json(&report)?;
    } else {
        println!("Vault: {} ({})", report.vault.path, report.vault.name);
        println!(
            "Embedding: {} ({})",
            report.vault.embedding_model, report.vault.embedding_dimension
        );
        println!();
        let s = &report.summary;
        println!(
            "Registered: {:<6} Indexed: {:<6} Pending: {:<6} Modified: {}",
            s.registered, s.indexed, s.pending, s.modified
        );
        println!(
            "Failed: {:<10} NeedsOcr: {:<5} Unsupported: {:<5} Excluded: {}",
            s.failed, s.needs_ocr, s.unsupported, s.excluded
        );
        println!(
            "TooLarge: {:<8} Missing: {:<6} Untracked: {}",
            s.too_large, s.missing, s.untracked
        );
        println!();
        println!(
            "Index would process: {} | Prune would remove: {}",
            report.actions.index_would_process, report.actions.prune_would_remove
        );

        if cmd.full {
            println!();
            for f in &report.files {
                println!(
                    "{}{:<12} {}",
                    if f.modified { "* " } else { "  " },
                    f.status,
                    f.path
                );
            }
        }
    }

    Ok(0)
}
