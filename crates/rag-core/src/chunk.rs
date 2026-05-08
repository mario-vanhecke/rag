use crate::config::ChunkingConfig;
use crate::extract::PageBoundary;
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub ordinal: u32,
    pub content: String,
    pub content_hash: String,
    pub heading_path: Option<String>,
    pub page_number: Option<u32>,
    pub token_count: u32,
}

pub struct ChunkInput<'a> {
    pub markdown: &'a str,
    pub page_boundaries: Option<&'a [PageBoundary]>,
    pub document_title: Option<&'a str>,
}

/// Approximate token count (a divide-by-four heuristic; precise counts are the
/// embedder's concern).
pub fn token_estimate(s: &str) -> u32 {
    (s.len() as u32).div_ceil(4)
}

pub fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    let out = h.finalize();
    let mut s = String::with_capacity(64);
    for b in out {
        use std::fmt::Write;
        let _ = write!(s, "{:02x}", b);
    }
    s
}

#[derive(Debug)]
struct Section {
    heading_path: Option<String>,
    text: String,
    start_byte: usize,
}

/// Heading-aware chunker. Splits markdown into `##`/`###` sections, merges short
/// sections, splits long ones, applies overlap.
pub fn chunk(input: &ChunkInput, config: &ChunkingConfig) -> Vec<Chunk> {
    if input.markdown.trim().is_empty() {
        return Vec::new();
    }

    let sections = split_into_sections(input.markdown, input.document_title);
    let mut chunks: Vec<Chunk> = Vec::new();
    let mut ordinal: u32 = 0;
    let target = config.target_tokens.max(1);
    let max = config.max_tokens.max(target);
    let overlap = config.overlap_tokens;

    let mut prev_tail: Option<String> = None;

    for section in sections {
        let parts = split_section(&section.text, target, max);
        for part in parts {
            let mut content = String::new();
            if let Some(tail) = prev_tail.take() {
                content.push_str(&tail);
                if !content.ends_with('\n') {
                    content.push('\n');
                }
            }
            content.push_str(&part);

            let token_count = token_estimate(&content);
            let hash = sha256_hex(part.as_bytes());
            let page_number = input
                .page_boundaries
                .and_then(|pb| page_for_offset(pb, section.start_byte));

            chunks.push(Chunk {
                ordinal,
                content,
                content_hash: hash,
                heading_path: section.heading_path.clone(),
                page_number,
                token_count,
            });
            ordinal += 1;

            if overlap > 0 {
                prev_tail = Some(tail_n_tokens(&part, overlap));
            }
        }
    }

    chunks
}

fn split_into_sections(markdown: &str, document_title: Option<&str>) -> Vec<Section> {
    let parser = Parser::new(markdown);
    let mut sections: Vec<Section> = Vec::new();
    let mut stack: Vec<(u32, String)> = Vec::new();
    let mut current = Section {
        heading_path: document_title.map(|s| s.to_string()),
        text: String::new(),
        start_byte: 0,
    };
    let mut in_heading_at: Option<u32> = None;
    let mut heading_buf = String::new();

    for (event, range) in parser.into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                let lv = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                };
                in_heading_at = Some(lv);
                heading_buf.clear();
                if !current.text.trim().is_empty() {
                    sections.push(std::mem::replace(
                        &mut current,
                        Section {
                            heading_path: None,
                            text: String::new(),
                            start_byte: range.start,
                        },
                    ));
                }
                current.start_byte = range.start;
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some(lv) = in_heading_at.take() {
                    while stack.last().map(|(l, _)| *l >= lv).unwrap_or(false) {
                        stack.pop();
                    }
                    let cleaned = heading_buf.trim().to_string();
                    if !cleaned.is_empty() {
                        stack.push((lv, cleaned));
                    }
                    current.heading_path = Some(
                        stack
                            .iter()
                            .map(|(_, t)| t.as_str())
                            .collect::<Vec<_>>()
                            .join(" > "),
                    );
                }
            }
            Event::Text(t) | Event::Code(t) => {
                if in_heading_at.is_some() {
                    heading_buf.push_str(&t);
                } else {
                    current.text.push_str(&t);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                current.text.push('\n');
            }
            Event::End(TagEnd::Paragraph) => {
                current.text.push_str("\n\n");
            }
            Event::End(TagEnd::Item) => {
                current.text.push('\n');
            }
            _ => {}
        }
    }
    if !current.text.trim().is_empty() {
        sections.push(current);
    }
    if sections.is_empty() {
        // Fallback: chunker fed prose with no headings.
        sections.push(Section {
            heading_path: document_title.map(|s| s.to_string()),
            text: markdown.to_string(),
            start_byte: 0,
        });
    }
    sections
}

/// Split a section's text into pieces respecting target/max token sizes.
fn split_section(text: &str, target: u32, max: u32) -> Vec<String> {
    let total = token_estimate(text);
    if total <= max {
        return vec![text.to_string()];
    }
    let paragraphs: Vec<&str> = text
        .split("\n\n")
        .filter(|p| !p.trim().is_empty())
        .collect();
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    for p in paragraphs {
        let p_tokens = token_estimate(p);
        if p_tokens > max {
            // Hard-split the paragraph on sentence boundaries.
            for sent in split_sentences(p) {
                let s_tokens = token_estimate(sent);
                if token_estimate(&current) + s_tokens > max && !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
                if !current.is_empty() {
                    current.push(' ');
                }
                current.push_str(sent);
            }
            continue;
        }
        if token_estimate(&current) + p_tokens > target && !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push_str("\n\n");
        }
        current.push_str(p);
    }
    if !current.is_empty() {
        out.push(current);
    }
    if out.is_empty() {
        out.push(text.to_string());
    }
    out
}

fn split_sentences(p: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let bytes = p.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'.' || c == b'!' || c == b'?' {
            let end = i + 1;
            let trim = p[start..end].trim();
            if !trim.is_empty() {
                out.push(trim);
            }
            start = end;
        }
        i += 1;
    }
    if start < bytes.len() {
        let trim = p[start..].trim();
        if !trim.is_empty() {
            out.push(trim);
        }
    }
    if out.is_empty() {
        out.push(p);
    }
    out
}

fn tail_n_tokens(s: &str, n: u32) -> String {
    // n tokens ≈ 4n bytes
    let want = (n as usize) * 4;
    if s.len() <= want {
        return s.to_string();
    }
    // Round to a UTF-8 boundary
    let mut start = s.len() - want;
    while start > 0 && !s.is_char_boundary(start) {
        start -= 1;
    }
    s[start..].to_string()
}

fn page_for_offset(pb: &[PageBoundary], offset: usize) -> Option<u32> {
    pb.iter()
        .find(|b| offset >= b.start_offset && offset < b.end_offset)
        .map(|b| b.page)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> ChunkingConfig {
        ChunkingConfig {
            target_tokens: 50,
            max_tokens: 100,
            overlap_tokens: 5,
        }
    }

    #[test]
    fn simple_section_produces_one_chunk() {
        let md = "## Heading\n\nA short paragraph.";
        let chunks = chunk(
            &ChunkInput {
                markdown: md,
                page_boundaries: None,
                document_title: None,
            },
            &cfg(),
        );
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].heading_path.as_ref().unwrap().contains("Heading"));
    }

    #[test]
    fn multiple_headings_yield_multiple_chunks() {
        let md = "## A\n\nFirst.\n\n## B\n\nSecond.";
        let chunks = chunk(
            &ChunkInput {
                markdown: md,
                page_boundaries: None,
                document_title: None,
            },
            &cfg(),
        );
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].heading_path.as_ref().unwrap().contains('A'));
        assert!(chunks[1].heading_path.as_ref().unwrap().contains('B'));
    }
}
