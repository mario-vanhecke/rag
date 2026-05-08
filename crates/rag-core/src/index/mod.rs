pub mod pipeline;
pub mod reconcile;

use crate::embed::Embedder;
use crate::error::{Error, Result};
use crate::extract::ExtractorRegistry;
use crate::vault::Vault;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, Default)]
pub struct IndexOptions {
    pub force: bool,
    pub retry_failed: bool,
    pub paths: Option<Vec<PathBuf>>,
    pub no_wait: bool,
    pub wait_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexReport {
    pub started_at: i64,
    pub completed_at: i64,
    pub duration_ms: i64,
    pub summary: IndexSummary,
    pub results: Vec<FileResult>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexSummary {
    pub indexed: u32,
    pub skipped: u32,
    pub failed: u32,
    pub missing: u32,
    pub unsupported: u32,
    pub excluded: u32,
    pub too_large: u32,
    pub needs_ocr: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileResult {
    pub path: String,
    pub outcome: Outcome,
    pub chunks_added: u32,
    pub chunks_replaced: u32,
    pub status_detail: Option<String>,
    pub status_note: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Indexed,
    Skipped,
    Failed,
    Unsupported,
    Excluded,
    TooLarge,
    NeedsOcr,
    Missing,
}

impl Outcome {
    pub fn tally(&self, s: &mut IndexSummary) {
        match self {
            Outcome::Indexed => s.indexed += 1,
            Outcome::Skipped => s.skipped += 1,
            Outcome::Failed => s.failed += 1,
            Outcome::Missing => s.missing += 1,
            Outcome::Unsupported => s.unsupported += 1,
            Outcome::Excluded => s.excluded += 1,
            Outcome::TooLarge => s.too_large += 1,
            Outcome::NeedsOcr => s.needs_ocr += 1,
        }
    }
}

pub fn run_index(
    vault: &mut Vault,
    embedder: &dyn Embedder,
    extractors: &ExtractorRegistry,
    opts: &IndexOptions,
    progress: Option<&dyn Fn(usize, usize, &str)>,
) -> Result<IndexReport> {
    if embedder.dimension() != 1024 {
        return Err(Error::Invariant(format!(
            "embedder dimension is {} but schema expects 1024",
            embedder.dimension()
        )));
    }

    // Acquire vault-level file lock
    let lock_path = vault.index_lock_path();
    let lock_file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)?;

    if opts.no_wait {
        lock_file
            .try_lock_exclusive()
            .map_err(|_| Error::LockContention)?;
    } else {
        // We don't have a true bounded blocking lock from fs2; emulate with
        // try_lock_exclusive in a short loop bounded by `wait_seconds`.
        let deadline =
            std::time::Instant::now() + Duration::from_secs(opts.wait_seconds.unwrap_or(60));
        loop {
            match lock_file.try_lock_exclusive() {
                Ok(()) => break,
                Err(_) => {
                    if std::time::Instant::now() >= deadline {
                        return Err(Error::LockContention);
                    }
                    std::thread::sleep(Duration::from_millis(200));
                }
            }
        }
    }

    let started_at = chrono::Utc::now().timestamp_millis();

    // Snapshot the rows we'll process.
    let target_paths = match &opts.paths {
        Some(ps) => Some(
            ps.iter()
                .map(|p| vault.relativize(p).map(|r| r.to_string_lossy().to_string()))
                .collect::<Result<Vec<_>>>()?,
        ),
        None => None,
    };

    let mut rows = crate::registry::list_all(&vault.conn)?;
    if let Some(paths) = target_paths {
        let set: std::collections::HashSet<&str> = paths.iter().map(|s| s.as_str()).collect();
        rows.retain(|r| set.contains(r.path.as_str()));
    }

    let total = rows.len();
    let mut results: Vec<FileResult> = Vec::with_capacity(total);
    let mut summary = IndexSummary::default();

    for (i, row) in rows.iter().enumerate() {
        if let Some(p) = progress {
            p(i, total, &row.path);
        }
        let res = reconcile::process_one(vault, row, extractors, embedder, opts)?;
        res.outcome.tally(&mut summary);
        results.push(res);
    }

    if let Some(p) = progress {
        p(total, total, "");
    }

    let _ = FileExt::unlock(&lock_file);

    let completed_at = chrono::Utc::now().timestamp_millis();
    Ok(IndexReport {
        started_at,
        completed_at,
        duration_ms: completed_at - started_at,
        summary,
        results,
    })
}
