-- v1.1.6-beta.2: public account, permission-driven RD workflows and idle sessions.
ALTER TABLE roles ADD COLUMN IF NOT EXISTS system_key TEXT NOT NULL DEFAULT '';
ALTER TABLE user_sessions ADD COLUMN IF NOT EXISTS last_seen_at TEXT;
UPDATE user_sessions SET last_seen_at=COALESCE(NULLIF(last_seen_at,''),created_at);

UPDATE roles SET system_key=CASE name
  WHEN '系统管理员' THEN 'system_admin'
  WHEN '分析检测员' THEN 'analyst'
  WHEN '分析检测组长' THEN 'analysis_leader'
  WHEN '研发送样员' THEN 'rd_sender'
  WHEN '研发送样组长' THEN 'rd_leader'
  ELSE system_key END
WHERE system_key='' AND name IN ('系统管理员','分析检测员','分析检测组长','研发送样员','研发送样组长');

INSERT INTO roles(name,description,is_system,sort_order,system_key)
VALUES('公共账号','不绑定部门或实验室，可代实验室人员提交研发送样',1,5,'public_account')
ON CONFLICT(name) DO UPDATE SET system_key='public_account',description=EXCLUDED.description,is_system=1,sort_order=EXCLUDED.sort_order;

INSERT INTO role_permissions(role_id,permission_key)
SELECT r.id,p.key FROM roles r CROSS JOIN (VALUES
 ('records:rd:select-sender'),('records:rd:edit-created'),('records:rd:edit-subject'),
 ('records:rd:return-confirm')
) AS p(key)
WHERE r.system_key IN ('rd_sender','rd_leader','system_admin')
ON CONFLICT(role_id,permission_key) DO NOTHING;

INSERT INTO role_permissions(role_id,permission_key)
SELECT r.id,'manage:settings' FROM roles r
WHERE r.system_key IN ('system_admin','analysis_leader')
ON CONFLICT(role_id,permission_key) DO NOTHING;

INSERT INTO role_permissions(role_id,permission_key)
SELECT r.id,p.key FROM roles r CROSS JOIN (VALUES
 ('entry:sample'),('records:rd:view-all'),('records:rd:portal-all-labs'),
 ('records:rd:select-sender'),('records:rd:edit-created'),('records:rd:return-confirm'),('session:extended'),('help:view')
) AS p(key)
WHERE r.system_key='public_account'
ON CONFLICT(role_id,permission_key) DO NOTHING;

-- Historical records previously stored the operator as subject user. Rebind to
-- the visible sender where a matching active user exists.
UPDATE rd_work_records wr
SET subject_user_id=(SELECT u.id FROM users u WHERE u.username=wr.user_name AND u.deleted_at IS NULL ORDER BY u.id LIMIT 1)
WHERE EXISTS(SELECT 1 FROM users u WHERE u.username=wr.user_name AND u.deleted_at IS NULL);

INSERT INTO system_settings(key,value,updated_at)
VALUES('rd_notice_options','["常规送样","加急处理","复测/补测","低温保存","避光保存","特殊安全要求","优先处理","其他"]',to_char(CURRENT_TIMESTAMP,'YYYY-MM-DD HH24:MI:SS'))
ON CONFLICT(key) DO NOTHING;

INSERT INTO schema_migrations(version) VALUES('1.1.6-beta.2-public-account-rd-permissions-sessions') ON CONFLICT DO NOTHING;
