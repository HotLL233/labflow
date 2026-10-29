CREATE TABLE IF NOT EXISTS legacy_auxiliary_batches (
    id TEXT PRIMARY KEY,
    reason TEXT NOT NULL,
    request_fingerprint TEXT NOT NULL,
    created_by BIGINT NOT NULL REFERENCES users(id),
    created_by_name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    record_count BIGINT NOT NULL CHECK(record_count BETWEEN 1 AND 100),
    total_quantity BIGINT NOT NULL CHECK(total_quantity > 0),
    total_workload DOUBLE PRECISION NOT NULL CHECK(total_workload >= 0 AND total_workload < 'Infinity'::DOUBLE PRECISION),
    revoked_at TIMESTAMPTZ,
    revoked_by BIGINT REFERENCES users(id),
    revoke_reason TEXT
);

CREATE TABLE IF NOT EXISTS legacy_auxiliary_links (
    id BIGSERIAL PRIMARY KEY,
    source_table TEXT NOT NULL DEFAULT 'work_records' CHECK(source_table = 'work_records'),
    source_record_id BIGINT NOT NULL REFERENCES work_records(id) ON DELETE RESTRICT,
    auxiliary_work_item_id BIGINT NOT NULL REFERENCES auxiliary_work_items(id) ON DELETE RESTRICT,
    area_type_id_snapshot BIGINT REFERENCES auxiliary_work_areas(id) ON DELETE RESTRICT,
    area_name_snapshot TEXT NOT NULL DEFAULT '',
    category_snapshot TEXT NOT NULL DEFAULT '',
    batch_id TEXT NOT NULL REFERENCES legacy_auxiliary_batches(id) ON DELETE RESTRICT,
    source_fingerprint TEXT NOT NULL,
    source_snapshot JSONB NOT NULL,
    created_by BIGINT NOT NULL REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    revoked_at TIMESTAMPTZ
);
CREATE UNIQUE INDEX IF NOT EXISTS uq_legacy_auxiliary_active_source
    ON legacy_auxiliary_links(source_table,source_record_id) WHERE revoked_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_legacy_auxiliary_source
    ON legacy_auxiliary_links(source_table,source_record_id);
CREATE INDEX IF NOT EXISTS idx_legacy_auxiliary_batch ON legacy_auxiliary_links(batch_id);
CREATE INDEX IF NOT EXISTS idx_legacy_auxiliary_item ON legacy_auxiliary_links(auxiliary_work_item_id)
    WHERE revoked_at IS NULL;

INSERT INTO schema_migrations(version) VALUES ('2.3.77-legacy-auxiliary-links')
ON CONFLICT DO NOTHING;
