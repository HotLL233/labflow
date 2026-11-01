-- Grant once; later administrator revocations survive restart.
INSERT INTO role_permissions(role_id,permission_key)
SELECT DISTINCT rp.role_id,p.permission_key
FROM role_permissions rp
CROSS JOIN (VALUES ('entry:auxiliary'),('records:auxiliary:edit-own')) p(permission_key)
WHERE rp.permission_key='entry:workload'
ON CONFLICT(role_id,permission_key) DO NOTHING;

INSERT INTO role_template_permissions(template_id,permission_key)
SELECT DISTINCT rp.template_id,p.permission_key
FROM role_template_permissions rp
CROSS JOIN (VALUES ('entry:auxiliary'),('records:auxiliary:edit-own')) p(permission_key)
WHERE rp.permission_key='entry:workload'
ON CONFLICT(template_id,permission_key) DO NOTHING;

INSERT INTO schema_migrations(version) VALUES('2.3.68-auxiliary-maintenance')
ON CONFLICT DO NOTHING;
