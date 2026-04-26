-- Add indexes for newsletter subscriber queries
-- Improves performance for email and status-based lookups

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_newsletter_subscribers_status
ON newsletter_subscribers (confirmed, unsubscribed_at)
WHERE unsubscribed_at IS NULL;

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_newsletter_subscribers_email_status
ON newsletter_subscribers (email, confirmed)
WHERE unsubscribed_at IS NULL;
