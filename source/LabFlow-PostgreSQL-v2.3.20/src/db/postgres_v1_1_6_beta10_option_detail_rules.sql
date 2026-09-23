-- v1.1.6-beta.10: generic select-option triggered supplementary inputs for RD records.
ALTER TABLE rd_record_columns
  ADD COLUMN IF NOT EXISTS option_detail_rules TEXT NOT NULL DEFAULT '';

-- Keep the old "select_other" data readable, but migrate its configuration to
-- the reusable rule model. Historical record values such as "其他：说明" are
-- intentionally left unchanged and are read compatibly by the application.
UPDATE rd_record_columns
SET option_detail_rules = '[{"trigger_value":"其他","label":"补充说明","placeholder":"请填写补充说明","required":false}]',
    data_type = 'select'
WHERE data_type = 'select_other'
  AND COALESCE(option_detail_rules, '') = '';

INSERT INTO schema_migrations(version)
VALUES ('1.1.6-beta.10-rd-option-detail-rules')
ON CONFLICT DO NOTHING;
