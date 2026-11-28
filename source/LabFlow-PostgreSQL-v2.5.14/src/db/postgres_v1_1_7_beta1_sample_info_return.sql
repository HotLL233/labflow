-- v1.1.7-beta.1: sample information return and resubmission workflow.
ALTER TABLE sample_info_records ADD COLUMN IF NOT EXISTS return_reason TEXT NOT NULL DEFAULT '';
ALTER TABLE sample_info_records ADD COLUMN IF NOT EXISTS returned_by TEXT NOT NULL DEFAULT '';
ALTER TABLE sample_info_records ADD COLUMN IF NOT EXISTS returned_at TEXT;
ALTER TABLE sample_info_records ADD COLUMN IF NOT EXISTS return_confirmed_by TEXT NOT NULL DEFAULT '';
ALTER TABLE sample_info_records ADD COLUMN IF NOT EXISTS return_confirmed_at TEXT;
ALTER TABLE sample_info_records ADD COLUMN IF NOT EXISTS source_record_id BIGINT;

CREATE INDEX IF NOT EXISTS idx_sample_info_records_source_record
  ON sample_info_records(source_record_id);

INSERT INTO role_permissions(role_id, permission_key)
SELECT id, 'sample-info:return'
FROM roles
WHERE name IN ('分析检测员', '分析检测组长')
ON CONFLICT(role_id, permission_key) DO NOTHING;

INSERT INTO role_permissions(role_id, permission_key)
SELECT id, 'sample-info:return-confirm'
FROM roles
WHERE name IN ('研发送样员', '研发送样组长', '分析检测员', '分析检测组长')
ON CONFLICT(role_id, permission_key) DO NOTHING;

INSERT INTO schema_migrations(version)
VALUES ('1.1.7-beta.1-sample-info-return-workflow')
ON CONFLICT DO NOTHING;
