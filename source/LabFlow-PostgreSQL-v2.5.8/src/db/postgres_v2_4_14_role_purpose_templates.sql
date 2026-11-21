ALTER TABLE roles ADD COLUMN IF NOT EXISTS role_purpose TEXT;
ALTER TABLE role_templates ADD COLUMN IF NOT EXISTS identity_key TEXT NOT NULL DEFAULT 'ordinary';
ALTER TABLE role_templates ADD COLUMN IF NOT EXISTS is_active BIGINT NOT NULL DEFAULT 1;

UPDATE roles SET role_purpose=CASE
  WHEN system_key IN ('public_account','rd_public_account') THEN 'public_rd'
  WHEN system_key='analysis_public_account' THEN 'public_analysis'
  ELSE 'personal' END WHERE role_purpose IS NULL;
ALTER TABLE roles ALTER COLUMN role_purpose SET DEFAULT 'personal';

UPDATE role_templates SET identity_key=CASE name
  WHEN '分析检测员模板' THEN 'analyst'
  WHEN '分析检测组长模板' THEN 'analysis_leader'
  WHEN '研发送样员模板' THEN 'rd_sender'
  WHEN '研发送样组长模板' THEN 'rd_leader'
  ELSE identity_key END;

CREATE OR REPLACE FUNCTION labflow_role_purpose(purpose TEXT, system_key TEXT, is_system BIGINT)
RETURNS TEXT LANGUAGE SQL IMMUTABLE AS $$
  SELECT CASE WHEN is_system=1 AND system_key IN ('public_account','rd_public_account') THEN 'public_rd'
              WHEN is_system=1 AND system_key='analysis_public_account' THEN 'public_analysis'
              ELSE COALESCE(purpose,CASE WHEN system_key IN ('public_account','rd_public_account') THEN 'public_rd'
                                        WHEN system_key='analysis_public_account' THEN 'public_analysis'
                                        ELSE 'personal' END) END
$$;

CREATE OR REPLACE FUNCTION labflow_role_identity(system_key TEXT, is_system BIGINT, template_identity TEXT, role_name TEXT)
RETURNS TEXT LANGUAGE SQL IMMUTABLE AS $$
  SELECT CASE WHEN system_key='system_admin' THEN 'system_admin'
              WHEN is_system=1 AND system_key IN ('analyst','analysis_leader','rd_sender','rd_leader') THEN system_key
              WHEN template_identity IS NOT NULL THEN template_identity
              WHEN is_system=1 AND role_name='分析检测员' THEN 'analyst'
              WHEN is_system=1 AND role_name='分析检测组长' THEN 'analysis_leader'
              WHEN is_system=1 AND role_name='研发送样员' THEN 'rd_sender'
              WHEN is_system=1 AND role_name='研发送样组长' THEN 'rd_leader'
              ELSE 'ordinary' END
$$;

CREATE OR REPLACE VIEW labflow_effective_user_roles AS
SELECT user_id, role_id FROM user_roles
UNION ALL
SELECT id, role_id FROM users u
WHERE role_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM user_roles ur WHERE ur.user_id=u.id);

INSERT INTO schema_migrations(version) VALUES('2.4.14-role-purpose-templates') ON CONFLICT DO NOTHING;
