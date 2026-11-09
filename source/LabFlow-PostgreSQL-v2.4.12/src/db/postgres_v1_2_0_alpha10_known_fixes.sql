-- v1.2.0-alpha.10: known-fix release marker.
-- This release contains application-level authorization and export fixes;
-- keeping a migration marker makes installation state auditable without
-- changing existing business data.
INSERT INTO schema_migrations(version)
VALUES ('1.2.0-alpha.10-known-fixes')
ON CONFLICT (version) DO NOTHING;
