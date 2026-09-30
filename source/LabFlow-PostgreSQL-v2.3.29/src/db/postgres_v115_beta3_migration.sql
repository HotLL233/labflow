-- v1.1.5-beta.3: portal visibility and common methods.
ALTER TABLE divisions ADD COLUMN IF NOT EXISTS show_in_work BIGINT NOT NULL DEFAULT 1;
ALTER TABLE divisions ADD COLUMN IF NOT EXISTS show_in_rd BIGINT NOT NULL DEFAULT 1;
ALTER TABLE divisions ADD COLUMN IF NOT EXISTS show_in_sample_info BIGINT NOT NULL DEFAULT 1;

ALTER TABLE project_groups ADD COLUMN IF NOT EXISTS show_in_sample_info BIGINT NOT NULL DEFAULT 1;

ALTER TABLE projects ADD COLUMN IF NOT EXISTS show_in_work BIGINT NOT NULL DEFAULT 1;
ALTER TABLE projects ADD COLUMN IF NOT EXISTS show_in_rd BIGINT NOT NULL DEFAULT 1;
ALTER TABLE projects ADD COLUMN IF NOT EXISTS show_in_sample_info BIGINT NOT NULL DEFAULT 1;

ALTER TABLE methods ADD COLUMN IF NOT EXISTS show_in_work BIGINT NOT NULL DEFAULT 1;
ALTER TABLE methods ADD COLUMN IF NOT EXISTS show_in_rd BIGINT NOT NULL DEFAULT 1;
ALTER TABLE methods ADD COLUMN IF NOT EXISTS show_in_sample_info BIGINT NOT NULL DEFAULT 1;
ALTER TABLE methods ADD COLUMN IF NOT EXISTS is_common BIGINT NOT NULL DEFAULT 0;

-- Some legacy projects kept their primary lab only in projects.group_id.
INSERT INTO project_lab_links(project_id, group_id)
SELECT p.id, p.group_id
FROM projects p
WHERE p.group_id IS NOT NULL
  AND NOT EXISTS (
      SELECT 1 FROM project_lab_links pll
      WHERE pll.project_id = p.id AND pll.group_id = p.group_id
  )
ON CONFLICT DO NOTHING;

CREATE INDEX IF NOT EXISTS idx_divisions_portals
    ON divisions(show_in_work, show_in_rd, show_in_sample_info);
CREATE INDEX IF NOT EXISTS idx_project_groups_portals
    ON project_groups(show_in_work, show_in_rd, show_in_sample_info);
CREATE INDEX IF NOT EXISTS idx_projects_portals
    ON projects(show_in_work, show_in_rd, show_in_sample_info);
CREATE INDEX IF NOT EXISTS idx_methods_portals_common
    ON methods(show_in_work, show_in_rd, show_in_sample_info, is_common);

INSERT INTO schema_migrations(version)
VALUES ('1.1.5-beta.3-portal-visibility-common-methods')
ON CONFLICT DO NOTHING;
