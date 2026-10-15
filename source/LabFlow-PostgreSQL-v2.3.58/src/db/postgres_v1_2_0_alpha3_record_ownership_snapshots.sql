-- v1.2.0-alpha.3: immutable business ownership snapshots.
-- The current group/division master data may change after a record is created.
-- Keep the ownership used at entry time so historical permissions and reports
-- do not move when a lab is reassigned to another department.

ALTER TABLE projects ADD COLUMN IF NOT EXISTS project_division_id BIGINT;
ALTER TABLE projects ADD COLUMN IF NOT EXISTS project_division_name_snapshot TEXT NOT NULL DEFAULT '';

ALTER TABLE work_records ADD COLUMN IF NOT EXISTS project_division_id BIGINT;
ALTER TABLE work_records ADD COLUMN IF NOT EXISTS project_division_name_snapshot TEXT NOT NULL DEFAULT '';
ALTER TABLE work_records ADD COLUMN IF NOT EXISTS execution_division_id BIGINT;
ALTER TABLE work_records ADD COLUMN IF NOT EXISTS execution_division_name_snapshot TEXT NOT NULL DEFAULT '';
ALTER TABLE work_records ADD COLUMN IF NOT EXISTS execution_group_id BIGINT;
ALTER TABLE work_records ADD COLUMN IF NOT EXISTS execution_group_name_snapshot TEXT NOT NULL DEFAULT '';

ALTER TABLE rd_work_records ADD COLUMN IF NOT EXISTS project_division_id BIGINT;
ALTER TABLE rd_work_records ADD COLUMN IF NOT EXISTS project_division_name_snapshot TEXT NOT NULL DEFAULT '';
ALTER TABLE rd_work_records ADD COLUMN IF NOT EXISTS execution_division_id BIGINT;
ALTER TABLE rd_work_records ADD COLUMN IF NOT EXISTS execution_division_name_snapshot TEXT NOT NULL DEFAULT '';
ALTER TABLE rd_work_records ADD COLUMN IF NOT EXISTS execution_group_id BIGINT;
ALTER TABLE rd_work_records ADD COLUMN IF NOT EXISTS execution_group_name_snapshot TEXT NOT NULL DEFAULT '';

UPDATE projects p
SET project_division_id = COALESCE(
        (SELECT g.division_id FROM project_groups g WHERE g.id = p.group_id),
        (SELECT g.division_id
         FROM project_lab_links pll
         JOIN project_groups g ON g.id = pll.group_id
         WHERE pll.project_id = p.id
           AND g.deleted_at IS NULL
           AND g.division_id IS NOT NULL
         ORDER BY pll.group_id
         LIMIT 1)
    ),
    project_division_name_snapshot = COALESCE(
        (SELECT d.name
         FROM project_groups g
         JOIN divisions d ON d.id = g.division_id
         WHERE g.id = p.group_id),
        (SELECT d.name
         FROM project_lab_links pll
         JOIN project_groups g ON g.id = pll.group_id
         JOIN divisions d ON d.id = g.division_id
         WHERE pll.project_id = p.id
           AND g.deleted_at IS NULL
         ORDER BY pll.group_id
         LIMIT 1),
        ''
    )
WHERE p.project_division_id IS NULL;

UPDATE work_records wr
SET project_division_id = (SELECT p.project_division_id FROM projects p WHERE p.id = wr.project_id),
    project_division_name_snapshot = COALESCE((SELECT p.project_division_name_snapshot FROM projects p WHERE p.id = wr.project_id), ''),
    execution_division_id = COALESCE(
        wr.division_id,
        (SELECT g.division_id FROM project_groups g WHERE g.id = wr.group_id)
    ),
    execution_division_name_snapshot = COALESCE(
        (SELECT d.name
         FROM divisions d
         WHERE d.id = COALESCE(wr.division_id, (SELECT g.division_id FROM project_groups g WHERE g.id = wr.group_id))),
        ''
    ),
    execution_group_id = wr.group_id,
    execution_group_name_snapshot = COALESCE(
        NULLIF(wr.lab_name_snapshot, ''),
        (SELECT g.name FROM project_groups g WHERE g.id = wr.group_id),
        ''
    )
WHERE wr.execution_division_id IS NULL OR wr.execution_group_id IS NULL;

UPDATE rd_work_records wr
SET project_division_id = (SELECT p.project_division_id FROM projects p WHERE p.id = wr.project_id),
    project_division_name_snapshot = COALESCE((SELECT p.project_division_name_snapshot FROM projects p WHERE p.id = wr.project_id), ''),
    execution_division_id = COALESCE(
        wr.division_id,
        (SELECT g.division_id FROM project_groups g WHERE g.id = wr.group_id)
    ),
    execution_division_name_snapshot = COALESCE(
        (SELECT d.name
         FROM divisions d
         WHERE d.id = COALESCE(wr.division_id, (SELECT g.division_id FROM project_groups g WHERE g.id = wr.group_id))),
        ''
    ),
    execution_group_id = wr.group_id,
    execution_group_name_snapshot = COALESCE(
        NULLIF(wr.lab_name_snapshot, ''),
        (SELECT g.name FROM project_groups g WHERE g.id = wr.group_id),
        ''
    )
WHERE wr.execution_division_id IS NULL OR wr.execution_group_id IS NULL;

CREATE INDEX IF NOT EXISTS idx_work_records_execution_division
    ON work_records(execution_division_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_rd_work_records_execution_division
    ON rd_work_records(execution_division_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_work_records_project_division
    ON work_records(project_division_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_rd_work_records_project_division
    ON rd_work_records(project_division_id) WHERE deleted_at IS NULL;

INSERT INTO schema_migrations(version)
VALUES ('1.2.0-alpha.3-record-ownership-snapshots')
ON CONFLICT DO NOTHING;
