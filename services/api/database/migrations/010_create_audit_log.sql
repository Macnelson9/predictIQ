-- Create audit log table for tracking admin operations
CREATE TABLE IF NOT EXISTS audit_log (
    id BIGSERIAL PRIMARY KEY,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    actor VARCHAR(255) NOT NULL,
    actor_ip INET,
    action VARCHAR(100) NOT NULL,
    resource_type VARCHAR(50) NOT NULL,
    resource_id VARCHAR(255),
    details JSONB,
    status VARCHAR(20) NOT NULL DEFAULT 'success',
    error_message TEXT,
    request_id UUID,
    user_agent TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for efficient querying
CREATE INDEX idx_audit_log_timestamp ON audit_log(timestamp DESC);
CREATE INDEX idx_audit_log_actor ON audit_log(actor);
CREATE INDEX idx_audit_log_action ON audit_log(action);
CREATE INDEX idx_audit_log_resource ON audit_log(resource_type, resource_id);
CREATE INDEX idx_audit_log_request_id ON audit_log(request_id);

-- Create a function to prevent updates/deletes (append-only)
CREATE OR REPLACE FUNCTION prevent_audit_log_modification()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Audit log is append-only. Modifications are not allowed.';
END;
$$ LANGUAGE plpgsql;

-- Create triggers to enforce append-only
CREATE TRIGGER prevent_audit_log_update
    BEFORE UPDATE ON audit_log
    FOR EACH ROW
    EXECUTE FUNCTION prevent_audit_log_modification();

CREATE TRIGGER prevent_audit_log_delete
    BEFORE DELETE ON audit_log
    FOR EACH ROW
    EXECUTE FUNCTION prevent_audit_log_modification();

-- Create a view for recent admin actions
CREATE OR REPLACE VIEW recent_admin_actions AS
SELECT 
    id,
    timestamp,
    actor,
    action,
    resource_type,
    resource_id,
    status,
    error_message
FROM audit_log
WHERE timestamp > NOW() - INTERVAL '30 days'
ORDER BY timestamp DESC;

-- Grant appropriate permissions
-- GRANT SELECT ON audit_log TO readonly_user;
-- GRANT INSERT ON audit_log TO api_user;
-- GRANT SELECT ON recent_admin_actions TO readonly_user;

COMMENT ON TABLE audit_log IS 'Append-only audit log for tracking all admin operations';
COMMENT ON COLUMN audit_log.actor IS 'Username or API key identifier of the actor';
COMMENT ON COLUMN audit_log.action IS 'Action performed (e.g., resolve_market, send_email, update_config)';
COMMENT ON COLUMN audit_log.resource_type IS 'Type of resource affected (e.g., market, email, config)';
COMMENT ON COLUMN audit_log.resource_id IS 'Identifier of the affected resource';
COMMENT ON COLUMN audit_log.details IS 'Additional context about the operation (JSON)';
COMMENT ON COLUMN audit_log.status IS 'Operation status: success, failure, partial';
