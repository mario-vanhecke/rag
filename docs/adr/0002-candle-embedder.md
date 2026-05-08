# ADR 0002: Candle as the embedding backend, bge-m3 as the default model

## Status

Accepted. 2026-05-08.

## Context

`rag` needs to produce dense embeddings for chunks at index time and for
queries at search time, on the user's machine, with no external service. It
must run on Apple Silicon (Metal) and Linux (CPU and CUDA), and ship as a
single binary.

## Decision

Use [Candle](https://github.com/huggingface/candle) (`candle-core`,
`candle-nn`, `candle-transformers`) for inference, with `BAAI/bge-m3` as the
default embedding model.

bge-m3 is XLM-RoBERTa-architectured (1024 dimensions, 8K max length —
truncated to 512 for inference cost). The embedder mean-pools the last hidden
states over the attention mask and L2-normalizes per row.

## Consequences

**Why Candle, not ONNX or llama.cpp:** Candle is Rust-native, links cleanly
into our binary, and supports both Metal and CUDA. ONNX would require shipping
a C++ runtime and resolving dynamic-linking pain on macOS. llama.cpp is
optimized for autoregressive LLM inference, not bidirectional encoder
embeddings.

**Why bge-m3:** strong multilingual retrieval, 1024-dim outputs (which sets
the dimension of `chunk_vectors`), permissive license, and direct compatibility
with sentence-transformers usage patterns Candle already implements. The
schema's `FLOAT[1024]` constant is locked to this dimension.

**The pytorch_model.bin reality:** the official `BAAI/bge-m3` repo on Hugging
Face only ships `pytorch_model.bin` (PyTorch pickle format, ~2.2 GB), not
safetensors. The loader probes for `.safetensors` first and falls back to
`pytorch_model.bin` via Candle's `VarBuilder::from_pth`. Switching to a
safetensors variant in the future is a config change.

**The download path:** `hf-hub` 0.3.x has a known bug where Hugging Face
returns a relative `Location:` header that ureq cannot parse as an absolute
URL. We sidestep this with a tiny direct downloader using `ureq` that resolves
relative redirects manually.

**Per-vault model cache:** weights live at `<vault>/.vault/cache/models/`. A
shared cache across vaults is explicit phase-2 work; for v1, vault
self-containedness wins over disk efficiency.

**Locked dimension:** `embedding.dimension` is a derived config key (read-only)
and `embedding.model` is mutable only when the chunks table is empty. Changing
the model in a non-empty vault requires `rag rm --all` first; this is enforced
at the config layer.
