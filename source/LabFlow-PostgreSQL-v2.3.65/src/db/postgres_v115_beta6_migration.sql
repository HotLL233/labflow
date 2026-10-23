-- v1.1.5-beta.6: per sample type field presentation and notification routing.
ALTER TABLE sample_info_column_visibility
  ADD COLUMN IF NOT EXISTS show_in_form BIGINT NOT NULL DEFAULT 1;
ALTER TABLE sample_info_column_visibility
  ADD COLUMN IF NOT EXISTS show_in_list BIGINT NOT NULL DEFAULT 1;
ALTER TABLE sample_info_column_visibility
  ADD COLUMN IF NOT EXISTS show_in_export BIGINT NOT NULL DEFAULT 1;
ALTER TABLE sample_info_column_visibility
  ADD COLUMN IF NOT EXISTS sort_order BIGINT NOT NULL DEFAULT 0;

UPDATE sample_info_column_visibility v
SET show_in_form = c.show_in_form,
    show_in_list = c.show_in_list,
    show_in_export = c.show_in_export,
    sort_order = c.sort_order
FROM sample_info_columns c
WHERE c.id = v.column_id;

INSERT INTO sample_info_column_visibility
  (type_key,column_id,is_visible,is_required,show_in_form,show_in_list,show_in_export,sort_order)
SELECT t.type_key,c.id,1,c.is_required,c.show_in_form,c.show_in_list,c.show_in_export,c.sort_order
FROM sample_info_types t
CROSS JOIN sample_info_columns c
WHERE t.deleted_at IS NULL AND c.deleted_at IS NULL
ON CONFLICT(type_key,column_id) DO NOTHING;

ALTER TABLE notification_rules
  ADD COLUMN IF NOT EXISTS sample_info_type_key TEXT NOT NULL DEFAULT '';

CREATE INDEX IF NOT EXISTS idx_notification_rules_sample_type
  ON notification_rules(event_type,sample_info_type_key,group_id,project_name,is_active);

INSERT INTO schema_migrations(version)
VALUES ('1.1.5-beta.6-sample-type-fields-and-notifications')
ON CONFLICT DO NOTHING;
