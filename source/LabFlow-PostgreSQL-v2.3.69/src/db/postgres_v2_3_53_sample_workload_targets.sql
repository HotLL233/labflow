-- Stable source binding and workload provenance. Existing names/classifications
-- remain historical snapshots; no bulk name matching or workload recalculation.
ALTER TABLE sample_info_records ADD COLUMN IF NOT EXISTS project_id BIGINT;
CREATE INDEX IF NOT EXISTS idx_sample_info_project_id ON sample_info_records(project_id);
ALTER TABLE work_records ADD COLUMN IF NOT EXISTS extra_fields TEXT DEFAULT '{}';
INSERT INTO schema_migrations(version)
VALUES ('2.3.53-sample-workload-targets') ON CONFLICT DO NOTHING;
