-- v2.2.8 性能优化索引
-- 用于批量查询实验室允许的方法ID，避免 N+1 查询问题

-- 为 project_lab_links 表的 group_id 添加索引，加速实验室关联项目查询
CREATE INDEX IF NOT EXISTS "idx_project_lab_links_group" ON "project_lab_links" ("group_id", "project_id");

-- 为 project_method_links 表的 project_id 添加索引，加速项目关联方法查询
CREATE INDEX IF NOT EXISTS "idx_project_method_links_project" ON "project_method_links" ("project_id", "method_id");

-- 为 methods 表的 is_active 和 instrument_id 添加部分索引，仅索引活跃方法
CREATE INDEX IF NOT EXISTS "idx_methods_active_instrument" ON "methods" ("is_active", "instrument_id") WHERE "is_active" = 1;

-- 插入迁移记录
INSERT INTO schema_migrations(version) VALUES ('2.2.8-performance-indexes') ON CONFLICT DO NOTHING;
