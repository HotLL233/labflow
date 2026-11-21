-- v1.1.5-beta.7: replace the legacy full unique method-code index.
-- Historical databases can contain one unnumbered method. New imports briefly
-- use an empty code before assigning the generated M-xxxxxxxx identifier.
DROP INDEX IF EXISTS idx_methods_code;
DROP INDEX IF EXISTS uq_methods_code_present;

UPDATE methods
SET method_code = ''
WHERE trim(COALESCE(method_code, '')) IN ('', '0');

CREATE UNIQUE INDEX uq_methods_code_present
  ON methods(method_code)
  WHERE method_code <> '';

INSERT INTO schema_migrations(version)
VALUES ('1.1.5-beta.7-method-code-index-compatibility')
ON CONFLICT DO NOTHING;
