-- v1.2.0-alpha.5: separate the actual detection organization from the
-- sending organization for every analysis workload record.

ALTER TABLE work_records ADD COLUMN IF NOT EXISTS detection_division_id BIGINT;
ALTER TABLE work_records ADD COLUMN IF NOT EXISTS detection_division_name_snapshot TEXT NOT NULL DEFAULT '';
ALTER TABLE work_records ADD COLUMN IF NOT EXISTS sending_division_id BIGINT;
ALTER TABLE work_records ADD COLUMN IF NOT EXISTS sending_division_name_snapshot TEXT NOT NULL DEFAULT '';
ALTER TABLE work_records ADD COLUMN IF NOT EXISTS sending_group_id BIGINT;
ALTER TABLE work_records ADD COLUMN IF NOT EXISTS sending_group_name_snapshot TEXT NOT NULL DEFAULT '';

-- The selected workload portal laboratory is the sending laboratory. This can
-- be reconstructed safely for legacy records. The actual detector department
-- is only filled when the recorded detector has one unambiguous primary
-- department, otherwise the record remains pending confirmation.
UPDATE work_records wr
SET sending_group_id = COALESCE(wr.sending_group_id, wr.group_id, wr.execution_group_id),
    sending_group_name_snapshot = COALESCE(
        NULLIF(wr.sending_group_name_snapshot, ''),
        NULLIF(wr.lab_name_snapshot, ''),
        (SELECT g.name FROM project_groups g WHERE g.id = COALESCE(wr.group_id, wr.execution_group_id)),
        ''
    ),
    sending_division_id = COALESCE(
        wr.sending_division_id,
        (SELECT g.division_id FROM project_groups g WHERE g.id = COALESCE(wr.group_id, wr.execution_group_id)),
        wr.execution_division_id,
        wr.division_id
    ),
    sending_division_name_snapshot = COALESCE(
        NULLIF(wr.sending_division_name_snapshot, ''),
        (SELECT d.name FROM divisions d WHERE d.id = COALESCE(
            wr.sending_division_id,
            (SELECT g.division_id FROM project_groups g WHERE g.id = COALESCE(wr.group_id, wr.execution_group_id)),
            wr.execution_division_id,
            wr.division_id
        )),
        ''
    ),
    detection_division_id = COALESCE(
        wr.detection_division_id,
        (SELECT u.division_id FROM users u WHERE u.id = wr.subject_user_id),
        (SELECT u.division_id FROM users u WHERE u.username = wr.user_name ORDER BY u.id LIMIT 1)
    ),
    detection_division_name_snapshot = COALESCE(
        NULLIF(wr.detection_division_name_snapshot, ''),
        (SELECT d.name FROM divisions d WHERE d.id = COALESCE(
            wr.detection_division_id,
            (SELECT u.division_id FROM users u WHERE u.id = wr.subject_user_id),
            (SELECT u.division_id FROM users u WHERE u.username = wr.user_name ORDER BY u.id LIMIT 1)
        )),
        ''
    );

UPDATE work_records
SET ownership_status = 'pending_confirmation'
WHERE sending_division_id IS NULL OR detection_division_id IS NULL;

CREATE INDEX IF NOT EXISTS idx_work_records_detection_division
    ON work_records(detection_division_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_work_records_sending_division
    ON work_records(sending_division_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_work_records_detection_sending_time
    ON work_records(detection_division_id, sending_division_id, recorded_at)
    WHERE deleted_at IS NULL;

INSERT INTO schema_migrations(version)
VALUES ('1.2.0-alpha.5-analysis-export-dimensions')
ON CONFLICT DO NOTHING;
