# ADR 0004: A vault is a single SQLite database

## Status

Accepted. 2026-05-08.

## Context

A vault has identity, configuration, a registry of files, chunks, vector
embeddings, and a full-text index. We could store these as separate files
(JSON manifests, sidecar `.npy` arrays, a SQLite db just for FTS, etc.), or
as a single SQLite database.

## Decision

Everything that defines a vault — `vault_meta`, `settings`, `files`, `chunks`,
`chunk_vectors` (sqlite-vec virtual table), `chunk_fts` (FTS5 virtual table) —
lives in one SQLite file at `<vault>/.vault/vault`. Filesystem state outside
the database is content (the user's documents) and user-authored input
(`.vaultignore`).

## Consequences

**One artifact, one transaction boundary.** This is what makes the consistency
invariant (ADR 0006) possible. Replacing chunks atomically means dropping
old chunks and inserting new ones in a single `BEGIN ... COMMIT` block.
Splitting state across files would make this fragile (filesystem crashes
mid-write, partial writes, recovery scripts).

**Backups are `cp .vault/vault backup.sqlite`.** No manifest reconciliation,
no orphaned files, no "did the JSON match the index?" questions.

**Tooling.** `sqlite3 .vault/vault` lets a curious user inspect or repair the
vault. The schema is documented in the README.

**Why not Postgres:** wrong shape — we need an embedded, single-file,
zero-admin database. SQLite + WAL is exactly that. Postgres would mean
running a server.

**Why not LanceDB / Chroma / Qdrant:** they're servers or libraries with
opinions about deployment we don't share. SQLite + sqlite-vec keeps us
embeddable, free of service dependencies, and lets us reuse the same
transaction for vectors, FTS, and registry rows.

**Cost:** SQLite limits the practical vault size (sqlite-vec scans are
not approximate-nearest-neighbour). For phase 1, vaults of tens of thousands
of chunks are fine; larger workloads are explicit phase-2 territory.
