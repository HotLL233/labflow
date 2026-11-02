-- v1.2.0-alpha.12: separate sample-info entry permission from create/edit permission.
-- Analysis users can enter the processing page without receiving the registration form.

UPDATE roles
SET template_id=(SELECT id FROM role_templates WHERE name='分析检测员模板' LIMIT 1)
WHERE name='分析检测员' AND template_id IS NULL;
UPDATE roles
SET template_id=(SELECT id FROM role_templates WHERE name='分析检测组长模板' LIMIT 1)
WHERE name='分析检测组长' AND template_id IS NULL;
UPDATE roles
SET template_id=(SELECT id FROM role_templates WHERE name='研发送样员模板' LIMIT 1)
WHERE name='研发送样员' AND template_id IS NULL;
UPDATE roles
SET template_id=(SELECT id FROM role_templates WHERE name='研发送样组长模板' LIMIT 1)
WHERE name='研发送样组长' AND template_id IS NULL;

INSERT INTO role_template_permissions(template_id, permission_key)
SELECT t.id, p.permission_key
FROM role_templates t
JOIN (VALUES
  ('研发送样员模板','entry:sample-info'),
  ('研发送样员模板','sample-info:create'),
  ('研发送样员模板','sample-info:edit-own'),
  ('分析检测员模板','entry:sample-info'),
  ('分析检测员模板','sample-info:collect'),
  ('分析检测员模板','sample-info:complete'),
  ('分析检测员模板','sample-info:return'),
  ('分析检测员模板','sample-info:withdraw'),
  ('分析检测组长模板','entry:sample-info'),
  ('分析检测组长模板','sample-info:collect'),
  ('分析检测组长模板','sample-info:complete'),
  ('分析检测组长模板','sample-info:return'),
  ('分析检测组长模板','sample-info:withdraw'),
  ('分析检测组长模板','records:work:view-scope'),
  ('部门统计查看员模板','records:work:view-scope'),
  ('样品登记员模板','sample-info:create'),
  ('样品登记员模板','sample-info:edit-own'),
  ('样品登记组长模板','sample-info:create'),
  ('样品登记组长模板','sample-info:edit-own')
) AS p(template_name, permission_key) ON p.template_name=t.name
WHERE NOT EXISTS (
  SELECT 1 FROM role_template_permissions x
  WHERE x.template_id=t.id AND x.permission_key=p.permission_key
);

INSERT INTO role_permissions(role_id, permission_key)
SELECT r.id, p.permission_key
FROM roles r
JOIN role_templates t ON t.id=r.template_id
JOIN (VALUES
  ('研发送样员模板','entry:sample-info'),
  ('研发送样员模板','sample-info:create'),
  ('研发送样员模板','sample-info:edit-own'),
  ('分析检测员模板','entry:sample-info'),
  ('分析检测员模板','sample-info:collect'),
  ('分析检测员模板','sample-info:complete'),
  ('分析检测员模板','sample-info:return'),
  ('分析检测员模板','sample-info:withdraw'),
  ('分析检测组长模板','entry:sample-info'),
  ('分析检测组长模板','sample-info:collect'),
  ('分析检测组长模板','sample-info:complete'),
  ('分析检测组长模板','sample-info:return'),
  ('分析检测组长模板','sample-info:withdraw'),
  ('分析检测组长模板','records:work:view-scope'),
  ('样品登记员模板','sample-info:create'),
  ('样品登记员模板','sample-info:edit-own'),
  ('样品登记组长模板','sample-info:create'),
  ('样品登记组长模板','sample-info:edit-own')
) AS p(template_name, permission_key) ON p.template_name=t.name
WHERE NOT EXISTS (
  SELECT 1 FROM role_permissions x
  WHERE x.role_id=r.id AND x.permission_key=p.permission_key
);

-- The historical built-in sending-leader role predates role templates.
-- Preserve its existing sample-info workflow after the create permission split.
INSERT INTO role_permissions(role_id, permission_key)
SELECT r.id, p.permission_key
FROM roles r
JOIN (VALUES
  ('研发送样员','entry:sample-info'),
  ('研发送样员','sample-info:create'),
  ('研发送样员','sample-info:edit-own'),
  ('研发送样组长','entry:sample-info'),
  ('研发送样组长','sample-info:create'),
  ('研发送样组长','sample-info:edit-own')
) AS p(role_name, permission_key) ON p.role_name=r.name
WHERE NOT EXISTS (
  SELECT 1 FROM role_permissions x
  WHERE x.role_id=r.id AND x.permission_key=p.permission_key
);

INSERT INTO schema_migrations(version)
VALUES ('1.2.0-alpha.12-sending-analysis-permissions')
ON CONFLICT DO NOTHING;
