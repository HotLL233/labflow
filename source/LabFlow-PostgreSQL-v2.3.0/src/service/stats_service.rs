use crate::db::DbPool;
use crate::error::Result;
use serde::Serialize;

/// Daily summary used by statistics
#[derive(Debug, Serialize)]
pub struct DailySummary {
    pub date: String,
    pub count: i64,
    pub total: i64,
}

#[derive(Debug, Serialize)]
pub struct UserSummary {
    pub user_name: String,
    pub count: i64,
    pub total: i64,
}

#[derive(Debug, Serialize)]
pub struct ProjectSummary {
    pub project_name: String,
    pub group_name: String,
    pub count: i64,
    pub total: i64,
}

#[derive(Debug, Serialize)]
pub struct TypeSummary {
    pub instrument_type: String,
    pub count: i64,
    pub total: i64,
}

#[derive(Debug, Serialize)]
pub struct InstrumentSummary {
    pub project_id: i64,
    pub project_name: String,
    pub group_name: String,
    pub count: i64,
    pub total: i64,
}

/// v0.4.28: 事业部统计
#[derive(Debug, Serialize)]
pub struct DivisionSummary {
    pub division_id: Option<i64>,
    pub division_name: String,
    pub total_quantity: i64,
    pub record_count: i64,
    pub coefficient_score: f64,
    pub lab_count: i64,
}

/// Aggregate statistics by day within a date range
pub fn daily_summary(pool: &DbPool, start: &str, end: &str) -> Result<Vec<DailySummary>> {
    let conn = pool.get()?;
    let end_closed = format!("{}T23:59:59", end);
    let mut stmt = conn.prepare(
        "SELECT date(wr.recorded_at) AS d, COUNT(*), SUM(wr.quantity)
         FROM work_records wr WHERE wr.deleted_at IS NULL AND wr.recorded_at>=?1 AND wr.recorded_at<=?2
         GROUP BY date(wr.recorded_at) ORDER BY d"
    )?;
    let rows = stmt
        .query_map(postgres_compat::params![start, end_closed], |row| {
            Ok(DailySummary {
                date: row.get(0)?,
                count: row.get(1)?,
                total: row.get(2)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Aggregate statistics by user within a date range
pub fn by_user(pool: &DbPool, start: &str, end: &str) -> Result<Vec<UserSummary>> {
    let conn = pool.get()?;
    let end_closed = format!("{}T23:59:59", end);
    let mut stmt = conn.prepare(
        "SELECT wr.user_name, COUNT(*), SUM(wr.quantity)
         FROM work_records wr WHERE wr.deleted_at IS NULL AND wr.recorded_at>=?1 AND wr.recorded_at<=?2
         GROUP BY wr.user_name ORDER BY wr.user_name"
    )?;
    let rows = stmt
        .query_map(postgres_compat::params![start, end_closed], |row| {
            Ok(UserSummary {
                user_name: row.get(0)?,
                count: row.get(1)?,
                total: row.get(2)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Aggregate statistics by project within a date range
pub fn by_project(pool: &DbPool, start: &str, end: &str) -> Result<Vec<ProjectSummary>> {
    let conn = pool.get()?;
    let end_closed = format!("{}T23:59:59", end);
    let mut stmt = conn.prepare(
        "SELECT p.name, COALESCE(pg.name, '未分组'), COUNT(*), SUM(wr.quantity)
         FROM work_records wr JOIN projects p ON wr.project_id=p.id LEFT JOIN project_groups pg ON pg.id=wr.group_id
         WHERE wr.deleted_at IS NULL AND wr.recorded_at>=?1 AND wr.recorded_at<=?2
         GROUP BY p.id ORDER BY pg.sort_order, p.sort_order"
    )?;
    let rows = stmt
        .query_map(postgres_compat::params![start, end_closed], |row| {
            Ok(ProjectSummary {
                project_name: row.get(0)?,
                group_name: row.get(1)?,
                count: row.get(2)?,
                total: row.get(3)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Aggregate by instrument type (液相/气相)
pub fn by_type(pool: &DbPool, start: &str, end: &str) -> Result<Vec<TypeSummary>> {
    let conn = pool.get()?;
    let end_closed = format!("{}T23:59:59", end);
    let mut stmt = conn.prepare(
        "SELECT CASE WHEN p.name LIKE '%-LC-%' THEN '液相' WHEN p.name LIKE '%-GC-%' THEN '气相' ELSE '其他' END AS itype,
         COUNT(*), SUM(wr.quantity)
         FROM work_records wr JOIN projects p ON wr.project_id=p.id
         WHERE wr.deleted_at IS NULL AND wr.recorded_at>=?1 AND wr.recorded_at<=?2
         GROUP BY itype ORDER BY itype"
    )?;
    let rows = stmt
        .query_map(postgres_compat::params![start, end_closed], |row| {
            Ok(TypeSummary {
                instrument_type: row.get(0)?,
                count: row.get(1)?,
                total: row.get(2)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Aggregate by individual instrument (project-level)
pub fn by_instrument(pool: &DbPool, start: &str, end: &str) -> Result<Vec<InstrumentSummary>> {
    let conn = pool.get()?;
    let end_closed = format!("{}T23:59:59", end);
    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, COALESCE(pg.name, '未分组'), COUNT(*), SUM(wr.quantity)
         FROM work_records wr JOIN projects p ON wr.project_id=p.id LEFT JOIN project_groups pg ON pg.id=wr.group_id
         WHERE wr.deleted_at IS NULL AND wr.recorded_at>=?1 AND wr.recorded_at<=?2
         GROUP BY p.id ORDER BY pg.sort_order, p.sort_order"
    )?;
    let rows = stmt
        .query_map(postgres_compat::params![start, end_closed], |row| {
            Ok(InstrumentSummary {
                project_id: row.get(0)?,
                project_name: row.get(1)?,
                group_name: row.get(2)?,
                count: row.get(3)?,
                total: row.get(4)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// v0.4.28: 按事业部聚合统计
/// JOIN work_records → project_groups → divisions
/// 未分配事业部的记录归入 "N/A"
pub fn by_division(
    pool: &DbPool,
    start: &str,
    end: &str,
    division_id: Option<i64>,
    subject_user_id: Option<i64>,
    allowed_division_ids: Option<&[i64]>,
    ownership_basis: &str,
    include_pending_ownership: bool,
) -> Result<Vec<DivisionSummary>> {
    let conn = pool.get()?;
    let end_closed = format!("{}T23:59:59", end);
    let ownership_column = match ownership_basis {
        "execution" => "COALESCE(wr.execution_division_id,wr.division_id,pg.division_id)",
        "project" => "wr.project_division_id",
        _ => {
            return Err(crate::error::AppError::Validation(
                "分析检测统计口径只能是 execution 或 project".into(),
            ))
        }
    };

    let mut clauses = vec![
        "wr.deleted_at IS NULL".to_string(),
        "wr.recorded_at>=?1".to_string(),
        "wr.recorded_at<=?2".to_string(),
    ];
    let mut params: Vec<Box<dyn postgres_compat::types::ToSql>> =
        vec![Box::new(start.to_string()), Box::new(end_closed)];
    if !include_pending_ownership {
        clauses
            .push("COALESCE(wr.ownership_status,'confirmed')<>'pending_confirmation'".to_string());
    }
    if let Some(ids) = allowed_division_ids {
        if ids.is_empty() {
            clauses.push("1=0".to_string());
        } else {
            let values = ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
            clauses.push(format!(
                "COALESCE(wr.execution_division_id,wr.division_id,pg.division_id) IN ({values})"
            ));
        }
    }
    if let Some(did) = division_id {
        let idx = params.len() + 1;
        clauses.push(format!("{ownership_column} = ?{idx}"));
        params.push(Box::new(did));
    }
    if let Some(user_id) = subject_user_id {
        let idx = params.len() + 1;
        clauses.push(format!("wr.subject_user_id = ?{}", idx));
        params.push(Box::new(user_id));
    }
    let where_clause = format!("WHERE {}", clauses.join(" AND "));

    let sql = format!(
        "SELECT d.id, COALESCE(d.name, 'N/A') AS division_name,
                COALESCE(SUM(wr.quantity), 0)::BIGINT AS total_quantity,
                COUNT(wr.id) AS record_count,
                COALESCE(SUM(wr.quantity * wr.coefficient_snapshot), 0.0)::DOUBLE PRECISION AS coefficient_score,
                COUNT(DISTINCT COALESCE(wr.execution_group_id,wr.group_id)) AS lab_count
         FROM work_records wr
         JOIN projects p ON wr.project_id = p.id
         LEFT JOIN project_groups pg ON wr.group_id = pg.id
         LEFT JOIN divisions d ON d.id = {ownership_column}
         {}
         GROUP BY d.id, d.name
         ORDER BY COALESCE(d.name, 'N/A')",
        where_clause
    );

    let param_refs: Vec<&dyn postgres_compat::types::ToSql> =
        params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(
            postgres_compat::params_from_iter(param_refs.iter()),
            |row| {
                Ok(DivisionSummary {
                    division_id: row.get(0)?,
                    division_name: row.get(1)?,
                    total_quantity: row.get(2)?,
                    record_count: row.get(3)?,
                    coefficient_score: row.get::<_, f64>(4).unwrap_or(0.0),
                    lab_count: row.get(5)?,
                })
            },
        )?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::record::RecordCreate;
    use crate::repo::record_repo;

    fn unique(prefix: &str) -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        format!("{prefix}_{nanos}")
    }

    #[test]
    fn project_basis_never_bypasses_execution_department_scope_and_hides_pending_by_default() {
        let pool = crate::db::init_pool("postgres-test");
        crate::db::test_migrations::run(&pool.get().expect("connection")).expect("migrations");
        let conn = pool.get().expect("connection");
        let suffix = unique("stats_scope");
        conn.execute(
            "INSERT INTO divisions(name,sort_order,color,is_active) VALUES(?1,1,'#1976d2',1)",
            postgres_compat::params![format!("execution_{suffix}")],
        )
        .expect("execution division");
        let execution_division_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO divisions(name,sort_order,color,is_active) VALUES(?1,2,'#1976d2',1)",
            postgres_compat::params![format!("project_{suffix}")],
        )
        .expect("project division");
        let project_division_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO project_groups(name,division_id) VALUES(?1,?2)",
            postgres_compat::params![format!("execution_lab_{suffix}"), execution_division_id],
        )
        .expect("execution laboratory");
        let execution_group_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO project_groups(name,division_id) VALUES(?1,?2)",
            postgres_compat::params![format!("project_lab_{suffix}"), project_division_id],
        )
        .expect("project laboratory");
        let project_group_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO projects(group_id,name,project_division_id) VALUES(?1,?2,?3)",
            postgres_compat::params![
                project_group_id,
                format!("project_{suffix}"),
                project_division_id
            ],
        )
        .expect("project");
        let project_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO project_lab_links(project_id,group_id) VALUES(?1,?2)",
            postgres_compat::params![project_id, execution_group_id],
        )
        .expect("execution laboratory link");
        conn.execute(
            "INSERT INTO project_collaboration_divisions(project_id,division_id) VALUES(?1,?2)",
            postgres_compat::params![project_id, execution_division_id],
        )
        .expect("collaboration department");
        conn.execute(
            "INSERT INTO instruments(code,name,instrument_type) VALUES(?1,?2,'液相')",
            postgres_compat::params![
                format!("instrument_{suffix}"),
                format!("instrument_{suffix}")
            ],
        )
        .expect("instrument");
        let instrument_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO methods(method_code,name,full_name,instrument_id) VALUES(?1,?2,?2,?3)",
            postgres_compat::params![
                format!("method_{suffix}"),
                format!("method_{suffix}"),
                instrument_id
            ],
        )
        .expect("method");
        let method_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO project_method_links(project_id,method_id) VALUES(?1,?2)",
            postgres_compat::params![project_id, method_id],
        )
        .expect("method link");
        conn.execute(
            "INSERT INTO users(username,password,is_active) VALUES(?1,'hash',1)",
            postgres_compat::params![format!("detector_{suffix}")],
        )
        .expect("detector");
        let detector_id = conn.last_insert_rowid();
        let detector_name: String = conn
            .query_row(
                "SELECT username FROM users WHERE id=?1",
                [detector_id],
                |row| row.get(0),
            )
            .expect("detector name");
        drop(conn);

        let create_record = |quantity| RecordCreate {
            project_id,
            method_id: Some(method_id),
            user_name: detector_name.clone(),
            sender_user_id: Some(detector_id),
            quantity,
            recorded_at: "2031-01-15T09:00:00".into(),
            group_id: Some(execution_group_id),
            multiplier: None,
            high_item: None,
            division_id: Some(execution_division_id),
            extra_fields: None,
        };
        let confirmed =
            record_repo::create(&pool, &create_record(3), "test").expect("confirmed record");
        let pending =
            record_repo::create(&pool, &create_record(5), "test").expect("pending record");
        pool.get()
            .expect("connection")
            .execute(
                "UPDATE work_records SET ownership_status='pending_confirmation' WHERE id=?1",
                [pending.id],
            )
            .expect("mark pending");

        let scoped_project_basis = by_division(
            &pool,
            "2031-01-01",
            "2031-01-31",
            None,
            Some(detector_id),
            Some(&[execution_division_id]),
            "project",
            false,
        )
        .expect("project basis within execution scope");
        assert_eq!(scoped_project_basis.len(), 1);
        assert_eq!(
            scoped_project_basis[0].division_id,
            Some(project_division_id)
        );
        assert_eq!(
            scoped_project_basis[0].total_quantity,
            confirmed.quantity as i64
        );

        let unauthorized_execution_scope = by_division(
            &pool,
            "2031-01-01",
            "2031-01-31",
            None,
            Some(detector_id),
            Some(&[project_division_id]),
            "project",
            true,
        )
        .expect("restricted query");
        assert!(unauthorized_execution_scope.is_empty());

        let including_pending = by_division(
            &pool,
            "2031-01-01",
            "2031-01-31",
            None,
            Some(detector_id),
            Some(&[execution_division_id]),
            "project",
            true,
        )
        .expect("pending inclusive query");
        assert_eq!(including_pending[0].total_quantity, 8);
    }
}
