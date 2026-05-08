# ADR 0003: Pandoc subprocess for DOCX/PDF extraction

## Status

Accepted. 2026-05-08.

## Context

`rag` must turn DOCX and PDF inputs into something its chunker can consume
(markdown). It also needs to detect image-only PDFs that have no extractable
text, so they can be marked `needs_ocr` instead of indexed as empty.

Native Rust DOCX/PDF parsers exist but have spotty coverage. The reference
implementation across Unix toolchains is Pandoc.

## Decision

Shell out to `pandoc` for `.docx` and `.pdf`. Locate the binary via `which`
at extractor-construction time. If pandoc is missing, the extractor is not
registered and any DOCX/PDF row gets `failed` with detail
`no_extractor_available`.

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
