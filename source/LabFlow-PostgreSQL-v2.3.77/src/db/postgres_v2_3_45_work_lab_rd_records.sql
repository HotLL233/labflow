-- v2.3.45: a separate, execution-department-scoped analysis record reader.
-- Grant the new capability once, without granting global RD access or actions.
INSERT INTO role_template_permissions(template_id, permission_key)
SELECT t.id, 'records:rd:view-work-lab'
FROM role_templates t
WHERE t.is_system=1 AND t.deleted_at IS NULL
  AND t.name IN ('分析检测员模板','分析检测组长模板')
ON CONFLICT (template_id, permission_key) DO NOTHING;

INSERT INTO role_permissions(role_id, permission_key)
SELECT r.id, 'records:rd:view-work-lab'
FROM roles r
WHERE r.deleted_at IS NULL
  AND (
    (r.is_system=1 AND r.system_key IN ('analyst','analysis_leader','analysis_public_account'))
    OR EXISTS (
      SELECT 1 FROM role_templates t
      WHERE t.id=r.template_id AND t.is_system=1 AND t.deleted_at IS NULL
        AND t.name IN ('分析检测员模板','分析检测组长模板')
    )
  )
ON CONFLICT (role_id, permission_key) DO NOTHING;

INSERT INTO schema_migrations(version)
VALUES ('2.3.45-work-lab-rd-records')
ON CONFLICT DO NOTHING;
