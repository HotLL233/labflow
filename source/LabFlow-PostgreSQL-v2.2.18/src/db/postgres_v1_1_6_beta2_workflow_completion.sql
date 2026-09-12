-- v1.1.6-beta.2 completion patch: make the public-account workflow and
-- permission-driven UI usable on both new and already upgraded databases.

INSERT INTO role_permissions(role_id,permission_key)
SELECT r.id,p.key FROM roles r CROSS JOIN (VALUES
  ('records:rd:select-sender'),('records:rd:edit-created'),
  ('records:rd:edit-subject'),('records:rd:return-confirm')
) AS p(key)
WHERE r.system_key IN ('rd_sender','rd_leader','system_admin')
ON CONFLICT(role_id,permission_key) DO NOTHING;

INSERT INTO role_permissions(role_id,permission_key)
SELECT r.id,p.key FROM roles r CROSS JOIN (VALUES
  ('entry:sample'),('records:rd:view-all'),('records:rd:portal-all-labs'),
  ('records:rd:select-sender'),('records:rd:edit-created'),
  ('records:rd:return-confirm'),('help:view')
) AS p(key)
WHERE r.system_key='public_account'
ON CONFLICT(role_id,permission_key) DO NOTHING;

INSERT INTO role_permissions(role_id,permission_key)
SELECT r.id,'manage:notifications' FROM roles r
WHERE r.system_key IN ('system_admin','analysis_leader')
ON CONFLICT(role_id,permission_key) DO NOTHING;

INSERT INTO role_permissions(role_id,permission_key)
SELECT r.id,'feedback:personnel:edit' FROM roles r
WHERE r.system_key IN ('system_admin','rd_leader')
ON CONFLICT(role_id,permission_key) DO NOTHING;

INSERT INTO schema_migrations(version) VALUES('1.1.6-beta.2-public-account-workflow-completion') ON CONFLICT DO NOTHING;
