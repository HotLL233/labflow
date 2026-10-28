CREATE TABLE IF NOT EXISTS auxiliary_work_areas (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    sort_order BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);

-- Preserve referenced IDs and old recording behavior, including soft-deleted source types.
INSERT INTO auxiliary_work_areas(id,name,sort_order)
SELECT mt.id,mt.name,COALESCE(mt.sort_order,0)
FROM method_types mt
WHERE EXISTS(SELECT 1 FROM auxiliary_work_items ai WHERE ai.area_type_id=mt.id)
   OR EXISTS(SELECT 1 FROM auxiliary_work_records ar WHERE ar.area_type_id=mt.id)
ON CONFLICT(id) DO NOTHING;

SELECT setval(
    pg_get_serial_sequence('auxiliary_work_areas','id'),
    COALESCE((SELECT MAX(id) FROM auxiliary_work_areas),1),
    EXISTS(SELECT 1 FROM auxiliary_work_areas)
);

ALTER TABLE auxiliary_work_items
    DROP CONSTRAINT IF EXISTS auxiliary_work_items_area_type_id_fkey;
ALTER TABLE auxiliary_work_items
    ADD CONSTRAINT auxiliary_work_items_area_type_id_fkey
    FOREIGN KEY(area_type_id) REFERENCES auxiliary_work_areas(id) ON DELETE RESTRICT;
ALTER TABLE auxiliary_work_records
    DROP CONSTRAINT IF EXISTS auxiliary_work_records_area_type_id_fkey;
ALTER TABLE auxiliary_work_records
    ADD CONSTRAINT auxiliary_work_records_area_type_id_fkey
    FOREIGN KEY(area_type_id) REFERENCES auxiliary_work_areas(id) ON DELETE RESTRICT;

UPDATE trash_entries te
SET dependency_summary='关联方法 '||(SELECT COUNT(*) FROM method_type_links ml WHERE ml.method_type_id=te.record_id)||' 个',
    can_purge=CASE WHEN EXISTS(SELECT 1 FROM method_type_links ml WHERE ml.method_type_id=te.record_id) THEN 0 ELSE 1 END
WHERE te.table_name='method_types' AND te.restored_at IS NULL AND te.purged_at IS NULL;

INSERT INTO schema_migrations(version)
VALUES ('2.3.71-independent-auxiliary-areas')
ON CONFLICT DO NOTHING;
