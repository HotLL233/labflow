CREATE TABLE IF NOT EXISTS notification_rule_targets (
    rule_id BIGINT NOT NULL REFERENCES notification_rules(id) ON DELETE CASCADE,
    target_kind TEXT NOT NULL CHECK(target_kind IN ('rd_work_record','sample_info')),
    sample_info_type_key TEXT NOT NULL DEFAULT '',
    PRIMARY KEY(rule_id,target_kind,sample_info_type_key)
);

CREATE INDEX IF NOT EXISTS idx_notification_rule_targets_match
    ON notification_rule_targets(target_kind,sample_info_type_key);

-- Preserve beta.6 semantics when upgrading existing rules. An empty historical
-- type meant that the rule covered both submission sources.
INSERT INTO notification_rule_targets(rule_id,target_kind,sample_info_type_key)
SELECT id,'sample_info',sample_info_type_key
FROM notification_rules
WHERE COALESCE(sample_info_type_key,'')<>''
ON CONFLICT DO NOTHING;

INSERT INTO notification_rule_targets(rule_id,target_kind,sample_info_type_key)
SELECT id,'rd_work_record',''
FROM notification_rules
WHERE COALESCE(sample_info_type_key,'')=''
ON CONFLICT DO NOTHING;

INSERT INTO notification_rule_targets(rule_id,target_kind,sample_info_type_key)
SELECT id,'sample_info',''
FROM notification_rules
WHERE COALESCE(sample_info_type_key,'')=''
ON CONFLICT DO NOTHING;

INSERT INTO schema_migrations(version) VALUES ('1.1.6-beta.7-notification-targets') ON CONFLICT DO NOTHING;
