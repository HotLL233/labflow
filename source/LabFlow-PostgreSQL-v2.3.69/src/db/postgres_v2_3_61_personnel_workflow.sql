-- New summary readers and analyst-leader maintenance; do not restore previously
-- revoked leader permissions that were already shipped in v2.3.38.
BEGIN;

INSERT INTO role_template_permissions(template_id, permission_key)
SELECT id, 'feedback:personnel:view' FROM role_templates
WHERE deleted_at IS NULL AND name IN ('分析检测员模板','研发送样员模板')
ON CONFLICT DO NOTHING;

INSERT INTO role_template_permissions(template_id, permission_key)
SELECT id, 'feedback:personnel:edit' FROM role_templates
WHERE deleted_at IS NULL AND name='分析检测组长模板'
ON CONFLICT DO NOTHING;

INSERT INTO role_permissions(role_id, permission_key)
SELECT r.id, 'feedback:personnel:view' FROM roles r
LEFT JOIN role_templates t ON t.id=r.template_id AND t.deleted_at IS NULL
WHERE r.deleted_at IS NULL
  AND COALESCE(r.system_key,'') NOT IN ('public_account','rd_public_account','analysis_public_account')
  AND (r.system_key IN ('analyst','rd_sender') OR t.name IN ('分析检测员模板','研发送样员模板'))
ON CONFLICT DO NOTHING;

INSERT INTO role_permissions(role_id, permission_key)
SELECT r.id, 'feedback:personnel:edit' FROM roles r
LEFT JOIN role_templates t ON t.id=r.template_id AND t.deleted_at IS NULL
WHERE r.deleted_at IS NULL
  AND COALESCE(r.system_key,'') NOT IN ('public_account','rd_public_account','analysis_public_account')
  AND (r.system_key='analysis_leader' OR t.name='分析检测组长模板')
ON CONFLICT DO NOTHING;

ALTER TABLE notification_events
  ADD COLUMN IF NOT EXISTS processing_token TEXT NOT NULL DEFAULT '';

ALTER TABLE notification_rule_targets
  DROP CONSTRAINT IF EXISTS notification_rule_targets_target_kind_check;
ALTER TABLE notification_rule_targets
  ADD CONSTRAINT notification_rule_targets_target_kind_check
  CHECK(target_kind IN ('rd_work_record','rd_work_record_rejected','rd_work_record_resubmitted','sample_info','personnel_change_feedback','personnel_change_feedback_rejected','personnel_change_feedback_approved'));

INSERT INTO schema_migrations(version)
VALUES ('2.3.61-personnel-workflow') ON CONFLICT DO NOTHING;
COMMIT;
