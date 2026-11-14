CREATE TABLE IF NOT EXISTS ai_actions (
    id TEXT PRIMARY KEY,
    user_id BIGINT NOT NULL,
    config_revision BIGINT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','processing','completed','unknown')),
    payload_json TEXT NOT NULL,
    results_json TEXT NOT NULL DEFAULT '[]',
    worker_id TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP + INTERVAL '10 minutes'),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_ai_actions_user_created ON ai_actions(user_id,created_at DESC);
INSERT INTO schema_migrations(version) VALUES ('2.5.2-ai-confirmed-actions') ON CONFLICT DO NOTHING;
