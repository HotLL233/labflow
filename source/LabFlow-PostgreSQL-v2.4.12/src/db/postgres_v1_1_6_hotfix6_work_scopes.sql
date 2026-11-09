-- v1.1.6-hotfix.6: independent department scope for the analysis/detection portal.
CREATE TABLE IF NOT EXISTS role_work_division_scopes (
  role_id BIGINT NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
  division_id BIGINT NOT NULL REFERENCES divisions(id) ON DELETE CASCADE,
  created_at TEXT NOT NULL DEFAULT to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS'),
  PRIMARY KEY (role_id, division_id)
);

-- Existing configured role ranges were historically used for cross-portal data.
-- Copy them into the new analysis scope so upgrading does not broaden access.
INSERT INTO role_work_division_scopes(role_id, division_id)
SELECT role_id, division_id FROM role_division_scopes
ON CONFLICT DO NOTHING;

CREATE INDEX IF NOT EXISTS idx_role_work_division_scopes_division
  ON role_work_division_scopes(division_id);

INSERT INTO schema_migrations(version)
VALUES ('1.1.6-hotfix.6-work-division-scopes')
ON CONFLICT DO NOTHING;
