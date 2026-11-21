-- v1.3.2: allow the independent personnel-change feedback notification target.
ALTER TABLE notification_rule_targets
  DROP CONSTRAINT IF EXISTS notification_rule_targets_target_kind_check;

ALTER TABLE notification_rule_targets
  ADD CONSTRAINT notification_rule_targets_target_kind_check
  CHECK(target_kind IN ('rd_work_record','rd_work_record_rejected','rd_work_record_resubmitted','sample_info','personnel_change_feedback'));

INSERT INTO schema_migrations(version)
VALUES ('1.3.2-personnel-feedback-notification-target')
ON CONFLICT DO NOTHING;
