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
            let coefficient =
                match crate::repo::record_repo::workload_coefficient(&tx, project_id, None) {
                    Ok(value) => value,
                    Err(error) => {
                        errors.push(format!(
                            "项目 {} 工作量系数无效: {}",
                            rec.project_name, error
                        ));
                        continue;
                    }
                };
            // 临时编号避免历史导入留下的空编号占住唯一索引。
            let temporary_business_no = format!("WK-TEMP-{}", uuid::Uuid::new_v4());
            match tx.execute(
                "INSERT INTO work_records (project_id, user_name, quantity, recorded_at, batch_no, extra_info, detection_type_snapshot, coefficient_snapshot, business_no) VALUES (?1,?2,?3,?4,?5,?6,'未分类',?7,?8)",
                postgres_compat::params![project_id, user_name, rec.quantity, rec.recorded_at, rec.batch_no, rec.extra_info, coefficient, temporary_business_no],
            ) {
                Ok(_) => {
                    let id = tx.last_insert_rowid();
                    let business_no = crate::repo::trace_repo::make_business_no("WK", &rec.recorded_at, id);
                    tx.execute("UPDATE work_records SET business_no=?1 WHERE id=?2", postgres_compat::params![business_no,id])?;
                    inserted += 1;
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_new_records_use_project_coefficient_but_accumulation_keeps_snapshot() {
        let conn = Connection::open_test_database().unwrap();
        crate::db::test_migrations::run(&conn).unwrap();
        let mut record = ImportRecord {
            project_name: "legacy_coefficient_project".into(),
            group_name: "legacy_coefficient_lab".into(),
            recorded_at: "2026-10-15".into(),
            batch_no: "COEFFICIENT-A".into(),
            quantity: 2,
            user_name: Some("detector".into()),
            extra_info: None,
        };
        let project = upsert_project(&conn, &record.group_name, &record.project_name).unwrap();
        conn.execute(
            "INSERT INTO work_records(project_id,user_name,quantity,recorded_at,batch_no,coefficient_snapshot) VALUES(?1,'legacy',1,'2000-01-01','LEGACY-EMPTY',1)",
            [project],
        ).unwrap();
        conn.execute("UPDATE projects SET coefficient=2.5 WHERE id=?1", [project])
            .unwrap();
        let first = batch_import(&conn, std::slice::from_ref(&record)).unwrap();
        assert_eq!((first.0, first.1, first.2.len()), (1, 0, 0));
        conn.execute("UPDATE projects SET coefficient=0 WHERE id=?1", [project])
            .unwrap();
        let accumulated = batch_import(&conn, std::slice::from_ref(&record)).unwrap();
        assert_eq!(
            (accumulated.0, accumulated.1, accumulated.2.len()),
            (0, 1, 0)
        );
        let saved: (i64, f64) = conn.query_row(
            "SELECT quantity,coefficient_snapshot FROM work_records WHERE batch_no='COEFFICIENT-A'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).unwrap();
        assert_eq!(saved, (4, 2.5));
        record.batch_no = "COEFFICIENT-B".into();
        let second = batch_import(&conn, &[record]).unwrap();
        assert_eq!(
            (second.0, second.1, second.2.len()),
            (1, 0, 0),
            "{:?}",
            second.2
        );
        let zero: f64 = conn
            .query_row(
                "SELECT coefficient_snapshot FROM work_records WHERE batch_no='COEFFICIENT-B'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(zero, 0.0);
        let old: (String, f64) = conn.query_row(
            "SELECT business_no,coefficient_snapshot FROM work_records WHERE batch_no='LEGACY-EMPTY'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).unwrap();
        assert_eq!(old, (String::new(), 1.0));
        let numbered: i64 = conn.query_row(
            "SELECT COUNT(DISTINCT business_no) FROM work_records WHERE batch_no IN ('COEFFICIENT-A','COEFFICIENT-B') AND business_no LIKE 'WK-20261015-%'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(numbered, 2);
    }

    #[test]
    fn legacy_import_captures_unclassified_without_rewriting_existing_classification() {
        let conn = Connection::open_test_database().unwrap();
        crate::db::test_migrations::run(&conn).unwrap();
        let records = [ImportRecord {
            project_name: "csv_snapshot_project".into(),
            group_name: "csv_snapshot_lab".into(),
            recorded_at: "2026-10-12".into(),
            batch_no: "CSV48".into(),
            quantity: 2,
            user_name: Some("detector".into()),
            extra_info: None,
        }];
        let inserted = batch_import(&conn, &records).unwrap();
        assert_eq!((inserted.0, inserted.1), (1, 0));
        assert!(inserted.2.is_empty());
        let first: String = conn
            .query_row(
                "SELECT detection_type_snapshot FROM work_records WHERE batch_no='CSV48'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(first, "未分类");
        conn.execute(
            "UPDATE work_records SET detection_type_snapshot='液相' WHERE batch_no='CSV48'",
            [],
        )
        .unwrap();
        let accumulated = batch_import(&conn, &records).unwrap();
        assert_eq!((accumulated.0, accumulated.1), (0, 1));
        let saved: (String, i64) = conn
            .query_row(
                "SELECT detection_type_snapshot,quantity FROM work_records WHERE batch_no='CSV48'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(saved, ("液相".into(), 4));
    }
}
