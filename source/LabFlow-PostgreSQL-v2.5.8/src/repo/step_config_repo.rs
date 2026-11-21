use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::repo::audit_repo;
use postgres_compat::{Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct StepConfigItem {
    pub id: Option<i64>,
    pub name: String,
    pub coefficient: f64,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct StepConfig {
    pub revision: i64,
    pub ordered: bool,
    pub steps: Vec<StepConfigItem>,
}

pub(crate) fn get(conn: &Connection, method_id: i64) -> Result<StepConfig> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM methods WHERE id=?1 AND deleted_at IS NULL)",
        [method_id],
        |row| row.get(0),
    )?;
    if !exists {
        return Err(AppError::NotFound("方法不存在".into()));
    }
    let (revision, ordered) = conn
        .query_row(
            "SELECT revision,ordered FROM method_step_settings WHERE method_id=?1",
            [method_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .unwrap_or((0, false));
    let steps = conn.prepare(
        "SELECT id,name,coefficient,enabled FROM method_work_steps WHERE method_id=?1 ORDER BY sort_order,id",
    )?.query_map([method_id], |row| Ok(StepConfigItem {
        id: Some(row.get(0)?), name: row.get(1)?, coefficient: row.get(2)?, enabled: row.get(3)?,
    }))?.collect::<postgres_compat::Result<Vec<_>>>()?;
    Ok(StepConfig {
        revision,
        ordered,
        steps,
    })
}

pub(crate) fn save(
    pool: &DbPool,
    method_id: i64,
    input: &StepConfig,
    operator: &str,
) -> Result<StepConfig> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let after = save_on_conn(&tx, method_id, input, operator)?;
    tx.commit()?;
    Ok(after)
}

pub(crate) fn save_on_conn(
    tx: &Transaction<'_>,
    method_id: i64,
    input: &StepConfig,
    operator: &str,
) -> Result<StepConfig> {
    if input.steps.len() > 100 {
        return Err(AppError::Validation("每个方法最多配置100个步骤".into()));
    }
    let mut ids = HashSet::new();
    for step in &input.steps {
        let name = step.name.trim();
        if name.is_empty()
            || name.chars().count() > 100
            || name.contains('\0')
            || !step.coefficient.is_finite()
            || step.coefficient < 0.0
        {
            return Err(AppError::Validation("步骤名称或系数无效".into()));
        }
        if let Some(id) = step.id {
            if id <= 0 || !ids.insert(id) {
                return Err(AppError::Validation("步骤标识无效或重复".into()));
            }
        }
    }
    tx.query_row(
        "SELECT id FROM methods WHERE id=?1 AND deleted_at IS NULL FOR UPDATE",
        [method_id],
        |row| row.get::<_, i64>(0),
    )?;
    let before = get(tx, method_id)?;
    if before.revision != input.revision {
        return Err(AppError::Conflict(
            "步骤配置已变化，请刷新后重新编辑".into(),
        ));
    }
    tx.execute(
        "UPDATE method_work_steps SET enabled=FALSE WHERE method_id=?1",
        [method_id],
    )?;
    for (order, step) in input.steps.iter().enumerate() {
        if let Some(id) = step.id {
            let changed = tx.execute(
                "UPDATE method_work_steps SET name=?1,coefficient=?2,enabled=?3,sort_order=?4 WHERE id=?5 AND method_id=?6",
                postgres_compat::params![step.name.trim(),step.coefficient,step.enabled,order as i32,id,method_id],
            )?;
            if changed != 1 {
                return Err(AppError::Validation("步骤不存在或不属于当前方法".into()));
            }
        } else {
            tx.execute(
                "INSERT INTO method_work_steps(method_id,name,coefficient,enabled,sort_order) VALUES(?1,?2,?3,?4,?5)",
                postgres_compat::params![method_id,step.name.trim(),step.coefficient,step.enabled,order as i32],
            )?;
        }
    }
    tx.execute(
        "INSERT INTO method_step_settings(method_id,ordered,revision) VALUES(?1,?2,1) ON CONFLICT(method_id) DO UPDATE SET ordered=EXCLUDED.ordered,revision=method_step_settings.revision+1",
        postgres_compat::params![method_id,input.ordered],
    )?;
    let after = get(tx, method_id)?;
    let before_json =
        serde_json::to_value(&before).map_err(|e| AppError::Internal(e.to_string()))?;
    let after_json = serde_json::to_value(&after).map_err(|e| AppError::Internal(e.to_string()))?;
    audit_repo::log_structured_on_conn(
        tx,
        "update",
        "method_work_steps",
        Some(method_id),
        operator,
        "修改方法步骤配置",
        "shared",
        "",
        Some(&before_json),
        Some(&after_json),
        "management",
    )?;
    Ok(after)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_preserves_identity_and_rejects_stale_writes() {
        if std::env::var("WORKLOAD_TEST_DATABASE_URL").is_err() {
            return;
        }
        let pool = crate::db::init_pool("postgres-test");
        let conn = pool.get().unwrap();
        crate::db::test_migrations::run(&conn).unwrap();
        conn.execute("INSERT INTO methods(name) VALUES('步骤配置测试方法')", [])
            .unwrap();
        let method_id = conn.last_insert_rowid();
        drop(conn);
        let input = StepConfig {
            revision: 0,
            ordered: true,
            steps: vec![
                StepConfigItem {
                    id: None,
                    name: "前处理".into(),
                    coefficient: 2.0,
                    enabled: true,
                },
                StepConfigItem {
                    id: None,
                    name: "后处理".into(),
                    coefficient: 3.0,
                    enabled: true,
                },
            ],
        };
        let saved = save(&pool, method_id, &input, "admin").unwrap();
        assert_eq!(saved.revision, 1);
        let reader = pool.get().unwrap();
        let reader_tx = reader.unchecked_transaction().unwrap();
        assert!(crate::repo::step_detection_repo::prepare(
            &reader_tx,
            None,
            None,
            None,
            Some(method_id),
            0,
            None,
            None,
            "admin",
            None,
        )
        .is_err());
        let writer = pool.get().unwrap();
        let writer_tx = writer.unchecked_transaction().unwrap();
        writer_tx
            .execute_batch("SET LOCAL lock_timeout = '100ms'")
            .unwrap();
        assert!(writer_tx
            .query_row(
                "SELECT id FROM methods WHERE id=?1 FOR UPDATE",
                [method_id],
                |row| row.get::<_, i64>(0)
            )
            .is_err());
        writer_tx.rollback().unwrap();
        reader_tx.rollback().unwrap();
        let first_id = saved.steps[0].id;
        assert!(save(&pool, method_id, &input, "admin").is_err());
        let mut update = saved.clone();
        update.steps[0].coefficient = 4.0;
        update.steps[1].enabled = false;
        let updated = save(&pool, method_id, &update, "admin").unwrap();
        assert_eq!(updated.revision, 2);
        assert_eq!(updated.steps[0].id, first_id);
        assert_eq!(updated.steps[0].coefficient, 4.0);
        assert!(!updated.steps[1].enabled);
        let mut invalid = updated.clone();
        invalid.steps[0].id = Some(i64::MAX);
        assert!(save(&pool, method_id, &invalid, "admin").is_err());
        let stored = get(&pool.get().unwrap(), method_id).unwrap();
        assert_eq!(stored.revision, 2);
        assert!(stored.steps[0].enabled);
    }
}
