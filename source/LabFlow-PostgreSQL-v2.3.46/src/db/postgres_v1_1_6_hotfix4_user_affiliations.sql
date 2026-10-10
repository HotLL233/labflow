-- v1.1.6-hotfix.4: allow one user to work across multiple departments and laboratories.
CREATE TABLE IF NOT EXISTS user_divisions (
  user_id BIGINT NOT NULL,
  division_id BIGINT NOT NULL,
  is_primary BIGINT NOT NULL DEFAULT 0,
  PRIMARY KEY (user_id, division_id)
);

CREATE TABLE IF NOT EXISTS user_groups (
  user_id BIGINT NOT NULL,
  group_id BIGINT NOT NULL,
  is_primary BIGINT NOT NULL DEFAULT 0,
  PRIMARY KEY (user_id, group_id)
);

CREATE INDEX IF NOT EXISTS idx_user_divisions_division ON user_divisions(division_id);
CREATE INDEX IF NOT EXISTS idx_user_groups_group ON user_groups(group_id);

INSERT INTO user_divisions(user_id, division_id, is_primary)
SELECT id, division_id, 1
FROM users
WHERE division_id IS NOT NULL
  AND NOT EXISTS (
    SELECT 1 FROM user_divisions ud
    WHERE ud.user_id=users.id AND ud.division_id=users.division_id
  );

INSERT INTO user_groups(user_id, group_id, is_primary)
SELECT id, group_id, 1
FROM users
WHERE group_id IS NOT NULL
  AND NOT EXISTS (
    SELECT 1 FROM user_groups ug
    WHERE ug.user_id=users.id AND ug.group_id=users.group_id
  );

INSERT INTO schema_migrations(version)
VALUES ('1.1.6-hotfix.4-user-multi-affiliations')
ON CONFLICT DO NOTHING;
