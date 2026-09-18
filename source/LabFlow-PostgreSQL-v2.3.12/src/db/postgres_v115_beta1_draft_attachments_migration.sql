-- v1.1.5-beta.1: keep the row index for draft attachments.
ALTER TABLE sample_info_draft_attachments
  ADD COLUMN IF NOT EXISTS row_index BIGINT NOT NULL DEFAULT 0;

INSERT INTO schema_migrations(version)
VALUES ('1.1.5-beta.1-draft-attachments') ON CONFLICT DO NOTHING;
