# ADR 0003: Pandoc for DOCX/EPUB; pdf-extract (with optional pdftotext) for PDF

## Status

Accepted. 2026-05-08.

## Context

`rag` must turn DOCX, EPUB, and PDF inputs into something its chunker can
consume (markdown). It also needs to detect image-only or
unrecoverable-encoding PDFs that have no extractable text, so they can be
marked `needs_ocr` instead of indexed as empty.

For DOCX and EPUB, the reference implementation across Unix toolchains is
Pandoc, and native Rust parsers have spotty coverage.

For PDF, pandoc cannot help: pandoc can write PDF but cannot read it. We
learned this the hard way in v0.1.0–v0.1.3. PDF extraction needs a
different tool.

## Decision

**DOCX and EPUB**: shell out to `pandoc` if it's on PATH. If pandoc is
missing, those rows get `failed` with detail `no_extractor_available`.

**PDF**: a dedicated `PdfExtractor` with a two-tier backend chosen at
construction time:
- If `pdftotext` (poppler) is on PATH → use it. High quality, robust on
  unusual font encodings and complex layouts.
- Otherwise → fall back to the pure-Rust `pdf-extract` crate. Works on
  most academic and textbook PDFs; some PDFs trigger panics inside the
  crate (encoding edge cases) which we catch and mark `failed` with a
  status_note suggesting the user install poppler.

Locate binaries via `which` at extractor-construction time.

## Consequences

**Why pandoc:** mature, ubiquitous, handles edge cases (tables, footnotes,
code blocks) better than any Rust crate available today. Output is markdown,
which the rest of our pipeline already speaks.

**Operational footprint:** users need to install pandoc separately. The README
calls this out. Markdown/plaintext vaults work without it.

**OCR detection heuristic:** for PDFs, after pandoc extraction we count
non-whitespace characters and divide by an estimated page count (file size /
5000). If the average is under 100 chars/page, we treat the file as image-only
and return `NeedsOcr`. This is intentionally crude — running OCR is out of
scope for v1; the goal is just to surface the right status so the user knows
why the file isn't searchable.

**Why a subprocess, not a library:** pandoc is Haskell. Embedding it would
mean shipping the Haskell runtime. The subprocess boundary is a feature here:
extraction crashes don't take down `rag index`.

**Failure modes:**
- pandoc not on PATH at construction → DOCX/PDF rows are `failed`
- pandoc exits non-zero → `failed` with `pandoc_failed` detail and stderr
  captured in `status_note`
- pandoc returns mostly-empty output for a PDF → `needs_ocr`

All of these are reachable via `rag status`/`rag info` and recoverable by
fixing the underlying cause.
