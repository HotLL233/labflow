-- v2.3.4: work records created from a sampling operation retain their origin.
ALTER TABLE work_records ADD COLUMN IF NOT EXISTS source_type TEXT;
ALTER TABLE work_records ADD COLUMN IF NOT EXISTS source_record_id BIGINT;

CREATE INDEX IF NOT EXISTS idx_work_records_sample_source
  ON work_records(source_type, source_record_id)
  WHERE source_type IS NOT NULL AND source_record_id IS NOT NULL;

INSERT INTO schema_migrations(version)
VALUES ('2.3.4-sample-workload')
ON CONFLICT DO NOTHING;
