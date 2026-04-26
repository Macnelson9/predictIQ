# Database Schema Documentation

This service uses PostgreSQL. Schema and seed scripts are in:

- `services/api/database/migrations/`
- `services/api/database/seeds/`

## Tables

- `newsletter_subscribers` — email opt-in list with double-opt-in confirmation
- `contact_form_submissions`
- `waitlist_entries`
- `analytics_events`
- `content_management`
- `audit_logs` — general audit trail (UUID primary key)
- `audit_log` — append-only admin-operation audit log (bigserial primary key)
- `email_jobs` — async email queue tracking

## Migration Files

1. `001_enable_pgcrypto.sql`
2. `002_create_newsletter_subscriptions.sql` — creates `newsletter_subscribers` table
3. `003_create_contact_form_submissions.sql`
4. `004_create_waitlist_entries.sql`
5. `005_create_content_management.sql`
6. `006_create_analytics_events.sql`
7. `007_create_audit_logs.sql`
8. `008_create_email_tracking.sql`
9. `009_add_newsletter_indexes.sql` — performance indexes on `newsletter_subscribers`
10. `010_create_audit_log.sql` — append-only `audit_log` table for admin operations
11. `010_add_soft_delete_newsletter.sql` — adds `deleted_at` to `newsletter_subscribers`

> **Note:** Two migration files share the `010_` prefix. Apply them in lexicographic
> order (`010_add_soft_delete_newsletter.sql` before `010_create_audit_log.sql`) or
> rename one to `011_` to avoid ambiguity with migration runners that sort by filename.

## Apply Migrations

Run from the workspace root:

```bash
for f in services/api/database/migrations/*.sql; do
  psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f "$f"
done
```

Or use the provided script:

```bash
bash services/api/scripts/run_migrations.sh
```

## Rollback

This repository uses forward-only SQL migrations. For rollback:

- Write explicit reverse scripts before production rollout.
- Restore from backup/snapshot for emergency rollback.

## Seeding

```bash
for f in services/api/database/seeds/*.sql; do
  psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -f "$f"
done
```

## Backup Strategy

- Daily logical backups with `pg_dump`, 30-day retention.
- Weekly full snapshot, 90-day retention.
- Quarterly restore drills in staging.
- Encrypt backup storage at rest.

## Data Retention Policy

- `analytics_events`: 13 months raw, then archive/aggregate.
- `audit_logs` / `audit_log`: 24 months minimum for compliance.
- `contact_form_submissions`: 12 months unless legal hold.
- `newsletter_subscribers` / `waitlist_entries`: retain active records; hard-delete on GDPR request.

## Notes

- UUID primary keys via `gen_random_uuid()` (most tables); `audit_log` uses `BIGSERIAL`.
- All tables include `created_at` / `updated_at` timestamps.
- Soft deletes via `deleted_at` in `content_management`, `audit_logs`, and `newsletter_subscribers`.
- Indexes on high-frequency query fields (`email`, `status`, `created_at`).
