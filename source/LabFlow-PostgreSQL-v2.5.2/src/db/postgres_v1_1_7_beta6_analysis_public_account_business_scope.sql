-- Keep the public-account personnel selection switch on during the scope upgrade.
INSERT INTO role_permissions(role_id,permission_key)
SELECT r.id,'records:work:portal-scoped'
FROM roles r
WHERE r.system_key='analysis_public_account'
ON CONFLICT(role_id,permission_key) DO NOTHING;

INSERT INTO schema_migrations(version)
VALUES ('1.1.7-beta.6-analysis-public-account-business-scope')
ON CONFLICT DO NOTHING;
