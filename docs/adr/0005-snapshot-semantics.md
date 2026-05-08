# ADR 0005: `rag add` is a snapshot, not a watch

## Status

Accepted. 2026-05-08.

## Context

When a user runs `rag add docs/`, what should happen to files added to `docs/`
later? Two reasonable choices: (a) record the *directory* as a tracked source
and re-walk it on each `rag index`; (b) walk *now*, register each matching
file individually, and require a fresh `rag add` for new files.

## Decision

`rag add` walks the filesystem at invocation time and writes one row per
matching file. Files added to that directory afterwards are not auto-tracked.

## Consequences

**Vault membership is explicit.** The registry is a snapshot: `rag ls` answers
"what's in this vault?" precisely. There are no surprise files appearing
between commits.

**`rag status --show-untracked`** is the escape hatch: it walks the vault root
and reports files on disk that aren't in the registry, so a user who *did*
add new files can discover them and decide whether to register.

**Composability with cron / git hooks.** A pre-commit hook running
`rag add . && rag index` does the right thing every time without surprising
side effects from yesterday's run.

**No move/rename detection.** Renaming a file is `rag rm <old>` +
`rag add <new>`. Hash-based rename detection is phase-2 work and would
complicate the model for marginal value (the search index is the same
chunks either way).

**Cost:** users must remember to re-add. We accept this. The alternative —
auto-discovery — turns `rag` into a stateful watcher with hidden behavior;
that violates the "runs to completion on every invocation" principle.
