-- Area is an auxiliary classification/navigation dimension, never a detection type snapshot.
-- Old items and records intentionally remain unassigned; historical workload is not recalculated.
ALTER TABLE auxiliary_work_items
    ADD COLUMN IF NOT EXISTS area_type_id BIGINT REFERENCES method_types(id) ON DELETE RESTRICT;
ALTER TABLE auxiliary_work_records
    ADD COLUMN IF NOT EXISTS area_type_id BIGINT REFERENCES method_types(id) ON DELETE RESTRICT;
ALTER TABLE auxiliary_work_records
    ADD COLUMN IF NOT EXISTS area_name_snapshot TEXT NOT NULL DEFAULT '';

CREATE INDEX IF NOT EXISTS idx_auxiliary_work_items_area
    ON auxiliary_work_items(area_type_id,sort_order,id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_auxiliary_work_records_area_time
    ON auxiliary_work_records(area_type_id,recorded_at DESC,id DESC) WHERE deleted_at IS NULL;

INSERT INTO schema_migrations(version)
VALUES ('2.3.70-auxiliary-areas')
ON CONFLICT DO NOTHING;
