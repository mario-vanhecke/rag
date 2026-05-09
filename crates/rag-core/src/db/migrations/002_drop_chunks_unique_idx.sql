-- Migration 002: drop the (file_id, content_hash) unique index on chunks.
--
-- The index was meant as defense against double-inserting the same chunk,
-- but it's overly restrictive: real documents (especially books) contain
-- legitimate duplicate content within a single file (e.g. repeated chapter
-- separators, empty section headers, recurring boilerplate). Two chunks
-- with identical text → identical SHA-256 → INSERT failure.
--
-- The chunk's actual identity is its UUIDv7 `id` (PRIMARY KEY, always
-- unique). Removing this index lets duplicates coexist with their distinct
-- ordinals/heading_paths preserved.

DROP INDEX IF EXISTS idx_chunks_file_hash;
