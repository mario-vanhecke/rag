use super::{ExtractedDocument, ExtractionResult, Extractor};
use serde_json::json;
use std::path::Path;

pub struct PlaintextExtractor;

impl Extractor for PlaintextExtractor {
    fn extensions(&self) -> &[&'static str] {
        &["txt"]
    }

    fn extract(&self, path: &Path) -> ExtractionResult {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                return ExtractionResult::Failed {
                    detail: "io_error".to_string(),
                    message: e.to_string(),
                };
            }
        };
        let text = match String::from_utf8(bytes) {
            Ok(s) => s,
            Err(e) => {
                let lossy = String::from_utf8_lossy(e.as_bytes()).to_string();
                lossy
            }
        };
        ExtractionResult::Ok(ExtractedDocument {
            markdown: text,
            metadata: json!({}),
            page_boundaries: None,
        })
    }
}
