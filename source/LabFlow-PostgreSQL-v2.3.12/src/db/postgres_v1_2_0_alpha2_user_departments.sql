-- v1.2.0-alpha.2: distinguish a user's primary department from business departments.
-- Existing user_divisions rows remain the business-department set.
INSERT INTO user_divisions(user_id, division_id, is_primary)
SELECT u.id, u.division_id, 1
FROM users u
WHERE u.division_id IS NOT NULL
  AND NOT EXISTS (
    SELECT 1 FROM user_divisions ud
    WHERE ud.user_id=u.id AND ud.division_id=u.division_id
  );

UPDATE user_divisions ud
SET is_primary = CASE
  WHEN ud.division_id = (SELECT u.division_id FROM users u WHERE u.id=ud.user_id)
  THEN 1 ELSE 0 END;

INSERT INTO schema_migrations(version)
VALUES ('1.2.0-alpha.2-user-primary-and-business-departments')
ON CONFLICT DO NOTHING;
