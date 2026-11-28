ALTER TABLE work_records ADD COLUMN IF NOT EXISTS unit_price_snapshot DOUBLE PRECISION;
ALTER TABLE rd_work_records ADD COLUMN IF NOT EXISTS unit_price_snapshot DOUBLE PRECISION;
ALTER TABLE rd_work_records ADD COLUMN IF NOT EXISTS billing_multiplier_snapshot DOUBLE PRECISION;

DROP TRIGGER IF EXISTS work_record_billing_snapshot ON work_records;
CREATE TRIGGER work_record_billing_snapshot BEFORE INSERT OR UPDATE OF method_id ON work_records
    FOR EACH ROW EXECUTE FUNCTION labflow_capture_billing_snapshot();
DROP TRIGGER IF EXISTS rd_record_billing_snapshot ON rd_work_records;
CREATE TRIGGER rd_record_billing_snapshot BEFORE INSERT OR UPDATE OF method_id ON rd_work_records
    FOR EACH ROW EXECUTE FUNCTION labflow_capture_billing_snapshot();

UPDATE step_detection_charges c
SET voided_at=CURRENT_TIMESTAMP,
    void_reason=CASE WHEN c.rework_record_id IS NULL THEN 'incomplete_after_legacy_delete' ELSE 'record_deleted' END
FROM step_detections d
WHERE c.detection_id=d.id AND c.voided_at IS NULL AND (
    (c.rework_record_id IS NOT NULL AND EXISTS (
        SELECT 1 FROM work_records w WHERE w.id=c.rework_record_id AND w.deleted_at IS NOT NULL
    )) OR (c.rework_record_id IS NULL AND (
        d.quantity <> (SELECT COALESCE(SUM(g.quantity),0) FROM detection_quantity_groups g WHERE g.detection_id=d.id)
        OR NOT EXISTS (SELECT 1 FROM detection_steps s WHERE s.detection_id=d.id)
        OR EXISTS (
            SELECT 1 FROM detection_quantity_groups g JOIN detection_steps s ON s.detection_id=g.detection_id
            WHERE g.detection_id=d.id AND g.quantity <> (
                SELECT COALESCE(SUM(w.quantity),0) FROM step_work_entries e JOIN work_records w ON w.id=e.work_record_id
                WHERE e.group_id=g.id AND e.step_id=s.id AND NOT e.rework AND w.deleted_at IS NULL
            )
        )
    ))
);

INSERT INTO schema_migrations(version) VALUES ('2.5.14-billing-snapshots') ON CONFLICT DO NOTHING;
