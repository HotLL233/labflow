-- v1.1.6-hotfix.14-beta
-- Sample-information record numbers are global submission numbers.
-- They must not be regenerated from the current sort/filter result.

CREATE SEQUENCE IF NOT EXISTS sample_info_records_seq_no_seq;

WITH ordered AS (
    SELECT id,
           ROW_NUMBER() OVER (ORDER BY submitted_at ASC, id ASC) AS next_seq_no
    FROM sample_info_records
)
UPDATE sample_info_records r
SET seq_no = ordered.next_seq_no
FROM ordered
WHERE r.id = ordered.id;

SELECT setval(
    'sample_info_records_seq_no_seq',
    GREATEST(COALESCE((SELECT MAX(seq_no) FROM sample_info_records), 1), 1),
    COALESCE((SELECT MAX(seq_no) FROM sample_info_records), 0) > 0
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_sample_info_records_seq_no
    ON sample_info_records (seq_no);
