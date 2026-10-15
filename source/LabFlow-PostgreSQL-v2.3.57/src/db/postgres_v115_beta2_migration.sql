-- v1.1.5-beta.2: unify notification templates and expose RD record columns.
ALTER TABLE notification_templates
  ADD COLUMN IF NOT EXISTS footer TEXT NOT NULL DEFAULT '';

UPDATE notification_templates
SET event_key='sample_info'
WHERE event_key='sample_submitted'
  AND NOT EXISTS (SELECT 1 FROM notification_templates WHERE event_key='sample_info');

UPDATE notification_templates
SET is_active=0
WHERE event_key='sample_submitted';

UPDATE notification_templates
SET footer='请分析检测人员进入系统处理。'
WHERE event_key IN ('sample_info','rd_work_record') AND BTRIM(footer)='';

INSERT INTO notification_templates(event_key,title,footer,field_config_json,is_active)
VALUES (
  'sample_info',
  '新样品待取样',
  '请分析检测人员进入系统处理。',
  '[{"key":"batch_no","label":"批号","visible":true,"enabled":true,"bold":true},{"key":"business_no","label":"送样编号","visible":true,"enabled":true,"bold":false},{"key":"submitted_at","label":"送样时间","visible":true,"enabled":true,"bold":false},{"key":"submitted_by","label":"送样人","visible":true,"enabled":true,"bold":false},{"key":"division_name","label":"部门","visible":true,"enabled":true,"bold":false},{"key":"lab_name","label":"实验室","visible":true,"enabled":true,"bold":false},{"key":"project_name","label":"项目","visible":true,"enabled":true,"bold":false},{"key":"detection_type","label":"检测类型","visible":true,"enabled":true,"bold":false},{"key":"method_name","label":"方法","visible":true,"enabled":true,"bold":false},{"key":"quantity","label":"数量","visible":true,"enabled":true,"bold":true},{"key":"main_components","label":"主要成分","visible":true,"enabled":true,"bold":false},{"key":"notes","label":"备注","visible":true,"enabled":true,"bold":false},{"key":"attachment_count","label":"附件数量","visible":true,"enabled":true,"bold":false},{"key":"status","label":"状态","visible":true,"enabled":true,"bold":false}]',
  1
)
ON CONFLICT(event_key) DO NOTHING;

INSERT INTO notification_templates(event_key,title,footer,field_config_json,is_active)
VALUES (
  'rd_work_record',
  '新研发送样记录',
  '请分析检测人员进入系统处理。',
  '[{"key":"batch_no","label":"批号","visible":true,"enabled":true,"bold":true},{"key":"business_no","label":"送样编号","visible":true,"enabled":true,"bold":false},{"key":"submitted_at","label":"送样时间","visible":true,"enabled":true,"bold":false},{"key":"submitted_by","label":"送样人","visible":true,"enabled":true,"bold":false},{"key":"division_name","label":"部门","visible":true,"enabled":true,"bold":false},{"key":"lab_name","label":"实验室","visible":true,"enabled":true,"bold":false},{"key":"project_name","label":"项目","visible":true,"enabled":true,"bold":false},{"key":"detection_type","label":"检测类型","visible":true,"enabled":true,"bold":false},{"key":"method_name","label":"方法","visible":true,"enabled":true,"bold":false},{"key":"instrument_code","label":"仪器","visible":true,"enabled":true,"bold":false},{"key":"quantity","label":"数量","visible":true,"enabled":true,"bold":true},{"key":"notes","label":"备注","visible":true,"enabled":true,"bold":false},{"key":"status","label":"状态","visible":true,"enabled":true,"bold":false}]',
  1
)
ON CONFLICT(event_key) DO NOTHING;

INSERT INTO rd_record_columns(name,label,data_type,width,sort_order,is_predefined,show_in_list,show_in_form)
SELECT 'batch_no','批号','text',100,13,1,1,1
WHERE NOT EXISTS (SELECT 1 FROM rd_record_columns WHERE name='batch_no');

INSERT INTO rd_record_columns(name,label,data_type,width,sort_order,is_predefined,show_in_list,show_in_form)
SELECT 'instrument_code','仪器','text',90,14,1,1,0
WHERE NOT EXISTS (SELECT 1 FROM rd_record_columns WHERE name='instrument_code');

UPDATE rd_record_columns
SET label='批号', show_in_list=1, updated_at=to_char(CURRENT_TIMESTAMP,'YYYY-MM-DD HH24:MI:SS')
WHERE name='batch_no';

UPDATE rd_record_columns
SET label='仪器', show_in_list=1, updated_at=to_char(CURRENT_TIMESTAMP,'YYYY-MM-DD HH24:MI:SS')
WHERE name='instrument_code';

INSERT INTO schema_migrations(version)
VALUES ('1.1.5-beta.2-notification-display-and-record-columns')
ON CONFLICT DO NOTHING;
