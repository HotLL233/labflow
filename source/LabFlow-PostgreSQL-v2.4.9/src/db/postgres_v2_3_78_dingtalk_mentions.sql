CREATE TABLE IF NOT EXISTS notification_dingtalk_contacts (
    user_id BIGINT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    mobile TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT to_char(CURRENT_TIMESTAMP,'YYYY-MM-DD HH24:MI:SS')
);
ALTER TABLE notification_rules ADD COLUMN IF NOT EXISTS dingtalk_mention_user_ids TEXT NOT NULL DEFAULT '[]';
ALTER TABLE notification_rules ADD COLUMN IF NOT EXISTS dingtalk_follow_recipients BIGINT NOT NULL DEFAULT 0;
INSERT INTO schema_migrations(version) VALUES ('2.3.78-dingtalk-mentions') ON CONFLICT DO NOTHING;
