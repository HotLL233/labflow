-- v2.3.18: allow cumulative workload entries for partial sampling quantities.
DROP INDEX IF EXISTS uq_work_records_sample_source;
CREATE INDEX IF NOT EXISTS idx_work_records_sample_source
  ON work_records(source_type, source_record_id)
  WHERE source_type IS NOT NULL AND source_record_id IS NOT NULL;

INSERT INTO schema_migrations(version)
VALUES ('2.3.18-sample-workload-quantity')
ON CONFLICT DO NOTHING;
