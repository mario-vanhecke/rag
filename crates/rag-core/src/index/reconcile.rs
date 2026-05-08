use super::pipeline::{write_indexed_content, write_status_only};
use super::{FileResult, IndexOptions, Outcome};
use crate::chunk::sha256_hex;
use crate::embed::Embedder;
use crate::error::Result;
use crate::extract::{ExtractionResult, ExtractorRegistry};
use crate::registry::{FileRow, FileStatus};
use crate::vault::Vault;
use std::path::Path;

pub fn process_one(
    vault: &Vault,
    row: &FileRow,
    extractors: &ExtractorRegistry,
    embedder: &dyn Embedder,
    opts: &IndexOptions,
) -> Result<FileResult> {
    let abs = vault.absolutize(&row.path);
    let meta = std::fs::metadata(&abs);

    let meta = match meta {
        Ok(m) => m,
        Err(_) => {
            write_status_only(
                vault,
                row,
                FileStatus::Missing,
                None,
                None,
                row.status != FileStatus::Missing,
            )?;
            return Ok(FileResult {
                path: row.path.clone(),
                outcome: Outcome::Missing,
                chunks_added: 0,
                chunks_replaced: 0,
                status_detail: None,
                status_note: None,
            });
        }
    };
    if !meta.is_file() {
        write_status_only(
            vault,
            row,
            FileStatus::Failed,
            Some("path_not_a_file"),
            Some("registered path is not a regular file"),
            true,
        )?;
        return Ok(FileResult {
            path: row.path.clone(),
            outcome: Outcome::Failed,
            chunks_added: 0,
            chunks_replaced: 0,
            status_detail: Some("path_not_a_file".to_string()),
            status_note: Some("registered path is not a regular file".to_string()),
        });
    }

    let ext = Path::new(&row.path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    if !vault
        .config
        .files
        .supported_extensions
        .iter()
        .any(|s| s.eq_ignore_ascii_case(&ext))
    {
        write_status_only(
            vault,
            row,
            FileStatus::Unsupported,
            Some("extension_not_supported"),
            None,
            row.status == FileStatus::Indexed,
        )?;
        return Ok(FileResult {
            path: row.path.clone(),
            outcome: Outcome::Unsupported,
            chunks_added: 0,
            chunks_replaced: 0,
            status_detail: Some("extension_not_supported".to_string()),
            status_note: None,
        });
    }

    if vault
        .config
        .files
        .excluded_extensions
        .iter()
        .any(|s| s.eq_ignore_ascii_case(&ext))
    {
        write_status_only(
            vault,
            row,
            FileStatus::Excluded,
            Some("extension_excluded_by_config"),
            None,
            row.status == FileStatus::Indexed,
        )?;
        return Ok(FileResult {
            path: row.path.clone(),
            outcome: Outcome::Excluded,
            chunks_added: 0,
            chunks_replaced: 0,
            status_detail: Some("extension_excluded_by_config".to_string()),
            status_note: None,
        });
    }

    if meta.len() > vault.config.files.size_cap_bytes {
        write_status_only(
            vault,
            row,
            FileStatus::TooLarge,
            Some("size_exceeds_cap"),
            None,
            row.status == FileStatus::Indexed,
        )?;
        return Ok(FileResult {
            path: row.path.clone(),
            outcome: Outcome::TooLarge,
            chunks_added: 0,
            chunks_replaced: 0,
            status_detail: Some("size_exceeds_cap".to_string()),
            status_note: None,
        });
    }

    let mtime_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64);
    let size = meta.len() as i64;

    if row.status == FileStatus::Indexed
        && !opts.force
        && row.last_mtime == mtime_ms
        && row.last_size == Some(size)
    {
        return Ok(FileResult {
            path: row.path.clone(),
            outcome: Outcome::Skipped,
            chunks_added: 0,
            chunks_replaced: 0,
            status_detail: None,
            status_note: None,
        });
    }

    if row.status == FileStatus::Failed && !opts.retry_failed && !opts.force {
        return Ok(FileResult {
            path: row.path.clone(),
            outcome: Outcome::Skipped,
            chunks_added: 0,
            chunks_replaced: 0,
            status_detail: row.status_detail.clone(),
            status_note: row.status_note.clone(),
        });
    }

    // Processing path.
    let extractor = match extractors.for_extension(&ext) {
        Some(e) => e.clone(),
        None => {
            // Supported extension but no extractor wired (e.g. pandoc missing
            // for docx/pdf). Mark failed with a clear detail.
            write_status_only(
                vault,
                row,
                FileStatus::Failed,
                Some("no_extractor_available"),
                Some(&format!("no extractor registered for .{}", ext)),
                row.status == FileStatus::Indexed,
            )?;
            return Ok(FileResult {
                path: row.path.clone(),
                outcome: Outcome::Failed,
                chunks_added: 0,
                chunks_replaced: 0,
                status_detail: Some("no_extractor_available".to_string()),
                status_note: Some(format!("no extractor registered for .{}", ext)),
            });
        }
    };

    let result = extractor.extract(&abs);
    match result {
        ExtractionResult::NeedsOcr => {
            write_status_only(
                vault,
                row,
                FileStatus::NeedsOcr,
                Some("no_extractable_text"),
                None,
                row.status == FileStatus::Indexed,
            )?;
            Ok(FileResult {
                path: row.path.clone(),
                outcome: Outcome::NeedsOcr,
                chunks_added: 0,
                chunks_replaced: 0,
                status_detail: Some("no_extractable_text".to_string()),
                status_note: None,
            })
        }
        ExtractionResult::Failed { detail, message } => {
            write_status_only(
                vault,
                row,
                FileStatus::Failed,
                Some(&detail),
                Some(&message),
                row.status == FileStatus::Indexed,
            )?;
            Ok(FileResult {
                path: row.path.clone(),
                outcome: Outcome::Failed,
                chunks_added: 0,
                chunks_replaced: 0,
                status_detail: Some(detail),
                status_note: Some(message),
            })
        }
        ExtractionResult::Ok(extracted) => {
            let title = Path::new(&row.path)
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string());
            let chunks = crate::chunk::chunk(
                &crate::chunk::ChunkInput {
                    markdown: &extracted.markdown,
                    page_boundaries: extracted.page_boundaries.as_deref(),
                    document_title: title.as_deref(),
                },
                &vault.config.chunking,
            );
            if chunks.is_empty() {
                write_status_only(
                    vault,
                    row,
                    FileStatus::Failed,
                    Some("no_chunks_produced"),
                    Some("extraction returned content but chunker produced no chunks"),
                    row.status == FileStatus::Indexed,
                )?;
                return Ok(FileResult {
                    path: row.path.clone(),
                    outcome: Outcome::Failed,
                    chunks_added: 0,
                    chunks_replaced: 0,
                    status_detail: Some("no_chunks_produced".to_string()),
                    status_note: Some(
                        "extraction returned content but chunker produced no chunks".to_string(),
                    ),
                });
            }

            let texts: Vec<&str> = chunks.iter().map(|c| c.content.as_str()).collect();
            let embeddings = match embedder.embed_batch(&texts) {
                Ok(e) => e,
                Err(e) => {
                    write_status_only(
                        vault,
                        row,
                        FileStatus::Failed,
                        Some("embedding_error"),
                        Some(&e.to_string()),
                        row.status == FileStatus::Indexed,
                    )?;
                    return Ok(FileResult {
                        path: row.path.clone(),
                        outcome: Outcome::Failed,
                        chunks_added: 0,
                        chunks_replaced: 0,
                        status_detail: Some("embedding_error".to_string()),
                        status_note: Some(e.to_string()),
                    });
                }
            };

            // Hash the on-disk file as documentary evidence of what was indexed.
            let bytes = std::fs::read(&abs)?;
            let content_hash = sha256_hex(&bytes);

            let chunks_added = chunks.len() as u32;
            let chunks_replaced = if row.status == FileStatus::Indexed {
                count_existing_chunks(vault, row.id)?
            } else {
                0
            };

            write_indexed_content(
                vault,
                row,
                &chunks,
                &embeddings,
                mtime_ms,
                size,
                &content_hash,
            )?;

            Ok(FileResult {
                path: row.path.clone(),
                outcome: Outcome::Indexed,
                chunks_added,
                chunks_replaced,
                status_detail: None,
                status_note: None,
            })
        }
    }
}

fn count_existing_chunks(vault: &Vault, file_id: i64) -> Result<u32> {
    let n: i64 = vault.conn.query_row(
        "SELECT COUNT(*) FROM chunks WHERE file_id = ?1",
        rusqlite::params![file_id],
        |r| r.get(0),
    )?;
    Ok(n as u32)
}
