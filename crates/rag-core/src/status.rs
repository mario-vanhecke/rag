use crate::error::Result;
use crate::registry::{self, FileStatus};
use crate::vault::Vault;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatusReport {
    pub vault: VaultBlock,
    pub summary: SummaryBlock,
    pub actions: ActionsBlock,
    pub files: Vec<FileBlock>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VaultBlock {
    pub path: String,
    pub name: String,
    pub embedding_model: String,
    pub embedding_dimension: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SummaryBlock {
    pub registered: u32,
    pub indexed: u32,
    pub pending: u32,
    pub modified: u32,
    pub failed: u32,
    pub needs_ocr: u32,
    pub unsupported: u32,
    pub missing: u32,
    pub excluded: u32,
    pub too_large: u32,
    pub untracked: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActionsBlock {
    pub index_would_process: u32,
    pub prune_would_remove: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileBlock {
    pub path: String,
    pub status: String,
    pub modified: bool,
}

#[derive(Debug, Clone, Default)]
pub struct StatusOptions {
    pub filter: Option<FileStatus>,
    pub no_stat: bool,
    pub show_untracked: bool,
}

pub fn compute(vault: &Vault, opts: &StatusOptions) -> Result<StatusReport> {
    let rows = registry::list_filtered(&vault.conn, opts.filter)?;
    let mut report = StatusReport::default();
    report.vault.path = vault.root.to_string_lossy().to_string();
    report.vault.name = if vault.config.vault_name.is_empty() {
        vault
            .root
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default()
    } else {
        vault.config.vault_name.clone()
    };
    report.vault.embedding_model = vault.config.embedding.model.clone();
    report.vault.embedding_dimension = vault.config.embedding.dimension;

    report.summary.registered = rows.len() as u32;

    let mut would_process = 0u32;
    let mut would_prune = 0u32;

    for r in &rows {
        let mut modified = false;
        if !opts.no_stat && r.status == FileStatus::Indexed {
            let abs = vault.absolutize(&r.path);
            if let Ok(meta) = std::fs::metadata(&abs) {
                let mtime_ms = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as i64);
                if mtime_ms != r.last_mtime || Some(meta.len() as i64) != r.last_size {
                    modified = true;
                }
            }
        }

        match r.status {
            FileStatus::Pending => {
                report.summary.pending += 1;
                would_process += 1;
            }
            FileStatus::Indexed => {
                report.summary.indexed += 1;
                if modified {
                    report.summary.modified += 1;
                    would_process += 1;
                }
            }
            FileStatus::Failed => {
                report.summary.failed += 1;
            }
            FileStatus::NeedsOcr => {
                report.summary.needs_ocr += 1;
            }
            FileStatus::Unsupported => {
                report.summary.unsupported += 1;
            }
            FileStatus::Missing => {
                report.summary.missing += 1;
                would_prune += 1;
            }
            FileStatus::Excluded => {
                report.summary.excluded += 1;
            }
            FileStatus::TooLarge => {
                report.summary.too_large += 1;
            }
        }

        report.files.push(FileBlock {
            path: r.path.clone(),
            status: r.status.as_str().to_string(),
            modified,
        });
    }

    if opts.show_untracked {
        let registered: std::collections::HashSet<String> =
            rows.iter().map(|r| r.path.clone()).collect();
        let mut untracked = 0u32;
        let walker = walkdir::WalkDir::new(&vault.root).follow_links(false);
        for entry in walker.into_iter().flatten() {
            if !entry.file_type().is_file() {
                continue;
            }
            let rel = match vault.relativize(entry.path()) {
                Ok(r) => r.to_string_lossy().to_string(),
                Err(_) => continue,
            };
            if rel.starts_with(".vault/") || rel == ".vault" {
                continue;
            }
            if !registered.contains(&rel) {
                untracked += 1;
            }
        }
        report.summary.untracked = untracked;
    }

    report.actions.index_would_process = would_process;
    report.actions.prune_would_remove = would_prune;

    Ok(report)
}
