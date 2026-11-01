-- v1.1.6-hotfix.6: common methods can be visible only in selected departments.
CREATE TABLE IF NOT EXISTS common_method_division_scopes (
  method_id BIGINT NOT NULL REFERENCES methods(id) ON DELETE CASCADE,
  division_id BIGINT NOT NULL REFERENCES divisions(id) ON DELETE CASCADE,
  created_at TEXT NOT NULL DEFAULT to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS'),
  PRIMARY KEY (method_id, division_id)
);

-- Existing common methods deliberately keep no mapping rows. An empty mapping means
-- “all departments”, so later-created departments retain the former global behaviour.

CREATE INDEX IF NOT EXISTS idx_common_method_division_scopes_division
  ON common_method_division_scopes(division_id);

INSERT INTO schema_migrations(version)
VALUES ('1.1.6-hotfix.6-common-method-division-scopes')
ON CONFLICT DO NOTHING;
