-- v1.1.7-beta.2: keep return permissions available on upgraded databases.
INSERT INTO role_permissions(role_id, permission_key)
SELECT id, 'sample-info:return'
FROM roles
WHERE name IN ('分析检测员', '分析检测组长')
ON CONFLICT(role_id, permission_key) DO NOTHING;

INSERT INTO role_permissions(role_id, permission_key)
SELECT id, 'sample-info:return-confirm'
FROM roles
WHERE name IN ('研发送样员', '研发送样组长', '分析检测员', '分析检测组长')
ON CONFLICT(role_id, permission_key) DO NOTHING;

INSERT INTO schema_migrations(version)
VALUES ('1.1.7-beta.2-sample-info-return-visibility')
ON CONFLICT DO NOTHING;
