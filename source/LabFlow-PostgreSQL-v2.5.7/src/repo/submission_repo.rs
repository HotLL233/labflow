use crate::error::{AppError, Result};
use postgres_compat::OptionalExtension;
use serde_json::Value;
use sha2::{Digest, Sha256};

pub(crate) struct Submission {
    key: String,
    fingerprint: String,
}

impl Submission {
    pub(crate) fn new(key: &str, payload: &Value) -> Result<Self> {
        if key.is_empty()
            || key.len() > 128
            || !key
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_.:".contains(&c))
        {
            return Err(AppError::Validation(
                "提交标识无效，请刷新页面后重试".into(),
            ));
        }
        let bytes = serde_json::to_vec(payload)
            .map_err(|error| AppError::Internal(format!("serialize submission: {error}")))?;
        Ok(Self {
            key: key.into(),
            fingerprint: format!("{:x}", Sha256::digest(bytes)),
        })
    }

    pub(crate) fn replay_on_conn(
        &self,
        conn: &postgres_compat::Connection,
        actor: i64,
        module: &str,
    ) -> Result<Option<i64>> {
        let digest = Sha256::digest(format!("sample-submission:{actor}:{module}:{}", self.key));
        let lock = i64::from_be_bytes(digest[..8].try_into().expect("eight hash bytes"));
        // The transaction lock also covers the first insert, before any receipt exists.
        conn.query_row(
            "SELECT TRUE FROM pg_advisory_xact_lock(?1)",
            [lock],
            |row| row.get::<_, bool>(0),
        )?;
        let stored: Option<(String, i64)> = conn
            .query_row(
                "SELECT request_fingerprint,record_id FROM sample_submission_receipts WHERE actor_user_id=?1 AND module=?2 AND submission_key=?3",
                postgres_compat::params![actor, module, &self.key],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        match stored {
            Some((fingerprint, id)) if fingerprint == self.fingerprint => Ok(Some(id)),
            Some(_) => Err(AppError::Conflict(
                "该提交标识已用于不同内容，请先核对原提交记录".into(),
            )),
            None => Ok(None),
        }
    }

    pub(crate) fn remember_on_conn(
        &self,
        conn: &postgres_compat::Connection,
        actor: i64,
        module: &str,
        record_id: i64,
    ) -> Result<()> {
        conn.execute(
            "INSERT INTO sample_submission_receipts(actor_user_id,module,submission_key,request_fingerprint,record_id) VALUES(?1,?2,?3,?4,?5)",
            postgres_compat::params![actor, module, &self.key, &self.fingerprint, record_id],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn keys_and_payload_fingerprints_are_strict() {
        for key in ["", "bad key", "\n", "中文"] {
            assert!(Submission::new(key, &json!({})).is_err());
        }
        assert!(Submission::new(&"x".repeat(129), &json!({})).is_err());
        let first = Submission::new("receipt-1", &json!({"b": 2, "a": {"z": 1}})).unwrap();
        let reordered = Submission::new("receipt-1", &json!({"a": {"z": 1}, "b": 2})).unwrap();
        assert_eq!(first.fingerprint, reordered.fingerprint);
        let changed = Submission::new("receipt-1", &json!({"a": {"z": 2}, "b": 2})).unwrap();
        assert_ne!(first.fingerprint, changed.fingerprint);
        let mut headers = axum::http::HeaderMap::new();
        assert!(crate::api::submission_request(&headers, &json!({}))
            .unwrap()
            .is_none());
        headers.insert("idempotency-key", "receipt-1".parse().unwrap());
        assert!(crate::api::submission_request(&headers, &json!({}))
            .unwrap()
            .is_some());
        headers.append("idempotency-key", "receipt-2".parse().unwrap());
        assert!(crate::api::submission_request(&headers, &json!({})).is_err());
    }

    #[test]
    fn receipts_are_transactional_and_scoped_to_actor_and_module() {
        let pool = crate::db::init_pool("postgres-test");
        crate::db::test_migrations::run(&pool.get().unwrap()).unwrap();
        let mut conn = pool.get().unwrap();
        let actor: i64 = conn
            .query_row("SELECT id FROM users WHERE username='admin'", [], |row| {
                row.get(0)
            })
            .unwrap();
        let request = Submission::new("receipt-atomic", &json!({"quantity": 1})).unwrap();
        {
            let tx = conn.transaction().unwrap();
            assert_eq!(request.replay_on_conn(&tx, actor, "rd").unwrap(), None);
            request.remember_on_conn(&tx, actor, "rd", 123).unwrap();
        }
        {
            let tx = conn.transaction().unwrap();
            assert_eq!(request.replay_on_conn(&tx, actor, "rd").unwrap(), None);
            request.remember_on_conn(&tx, actor, "rd", 123).unwrap();
            tx.commit().unwrap();
        }
        let tx = conn.transaction().unwrap();
        assert_eq!(request.replay_on_conn(&tx, actor, "rd").unwrap(), Some(123));
        assert_eq!(request.replay_on_conn(&tx, actor + 1, "rd").unwrap(), None);
        assert_eq!(
            request.replay_on_conn(&tx, actor, "sample_info").unwrap(),
            None
        );
        let changed = Submission::new("receipt-atomic", &json!({"quantity": 2})).unwrap();
        assert!(matches!(
            changed.replay_on_conn(&tx, actor, "rd"),
            Err(AppError::Conflict(_))
        ));
    }
}
