-- Existing nonempty identifiers and historical references remain unchanged.
-- A separate prefix cannot collide with the normal M-xxxxxxxx creation sequence.
UPDATE methods
SET method_code = 'M-LEGACY-' || gen_random_uuid()::text
WHERE method_code = '';

INSERT INTO schema_migrations(version)
VALUES ('2.3.70-stable-method-ids')
ON CONFLICT DO NOTHING;
