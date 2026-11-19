use super::rd_export_data;
use super::rd_export_handler::ExportQuery;
use crate::db::DbPool;
use crate::error::Result;
use crate::models::ApiResponse;
use crate::service::authz_service;
/// 导出数据预览 API - v0.3.7
/// 提供 10 个预览端点，对应 10 个 Sheet 的数据查询
use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::Json,
    routing::get,
    Router,
};
use chrono::Datelike;

fn export_scope(
    pool: &DbPool,
    headers: &HeaderMap,
) -> Result<(Option<i64>, Option<i64>, Option<Vec<i64>>)> {
    let ctx = authz_service::authenticate(pool, headers)?;
    authz_service::require_permission(&ctx, "stats:portal:rd")?;
    let allowed_division_ids = authz_service::rd_allowed_division_ids(pool, &ctx)?;
    if ctx.is_system_admin() || ctx.has_permission("stats:rd:view-all") {
        Ok((None, None, allowed_division_ids))
    } else if ctx.has_permission("stats:rd:view-lab") {
        Ok((
            None,
            Some(ctx.user.group_id.ok_or_else(|| {
                crate::error::AppError::Validation("当前统计角色尚未设置所属实验室".into())
            })?),
            allowed_division_ids,
        ))
    } else {
        Ok((Some(ctx.user.id), None, allowed_division_ids))
    }
}

/// 解析日期范围（复用 export_handler 逻辑）
fn resolve_date_range(q: &ExportQuery) -> (String, String) {
    if let Some(ref s) = q.start {
        let end = q.end.as_ref().cloned().unwrap_or_else(|| s.clone());
        (s.clone(), end)
    } else {
        let now = chrono::Local::now();
        let start = format!("{}-{:02}-01", now.year(), now.month());

        let last_day = if now.month() == 12 {
            chrono::NaiveDate::from_ymd_opt(now.year() + 1, 1, 1).and_then(|d| d.pred_opt())
        } else {
            chrono::NaiveDate::from_ymd_opt(now.year(), now.month() + 1, 1)
                .and_then(|d| d.pred_opt())
        };

        let end = if let Some(d) = last_day {
            d.format("%Y-%m-%d").to_string()
        } else {
            format!("{}-{:02}-28", now.year(), now.month())
        };

        (start, end)
    }
}

/// Preview queries use the same ownership scope as the final Excel export.
/// These values are read by `rd_export_data` on the current pooled connection.
fn configure_export_scope(conn: &postgres_compat::Connection, q: &ExportQuery) -> Result<()> {
    let ownership_basis = match q.ownership_basis.as_deref().unwrap_or("execution") {
        "execution" | "submitted" | "project" => {
            q.ownership_basis.as_deref().unwrap_or("execution")
        }
        _ => {
            return Err(crate::error::AppError::Validation(
                "统计口径只能是执行部门、送样部门或项目归属部门".into(),
            ))
        }
    };
    conn.execute(
        "SELECT set_config('workload.export_ownership_basis',?1,true)",
        [ownership_basis],
    )?;
    conn.execute(
        "SELECT set_config('workload.export_division_id',?1,true)",
        [q.division_id
            .map(|id| id.to_string())
            .as_deref()
            .unwrap_or("0")],
    )?;
    conn.execute(
        "SELECT set_config('workload.export_include_pending_ownership',?1,true)",
        [q.include_pending_ownership.unwrap_or(false).to_string()],
    )?;
    Ok(())
}

fn begin_export_scope<'a>(
    conn: &'a postgres_compat::Connection,
    q: &ExportQuery,
) -> Result<postgres_compat::Transaction<'a>> {
    let tx = conn.unchecked_transaction()?;
    configure_export_scope(&tx, q)?;
    Ok(tx)
}

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/rd-export/preview/sheet1", get(preview_sheet1))
        .route("/api/rd-export/preview/sheet2", get(preview_sheet2))
        .route("/api/rd-export/preview/sheet3", get(preview_sheet3))
        .route("/api/rd-export/preview/sheet4", get(preview_sheet4))
        .route("/api/rd-export/preview/sheet5", get(preview_sheet5))
        .route("/api/rd-export/preview/sheet6", get(preview_sheet6))
        .route("/api/rd-export/preview/sheet7", get(preview_sheet7))
        .route("/api/rd-export/preview/sheet8", get(preview_sheet8))
        .route("/api/rd-export/preview/sheet9", get(preview_sheet9))
        .route("/api/rd-export/preview/sheet10", get(preview_sheet10))
        .route("/api/rd-export/preview/sheet11", get(preview_sheet11))
        .with_state(pool)
}

// ========== Sheet 1: 各实验室项目方法对应表 ==========

async fn preview_sheet1(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<ExportQuery>,
) -> Result<Json<ApiResponse<Vec<rd_export_data::FlatRow>>>> {
    let (start, end) = resolve_date_range(&q);
    let conn = pool.get()?;
    let _scope = begin_export_scope(&conn, &q)?;
    let (user_id, group_id, allowed_division_ids) = export_scope(&pool, &headers)?;
    let data = rd_export_data::query_sheet1_data(
        &conn,
        &start,
        &end,
        q.group_id,
        user_id,
        group_id,
        allowed_division_ids.as_deref(),
    )?;
    Ok(Json(ApiResponse::ok(data)))
}

// ========== Sheet 2: 仪器-汇总 ==========

async fn preview_sheet2(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<ExportQuery>,
) -> Result<Json<ApiResponse<Vec<rd_export_data::InstrumentDailyRow>>>> {
    let (start, end) = resolve_date_range(&q);
    let conn = pool.get()?;
    let _scope = begin_export_scope(&conn, &q)?;
    let (user_id, group_id, allowed_division_ids) = export_scope(&pool, &headers)?;
    let data = rd_export_data::query_sheet2_data(
        &conn,
        &start,
        &end,
        user_id,
        group_id,
        allowed_division_ids.as_deref(),
    )?;
    Ok(Json(ApiResponse::ok(data)))
}

// ========== Sheet 3: 项目-汇总 ==========

async fn preview_sheet3(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<ExportQuery>,
) -> Result<Json<ApiResponse<Vec<rd_export_data::ProjectSummaryRow>>>> {
    let (start, end) = resolve_date_range(&q);
    let conn = pool.get()?;
    let _scope = begin_export_scope(&conn, &q)?;
    let (user_id, group_id, allowed_division_ids) = export_scope(&pool, &headers)?;
    let data = rd_export_data::query_sheet3_data(
        &conn,
        &start,
        &end,
        user_id,
        group_id,
        allowed_division_ids.as_deref(),
    )?;
    Ok(Json(ApiResponse::ok(data)))
}

// ========== Sheet 4: 实验室-汇总 ==========

async fn preview_sheet4(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<ExportQuery>,
) -> Result<Json<ApiResponse<Vec<rd_export_data::LabSummaryRow>>>> {
    let (start, end) = resolve_date_range(&q);
    let conn = pool.get()?;
    let _scope = begin_export_scope(&conn, &q)?;
    let (user_id, group_id, allowed_division_ids) = export_scope(&pool, &headers)?;
    let data = rd_export_data::query_sheet4_data(
        &conn,
        &start,
        &end,
        user_id,
        group_id,
        allowed_division_ids.as_deref(),
    )?;
    Ok(Json(ApiResponse::ok(data)))
}

// ========== Sheet 5: 人员-汇总（原始记录） ==========

async fn preview_sheet5(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<ExportQuery>,
) -> Result<Json<ApiResponse<Vec<rd_export_data::PersonRecordRow>>>> {
    let (start, end) = resolve_date_range(&q);
    let conn = pool.get()?;
    let _scope = begin_export_scope(&conn, &q)?;
    let (user_id, group_id, allowed_division_ids) = export_scope(&pool, &headers)?;
    let data = rd_export_data::query_sheet5_data(
        &conn,
        &start,
        &end,
        user_id,
        group_id,
        allowed_division_ids.as_deref(),
    )?;
    Ok(Json(ApiResponse::ok(data)))
}

// ========== Sheet 6: 人员汇总表 ==========

async fn preview_sheet6(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<ExportQuery>,
) -> Result<Json<ApiResponse<Vec<rd_export_data::PersonSummaryRow>>>> {
    let (start, end) = resolve_date_range(&q);
    let conn = pool.get()?;
    let _scope = begin_export_scope(&conn, &q)?;
    let (user_id, group_id, allowed_division_ids) = export_scope(&pool, &headers)?;
    let data = rd_export_data::query_sheet6_data(
        &conn,
        &start,
        &end,
        user_id,
        group_id,
        allowed_division_ids.as_deref(),
    )?;
    Ok(Json(ApiResponse::ok(data)))
}

// ========== Sheet 7: 实验室总表 ==========

async fn preview_sheet7(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<ExportQuery>,
) -> Result<Json<ApiResponse<Vec<rd_export_data::LabTotalRow>>>> {
    let (start, end) = resolve_date_range(&q);
    let conn = pool.get()?;
    let _scope = begin_export_scope(&conn, &q)?;
    let (user_id, group_id, allowed_division_ids) = export_scope(&pool, &headers)?;
    let data = rd_export_data::query_sheet7_data(
        &conn,
        &start,
        &end,
        user_id,
        group_id,
        allowed_division_ids.as_deref(),
    )?;
    Ok(Json(ApiResponse::ok(data)))
}

// ========== Sheet 8: 项目总表 ==========

async fn preview_sheet8(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<ExportQuery>,
) -> Result<Json<ApiResponse<Vec<rd_export_data::ProjectTotalRow>>>> {
    let (start, end) = resolve_date_range(&q);
    let conn = pool.get()?;
    let _scope = begin_export_scope(&conn, &q)?;
    let (user_id, group_id, allowed_division_ids) = export_scope(&pool, &headers)?;
    let data = rd_export_data::query_sheet8_data(
        &conn,
        &start,
        &end,
        user_id,
        group_id,
        allowed_division_ids.as_deref(),
    )?;
    Ok(Json(ApiResponse::ok(data)))
}

// ========== Sheet 9: 仪器汇总表 ==========

async fn preview_sheet9(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<ExportQuery>,
) -> Result<Json<ApiResponse<Vec<rd_export_data::InstrumentSummaryRow>>>> {
    let (start, end) = resolve_date_range(&q);
    let conn = pool.get()?;
    let _scope = begin_export_scope(&conn, &q)?;
    let (user_id, group_id, allowed_division_ids) = export_scope(&pool, &headers)?;
    let data = rd_export_data::query_sheet9_data(
        &conn,
        &start,
        &end,
        user_id,
        group_id,
        allowed_division_ids.as_deref(),
    )?;
    Ok(Json(ApiResponse::ok(data)))
}

// ========== Sheet 10: 理化汇总表 ==========

async fn preview_sheet10(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<ExportQuery>,
) -> Result<Json<ApiResponse<Vec<rd_export_data::PhysChemRow>>>> {
    let (start, end) = resolve_date_range(&q);
    let conn = pool.get()?;
    let _scope = begin_export_scope(&conn, &q)?;
    let (user_id, group_id, allowed_division_ids) = export_scope(&pool, &headers)?;
    let data = rd_export_data::query_sheet10_data(
        &conn,
        &start,
        &end,
        user_id,
        group_id,
        allowed_division_ids.as_deref(),
    )?;
    Ok(Json(ApiResponse::ok(data)))
}

// ========== Sheet 11: 类型汇总表 ==========

async fn preview_sheet11(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<ExportQuery>,
) -> Result<Json<ApiResponse<Vec<rd_export_data::TypeSummaryRow>>>> {
    let (start, end) = resolve_date_range(&q);
    let conn = pool.get()?;
    let _scope = begin_export_scope(&conn, &q)?;
    let (user_id, group_id, allowed_division_ids) = export_scope(&pool, &headers)?;
    let data = rd_export_data::query_sheet11_data(
        &conn,
        &start,
        &end,
        user_id,
        group_id,
        allowed_division_ids.as_deref(),
    )?;
    Ok(Json(ApiResponse::ok(data)))
}
