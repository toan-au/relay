CREATE TABLE admin_sessions (
    token       TEXT PRIMARY KEY,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at  TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_admin_sessions_expires_at ON admin_sessions(expires_at);
