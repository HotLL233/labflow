ALTER TABLE notification_events ADD COLUMN IF NOT EXISTS sample_acknowledged_at TEXT;
CREATE INDEX IF NOT EXISTS idx_notification_events_sample_source
    ON notification_events(resource_type,resource_id)
    WHERE event_type IN ('sample_submitted','rd_record_resubmitted');

-- A sampled delivery remains acknowledged even if its notification is dispatched later.
UPDATE notification_events e
SET sample_acknowledged_at=to_char(CURRENT_TIMESTAMP,'YYYY-MM-DD HH24:MI:SS')
WHERE e.sample_acknowledged_at IS NULL AND (
    (e.event_type IN ('sample_submitted','rd_record_resubmitted')
     AND e.resource_type IN ('rd_work_record','rd_work_record_resubmitted')
     AND EXISTS(SELECT 1 FROM rd_work_records r WHERE r.id=e.resource_id AND NULLIF(r.sampled_at,'') IS NOT NULL))
    OR (e.event_type='sample_submitted' AND e.resource_type='sample_info'
     AND EXISTS(SELECT 1 FROM sample_info_records r WHERE r.id=e.resource_id AND NULLIF(r.sampled_at,'') IS NOT NULL))
);
UPDATE in_app_notifications n SET is_read=1
WHERE n.is_read=0 AND EXISTS(
    SELECT 1 FROM notification_events e WHERE e.id=n.event_id AND e.sample_acknowledged_at IS NOT NULL
);

INSERT INTO schema_migrations(version) VALUES('2.3.70-sample-notification-read') ON CONFLICT DO NOTHING;
