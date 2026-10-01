-- v1.1.6-beta.6: global role data scopes and multi-laboratory notification rules.
CREATE TABLE IF NOT EXISTS role_division_scopes (
  role_id BIGINT NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
  division_id BIGINT NOT NULL REFERENCES divisions(id) ON DELETE CASCADE,
  created_at TEXT NOT NULL DEFAULT to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS'),
  PRIMARY KEY (role_id, division_id)
);

CREATE TABLE IF NOT EXISTS role_sample_info_type_scopes (
  role_id BIGINT NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
  type_key TEXT NOT NULL REFERENCES sample_info_types(type_key) ON UPDATE CASCADE ON DELETE CASCADE,
  created_at TEXT NOT NULL DEFAULT to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS'),
  PRIMARY KEY (role_id, type_key)
);

CREATE TABLE IF NOT EXISTS notification_rule_groups (
  rule_id BIGINT NOT NULL REFERENCES notification_rules(id) ON DELETE CASCADE,
  group_id BIGINT NOT NULL REFERENCES project_groups(id) ON DELETE CASCADE,
  created_at TEXT NOT NULL DEFAULT to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS'),
  PRIMARY KEY (rule_id, group_id)
);

-- Preserve the meaning of existing single-laboratory rules before the UI starts writing multi-select values.
INSERT INTO notification_rule_groups(rule_id, group_id)
SELECT id, group_id FROM notification_rules WHERE group_id IS NOT NULL
ON CONFLICT DO NOTHING;

CREATE INDEX IF NOT EXISTS idx_role_division_scopes_division ON role_division_scopes(division_id);
CREATE INDEX IF NOT EXISTS idx_role_sample_info_type_scopes_type ON role_sample_info_type_scopes(type_key);
CREATE INDEX IF NOT EXISTS idx_notification_rule_groups_group ON notification_rule_groups(group_id);

INSERT INTO schema_migrations(version)
VALUES ('1.1.6-beta.6-role-scopes-notification-multi-groups')
ON CONFLICT DO NOTHING;
