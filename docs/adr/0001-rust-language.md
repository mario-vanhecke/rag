# ADR 0001: Rust as the implementation language

## Status

Accepted. 2026-05-08.

## Context

`rag` is a CLI invoked many times per workflow (add, status, index, search). It
must produce trustworthy retrieval state, hold the consistency invariant under
crashes, and ship as a single binary that a user can `curl` and run.

We considered Python, Go, and Rust.

## Decision

Use Rust (stable, edition 2021, MSRV 1.75).

## Consequences

**Why not Python:** the embedder loop wants tight control over batching and
memory, and the `cargo build --release` → static binary story is materially
better than freezing a Python interpreter. Cold-start latency (`rag --help` in
<100 ms per acceptance criterion 10) is hard to hit when Python pays import
overhead on every invocation.

**Why not Go:** Candle (the embedder backend) is Rust-native; the Go ML story
is currently weaker, especially for transformer inference on Metal/CUDA. SQLite
+ extensions and tight lifecycle management against `sqlite-vec` are also more
ergonomic in Rust.

**Why Rust:** typed errors with `thiserror`, transactional guarantees that
match the consistency invariant well, mature `rusqlite` + `sqlite-vec` story,
single static binary out of the box, and a Candle ecosystem that already ships
the BERT/XLM-RoBERTa models we need.

**Cost:** longer first-time build (Candle pulls a lot of code) and a steeper
contributor on-ramp than Python. We accept these because the tool's core
guarantees are easier to enforce in a typed, transactional language.
