use super::{ExtractedDocument, ExtractionResult, Extractor};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct PandocExtractor {
    binary: PathBuf,
}

impl PandocExtractor {
    /// Construct iff `pandoc` is on PATH. If not, returns None and the index
    /// pipeline will treat docx/pdf as failed with `pandoc_not_found`.
    pub fn try_new() -> Option<Self> {
        which::which("pandoc").ok().map(|binary| Self { binary })
    }
}

impl Extractor for PandocExtractor {
    fn extensions(&self) -> &[&'static str] {
        &["docx", "pdf"]
    }

    fn extract(&self, path: &Path) -> ExtractionResult {
        let output = Command::new(&self.binary)
            .arg(path)
            .arg("-t")
            .arg("markdown")
            .arg("--wrap=none")
            .output();
        let output = match output {
            Ok(o) => o,
            Err(e) => {
                return ExtractionResult::Failed {
                    detail: "pandoc_spawn_failed".to_string(),
                    message: e.to_string(),
                };
            }
        };
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return ExtractionResult::Failed {
                detail: "pandoc_failed".to_string(),
                message: stderr,
            };
        }
        let markdown = String::from_utf8_lossy(&output.stdout).to_string();

        // Heuristic: a PDF that pandoc returns with extremely sparse text is
        // probably image-only and needs OCR.
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();
        if ext == "pdf" {
            let chars = markdown.chars().filter(|c| !c.is_whitespace()).count();
            // Crude page estimate: pandoc loses page boundaries when it
            // converts to markdown, so we approximate from input file size.
            let pages_est = (path
                .metadata()
                .map(|m| m.len() as usize / 5000)
                .unwrap_or(1))
            .max(1);
            if chars < 100 * pages_est {
                return ExtractionResult::NeedsOcr;
            }
        }

        ExtractionResult::Ok(ExtractedDocument {
            markdown,
            metadata: json!({}),
            page_boundaries: None,
        })
    }
}
