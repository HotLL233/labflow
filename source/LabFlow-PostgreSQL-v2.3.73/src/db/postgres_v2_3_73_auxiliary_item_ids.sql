ALTER TABLE auxiliary_work_items
    ADD COLUMN IF NOT EXISTS item_code TEXT NOT NULL DEFAULT '';

UPDATE auxiliary_work_items
SET item_code = 'A-' || lpad(id::text, GREATEST(8, length(id::text)), '0')
WHERE item_code = '';

CREATE UNIQUE INDEX IF NOT EXISTS uq_auxiliary_work_item_code
    ON auxiliary_work_items(item_code)
    WHERE item_code <> '';

INSERT INTO schema_migrations(version)
VALUES ('2.3.73-auxiliary-item-ids')
ON CONFLICT DO NOTHING;
