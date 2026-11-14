ALTER TABLE notification_events ADD COLUMN IF NOT EXISTS status_reason TEXT NOT NULL DEFAULT '';
ALTER TABLE notification_events ADD COLUMN IF NOT EXISTS processing_failures BIGINT NOT NULL DEFAULT 0;
ALTER TABLE notification_rule_targets DROP CONSTRAINT IF EXISTS notification_rule_targets_target_kind_check;
ALTER TABLE notification_rule_targets ADD CONSTRAINT notification_rule_targets_target_kind_check
CHECK(target_kind IN ('rd_work_record','rd_work_record_rejected','rd_work_record_resubmitted','sample_info','sample_info_rejected','personnel_change_feedback','personnel_change_feedback_rejected','personnel_change_feedback_approved'));
CREATE INDEX IF NOT EXISTS idx_notification_deliveries_event_created ON notification_deliveries(event_id,created_at,id);
INSERT INTO schema_migrations(version) VALUES ('2.4.3-notification-integrity') ON CONFLICT DO NOTHING;
