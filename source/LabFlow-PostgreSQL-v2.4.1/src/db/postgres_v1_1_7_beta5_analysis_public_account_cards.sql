-- v1.1.7-beta.5: analysis public account portal is scoped by its bound departments.
-- Existing department affiliations remain the source of truth and no new business
-- record column is required.
INSERT INTO role_permissions(role_id,permission_key)
SELECT r.id,'records:work:portal-scoped'
FROM roles r
WHERE r.system_key='analysis_public_account'
ON CONFLICT(role_id,permission_key) DO NOTHING;

DELETE FROM role_permissions
WHERE permission_key='records:work:portal-all-labs'
  AND role_id IN (SELECT id FROM roles WHERE system_key='analysis_public_account');

INSERT INTO schema_migrations(version)
VALUES ('1.1.7-beta.5-analysis-public-account-cards')
ON CONFLICT DO NOTHING;
