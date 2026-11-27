ALTER TABLE user_sessions ADD COLUMN IF NOT EXISTS oidc_deadline BIGINT;

CREATE TABLE IF NOT EXISTS labflow_oidc_bindings (
    user_id BIGINT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    employee_no TEXT NOT NULL CONSTRAINT oidc_employee_unique UNIQUE,
    issuer TEXT NOT NULL,
    subject TEXT,
    confirmed BOOLEAN NOT NULL DEFAULT FALSE,
    invite_digest TEXT UNIQUE,
    invite_expires_at TIMESTAMPTZ,
    CONSTRAINT oidc_identity_unique UNIQUE (issuer, subject),
    CHECK (NOT confirmed OR subject IS NOT NULL),
    CHECK ((invite_digest IS NULL) = (invite_expires_at IS NULL)),
    CHECK (subject IS NULL OR invite_digest IS NULL)
);

CREATE OR REPLACE VIEW labflow_oidc_eligible_users AS
SELECT u.id FROM users u
WHERE u.is_active=1 AND u.deleted_at IS NULL
  AND (u.is_admin=1 OR EXISTS (SELECT 1 FROM labflow_effective_user_roles WHERE user_id=u.id))
  AND NOT EXISTS (
    SELECT 1 FROM labflow_effective_user_roles ur JOIN roles r ON r.id=ur.role_id
    WHERE ur.user_id=u.id AND labflow_role_purpose(r.role_purpose,r.system_key,r.is_system)<>'personal'
  );

INSERT INTO schema_migrations(version) VALUES ('2.5.12-oidc-pilot') ON CONFLICT DO NOTHING;
