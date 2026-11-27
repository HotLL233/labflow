-- v2.3.8: sample workload belongs to the account that confirmed the entry.
-- The pre-v2.3.8 implementation used the sampling operator instead. The
-- immutable creator fields identify the account that actually submitted it.

UPDATE work_records
SET subject_user_id = created_by_user_id,
    user_name = COALESCE(
        (SELECT u.username FROM users u WHERE u.id = created_by_user_id),
        user_name
    )
WHERE source_type = 'rd_sample'
  AND created_by_user_id IS NOT NULL
  AND subject_user_id IS DISTINCT FROM created_by_user_id;

INSERT INTO schema_migrations(version)
VALUES ('2.3.8-sample-workload-operator-ownership')
ON CONFLICT DO NOTHING;
