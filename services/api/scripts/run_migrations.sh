#!/usr/bin/env bash
# =============================================================================
# run_migrations.sh — Idempotent database migration runner with version tracking
#
# Usage:
#   ./scripts/run_migrations.sh [--status] [--rollback <version>]
#
# Options:
#   (none)               Run all pending migrations
#   --status             Print applied / pending migration state and exit
#   --rollback <ver>     Print rollback instructions for <ver> and exit (manual)
#
# Environment:
#   DATABASE_URL         PostgreSQL connection string (required)
#                        e.g. postgres://user:pass@localhost/predictiq
#
# How it works:
#   1. Ensures the schema_migrations tracking table exists (000_ migration).
#   2. For each *.sql file in migrations/ (sorted by filename):
#        a. Derives the version from the numeric prefix (e.g. "001").
#        b. Skips the file if that version is already recorded.
#        c. Computes a SHA-256 checksum of the file.
#        d. Runs the SQL inside a transaction.
#        e. Records the version + checksum in schema_migrations.
#   3. Exits non-zero on any failure; the transaction is rolled back automatically.
# =============================================================================
set -euo pipefail

MIGRATIONS_DIR="$(cd "$(dirname "$0")/../database/migrations" && pwd)"
DATABASE_URL="${DATABASE_URL:-postgres://postgres:postgres@localhost/predictiq}"

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
log()  { echo "[migrations] $*"; }
err()  { echo "[migrations] ERROR: $*" >&2; }
die()  { err "$*"; exit 1; }

require_psql() {
    command -v psql >/dev/null 2>&1 || die "psql not found — install postgresql-client"
}

require_sha256() {
    if command -v sha256sum >/dev/null 2>&1; then
        SHA256_CMD="sha256sum"
    elif command -v shasum >/dev/null 2>&1; then
        SHA256_CMD="shasum -a 256"
    else
        die "No SHA-256 tool found (sha256sum or shasum)"
    fi
}

checksum_file() {
    $SHA256_CMD "$1" | awk '{print $1}'
}

psql_exec() {
    psql "$DATABASE_URL" --no-psqlrc -v ON_ERROR_STOP=1 "$@"
}

# ---------------------------------------------------------------------------
# Ensure tracking table exists
# ---------------------------------------------------------------------------
ensure_tracking_table() {
    local bootstrap="$MIGRATIONS_DIR/000_create_schema_migrations.sql"
    [[ -f "$bootstrap" ]] || die "Bootstrap migration not found: $bootstrap"
    log "Ensuring schema_migrations table exists..."
    psql_exec -f "$bootstrap" -q
}

# ---------------------------------------------------------------------------
# Check if a version is already applied
# ---------------------------------------------------------------------------
is_applied() {
    local version="$1"
    local count
    count=$(psql_exec -t -c \
        "SELECT COUNT(*) FROM schema_migrations WHERE version = '$version';" \
        2>/dev/null | tr -d '[:space:]')
    [[ "$count" == "1" ]]
}

# ---------------------------------------------------------------------------
# Apply a single migration file inside a transaction
# ---------------------------------------------------------------------------
apply_migration() {
    local file="$1"
    local version="$2"
    local name
    name="$(basename "$file" .sql)"
    local checksum
    checksum="$(checksum_file "$file")"

    log "Applying [$version] $name ..."

    # Wrap in a transaction so a partial failure rolls back cleanly
    psql_exec -q <<SQL
BEGIN;
\i $file
INSERT INTO schema_migrations (version, name, applied_at, checksum)
VALUES ('$version', '$name', NOW(), '$checksum');
COMMIT;
SQL

    log "  ✓ [$version] applied (checksum: ${checksum:0:12}...)"
}

# ---------------------------------------------------------------------------
# --status: print applied / pending state
# ---------------------------------------------------------------------------
cmd_status() {
    ensure_tracking_table
    log "Migration status:"
    echo ""
    printf "  %-8s %-50s %-26s %s\n" "VERSION" "NAME" "APPLIED_AT" "CHECKSUM"
    printf "  %-8s %-50s %-26s %s\n" "-------" "----" "----------" "--------"

    local any_pending=false
    for file in $(ls "$MIGRATIONS_DIR"/*.sql 2>/dev/null | sort); do
        local fname version name
        fname="$(basename "$file")"
        version="${fname%%_*}"
        name="${fname%.sql}"

        if is_applied "$version"; then
            local row
            row=$(psql_exec -t -c \
                "SELECT applied_at, checksum FROM schema_migrations WHERE version = '$version';" \
                | tr -d '[:space:]')
            local applied_at="${row%%|*}"
            local chk="${row##*|}"
            printf "  %-8s %-50s %-26s %s\n" "$version" "$name" "$applied_at" "${chk:0:12}..."
        else
            printf "  %-8s %-50s %-26s %s\n" "$version" "$name" "(pending)" "-"
            any_pending=true
        fi
    done

    echo ""
    if $any_pending; then
        log "There are pending migrations. Run without --status to apply them."
    else
        log "All migrations are up to date."
    fi
}

# ---------------------------------------------------------------------------
# --rollback: print rollback instructions (manual process)
# ---------------------------------------------------------------------------
cmd_rollback() {
    local target_version="$1"
    [[ -n "$target_version" ]] || die "--rollback requires a version argument (e.g. --rollback 005)"

    ensure_tracking_table

    if ! is_applied "$target_version"; then
        die "Version $target_version is not in schema_migrations — nothing to roll back."
    fi

    local name
    name=$(psql_exec -t -c \
        "SELECT name FROM schema_migrations WHERE version = '$target_version';" \
        | tr -d '[:space:]')

    cat <<EOF

[migrations] Rollback instructions for version $target_version ($name):

  Automatic rollback is intentionally NOT supported to prevent accidental
  data loss in production. Follow these manual steps:

  1. Write a new DOWN migration SQL file that reverses the changes made by
     $name.sql (e.g. DROP TABLE, ALTER TABLE ... DROP COLUMN, etc.).

  2. Apply it manually:
       psql "\$DATABASE_URL" -f your_down_migration.sql

  3. Remove the version record from the tracking table:
       psql "\$DATABASE_URL" -c \\
         "DELETE FROM schema_migrations WHERE version = '$target_version';"

  4. Verify state:
       $0 --status

EOF
}

# ---------------------------------------------------------------------------
# Main: run all pending migrations
# ---------------------------------------------------------------------------
cmd_run() {
    ensure_tracking_table

    local files
    files=$(ls "$MIGRATIONS_DIR"/*.sql 2>/dev/null | sort)

    if [[ -z "$files" ]]; then
        log "No migration files found in $MIGRATIONS_DIR"
        exit 0
    fi

    local applied=0
    local skipped=0

    for file in $files; do
        local fname version
        fname="$(basename "$file")"
        version="${fname%%_*}"

        if is_applied "$version"; then
            log "Skipping [$version] — already applied"
            (( skipped++ )) || true
        else
            apply_migration "$file" "$version"
            (( applied++ )) || true
        fi
    done

    echo ""
    log "Done. Applied: $applied  Skipped (already up to date): $skipped"
}

# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------
require_psql
require_sha256

case "${1:-}" in
    --status)
        cmd_status
        ;;
    --rollback)
        cmd_rollback "${2:-}"
        ;;
    "")
        cmd_run
        ;;
    *)
        die "Unknown option: $1  (use --status or --rollback <version>)"
        ;;
esac
