-- Migration version tracking table
-- Must be applied first (filename 000_) before any other migrations.
-- Running this file twice is safe: CREATE TABLE IF NOT EXISTS is idempotent.

CREATE TABLE IF NOT EXISTS schema_migrations (
    version     TEXT        NOT NULL PRIMARY KEY,   -- e.g. "001", "002"
    name        TEXT        NOT NULL,               -- human-readable filename stem
    applied_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    checksum    TEXT        NOT NULL                -- SHA-256 of the SQL file contents
);

COMMENT ON TABLE schema_migrations IS
    'Tracks which migration files have been applied and their checksums.';
