-- v2.3.7: expose sampling workload permissions in role management and repair
-- existing role/template assignments on upgraded databases.

INSERT INTO role_template_permissions(template_id, permission_key)
SELECT t.id, 'sample:record-workload'
FROM role_templates t
WHERE EXISTS (
  SELECT 1 FROM role_template_permissions p
  WHERE p.template_id=t.id AND p.permission_key IN ('sample:collect','entry:workload')
)
AND NOT EXISTS (
  SELECT 1 FROM role_template_permissions p
  WHERE p.template_id=t.id AND p.permission_key='sample:record-workload'
);

INSERT INTO role_template_permissions(template_id, permission_key)
SELECT t.id, 'sample-info:record-workload'
FROM role_templates t
WHERE EXISTS (
  SELECT 1 FROM role_template_permissions p
  WHERE p.template_id=t.id AND p.permission_key IN ('sample-info:collect','entry:workload')
)
AND NOT EXISTS (
  SELECT 1 FROM role_template_permissions p
  WHERE p.template_id=t.id AND p.permission_key='sample-info:record-workload'
);

INSERT INTO role_permissions(role_id, permission_key)
SELECT r.id, 'sample:record-workload'
FROM roles r
WHERE r.deleted_at IS NULL
  AND (
    EXISTS (SELECT 1 FROM role_permissions p WHERE p.role_id=r.id AND p.permission_key IN ('sample:collect','entry:workload'))
    OR EXISTS (
      SELECT 1 FROM role_template_permissions tp
      WHERE tp.template_id=r.template_id AND tp.permission_key IN ('sample:collect','entry:workload')
    )
  )
  AND NOT EXISTS (
    SELECT 1 FROM role_permissions p
    WHERE p.role_id=r.id AND p.permission_key='sample:record-workload'
  );

INSERT INTO role_permissions(role_id, permission_key)
SELECT r.id, 'sample-info:record-workload'
FROM roles r
WHERE r.deleted_at IS NULL
  AND (
    EXISTS (SELECT 1 FROM role_permissions p WHERE p.role_id=r.id AND p.permission_key IN ('sample-info:collect','entry:workload'))
    OR EXISTS (
      SELECT 1 FROM role_template_permissions tp
      WHERE tp.template_id=r.template_id AND tp.permission_key IN ('sample-info:collect','entry:workload')
    )
  )
  AND NOT EXISTS (
    SELECT 1 FROM role_permissions p
    WHERE p.role_id=r.id AND p.permission_key='sample-info:record-workload'
  );

INSERT INTO schema_migrations(version)
VALUES ('2.3.7-workload-permission-whitelist-and-role-repair')
ON CONFLICT DO NOTHING;
