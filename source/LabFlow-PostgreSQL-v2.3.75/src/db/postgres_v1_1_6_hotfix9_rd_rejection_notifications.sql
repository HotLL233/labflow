-- Add a distinct target kind for research-sample rejection notifications.
-- Existing submission targets and rules remain unchanged.
ALTER TABLE notification_rule_targets
  DROP CONSTRAINT IF EXISTS notification_rule_targets_target_kind_check;

ALTER TABLE notification_rule_targets
  ADD CONSTRAINT notification_rule_targets_target_kind_check
  CHECK(target_kind IN ('rd_work_record','rd_work_record_rejected','sample_info'));

INSERT INTO schema_migrations(version)
VALUES ('1.1.6-hotfix.9-rd-rejection-group-notifications')
ON CONFLICT DO NOTHING;
