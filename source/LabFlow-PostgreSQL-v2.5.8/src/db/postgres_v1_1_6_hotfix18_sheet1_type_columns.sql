UPDATE system_settings
SET value = jsonb_set(
        value::jsonb,
        '{sheets,sheet1,columns}',
        '{"lab_name":{"label":"使用实验室","width":14},"project_name":{"label":"项目代号","width":18},"instrument":{"label":"仪器","width":18},"method_name":{"label":"检测方法","width":30},"method_type":{"label":"检测类型","width":14},"quantity":{"label":"检测数量","width":12},"project_total":{"label":"项目检测总量","width":15},"high_item":{"label":"高项","width":12}}'::jsonb,
        true
    )::text,
    updated_at = to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS')
WHERE key IN ('export_template_workload', 'export_template_rd')
  AND value LIKE '%"lc_instrument"%';

INSERT INTO schema_migrations(version)
VALUES ('1.1.6-hotfix.18-sheet1-type-columns')
ON CONFLICT DO NOTHING;
