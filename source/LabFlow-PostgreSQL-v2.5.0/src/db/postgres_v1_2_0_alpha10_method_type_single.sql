-- v1.2.0-alpha.10: enforce the master-data rule that one method has one detection type.
-- Keep the first type by the configured display order, then remove historical extras.
WITH ranked_links AS (
    SELECT
        mtl.method_id,
        mtl.method_type_id,
        ROW_NUMBER() OVER (
            PARTITION BY mtl.method_id
            ORDER BY COALESCE(mt.sort_order, 0), mt.id
        ) AS row_no
    FROM method_type_links mtl
    LEFT JOIN method_types mt ON mt.id = mtl.method_type_id
)
DELETE FROM method_type_links mtl
USING ranked_links ranked
WHERE ranked.method_id = mtl.method_id
  AND ranked.method_type_id = mtl.method_type_id
  AND ranked.row_no > 1;

CREATE UNIQUE INDEX IF NOT EXISTS uq_method_type_links_method_id
    ON method_type_links(method_id);

INSERT INTO schema_migrations(version)
VALUES ('1.2.0-alpha.10-method-type-single')
ON CONFLICT (version) DO NOTHING;
