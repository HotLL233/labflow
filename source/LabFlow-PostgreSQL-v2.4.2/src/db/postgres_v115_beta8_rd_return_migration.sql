-- v1.1.5-beta.8: analyst return workflow for RD sample submissions.
ALTER TABLE rd_work_records
  ADD COLUMN IF NOT EXISTS return_reason TEXT NOT NULL DEFAULT '';
ALTER TABLE rd_work_records
  ADD COLUMN IF NOT EXISTS returned_by TEXT NOT NULL DEFAULT '';
ALTER TABLE rd_work_records
  ADD COLUMN IF NOT EXISTS returned_at TEXT;
ALTER TABLE rd_work_records
  ADD COLUMN IF NOT EXISTS return_confirmed_at TEXT;
ALTER TABLE rd_work_records
  ADD COLUMN IF NOT EXISTS return_confirmed_by TEXT NOT NULL DEFAULT '';

CREATE INDEX IF NOT EXISTS idx_rd_records_returned_group
  ON rd_work_records (group_id, status)
  WHERE deleted_at IS NULL AND status = '已退回';

-- Analysis roles receive the new operation by default. Existing custom roles
-- remain unchanged and can grant the permission explicitly in role management.
INSERT INTO role_permissions(role_id, permission_key)
SELECT id, 'sample:return'
FROM roles
WHERE id IN (2, 8)
ON CONFLICT DO NOTHING;

INSERT INTO schema_migrations(version)
VALUES ('1.1.5-beta.8-rd-return-workflow')
ON CONFLICT DO NOTHING;
