CREATE TABLE IF NOT EXISTS sample_submission_receipts (
    actor_user_id BIGINT NOT NULL REFERENCES users(id),
    module TEXT NOT NULL CHECK (module IN ('rd','sample_info')),
    submission_key TEXT NOT NULL CHECK (length(submission_key) BETWEEN 1 AND 128),
    request_fingerprint TEXT NOT NULL CHECK (length(request_fingerprint)=64),
    record_id BIGINT NOT NULL CHECK (record_id>0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY(actor_user_id,module,submission_key)
);
INSERT INTO schema_migrations(version) VALUES('2.5.3-sample-submissions') ON CONFLICT DO NOTHING;
