ALTER TABLE users ADD COLUMN IF NOT EXISTS require_actual_submitter INTEGER NOT NULL DEFAULT 0;
INSERT INTO schema_migrations(version) VALUES ('2.4.13-manual-sample-submitter') ON CONFLICT DO NOTHING;
