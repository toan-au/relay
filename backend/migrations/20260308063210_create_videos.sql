CREATE EXTENSION IF NOT EXISTS "pgcrypto";

CREATE TABLE videos (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    share_token  TEXT UNIQUE NOT NULL,
    status       TEXT NOT NULL DEFAULT 'processing',
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at   TIMESTAMPTZ NOT NULL DEFAULT now() + INTERVAL '30 days'
);

CREATE INDEX idx_videos_share_token ON videos(share_token);

