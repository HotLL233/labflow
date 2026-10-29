-- v1.1.6-hotfix.16: independent notification event for returned records resubmitted after editing.
ALTER TABLE notification_rule_targets
  DROP CONSTRAINT IF EXISTS notification_rule_targets_target_kind_check;

ALTER TABLE notification_rule_targets
  ADD CONSTRAINT notification_rule_targets_target_kind_check
  CHECK(target_kind IN ('rd_work_record','rd_work_record_rejected','rd_work_record_resubmitted','sample_info'));

INSERT INTO schema_migrations(version)
VALUES ('1.1.6-hotfix.16-rd-resubmission-notifications')
ON CONFLICT DO NOTHING;
