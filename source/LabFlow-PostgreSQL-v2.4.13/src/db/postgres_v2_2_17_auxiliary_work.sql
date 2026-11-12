CREATE TABLE IF NOT EXISTS auxiliary_work_items (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    code TEXT NOT NULL DEFAULT '',
    coefficient DOUBLE PRECISION NOT NULL DEFAULT 1.0,
    scope_type TEXT NOT NULL DEFAULT 'laboratory',
    group_id BIGINT REFERENCES project_groups(id),
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    sort_order INTEGER NOT NULL DEFAULT 0,
    notes TEXT NOT NULL DEFAULT '',
    created_by BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deleted_at TIMESTAMPTZ
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_auxiliary_work_item_name_scope
    ON auxiliary_work_items(name, COALESCE(group_id, 0))
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS auxiliary_work_records (
    id BIGSERIAL PRIMARY KEY,
    auxiliary_work_item_id BIGINT REFERENCES auxiliary_work_items(id),
    group_id BIGINT REFERENCES project_groups(id),
    project_id BIGINT REFERENCES projects(id),
    user_id BIGINT REFERENCES users(id),
    user_name_snapshot TEXT NOT NULL,
    group_name_snapshot TEXT NOT NULL DEFAULT '',
    project_name_snapshot TEXT NOT NULL DEFAULT '',
    auxiliary_work_name_snapshot TEXT NOT NULL,
    coefficient_snapshot DOUBLE PRECISION NOT NULL DEFAULT 1.0,
    quantity BIGINT NOT NULL CHECK (quantity > 0),
    recorded_at TIMESTAMPTZ NOT NULL,
    notes TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT '有效',
    created_by BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deleted_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_auxiliary_work_records_scope_time
    ON auxiliary_work_records(group_id, recorded_at)
    WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_auxiliary_work_records_user_time
    ON auxiliary_work_records(user_id, recorded_at)
    WHERE deleted_at IS NULL;

INSERT INTO schema_migrations(version)
VALUES ('2.2.17-auxiliary-work')
ON CONFLICT DO NOTHING;
