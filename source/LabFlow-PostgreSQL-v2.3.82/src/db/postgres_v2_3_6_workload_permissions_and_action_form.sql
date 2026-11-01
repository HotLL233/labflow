-- v2.3.6: expose sampling workload actions in form configuration and grant
-- the workload permissions to the built-in analysis roles.

UPDATE sample_info_columns
SET show_in_form=1, updated_at=to_char(CURRENT_TIMESTAMP,'YYYY-MM-DD HH24:MI:SS')
WHERE data_type='action'
  AND field_key IN ('action_edit','action_record_workload','action_return');

UPDATE sample_info_column_visibility
SET show_in_form=1
WHERE column_id IN (
  SELECT id FROM sample_info_columns
  WHERE data_type='action'
    AND field_key IN ('action_edit','action_record_workload','action_return')
);

INSERT INTO role_template_permissions(template_id, permission_key)
SELECT t.id, p.permission_key
FROM role_templates t
JOIN (VALUES
  ('分析检测员模板','sample:record-workload'),
  ('分析检测员模板','sample-info:record-workload'),
  ('分析检测组长模板','sample:record-workload'),
  ('分析检测组长模板','sample-info:record-workload')
) AS p(template_name, permission_key) ON p.template_name=t.name
WHERE NOT EXISTS (
  SELECT 1 FROM role_template_permissions x
  WHERE x.template_id=t.id AND x.permission_key=p.permission_key
);

INSERT INTO role_permissions(role_id, permission_key)
SELECT r.id, p.permission_key
FROM roles r
JOIN (VALUES
  ('分析检测员','sample:record-workload'),
  ('分析检测员','sample-info:record-workload'),
  ('分析检测组长','sample:record-workload'),
  ('分析检测组长','sample-info:record-workload')
) AS p(role_name, permission_key) ON p.role_name=r.name
WHERE NOT EXISTS (
  SELECT 1 FROM role_permissions x
  WHERE x.role_id=r.id AND x.permission_key=p.permission_key
);

INSERT INTO schema_migrations(version)
VALUES ('2.3.6-workload-permissions-and-action-form')
ON CONFLICT DO NOTHING;
