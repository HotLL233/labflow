-- Let analysis public accounts use the existing configurable sample-info card.
INSERT INTO role_permissions(role_id,permission_key)
SELECT r.id,'entry:sample-info'
FROM roles r
WHERE r.system_key='analysis_public_account'
ON CONFLICT(role_id,permission_key) DO NOTHING;

INSERT INTO schema_migrations(version)
VALUES ('1.1.7-beta.5-analysis-public-account-home-cards')
ON CONFLICT DO NOTHING;
