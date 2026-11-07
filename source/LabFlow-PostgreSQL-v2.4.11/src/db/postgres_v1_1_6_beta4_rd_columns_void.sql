-- v1.1.6-beta.4: full administrator-managed RD entry fields and returned-record voiding.
ALTER TABLE rd_record_columns ADD COLUMN IF NOT EXISTS is_required BIGINT NOT NULL DEFAULT 0;
ALTER TABLE rd_record_columns ADD COLUMN IF NOT EXISTS is_active BIGINT NOT NULL DEFAULT 1;
ALTER TABLE rd_record_columns ADD COLUMN IF NOT EXISTS show_in_export BIGINT NOT NULL DEFAULT 1;
ALTER TABLE rd_record_columns ADD COLUMN IF NOT EXISTS options TEXT NOT NULL DEFAULT '';
ALTER TABLE rd_record_columns ADD COLUMN IF NOT EXISTS default_value TEXT NOT NULL DEFAULT '';
ALTER TABLE rd_record_columns ADD COLUMN IF NOT EXISTS placeholder TEXT NOT NULL DEFAULT '';
ALTER TABLE rd_record_columns ADD COLUMN IF NOT EXISTS applicable_types TEXT NOT NULL DEFAULT '';
ALTER TABLE rd_record_columns ADD COLUMN IF NOT EXISTS entry_row BIGINT NOT NULL DEFAULT 1;

ALTER TABLE rd_work_records ADD COLUMN IF NOT EXISTS voided_at TEXT;
ALTER TABLE rd_work_records ADD COLUMN IF NOT EXISTS voided_by TEXT NOT NULL DEFAULT '';
ALTER TABLE rd_work_records ADD COLUMN IF NOT EXISTS void_reason TEXT NOT NULL DEFAULT '';

UPDATE rd_record_columns
SET data_type = CASE name
  WHEN 'quantity' THEN 'number'
  WHEN 'notes' THEN 'select_other'
  WHEN 'submitted_at' THEN 'datetime'
  WHEN 'sampling_time' THEN 'datetime'
  ELSE data_type
END,
is_required = CASE WHEN name IN ('user_name','project_name','detection_type','method_name','quantity') THEN 1 ELSE is_required END,
show_in_export = 1,
entry_row = CASE WHEN name IN ('method_name','quantity','batch_no','notes') THEN 2 ELSE 1 END;

UPDATE rd_record_columns
SET options = COALESCE((SELECT value FROM system_settings WHERE key='rd_notice_options'),
  '["常规送样","加急处理","复测/补测","低温保存","避光保存","特殊安全要求","优先处理","其他"]')
WHERE name='notes' AND (options='' OR options IS NULL);

INSERT INTO schema_migrations(version) VALUES('1.1.6-beta.4-rd-columns-and-return-void') ON CONFLICT DO NOTHING;
