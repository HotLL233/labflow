-- Split the former generic public account into distinct RD and analysis workflows.
-- Existing accounts keep their role assignment and only see the RD role label renamed.
UPDATE roles
SET name='研发送样公共账号',
    description='不绑定部门或实验室，可代实验室人员提交研发送样',
    is_system=1,
    sort_order=5,
    system_key='rd_public_account'
WHERE system_key='public_account';

UPDATE roles
SET description='不绑定部门或实验室，可代实验室人员提交研发送样',
    is_system=1,
    sort_order=5
WHERE system_key='rd_public_account';

INSERT INTO roles(name,description,is_system,sort_order,system_key)
VALUES('分析检测公共账号','不绑定部门或实验室，可代实际检测人员录入分析检测工作量',1,6,'analysis_public_account')
ON CONFLICT(name) DO UPDATE
SET description=EXCLUDED.description,
    is_system=1,
    sort_order=EXCLUDED.sort_order,
    system_key=EXCLUDED.system_key;

INSERT INTO role_permissions(role_id,permission_key)
SELECT r.id,p.key FROM roles r CROSS JOIN (VALUES
  ('entry:workload'),
  ('records:work:portal-all-labs'),
  ('records:work:select-detector'),
  ('session:extended'),
  ('help:view')
) AS p(key)
WHERE r.system_key='analysis_public_account'
ON CONFLICT(role_id,permission_key) DO NOTHING;

INSERT INTO schema_migrations(version)
VALUES ('1.1.6-hotfix.19-analysis-public-account')
ON CONFLICT DO NOTHING;
