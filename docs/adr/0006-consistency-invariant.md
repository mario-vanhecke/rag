# ADR 0006: The consistency invariant — chunks exist iff status='indexed'

## Status

Accepted. 2026-05-08.

## Context

If `rag search` has to filter out chunks belonging to files in unhealthy
states (failed, missing, excluded), every query carries that complexity, and
the cost is paid at *read* time — the hot path. Worse, every future feature
has to remember to filter.

The alternative is to enforce, at every transaction boundary, that chunks
only exist for files in `indexed` state. Search becomes a clean lookup; the
complexity moves to a small, well-tested write path.

## Decision

> **Chunks exist in `chunks`, `chunk_vectors`, and `chunk_fts` if and only
> if their corresponding `files` row has `status = 'indexed'`.**

Practically:

1. `rag search` never filters by status. Every chunk in the index is from a
   successfully indexed file.

2. Every transition out of `indexed` (to `failed`, `missing`, `excluded`,
   etc.) deletes that file's chunks **in the same transaction** as the status
   update. This is true even if there were no chunks to begin with — we DELETE
   defensively.

3. A successful re-index of a modified file replaces chunks atomically:
   `DELETE old chunks; INSERT new chunks; UPDATE files row;` all in one
   `BEGIN ... COMMIT`.

4. Partial work never lands. Extraction → chunking → embedding all happen
   *before* the write transaction opens. If embedding fails (or the process
   is killed) before the transaction starts, no DB state changed. If a crash
   happens during the transaction, SQLite rolls back.

## Consequences

**The cascade enforces the invariant in schema:**

```sql
chunks: file_id REFERENCES files(id) ON DELETE CASCADE
trigger trg_chunks_after_delete: AFTER DELETE ON chunks → DELETE FROM
  chunk_vectors and chunk_fts WHERE chunk_id = OLD.id
```

Removing a `files` row drops chunks → trigger drops vectors and FTS rows.
Manual cleanup is never required.

**`rag info --check`** runs three queries: `vectors == chunks`, `fts ==
chunks`, and `every chunk's file is 'indexed'`. The integration test
`consistency.rs` verifies the invariant under five scenarios:

- normal index of multiple files
- file deletion → missing transition
- embedder failure mid-batch
- config change excluding a previously-indexed extension
- atomic chunk replacement on re-index (old chunk IDs are gone)

**Why not enforce at the application layer alone?** Schema-level cascade
turns the invariant into a property of the database, not a discipline of
the indexing code. A future bug in a new code path can't violate it without
also violating the foreign-key constraint, which fails loudly.

**Why DELETE first then INSERT?** SQLite's `INSERT OR REPLACE` on the chunks
table would conflict with the per-`(file_id, content_hash)` unique index in
ways that are harder to reason about than an explicit DELETE+INSERT pair.
Performance is fine: vault sizes in v1 are well within SQLite's comfortable
range.
