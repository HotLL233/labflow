-- v1.1.6: keep administrator-customized templates, but upgrade the old
-- built-in RD title so the detection type is visible by default.
UPDATE notification_templates
SET title='【{{检测类型}}】新研发送样记录',
    updated_at=to_char(CURRENT_TIMESTAMP,'YYYY-MM-DD HH24:MI:SS')
WHERE event_key='rd_work_record'
  AND title IN ('新研发送样记录', '新送样待取样');

INSERT INTO schema_migrations(version)
VALUES ('1.1.6-beta.1-notification-dedup-and-title')
ON CONFLICT DO NOTHING;
