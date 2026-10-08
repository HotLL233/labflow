-- v1.2.0-alpha.7: historical analysis ownership confirmation must populate
-- the same detection and sending dimensions used by statistics and export.

CREATE INDEX IF NOT EXISTS idx_work_records_confirmed_dimensions
    ON work_records(detection_division_id, sending_division_id, ownership_status)
    WHERE deleted_at IS NULL;

INSERT INTO schema_migrations(version)
VALUES ('1.2.0-alpha.7-ownership-confirmation-snapshots')
ON CONFLICT DO NOTHING;
