-- v1.1.5: keep immutable submission time while ordering workflow records by
-- their latest business action.
ALTER TABLE rd_work_records
  ADD COLUMN IF NOT EXISTS last_activity_at TEXT;

UPDATE rd_work_records
SET last_activity_at = to_char(
  COALESCE(
    NULLIF(return_confirmed_at, '')::timestamp,
    NULLIF(returned_at, '')::timestamp,
    NULLIF(sampled_at, '')::timestamp,
    NULLIF(recorded_at, '')::timestamp,
    CURRENT_TIMESTAMP
  ),
  'YYYY-MM-DD"T"HH24:MI:SS'
)
WHERE last_activity_at IS NULL OR last_activity_at = '';

ALTER TABLE rd_work_records
  ALTER COLUMN last_activity_at SET DEFAULT to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD"T"HH24:MI:SS');

ALTER TABLE rd_work_records
  ALTER COLUMN last_activity_at SET NOT NULL;

CREATE INDEX IF NOT EXISTS idx_rd_records_activity
  ON rd_work_records (last_activity_at DESC, id DESC);

INSERT INTO schema_migrations(version)
VALUES ('1.1.5-final-rd-sender-and-return-workflow')
ON CONFLICT DO NOTHING;
