use crate::db::DbPool;
use crate::error::Result;
use crate::models::ApiResponse;
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
    pub subject_user_id: Option<i64>,
    pub lab_name: Option<String>,
    pub project_name: Option<String>,
    pub instrument_code: Option<String>,
    pub method_name: Option<String>,
    pub group_id: Option<i64>,
    pub division_id: Option<i64>, // v0.4.28: 事业部过滤
    pub detection_division_ids: Option<String>,
    pub sending_division_ids: Option<String>,
    /// execution (default) or project. Analysis records have no submitter
    /// department because they are the execution-side business record.
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
    pub user_id: Option<i64>,
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

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/stats/summary", get(summary))
        .route("/api/stats/by-user", get(by_user))
        .route("/api/stats/by-project", get(by_project))
        .route("/api/stats/by-type", get(by_type))
        .route("/api/stats/by-instrument", get(by_instrument))
        .route("/api/stats/by-division", get(by_division)) // v0.4.28
        .with_state(pool)
}

fn stats_scope(
    pool: &DbPool,
    headers: &HeaderMap,
    requested_subject: Option<i64>,
) -> Result<(Option<i64>, Option<Vec<i64>>)> {
    let ctx = crate::service::authz_service::authenticate(pool, headers)?;
    crate::service::authz_service::require_permission(&ctx, "stats:portal:workload")?;
    let allowed_division_ids =
        crate::service::authz_service::work_allowed_division_ids(pool, &ctx)?;
    // The statistics portal permission only opens the portal. The explicit
    // workload-view permission controls whether the result is own-only or
    // the configured department range.
    let subject_user_id =
        super::workload_report_handler::scoped_subject(&ctx, requested_subject, false)?;
    Ok((subject_user_id, allowed_division_ids))
}

fn build_where(
    start: Option<&str>,
    end: Option<&str>,
    user_id: Option<i64>,
    allowed_division_ids: Option<&[i64]>,
) -> (String, Vec<String>) {
    let mut clauses = vec!["wr.deleted_at IS NULL".to_string()];
    let mut params = vec![];
    if let Some(s) = start {
        let i = params.len() + 1;
        clauses.push(format!(
            "CAST(wr.recorded_at AS TIMESTAMPTZ)>=CAST(CAST(?{i} AS TEXT) AS TIMESTAMPTZ)"
        ));
        params.push(s.to_string());
    }
    if let Some(e) = end {
        let i = params.len() + 1;
        clauses.push(format!(
            "CAST(wr.recorded_at AS TIMESTAMPTZ)<=CAST(CAST(?{i} AS TEXT) AS TIMESTAMPTZ)"
        ));
        params.push(super::export_data::end_of_day_bound(e));
    }
    if let Some(user) = user_id {
        let i = params.len() + 1;
        clauses.push(format!("wr.subject_user_id=?{}", i));
        params.push(user.to_string());
    }
    if let Some(ids) = allowed_division_ids {
        if ids.is_empty() {
            clauses.push("1=0".to_string());
        } else {
            let values = ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
            clauses.push(format!(
                "{} IN ({values})",
                crate::service::authz_service::work_record_authorization_division_sql("wr")
            ));
        }
    }
    (clauses.join(" AND "), params)
}

fn apply_ownership_filters(
    (mut where_clause, mut params): (String, Vec<String>),
    query: &StatsQuery,
) -> Result<(String, Vec<String>)> {
    if let Some(group_id) = query.group_id {
        let idx = params.len() + 1;
        where_clause.push_str(&format!(" AND wr.group_id=?{idx}"));
        params.push(group_id.to_string());
    }
    if !query.include_pending_ownership.unwrap_or(false) {
        where_clause
            .push_str(" AND COALESCE(wr.ownership_status,'confirmed')<>'pending_confirmation'");
    }
    if let Some(division_id) = query.division_id {
        let idx = params.len() + 1;
        where_clause.push_str(&format!(
            " AND {}=?{idx}",
            crate::service::authz_service::work_record_authorization_division_sql("wr")
        ));
        params.push(division_id.to_string());
    }
    if let Some(ids) = query
        .detection_division_ids
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        let idx = params.len() + 1;
        where_clause.push_str(&format!(
            " AND wr.detection_division_id = ANY(string_to_array(?{idx}, ',')::BIGINT[])"
        ));
        params.push(ids.to_string());
    }
    if let Some(ids) = query
        .sending_division_ids
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        let idx = params.len() + 1;
        where_clause.push_str(&format!(
            " AND {} = ANY(string_to_array(?{idx}, ',')::BIGINT[])",
            crate::service::authz_service::work_record_sending_division_sql("wr")
        ));
        params.push(ids.to_string());
    }
    where_clause.push_str(&super::export_data::text_filter_predicate_sql("wr"));
    Ok((where_clause, params))
}

fn coeff_sql() -> &'static str {
    "COALESCE(SUM(wr.quantity * wr.coefficient_snapshot), 0.0)::DOUBLE PRECISION"
}

/// SQL FROM 片段：work_records + projects（不含实验室关联，避免笛卡尔积）
/// 注意：summary 等聚合查询不能用 LEFT JOIN project_lab_links，
/// 否则一个项目关联 N 个实验室时，同一条记录会被重复计算 N 次
fn from_base() -> &'static str {
    "work_records wr \
     JOIN projects p ON wr.project_id=p.id"
}

async fn summary(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<StatsQuery>,
) -> Result<Json<ApiResponse<StatsSummary>>> {
    let (scoped_user, allowed_division_ids) = stats_scope(&pool, &headers, q.subject_user_id)?;
    let (wc, params) = apply_ownership_filters(
        build_where(
            q.start.as_deref(),
            q.end.as_deref(),
            scoped_user,
            allowed_division_ids.as_deref(),
        ),
        &q,
    )?;
    let conn = pool.get()?;
    let _scope = conn.unchecked_transaction()?;
    super::export_data::configure_text_filters(
        &conn,
        q.lab_name.as_deref(),
        q.project_name.as_deref(),
        q.instrument_code.as_deref(),
        q.method_name.as_deref(),
    )?;
    let param_refs: Vec<&dyn postgres_compat::types::ToSql> = params
        .iter()
        .map(|s| s as &dyn postgres_compat::types::ToSql)
        .collect();

    // 使用 from_base()（不含 project_lab_links JOIN），避免一个项目关联多个实验室时产生笛卡尔积导致数量翻倍
    // 实验室筛选统一使用记录的 group_id。
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
    let (scoped_user, allowed_division_ids) = stats_scope(&pool, &headers, q.subject_user_id)?;
    let (wc, params) = apply_ownership_filters(
        build_where(
            q.start.as_deref(),
            q.end.as_deref(),
            scoped_user,
            allowed_division_ids.as_deref(),
        ),
        &q,
    )?;
    let conn = pool.get()?;
    let _scope = conn.unchecked_transaction()?;
    super::export_data::configure_text_filters(
        &conn,
        q.lab_name.as_deref(),
        q.project_name.as_deref(),
        q.instrument_code.as_deref(),
        q.method_name.as_deref(),
    )?;
    let param_refs: Vec<&dyn postgres_compat::types::ToSql> = params
        .iter()
        .map(|s| s as &dyn postgres_compat::types::ToSql)
        .collect();

    // 实验室条件已在共享筛选中应用。
    let full_wc = wc;

    let mut stmt = conn.prepare(&format!(
        "SELECT COALESCE(MAX(u.username),MAX(wr.user_name)), SUM(wr.quantity)::BIGINT, COUNT(*), {}, MAX(wr.subject_user_id)
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
                user_id: row.get(4)?,
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
    let (scoped_user, allowed_division_ids) = stats_scope(&pool, &headers, q.subject_user_id)?;
    let (wc, params) = apply_ownership_filters(
        build_where(
            q.start.as_deref(),
            q.end.as_deref(),
            scoped_user,
            allowed_division_ids.as_deref(),
        ),
        &q,
    )?;
    let wc = format!(
        "{wc} AND NOT ({})",
        super::export_data::legacy_auxiliary_record_sql("wr")
    );
    let conn = pool.get()?;
    let _scope = conn.unchecked_transaction()?;
    super::export_data::configure_text_filters(
        &conn,
        q.lab_name.as_deref(),
        q.project_name.as_deref(),
        q.instrument_code.as_deref(),
        q.method_name.as_deref(),
    )?;
    let param_refs: Vec<&dyn postgres_compat::types::ToSql> = params
        .iter()
        .map(|s| s as &dyn postgres_compat::types::ToSql)
        .collect();

    // 实验室条件已在共享筛选中应用。
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
    let (scoped_user, allowed_division_ids) = stats_scope(&pool, &headers, q.subject_user_id)?;
    let (wc, params) = apply_ownership_filters(
        build_where(
            q.start.as_deref(),
            q.end.as_deref(),
            scoped_user,
            allowed_division_ids.as_deref(),
        ),
        &q,
    )?;
    let wc = format!(
        "{wc} AND NOT ({})",
        super::export_data::legacy_auxiliary_record_sql("wr")
    );
    let conn = pool.get()?;
    let _scope = conn.unchecked_transaction()?;
    super::export_data::configure_text_filters(
        &conn,
        q.lab_name.as_deref(),
        q.project_name.as_deref(),
        q.instrument_code.as_deref(),
        q.method_name.as_deref(),
    )?;
    let param_refs: Vec<&dyn postgres_compat::types::ToSql> = params
        .iter()
        .map(|s| s as &dyn postgres_compat::types::ToSql)
        .collect();
    let sql = format!(
        "SELECT CASE WHEN COALESCE(NULLIF(wr.instrument_type_snapshot,''),'其他')='辅助工作'
                     THEN '辅助工作-' || COALESCE(NULLIF(wr.method_name_snapshot,''),'未知方法')
                     ELSE COALESCE(NULLIF(wr.instrument_type_snapshot,''),'其他')
                END AS itype,
                SUM(wr.quantity)::BIGINT, COUNT(*), {}
         FROM work_records wr
         WHERE {} GROUP BY itype ORDER BY itype",
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
    let (scoped_user, allowed_division_ids) = stats_scope(&pool, &headers, q.subject_user_id)?;
    let (wc, params) = apply_ownership_filters(
        build_where(
            q.start.as_deref(),
            q.end.as_deref(),
            scoped_user,
            allowed_division_ids.as_deref(),
        ),
        &q,
    )?;
    let wc = format!(
        "{wc} AND NOT ({})",
        super::export_data::legacy_auxiliary_record_sql("wr")
    );
    let conn = pool.get()?;
    let _scope = conn.unchecked_transaction()?;
    super::export_data::configure_text_filters(
        &conn,
        q.lab_name.as_deref(),
        q.project_name.as_deref(),
        q.instrument_code.as_deref(),
        q.method_name.as_deref(),
    )?;
    let param_refs: Vec<&dyn postgres_compat::types::ToSql> = params
        .iter()
        .map(|s| s as &dyn postgres_compat::types::ToSql)
        .collect();
    let mut stmt = conn.prepare(&format!(
        "SELECT COALESCE(NULLIF(wr.instrument_code_snapshot,''),'未绑定') AS instrument,
                COALESCE(NULLIF(wr.instrument_type_snapshot,''),'其他') AS instrument_type,
                SUM(wr.quantity)::BIGINT, COUNT(*), COUNT(DISTINCT COALESCE(CAST(wr.subject_user_id AS TEXT),'legacy:' || wr.user_name)), {}
         FROM work_records wr WHERE {}
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

/// v0.4.28: 按事业部统计
#[derive(Serialize)]
pub struct DivisionStats {
    pub division_id: Option<i64>,
    pub division_name: String,
    pub total_quantity: i64,
    pub record_count: i64,
    pub coefficient_score: f64,
    pub lab_count: i64,
}

async fn by_division(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<StatsQuery>,
) -> Result<Json<ApiResponse<Vec<DivisionStats>>>> {
    let (scoped_user, allowed_division_ids) = stats_scope(&pool, &headers, q.subject_user_id)?;
    let (wc, params) = apply_ownership_filters(
        build_where(
            q.start.as_deref(),
            q.end.as_deref(),
            scoped_user,
            allowed_division_ids.as_deref(),
        ),
        &q,
    )?;
    let wc = format!(
        "{wc} AND NOT ({})",
        super::export_data::legacy_auxiliary_record_sql("wr")
    );
    let conn = pool.get()?;
    let _scope = conn.unchecked_transaction()?;
    super::export_data::configure_text_filters(
        &conn,
        q.lab_name.as_deref(),
        q.project_name.as_deref(),
        q.instrument_code.as_deref(),
        q.method_name.as_deref(),
    )?;
    let refs: Vec<&dyn postgres_compat::types::ToSql> = params
        .iter()
        .map(|value| value as &dyn postgres_compat::types::ToSql)
        .collect();
    let sql = format!(
        "SELECT d.id,COALESCE(d.name,'N/A'),COALESCE(SUM(wr.quantity),0)::BIGINT,COUNT(wr.id),{},COUNT(DISTINCT wr.group_id)
         FROM work_records wr JOIN projects p ON p.id=wr.project_id
         LEFT JOIN divisions d ON d.id=wr.detection_division_id
         WHERE {wc} GROUP BY d.id,d.name ORDER BY COALESCE(d.name,'N/A')",
        coeff_sql(),
    );
    let mut statement = conn.prepare(&sql)?;
    let rows = statement.query_map(postgres_compat::params_from_iter(refs.iter()), |row| {
        Ok(DivisionStats {
            division_id: row.get(0)?,
            division_name: row.get(1)?,
            total_quantity: row.get(2)?,
            record_count: row.get(3)?,
            coefficient_score: row.get::<_, f64>(4).unwrap_or(0.0),
            lab_count: row.get(5)?,
        })
    })?;
    Ok(Json(ApiResponse::ok(
        rows.collect::<std::result::Result<Vec<_>, _>>()?,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_stats_date_bounds_include_fractional_last_second() {
        let pool = crate::db::init_pool("postgres-test");
        let conn = pool.get().unwrap();
        for end in ["2091-01-31", "2091-01-31T23:59:59", "2091-01-31 23:59:59"] {
            let (wc, params) = build_where(Some("2091-01-31"), Some(end), None, None);
            let sql=format!("WITH wr AS (SELECT recorded_at,NULL::TEXT AS deleted_at FROM (VALUES ('2091-01-31T23:59:59.500'),('2091-01-31T23:59:59.999999'),('2091-02-01T00:00:00'),('2091-02-01T23:00:00')) AS sample(recorded_at)) SELECT COUNT(*) FROM wr WHERE {wc}");
            let n: i64 = conn
                .query_row(
                    &sql,
                    postgres_compat::params_from_iter(params.iter()),
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 2, "end={end}");
        }
    }

    #[test]
    fn workload_score_does_not_apply_unit_price_multiplier() {
        assert_eq!(
            coeff_sql(),
            "COALESCE(SUM(wr.quantity * wr.coefficient_snapshot), 0.0)::DOUBLE PRECISION"
        );
    }
}
