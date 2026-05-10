use super::{ExtractedDocument, ExtractionResult, Extractor};
use serde_json::{json, Value};
use std::path::Path;

pub struct MarkdownExtractor;

impl Extractor for MarkdownExtractor {
    fn extensions(&self) -> &[&'static str] {
        &["md", "markdown"]
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
            Err(_) => {
                return ExtractionResult::Failed {
                    detail: "not_utf8".to_string(),
                    message: format!("file is not valid UTF-8: {}", path.display()),
                };
            }
        };
        let (metadata, body) = strip_frontmatter(&text);
        ExtractionResult::Ok(ExtractedDocument {
            markdown: body.to_string(),
            metadata,
            page_boundaries: None,
        })
    }
}

/// Strip a YAML frontmatter block delimited by leading `---` lines. Stored as a
/// JSON object whose keys are the YAML keys; values are kept as raw strings
/// (no full YAML parser is in our dependency budget).
fn strip_frontmatter(input: &str) -> (Value, &str) {
    if !input.starts_with("---") {
        return (json!({}), input);
    }
    let after_open = &input[3..];
    // Find the closing fence on its own line
    let rest = after_open.trim_start_matches('\n');
    if let Some(pos) = rest.find("\n---") {
        let yaml = &rest[..pos];
        let after_close = &rest[pos + 4..];
        // strip up to one newline after fence
        let body = after_close.strip_prefix('\n').unwrap_or(after_close);
        let mut obj = serde_json::Map::new();
        for line in yaml.lines() {
            if let Some((k, v)) = line.split_once(':') {
                obj.insert(
                    k.trim().to_string(),
                    Value::String(v.trim().trim_matches('"').to_string()),
                );
            }
        }
        return (Value::Object(obj), body);
    }
    (json!({}), input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_parsed() {
        let s = "---\ntitle: foo\nauthor: bar\n---\n# body";
        let (m, b) = strip_frontmatter(s);
        assert_eq!(m["title"], "foo");
        assert_eq!(m["author"], "bar");
        assert_eq!(b, "# body");
    }

    #[test]
    fn no_frontmatter_passes_through() {
        let s = "# hello";
        let (m, b) = strip_frontmatter(s);
        assert!(m.as_object().unwrap().is_empty());
        assert_eq!(b, s);
    }
}
