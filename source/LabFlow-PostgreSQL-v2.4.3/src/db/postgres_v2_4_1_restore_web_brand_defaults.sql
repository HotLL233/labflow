-- Restore only untouched defaults written by v2.4.0, including their original seed timestamp.
UPDATE system_settings AS settings
SET value = defaults.new_value,
    updated_at = to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS')
FROM (VALUES
    ('theme', '{"primaryColor":"#667eea","secondaryColor":"#764ba2","bgColor":"#f8fafc","cardRadius":2,"loginBg":"linear-gradient(135deg, #f0f4f8, #e8f5e9)","loginButtonColor":"#f4511e","logoText":"labflow"}', '{"primaryColor":"#667eea","secondaryColor":"#764ba2","bgColor":"#f8fafc","cardRadius":2,"loginBg":"linear-gradient(135deg, #f0f4f8, #e8f5e9)","loginButtonColor":"#f4511e","logoText":"知微"}'),
    ('portal_styles', '{"sampleColor":"#e65100","workloadColor":"#1976d2","brandName":"labflow"}', '{"sampleColor":"#e65100","workloadColor":"#1976d2","brandName":"知微"}'),
    ('layout_home', '{"sections":{"brand-title":{"visible":true,"label":"labflow","color":"#667eea"},"entry-cards":{"visible":true},"footer-text":{"visible":true,"label":"选择功能入口，开始操作"}}}', '{"sections":{"brand-title":{"visible":true,"label":"知微","color":"#667eea"},"entry-cards":{"visible":true},"footer-text":{"visible":true,"label":"选择功能入口，开始操作"}}}')
) AS defaults(key, old_value, new_value)
WHERE settings.key = defaults.key
  AND settings.value = defaults.old_value
  AND settings.updated_at = '2026-07-22 14:39:46';
INSERT INTO schema_migrations(version) VALUES ('2.4.1-restore-web-brand-defaults') ON CONFLICT DO NOTHING;