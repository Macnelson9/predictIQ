-- Add soft delete support for newsletter subscribers
-- Adds deleted_at timestamp for GDPR compliance and audit trails

ALTER TABLE newsletter_subscribers ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ;

-- Create index for soft-deleted records
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_newsletter_subscribers_deleted_at
ON newsletter_subscribers (deleted_at)
WHERE deleted_at IS NOT NULL;

-- Create index for active records (not deleted)
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_newsletter_subscribers_active
ON newsletter_subscribers (email)
WHERE deleted_at IS NULL;
