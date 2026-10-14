-- v2.3.20: 记录表列宽模式。
--
-- 背景：此前 `width` 是列宽的唯一真值，前端直接把它归一化成百分比，导致列宽与内容无关：
-- 内容很短的列（部门/实验室/高项）拿到大量宽度，而批号、主要成分、注意事项这类长内容列
-- 装不下自身内容（样品信息登记记录的批号列只有 78px，却要显示 18 个字符）。
--
-- 本次把 `width` 的语义收窄为「custom 模式下的固定宽度」，并新增：
--   width_mode = 'auto'   由前端按内容测量（默认，升级即生效）
--   width_mode = 'custom' 使用 width 作为固定宽度（等同升级前行为）
-- 升级时保留原有 width 值不删除，管理员切回 custom 即可直接复用原配置。
--
-- min_width / max_width 为 0 表示沿用前端按输入方式给出的系统默认区间。
ALTER TABLE rd_record_columns ADD COLUMN IF NOT EXISTS width_mode TEXT NOT NULL DEFAULT 'auto';
ALTER TABLE rd_record_columns ADD COLUMN IF NOT EXISTS min_width BIGINT NOT NULL DEFAULT 0;
ALTER TABLE rd_record_columns ADD COLUMN IF NOT EXISTS max_width BIGINT NOT NULL DEFAULT 0;

ALTER TABLE sample_info_columns ADD COLUMN IF NOT EXISTS width_mode TEXT NOT NULL DEFAULT 'auto';
ALTER TABLE sample_info_columns ADD COLUMN IF NOT EXISTS min_width BIGINT NOT NULL DEFAULT 0;
ALTER TABLE sample_info_columns ADD COLUMN IF NOT EXISTS max_width BIGINT NOT NULL DEFAULT 0;

-- 记录表显示开关：
--   record_table_action_mode = auto（默认，按容器宽度收敛操作列）/ full（始终显示全部按钮）
--   record_table_density       = auto（按容器宽度自动选密度档）/ compact / standard / comfortable
INSERT INTO system_settings(key, value, updated_at)
VALUES ('record_table_action_mode', 'auto', to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS'))
ON CONFLICT DO NOTHING;

INSERT INTO system_settings(key, value, updated_at)
VALUES ('record_table_density', 'auto', to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS'))
ON CONFLICT DO NOTHING;

INSERT INTO schema_migrations(version)
VALUES ('2.3.20-column-width-mode')
ON CONFLICT DO NOTHING;
