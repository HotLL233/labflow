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
) -> Result<Vec<DivisionSummary>> {
    let conn = pool.get()?;
    let end_closed = format!("{}T23:59:59", end);

    let mut clauses = vec![
        "wr.deleted_at IS NULL".to_string(),
        "wr.recorded_at>=?1".to_string(),
        "wr.recorded_at<=?2".to_string(),
    ];
    let mut params: Vec<Box<dyn postgres_compat::types::ToSql>> =
        vec![Box::new(start.to_string()), Box::new(end_closed)];
    if let Some(did) = division_id {
        let idx = params.len() + 1;
        clauses.push(format!("d.id = ?{}", idx));
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
                (SELECT COUNT(DISTINCT pg2.id) FROM project_groups pg2 WHERE pg2.division_id = d.id) AS lab_count
         FROM work_records wr
         JOIN projects p ON wr.project_id = p.id
         LEFT JOIN project_groups pg ON wr.group_id = pg.id
         LEFT JOIN divisions d ON d.id = COALESCE(pg.division_id, wr.division_id)
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
