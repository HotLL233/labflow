-- Allow a mistaken RD sampling action to be reverted with an auditable reason.
-- Existing custom roles remain unchanged and can grant this permission explicitly.
INSERT INTO role_permissions(role_id,permission_key)
SELECT id,'sample:withdraw'
FROM roles
WHERE name IN ('分析检测员','分析检测组长')
ON CONFLICT(role_id,permission_key) DO NOTHING;

INSERT INTO schema_migrations(version)
VALUES ('1.1.6-hotfix.20-rd-sample-withdraw')
ON CONFLICT DO NOTHING;
