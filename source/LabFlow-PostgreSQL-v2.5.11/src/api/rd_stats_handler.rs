use crate::db::DbPool;
use crate::error::Result;
use crate::models::ApiResponse;
use crate::service::authz_service;
use axum::{
    extract::{Query, State},
    http::HeaderMap,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct StatsQuery {
    pub start: Option<String>,
    pub end: Option<String>,
    pub group_by: Option<String>, // day | week | month
    pub group_id: Option<i64>,
    pub division_id: Option<i64>,
    /// execution (default), submitted or project. This changes the
    /// statistical dimension only; authorization always remains execution-based.
    pub ownership_basis: Option<String>,
    pub include_pending_ownership: Option<bool>,
}

#[derive(Serialize)]
pub struct StatsSummary {
    pub total_quantity: i64,
    pub total_records: i64,
    pub user_count: i64,
    pub project_count: i64,
    pub coefficient_score: f64,
    #[serde(rename = "details")]
    pub breakdown: Vec<PeriodBreakdown>,
}

#[derive(Serialize)]
pub struct PeriodBreakdown {
    pub period: String,
    pub total_quantity: i64,
    pub record_count: i64,
    pub coefficient_score: f64,
}

#[derive(Serialize)]
pub struct UserStats {
    pub user_name: String,
    pub total_quantity: i64,
    pub record_count: i64,
    pub coefficient_score: f64,
}

#[derive(Serialize)]
pub struct ProjectStats {
    pub project_id: i64,
    pub project_name: String,
    pub group_name: String,
    pub total_quantity: i64,
    pub record_count: i64,
    pub coefficient_score: f64,
}

#[derive(Serialize)]
pub struct TypeStats {
    pub instrument_type: String,
    pub total_quantity: i64,
    pub record_count: i64,
    pub coefficient_score: f64,
}

#[derive(Serialize)]
pub struct InstrumentStats {
    pub instrument: String,
    pub instrument_type: String,
    pub total_quantity: i64,
    pub record_count: i64,
    pub user_count: i64,
    pub coefficient_score: f64,
}

#[derive(Serialize)]
pub struct DivisionStats {
    pub division_id: Option<i64>,
    pub division_name: String,
    pub total_quantity: i64,
    pub record_count: i64,
    pub coefficient_score: f64,
    pub lab_count: i64,
}

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/rd-stats/summary", get(summary))
        .route("/api/rd-stats/by-user", get(by_user))
        .route("/api/rd-stats/by-project", get(by_project))
        .route("/api/rd-stats/by-type", get(by_type))
        .route("/api/rd-stats/by-instrument", get(by_instrument))
        .route("/api/rd-stats/by-division", get(by_division))
        .with_state(pool)
}

fn stats_scope(
    pool: &DbPool,
    headers: &HeaderMap,
) -> Result<(Option<i64>, Option<i64>, Option<Vec<i64>>)> {
    let ctx = authz_service::authenticate(pool, headers)?;
    authz_service::require_permission(&ctx, "stats:portal:rd")?;
    let allowed_division_ids = authz_service::rd_allowed_division_ids(pool, &ctx)?;
    if ctx.is_system_admin() || ctx.has_permission("stats:rd:view-all") {
        return Ok((None, None, allowed_division_ids));
    }
    if ctx.has_permission("stats:rd:view-lab") {
        let group_id = ctx.user.group_id.ok_or_else(|| {
            crate::error::AppError::Validation("研发送样组长尚未设置所属实验室".into())
        })?;
        return Ok((None, Some(group_id), allowed_division_ids));
    }
    Ok((Some(ctx.user.id), None, allowed_division_ids))
}

fn build_where(
    start: Option<&str>,
    end: Option<&str>,
    user_id: Option<i64>,
    group_id: Option<i64>,
    allowed_division_ids: Option<&[i64]>,
) -> (String, Vec<String>) {
    // Returned originals and their editable resubmission drafts are trace data,
    // not completed sample submissions. They must never affect workload totals.
    let mut clauses = vec![
        "wr.deleted_at IS NULL".to_string(),
        "wr.status NOT IN ('已退回','已退回已确认','退回待修改','已作废')".to_string(),
    ];
    let mut params = vec![];
    if let Some(s) = start {
        let i = params.len() + 1;
        clauses.push(format!("wr.recorded_at>=?{}", i));
        params.push(s.to_string());
    }
    if let Some(e) = end {
        let i = params.len() + 1;
        clauses.push(format!("wr.recorded_at<=?{}", i));
        params.push(format!("{}T23:59:59", e));
    }
    if let Some(user) = user_id {
        let i = params.len() + 1;
        clauses.push(format!("wr.subject_user_id=?{}", i));
        params.push(user.to_string());
    }
    if let Some(group_id) = group_id {
        let i = params.len() + 1;
        clauses.push(format!("wr.group_id=?{}", i));
        params.push(group_id.to_string());
    }
    if let Some(ids) = allowed_division_ids {
        if ids.is_empty() {
            clauses.push("1=0".to_string());
        } else {
            let values = ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
            clauses.push(format!(
                "COALESCE(wr.execution_division_id,wr.division_id,(SELECT division_id FROM project_groups WHERE id=wr.group_id)) IN ({values})"
            ));
        }
    }
    (clauses.join(" AND "), params)
}

fn ownership_column(basis: Option<&str>) -> Result<&'static str> {
    match basis.unwrap_or("execution") {
        "execution" => Ok("wr.execution_division_id"),
        "submitted" => Ok("wr.submitted_division_id"),
        "project" => Ok("wr.project_division_id"),
        _ => Err(crate::error::AppError::Validation(
            "统计口径只能是 execution、submitted 或 project".into(),
        )),
    }
}

fn apply_ownership_filters(
    (mut where_clause, mut params): (String, Vec<String>),
    query: &StatsQuery,
) -> Result<(String, Vec<String>)> {
    let column = ownership_column(query.ownership_basis.as_deref())?;
    if !query.include_pending_ownership.unwrap_or(false) {
        where_clause
            .push_str(" AND COALESCE(wr.ownership_status,'confirmed')<>'pending_confirmation'");
    }
    if let Some(division_id) = query.division_id {
        let idx = params.len() + 1;
        where_clause.push_str(&format!(" AND {column}=?{idx}"));
        params.push(division_id.to_string());
    }
    Ok((where_clause, params))
}

fn coeff_sql() -> &'static str {
    "COALESCE(SUM(wr.quantity * wr.coefficient_snapshot), 0.0)::DOUBLE PRECISION"
}

#[cfg(test)]
mod tests {
    use super::{build_where, coeff_sql};

    #[test]
    fn rd_score_uses_record_snapshot() {
        assert_eq!(
            coeff_sql(),
            "COALESCE(SUM(wr.quantity * wr.coefficient_snapshot), 0.0)::DOUBLE PRECISION"
        );
    }

    #[test]
    fn returned_and_resubmission_draft_records_are_excluded_from_stats() {
        let (where_clause, _) = build_where(None, None, None, None, None);
        assert!(where_clause
            .contains("wr.status NOT IN ('已退回','已退回已确认','退回待修改','已作废')"));
    }

    #[test]
    fn configured_division_scope_is_part_of_every_stat_query() {
        let (where_clause, _) = build_where(None, None, None, None, Some(&[3, 9]));
        assert!(where_clause.contains("execution_division_id"));
    }
}

/// SQL FROM 片段：rd_work_records + projects（不含实验室关联，避免笛卡尔积）
/// 注意：summary 等聚合查询不能用 LEFT JOIN project_lab_links，
/// 否则一个项目关联 N 个实验室时，同一条记录会被重复计算 N 次
fn from_base() -> &'static str {
    "rd_work_records wr \
     JOIN projects p ON wr.project_id=p.id"
}

async fn summary(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<StatsQuery>,
) -> Result<Json<ApiResponse<StatsSummary>>> {
    let (scoped_user, forced_group, allowed_division_ids) = stats_scope(&pool, &headers)?;
    let group_id = forced_group.or(q.group_id);
    let (wc, params) = apply_ownership_filters(
        build_where(
            q.start.as_deref(),
            q.end.as_deref(),
            scoped_user,
            group_id,
            allowed_division_ids.as_deref(),
        ),
        &q,
    )?;
    let conn = pool.get()?;
    let param_refs: Vec<&dyn postgres_compat::types::ToSql> = params
        .iter()
        .map(|s| s as &dyn postgres_compat::types::ToSql)
        .collect();

    // 使用 from_base()（不含 project_lab_links JOIN），避免一个项目关联多个实验室时产生笛卡尔积导致数量翻倍
    // group_id 过滤通过 WHERE EXISTS 子查询实现
    let base_from = from_base();
    let full_wc = wc;

    let (tq, tr, uc, pc, cs): (i64, i64, i64, i64, f64) = conn.query_row(
        &format!("SELECT COALESCE(SUM(wr.quantity),0)::BIGINT, COUNT(*), COUNT(DISTINCT COALESCE(CAST(wr.subject_user_id AS TEXT),'legacy:' || wr.user_name)), COUNT(DISTINCT wr.project_id), {} FROM {} WHERE {}", coeff_sql(), base_from, full_wc),
        postgres_compat::params_from_iter(param_refs.iter()), |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
    )?;

    let gb = q.group_by.as_deref().unwrap_or("day");
    let (period_expr, group_expr) = match gb {
        "week" => (
            "to_char(CAST(wr.recorded_at AS timestamp), 'IYYY-\"W\"IW')".to_string(),
            "to_char(CAST(wr.recorded_at AS timestamp), 'IYYY-\"W\"IW')".to_string(),
        ),
        "month" => (
            "LEFT(wr.recorded_at, 7)".to_string(),
            "LEFT(wr.recorded_at, 7)".to_string(),
        ),
        _ => (
            "LEFT(wr.recorded_at, 10)".to_string(),
            "LEFT(wr.recorded_at, 10)".to_string(),
        ),
    };
    let breakdown_sql = format!(
        "SELECT {} AS period, SUM(wr.quantity)::BIGINT, COUNT(*), {} FROM {} WHERE {} GROUP BY {} ORDER BY {}",
        period_expr, coeff_sql(), base_from, full_wc, group_expr, group_expr
    );
    let mut stmt = conn.prepare(&breakdown_sql)?;
    let rows = stmt.query_map(
        postgres_compat::params_from_iter(param_refs.iter()),
        |row| {
            Ok(PeriodBreakdown {
                period: row.get(0)?,
                total_quantity: row.get(1)?,
                record_count: row.get(2)?,
                coefficient_score: row.get::<_, f64>(3).unwrap_or(0.0),
            })
        },
    )?;
    let breakdown: Vec<PeriodBreakdown> = rows.collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(Json(ApiResponse::ok(StatsSummary {
        total_quantity: tq,
        total_records: tr,
        user_count: uc,
        project_count: pc,
        coefficient_score: cs,
        breakdown,
    })))
}

async fn by_user(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<StatsQuery>,
) -> Result<Json<ApiResponse<Vec<UserStats>>>> {
    let (scoped_user, forced_group, allowed_division_ids) = stats_scope(&pool, &headers)?;
    let group_id = forced_group.or(q.group_id);
    let (wc, params) = apply_ownership_filters(
        build_where(
            q.start.as_deref(),
            q.end.as_deref(),
            scoped_user,
            group_id,
            allowed_division_ids.as_deref(),
        ),
        &q,
    )?;
    let conn = pool.get()?;
    let param_refs: Vec<&dyn postgres_compat::types::ToSql> = params
        .iter()
        .map(|s| s as &dyn postgres_compat::types::ToSql)
        .collect();

    // group_id 过滤：通过 EXISTS 子查询实现，避免 JOIN 产生笛卡尔积
    let full_wc = wc;

    let mut stmt = conn.prepare(&format!(
        "SELECT COALESCE(MAX(u.username),MAX(wr.user_name)), SUM(wr.quantity)::BIGINT, COUNT(*), {}
         FROM {} LEFT JOIN users u ON u.id=wr.subject_user_id WHERE {}
         GROUP BY COALESCE(CAST(wr.subject_user_id AS TEXT),'legacy:' || wr.user_name)
         ORDER BY SUM(wr.quantity) DESC",
        coeff_sql(),
        from_base(),
        full_wc
    ))?;
    let rows = stmt.query_map(
        postgres_compat::params_from_iter(param_refs.iter()),
        |row| {
            Ok(UserStats {
                user_name: row.get(0)?,
                total_quantity: row.get(1)?,
                record_count: row.get(2)?,
                coefficient_score: row.get::<_, f64>(3).unwrap_or(0.0),
            })
        },
    )?;
    Ok(Json(ApiResponse::ok(
        rows.collect::<std::result::Result<Vec<_>, _>>()?,
    )))
}

async fn by_project(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<StatsQuery>,
) -> Result<Json<ApiResponse<Vec<ProjectStats>>>> {
    let (scoped_user, forced_group, allowed_division_ids) = stats_scope(&pool, &headers)?;
    let group_id = forced_group.or(q.group_id);
    let (wc, params) = apply_ownership_filters(
        build_where(
            q.start.as_deref(),
            q.end.as_deref(),
            scoped_user,
            group_id,
            allowed_division_ids.as_deref(),
        ),
        &q,
    )?;
    let conn = pool.get()?;
    let param_refs: Vec<&dyn postgres_compat::types::ToSql> = params
        .iter()
        .map(|s| s as &dyn postgres_compat::types::ToSql)
        .collect();

    // group_id 过滤：通过 EXISTS 子查询实现
    let full_wc = wc;

    // v0.3.23 修复：用 group_concat 子查询替代 LEFT JOIN project_lab_links，
    // 避免一个项目关联多个实验室时产生笛卡尔积导致数量翻倍
    let mut stmt = conn.prepare(&format!(
        "SELECT p.id, p.name,
                COALESCE(pg.name, '未分组') AS group_name,
                SUM(wr.quantity)::BIGINT, COUNT(*), {}
         FROM {} LEFT JOIN project_groups pg ON pg.id = wr.group_id WHERE {} GROUP BY p.id, p.name, pg.id, pg.name ORDER BY p.name",
        coeff_sql(), from_base(), full_wc
    ))?;
    let rows = stmt.query_map(
        postgres_compat::params_from_iter(param_refs.iter()),
        |row| {
            Ok(ProjectStats {
                project_id: row.get(0)?,
                project_name: row.get(1)?,
                group_name: row.get(2).unwrap_or_else(|_| "未分组".to_string()),
                total_quantity: row.get(3)?,
                record_count: row.get(4)?,
                coefficient_score: row.get::<_, f64>(5).unwrap_or(0.0),
            })
        },
    )?;
    Ok(Json(ApiResponse::ok(
        rows.collect::<std::result::Result<Vec<_>, _>>()?,
    )))
}

async fn by_type(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<StatsQuery>,
) -> Result<Json<ApiResponse<Vec<TypeStats>>>> {
    let (scoped_user, forced_group, allowed_division_ids) = stats_scope(&pool, &headers)?;
    let group_id = forced_group.or(q.group_id);
    let (wc, params) = apply_ownership_filters(
        build_where(
            q.start.as_deref(),
            q.end.as_deref(),
            scoped_user,
            group_id,
            allowed_division_ids.as_deref(),
        ),
        &q,
    )?;
    let conn = pool.get()?;
    let param_refs: Vec<&dyn postgres_compat::types::ToSql> = params
        .iter()
        .map(|s| s as &dyn postgres_compat::types::ToSql)
        .collect();
    let sql = format!(
        "SELECT COALESCE(NULLIF(wr.instrument_type_snapshot,''),'其他') AS itype,
                SUM(wr.quantity)::BIGINT, COUNT(*), {}
         FROM rd_work_records wr
         WHERE {} GROUP BY COALESCE(NULLIF(wr.instrument_type_snapshot,''),'其他') ORDER BY itype",
        coeff_sql(),
        wc
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        postgres_compat::params_from_iter(param_refs.iter()),
        |row| {
            Ok(TypeStats {
                instrument_type: row.get(0)?,
                total_quantity: row.get(1)?,
                record_count: row.get(2)?,
                coefficient_score: row.get::<_, f64>(3).unwrap_or(0.0),
            })
        },
    )?;
    Ok(Json(ApiResponse::ok(
        rows.collect::<std::result::Result<Vec<_>, _>>()?,
    )))
}

async fn by_instrument(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<StatsQuery>,
) -> Result<Json<ApiResponse<Vec<InstrumentStats>>>> {
    let (scoped_user, forced_group, allowed_division_ids) = stats_scope(&pool, &headers)?;
    let group_id = forced_group.or(q.group_id);
    let (wc, params) = apply_ownership_filters(
        build_where(
            q.start.as_deref(),
            q.end.as_deref(),
            scoped_user,
            group_id,
            allowed_division_ids.as_deref(),
        ),
        &q,
    )?;
    let conn = pool.get()?;
    let param_refs: Vec<&dyn postgres_compat::types::ToSql> = params
        .iter()
        .map(|s| s as &dyn postgres_compat::types::ToSql)
        .collect();
    let mut stmt = conn.prepare(&format!(
        "SELECT COALESCE(NULLIF(wr.instrument_code_snapshot,''),'未绑定') AS instrument,
                COALESCE(NULLIF(wr.instrument_type_snapshot,''),'其他') AS instrument_type,
                SUM(wr.quantity)::BIGINT, COUNT(*), COUNT(DISTINCT COALESCE(CAST(wr.subject_user_id AS TEXT),'legacy:' || wr.user_name)), {}
         FROM rd_work_records wr WHERE {}
         GROUP BY COALESCE(CAST(wr.instrument_id_snapshot AS TEXT), 'legacy:' || COALESCE(NULLIF(wr.instrument_code_snapshot,''),'未绑定')),
                  COALESCE(NULLIF(wr.instrument_code_snapshot,''),'未绑定'),
                  COALESCE(NULLIF(wr.instrument_type_snapshot,''),'其他')
         ORDER BY COALESCE(NULLIF(wr.instrument_code_snapshot,''),'未绑定')", coeff_sql(), wc
    ))?;
    let rows = stmt.query_map(
        postgres_compat::params_from_iter(param_refs.iter()),
        |row| {
            Ok(InstrumentStats {
                instrument: row.get(0)?,
                instrument_type: row.get(1)?,
                total_quantity: row.get(2)?,
                record_count: row.get(3)?,
                user_count: row.get(4)?,
                coefficient_score: row.get::<_, f64>(5).unwrap_or(0.0),
            })
        },
    )?;
    Ok(Json(ApiResponse::ok(
        rows.collect::<std::result::Result<Vec<_>, _>>()?,
    )))
}

async fn by_division(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<StatsQuery>,
) -> Result<Json<ApiResponse<Vec<DivisionStats>>>> {
    let (scoped_user, forced_group, allowed_division_ids) = stats_scope(&pool, &headers)?;
    let group_id = forced_group.or(q.group_id);
    let (wc, params) = apply_ownership_filters(
        build_where(
            q.start.as_deref(),
            q.end.as_deref(),
            scoped_user,
            group_id,
            allowed_division_ids.as_deref(),
        ),
        &q,
    )?;
    let conn = pool.get()?;
    let full_wc = wc;
    let param_refs: Vec<&dyn postgres_compat::types::ToSql> = params
        .iter()
        .map(|s| s as &dyn postgres_compat::types::ToSql)
        .collect();

    let ownership_column = ownership_column(q.ownership_basis.as_deref())?;
    let sql = format!(
        r##"SELECT d.id,
                COALESCE(d.name, 'N/A') AS division_name,
                COALESCE(SUM(wr.quantity), 0)::BIGINT AS total_quantity,
                COUNT(wr.id) AS record_count,
                {} AS coefficient_score,
                COUNT(DISTINCT pg.id) AS lab_count
         FROM rd_work_records wr
         JOIN projects p ON wr.project_id = p.id
         LEFT JOIN project_groups pg ON pg.id = wr.group_id
         LEFT JOIN divisions d ON d.id = {ownership_column}
         WHERE {}
         GROUP BY d.id, d.name
         ORDER BY COALESCE(d.name, 'N/A')"##,
        coeff_sql(),
        full_wc,
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        postgres_compat::params_from_iter(param_refs.iter()),
        |row| {
            Ok(DivisionStats {
                division_id: row.get(0)?,
                division_name: row.get(1)?,
                total_quantity: row.get(2)?,
                record_count: row.get(3)?,
                coefficient_score: row.get::<_, f64>(4).unwrap_or(0.0),
                lab_count: row.get(5)?,
            })
        },
    )?;
    Ok(Json(ApiResponse::ok(
        rows.collect::<std::result::Result<Vec<_>, _>>()?,
    )))
}
