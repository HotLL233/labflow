ALTER TABLE work_records ADD COLUMN IF NOT EXISTS detection_type_snapshot TEXT NOT NULL DEFAULT '';
ALTER TABLE work_records ADD COLUMN IF NOT EXISTS auxiliary_type_snapshot TEXT NOT NULL DEFAULT '';
ALTER TABLE auxiliary_work_records ADD COLUMN IF NOT EXISTS auxiliary_type_snapshot TEXT NOT NULL DEFAULT '';

INSERT INTO schema_migrations(version)
VALUES ('2.3.48-workload-classification-snapshots')
ON CONFLICT DO NOTHING;
