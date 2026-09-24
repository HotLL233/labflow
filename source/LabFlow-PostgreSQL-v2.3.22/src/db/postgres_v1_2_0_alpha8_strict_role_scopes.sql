-- Alpha.8 records the authorization rule change separately so deployments can
-- verify that formal role data-source scopes are the only business scope.
INSERT INTO schema_migrations(version)
VALUES ('1.2.0-alpha.8-strict-role-data-scopes')
ON CONFLICT (version) DO NOTHING;
