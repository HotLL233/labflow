-- Historical department membership cannot be reconstructed from today's user profile.
-- Leave old records unassigned; non-admin readers may only see their own unassigned history.
ALTER TABLE auxiliary_work_records
    ADD COLUMN IF NOT EXISTS detection_division_id BIGINT,
    ADD COLUMN IF NOT EXISTS detection_division_name_snapshot TEXT NOT NULL DEFAULT '';

COMMENT ON COLUMN auxiliary_work_records.detection_division_id IS
    'Detection department captured by the writer at creation, NULL historical ownership is not inferred';

CREATE INDEX IF NOT EXISTS idx_auxiliary_work_records_department_time
    ON auxiliary_work_records(detection_division_id, recorded_at)
    WHERE deleted_at IS NULL;

INSERT INTO schema_migrations(version) VALUES('2.3.44-auxiliary-department-snapshot')
    ON CONFLICT DO NOTHING;
