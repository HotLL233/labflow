-- A row that has been restored or permanently purged is historical data.
-- It must not block a later delete of the same business record.
DROP INDEX IF EXISTS idx_trash_active_entity;

CREATE UNIQUE INDEX idx_trash_active_entity
ON trash_entries(table_name, record_id)
WHERE restored_at IS NULL AND purged_at IS NULL;

INSERT INTO schema_migrations(version)
VALUES ('1.1.6-hotfix.8-trash-active-index')
ON CONFLICT DO NOTHING;
