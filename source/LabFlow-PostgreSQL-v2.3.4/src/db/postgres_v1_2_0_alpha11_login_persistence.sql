-- v1.2.0-alpha.11: keep browser sessions separate from browser-managed passwords.
ALTER TABLE user_sessions
  ADD COLUMN IF NOT EXISTS keep_signed_in BIGINT NOT NULL DEFAULT 0;

INSERT INTO schema_migrations(version)
VALUES ('1.2.0-alpha.11-login-persistence')
ON CONFLICT (version) DO NOTHING;
