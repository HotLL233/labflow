-- v2.3.5: make sample-information record actions configurable columns.
INSERT INTO sample_info_columns
  (field_key,label,data_type,is_predefined,is_required,is_active,width,sort_order,options,show_in_list,show_in_export,show_in_form)
VALUES
  ('action_edit','编辑','action',1,0,1,84,17,NULL,1,0,0),
  ('action_record_workload','录入工作量','action',1,0,1,112,18,NULL,1,0,0),
  ('action_return','退回','action',1,0,1,84,19,NULL,1,0,0)
ON CONFLICT (field_key) DO NOTHING;

INSERT INTO sample_info_column_visibility
  (column_id,type_key,is_visible,is_required,show_in_form,show_in_list,show_in_export,sort_order)
SELECT c.id,t.type_key,1,0,0,1,0,c.sort_order
FROM sample_info_columns c
JOIN sample_info_types t ON t.is_active=1 AND t.deleted_at IS NULL
WHERE c.field_key IN ('action_edit','action_record_workload','action_return')
ON CONFLICT (type_key,column_id) DO NOTHING;

INSERT INTO schema_migrations(version) VALUES ('2.3.5-sample-info-action-columns')
ON CONFLICT DO NOTHING;
