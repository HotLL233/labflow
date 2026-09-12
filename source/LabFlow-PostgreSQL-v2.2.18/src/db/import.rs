use crate::error::AppError;
use crate::models::import_record::ImportRecord;
use postgres_compat::Connection;

/// Upsert project_group and project, return project_id.
/// Uses a SELECT-first approach so imports remain deterministic across PostgreSQL upgrades.
pub fn upsert_project(
    conn: &Connection,
    group_name: &str,
    project_name: &str,
) -> Result<i64, AppError> {
    // Try find existing group
    let group_id: Option<i64> = conn
        .query_row(
            "SELECT id FROM project_groups WHERE name=?1",
            [group_name],
            |r| r.get(0),
        )
        .ok();

    let group_id = match group_id {
        Some(id) => id,
        None => {
            conn.execute(
                "INSERT INTO project_groups (name, sort_order) VALUES (?1, (SELECT COALESCE(MAX(sort_order),0)+1 FROM project_groups))",
                [group_name],
            )?;
            conn.last_insert_rowid()
        }
    };

    // Try find existing project
    let project_id: Option<i64> = conn
        .query_row(
            "SELECT id FROM projects WHERE group_id=?1 AND name=?2",
            postgres_compat::params![group_id, project_name],
            |r| r.get(0),
        )
        .ok();

    let project_id = match project_id {
        Some(id) => id,
        None => {
            conn.execute(
                "INSERT INTO projects (group_id, name, sort_order) VALUES (?1, ?2, (SELECT COALESCE(MAX(sort_order),0)+1 FROM projects WHERE group_id=?1))",
                postgres_compat::params![group_id, project_name],
            )?;
            conn.last_insert_rowid()
        }
    };

    Ok(project_id)
}

/// Batch import records with transaction and quantity accumulation.
/// Same (project_id, recorded_at, batch_no) → quantities accumulate.
/// Returns (inserted_count, updated_count, errors).
pub fn batch_import(
    conn: &Connection,
    records: &[ImportRecord],
) -> Result<(usize, usize, Vec<String>), AppError> {
    let tx = conn.unchecked_transaction()?;
    let mut inserted = 0usize;
    let mut updated = 0usize;
    let mut errors: Vec<String> = vec![];

    for rec in records {
        // Attempt to upsert project - log but continue on failure
        let project_id = match upsert_project(&tx, &rec.group_name, &rec.project_name) {
            Ok(id) => id,
            Err(e) => {
                errors.push(format!(
                    "项目 {} - {} 创建失败: {}",
                    rec.group_name, rec.project_name, e
                ));
                continue;
            }
        };

        let user_name = rec.user_name.as_deref().unwrap_or("导入");

        // Try UPDATE existing row first
        let rows_updated = match tx.execute(
            "UPDATE work_records SET quantity = quantity + ?1 WHERE project_id=?2 AND recorded_at=?3 AND batch_no=?4 AND deleted_at IS NULL",
            postgres_compat::params![rec.quantity, project_id, rec.recorded_at, rec.batch_no],
        ) {
            Ok(n) => n,
            Err(e) => {
                errors.push(format!("更新记录失败(日期:{},批号:{}): {}", rec.recorded_at, rec.batch_no, e));
                continue;
            }
        };

        if rows_updated > 0 {
            updated += 1;
        } else {
            match tx.execute(
                "INSERT INTO work_records (project_id, user_name, quantity, recorded_at, batch_no, extra_info) VALUES (?1,?2,?3,?4,?5,?6)",
                postgres_compat::params![project_id, user_name, rec.quantity, rec.recorded_at, rec.batch_no, rec.extra_info],
            ) {
                Ok(_) => { inserted += 1; }
                Err(e) => {
                    errors.push(format!("插入记录失败(日期:{},批号:{}): {}", rec.recorded_at, rec.batch_no, e));
                }
            }
        }
    }

    // Commit even if some records had errors
    tx.commit()?;
    Ok((inserted, updated, errors))
}
