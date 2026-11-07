-- v2.3.38: make personnel feedback permissions survive template-based role creation.
-- This migration is additive and intentionally does not remove or overwrite
-- administrator-customized permissions.

-- Keep the template definitions complete for newly created roles.
INSERT INTO role_template_permissions(template_id, permission_key)
SELECT rt.id, p.permission_key
FROM role_templates rt
CROSS JOIN (VALUES
  ('feedback:personnel:view'),
  ('feedback:personnel:edit')
) AS p(permission_key)
WHERE rt.name = '研发送样组长模板'
  AND rt.deleted_at IS NULL
ON CONFLICT (template_id, permission_key) DO NOTHING;

INSERT INTO role_template_permissions(template_id, permission_key)
SELECT rt.id, 'feedback:personnel:view'
FROM role_templates rt
WHERE rt.name = '分析检测组长模板'
  AND rt.deleted_at IS NULL
ON CONFLICT (template_id, permission_key) DO NOTHING;

-- Backfill roles already created from the templates. Only missing permissions
-- are added; existing permissions and manually customized role settings remain.
INSERT INTO role_permissions(role_id, permission_key)
SELECT r.id, rtp.permission_key
FROM roles r
JOIN role_templates rt ON rt.id = r.template_id
JOIN role_template_permissions rtp ON rtp.template_id = rt.id
WHERE rt.name IN ('研发送样组长模板', '分析检测组长模板')
  AND rt.deleted_at IS NULL
  AND r.deleted_at IS NULL
  AND rtp.permission_key IN ('feedback:personnel:view', 'feedback:personnel:edit')
ON CONFLICT (role_id, permission_key) DO NOTHING;

-- Preserve compatibility for built-in sender/system roles and older databases.
INSERT INTO role_permissions(role_id, permission_key)
SELECT r.id, p.permission_key
FROM roles r
CROSS JOIN (VALUES
  ('feedback:personnel:view'),
  ('feedback:personnel:edit')
) AS p(permission_key)
WHERE r.system_key IN ('system_admin', 'rd_leader')
ON CONFLICT (role_id, permission_key) DO NOTHING;

INSERT INTO schema_migrations(version)
VALUES ('2.3.38-personnel-feedback-template-permissions')
ON CONFLICT DO NOTHING;
