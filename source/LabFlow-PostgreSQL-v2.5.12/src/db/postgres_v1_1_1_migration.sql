ALTER TABLE rd_work_records
  ADD COLUMN IF NOT EXISTS extra_fields TEXT NOT NULL DEFAULT '{}';

INSERT INTO schema_migrations(version)
VALUES ('1.1.1-form-config-and-rd-custom-fields')
ON CONFLICT DO NOTHING;
