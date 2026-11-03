-- v1.2.0-alpha.6: separate analysis-statistics export permission from view permission.

INSERT INTO role_template_permissions(template_id, permission_key)
SELECT t.id, 'stats:workload:export'
FROM role_templates t
WHERE t.name IN ('分析检测组长模板', '部门统计查看员模板')
ON CONFLICT DO NOTHING;

INSERT INTO role_permissions(role_id, permission_key)
SELECT r.id, 'stats:workload:export'
FROM roles r
LEFT JOIN role_templates t ON t.id = r.template_id
WHERE (
    t.name IN ('分析检测组长模板', '部门统计查看员模板')
    OR r.name IN ('分析检测组长', '部门统计查看员')
)
AND NOT EXISTS (
    SELECT 1 FROM role_permissions existing
    WHERE existing.role_id = r.id
      AND existing.permission_key = 'stats:workload:export'
);

INSERT INTO schema_migrations(version)
VALUES ('1.2.0-alpha.6-scoped-workload-export')
ON CONFLICT DO NOTHING;
