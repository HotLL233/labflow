UPDATE sample_info_records s
SET business_user_id = u.id,
    business_username_snapshot = u.username
FROM users u
WHERE s.user_name = u.username
  AND s.business_user_id IS NULL
  AND u.deleted_at IS NULL;

INSERT INTO schema_migrations(version)
VALUES ('2.2.4-sample-info-editor')
ON CONFLICT DO NOTHING;
