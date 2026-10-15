-- v2.1.0: add an independent notification target for rejected personnel feedback.
ALTER TABLE notification_rule_targets
  DROP CONSTRAINT IF EXISTS notification_rule_targets_target_kind_check;

ALTER TABLE notification_rule_targets
  ADD CONSTRAINT notification_rule_targets_target_kind_check
  CHECK(target_kind IN ('rd_work_record','rd_work_record_rejected','rd_work_record_resubmitted','sample_info','personnel_change_feedback','personnel_change_feedback_rejected'));

INSERT INTO schema_migrations(version)
VALUES ('2.1.0-personnel-feedback-rejection-notification')
ON CONFLICT DO NOTHING;
