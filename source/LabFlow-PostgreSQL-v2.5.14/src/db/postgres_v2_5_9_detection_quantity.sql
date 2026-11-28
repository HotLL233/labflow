-- Attribute each completed sample to the record that completed its last required step.
CREATE INDEX IF NOT EXISTS idx_step_work_entries_group_step_normal
    ON step_work_entries(group_id, step_id) WHERE NOT rework;
CREATE OR REPLACE FUNCTION labflow_detection_quantity(record_id BIGINT)
RETURNS BIGINT LANGUAGE SQL STABLE AS $$
    SELECT CASE WHEN entry.work_record_id IS NULL THEN current_record.quantity::BIGINT
                WHEN entry.rework THEN 0
                ELSE COALESCE(progress.after_quantity - progress.before_quantity, 0) END
    FROM work_records current_record
    LEFT JOIN step_work_entries entry ON entry.work_record_id = current_record.id
    LEFT JOIN LATERAL (
        SELECT MIN(step_progress.after_quantity) AS after_quantity,
               MIN(step_progress.before_quantity) AS before_quantity
        FROM detection_steps step
        CROSS JOIN LATERAL (
            SELECT COALESCE(SUM(w.quantity) FILTER (WHERE
                       (labflow_try_timestamptz(w.recorded_at), w.id) <=
                       (labflow_try_timestamptz(current_record.recorded_at), current_record.id)), 0)::BIGINT AS after_quantity,
                   COALESCE(SUM(w.quantity) FILTER (WHERE
                       (labflow_try_timestamptz(w.recorded_at), w.id) <
                       (labflow_try_timestamptz(current_record.recorded_at), current_record.id)), 0)::BIGINT AS before_quantity
            FROM step_work_entries e JOIN work_records w ON w.id=e.work_record_id
            WHERE e.group_id=entry.group_id AND e.step_id=step.id
              AND NOT e.rework AND w.deleted_at IS NULL
        ) step_progress
        WHERE step.detection_id=entry.detection_id
    ) progress ON entry.work_record_id IS NOT NULL AND NOT entry.rework
    WHERE current_record.id=record_id
$$;
INSERT INTO schema_migrations(version) VALUES ('2.5.9-detection-quantity') ON CONFLICT DO NOTHING;
