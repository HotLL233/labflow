-- v2.3.25：研发送样和样品信息登记的记录表字段显示模式。
-- display_mode：single 单行省略，wrap 自动换行，clamp2 最多两行。
-- header_display_mode：single 单行省略，wrap 自动换行。
ALTER TABLE rd_record_columns ADD COLUMN IF NOT EXISTS display_mode TEXT NOT NULL DEFAULT 'single';
ALTER TABLE rd_record_columns ADD COLUMN IF NOT EXISTS header_display_mode TEXT NOT NULL DEFAULT 'single';
ALTER TABLE sample_info_columns ADD COLUMN IF NOT EXISTS display_mode TEXT NOT NULL DEFAULT 'single';
ALTER TABLE sample_info_columns ADD COLUMN IF NOT EXISTS header_display_mode TEXT NOT NULL DEFAULT 'single';

INSERT INTO schema_migrations(version)
VALUES ('2.3.25-record-column-display-mode')
ON CONFLICT DO NOTHING;
