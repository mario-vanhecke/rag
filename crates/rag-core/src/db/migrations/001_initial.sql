-- Migration 001: initial schema
-- (foreign_keys / journal_mode are configured per-connection, not per-migration)

CREATE TABLE IF NOT EXISTS schema_migrations (
  version    INTEGER PRIMARY KEY,
  applied_at INTEGER NOT NULL
);

CREATE TABLE vault_meta (
  id           INTEGER PRIMARY KEY CHECK (id = 1),
  vault_id     TEXT NOT NULL,
  created_at   INTEGER NOT NULL,
  tool_version TEXT NOT NULL
);

CREATE TABLE settings (
  key        TEXT PRIMARY KEY,
  value      TEXT NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE files (
  id            INTEGER PRIMARY KEY,
  path          TEXT NOT NULL UNIQUE,
  added_at      INTEGER NOT NULL,
  status        TEXT NOT NULL,
  status_detail TEXT,
  status_note   TEXT,
  last_mtime    INTEGER,
  last_size     INTEGER,
  last_hash     TEXT,
  last_indexed  INTEGER,
  attempts      INTEGER NOT NULL DEFAULT 0,
  last_attempt  INTEGER
);

CREATE INDEX idx_files_status ON files(status);

CREATE TABLE chunks (
  id           TEXT PRIMARY KEY,
  file_id      INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
  ordinal      INTEGER NOT NULL,
  content      TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  heading_path TEXT,
  page_number  INTEGER,
  token_count  INTEGER NOT NULL,
  created_at   INTEGER NOT NULL
);

CREATE INDEX idx_chunks_file ON chunks(file_id, ordinal);
CREATE UNIQUE INDEX idx_chunks_file_hash ON chunks(file_id, content_hash);

CREATE VIRTUAL TABLE chunk_vectors USING vec0(
  chunk_id  TEXT PRIMARY KEY,
  embedding FLOAT[1024]
);

CREATE VIRTUAL TABLE chunk_fts USING fts5(
  chunk_id     UNINDEXED,
  content,
  heading_path,
  tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TRIGGER trg_chunks_after_delete
AFTER DELETE ON chunks
BEGIN
  DELETE FROM chunk_vectors WHERE chunk_id = OLD.id;
  DELETE FROM chunk_fts     WHERE chunk_id = OLD.id;
END;
