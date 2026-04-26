# Migration Rollback Procedure

Automatic rollback is intentionally **not supported** to prevent accidental
data loss in production. Follow the manual steps below.

## Check current state

```bash
DATABASE_URL=postgres://... ./scripts/run_migrations.sh --status
```

## Rolling back a specific migration

### 1. Write a DOWN migration

Create a SQL file that reverses the changes made by the target migration.
Common patterns:

| Original operation | Rollback operation |
|---|---|
| `CREATE TABLE foo` | `DROP TABLE foo;` |
| `ALTER TABLE foo ADD COLUMN bar` | `ALTER TABLE foo DROP COLUMN bar;` |
| `CREATE INDEX idx_foo` | `DROP INDEX idx_foo;` |
| `CREATE EXTENSION pgcrypto` | `DROP EXTENSION pgcrypto;` |

### 2. Apply the DOWN migration manually

```bash
psql "$DATABASE_URL" -f your_down_migration.sql
```

### 3. Remove the version record

```sql
DELETE FROM schema_migrations WHERE version = '005';
```

### 4. Verify

```bash
./scripts/run_migrations.sh --status
```

The version will now appear as `(pending)` and will be re-applied on the
next `run_migrations.sh` run.

## Rolling back multiple versions

Repeat steps 1–4 in **reverse order** (highest version first).

## Emergency: drop and recreate schema

Only use this in a development/staging environment where data loss is acceptable.

```bash
psql "$DATABASE_URL" -c "DROP SCHEMA public CASCADE; CREATE SCHEMA public;"
./scripts/run_migrations.sh
```
