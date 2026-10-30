-- v2.3.12: expose the complete sample-information workflow action set.
INSERT INTO sample_info_columns
  (field_key,label,data_type,is_predefined,is_required,is_active,width,sort_order,options,show_in_list,show_in_export,show_in_form)
VALUES
  ('action_collect','取样','action',1,0,1,84,14,NULL,1,0,0),
  ('action_withdraw','撤回取样','action',1,0,1,100,15,NULL,1,0,0),
  ('action_complete','完成检测','action',1,0,1,100,16,NULL,1,0,0)
ON CONFLICT (field_key) DO NOTHING;

INSERT INTO sample_info_column_visibility
  (column_id,type_key,is_visible,is_required,show_in_form,show_in_list,show_in_export,sort_order)
SELECT c.id,t.type_key,1,0,0,1,0,c.sort_order
FROM sample_info_columns c
JOIN sample_info_types t ON t.is_active=1 AND t.deleted_at IS NULL
WHERE c.field_key IN ('action_collect','action_withdraw','action_complete')
ON CONFLICT (type_key,column_id) DO NOTHING;

INSERT INTO schema_migrations(version)
VALUES ('2.3.12-sample-info-action-columns')
ON CONFLICT DO NOTHING;
