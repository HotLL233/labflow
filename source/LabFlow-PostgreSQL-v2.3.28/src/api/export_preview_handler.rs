use super::export_data;
use super::export_handler::ExportQuery;
use crate::db::DbPool;
use crate::error::Result;
use crate::models::ApiResponse;
use crate::service::authz_service;
/// 导出数据预览 API - v0.3.7
/// 提供 11 个预览端点，对应 11 个 Sheet 的数据查询；Sheet 11 为检测部门-送样部门汇总。
use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::Json,
    routing::get,
    Router,
};
use chrono::Datelike;

fn export_scope(pool: &DbPool, headers: &HeaderMap) -> Result<(Option<i64>, Option<Vec<i64>>)> {
    let ctx = authz_service::authenticate(pool, headers)?;
    authz_service::require_permission(&ctx, "stats:portal:workload")?;
    let allowed_division_ids = authz_service::work_allowed_division_ids(pool, &ctx)?;
    let subject_user_id = if ctx.can_view_workload_scope() {
        None
    } else {
        Some(ctx.user.id)
    };
    Ok((subject_user_id, allowed_division_ids))
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
/// These values are read by `export_data` on the current pooled connection.
fn configure_export_scope(
    conn: &postgres_compat::Connection,
    q: &ExportQuery,
    allowed_division_ids: Option<&[i64]>,
) -> Result<()> {
    export_data::configure_dimension_filters(
        conn,
        q.detection_division_ids.as_deref(),
        q.sending_division_ids.as_deref(),
        q.include_pending_ownership.unwrap_or(false),
        allowed_division_ids,
    )
}

fn begin_export_scope<'a>(
    conn: &'a postgres_compat::Connection,
    q: &ExportQuery,
    allowed_division_ids: Option<&[i64]>,
) -> Result<postgres_compat::Transaction<'a>> {
    let tx = conn.unchecked_transaction()?;
    configure_export_scope(&tx, q, allowed_division_ids)?;
    Ok(tx)
}

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/export/preview/sheet1", get(preview_sheet1))
        .route("/api/export/preview/sheet2", get(preview_sheet2))
        .route("/api/export/preview/sheet3", get(preview_sheet3))
        .route("/api/export/preview/sheet4", get(preview_sheet4))
        .route("/api/export/preview/sheet5", get(preview_sheet5))
        .route("/api/export/preview/sheet6", get(preview_sheet6))
        .route("/api/export/preview/sheet7", get(preview_sheet7))
        .route("/api/export/preview/sheet8", get(preview_sheet8))
        .route("/api/export/preview/sheet9", get(preview_sheet9))
        .route("/api/export/preview/sheet10", get(preview_sheet10))
        .route("/api/export/preview/sheet11", get(preview_sheet11))
        .route("/api/export/preview/sheet13", get(preview_sheet13))
        .route("/api/export/filter-options", get(filter_options))
        .with_state(pool)
}

// ========== Sheet 1: 各实验室项目方法对应表 ==========

async fn preview_sheet1(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<ExportQuery>,
) -> Result<Json<ApiResponse<Vec<export_data::FlatRow>>>> {
    let (start, end) = resolve_date_range(&q);
    let conn = pool.get()?;
    let (subject_user_id, allowed_division_ids) = export_scope(&pool, &headers)?;
    let _scope = begin_export_scope(&conn, &q, allowed_division_ids.as_deref())?;
    let data = export_data::query_sheet1_data(
        &conn,
        &start,
        &end,
        q.group_id,
        subject_user_id,
        allowed_division_ids.as_deref(),
    )?;
    Ok(Json(ApiResponse::ok(data)))
}

// ========== Sheet 2: 仪器-汇总 ==========

async fn preview_sheet2(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<ExportQuery>,
) -> Result<Json<ApiResponse<Vec<export_data::InstrumentDailyRow>>>> {
    let (start, end) = resolve_date_range(&q);
    let conn = pool.get()?;
    let (subject_user_id, allowed_division_ids) = export_scope(&pool, &headers)?;
    let _scope = begin_export_scope(&conn, &q, allowed_division_ids.as_deref())?;
    let data = export_data::query_sheet2_data(
        &conn,
        &start,
        &end,
        subject_user_id,
        allowed_division_ids.as_deref(),
    )?;
    Ok(Json(ApiResponse::ok(data)))
}

// ========== Sheet 3: 项目-汇总 ==========

async fn preview_sheet3(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<ExportQuery>,
) -> Result<Json<ApiResponse<Vec<export_data::ProjectSummaryRow>>>> {
    let (start, end) = resolve_date_range(&q);
    let conn = pool.get()?;
    let (subject_user_id, allowed_division_ids) = export_scope(&pool, &headers)?;
    let _scope = begin_export_scope(&conn, &q, allowed_division_ids.as_deref())?;
    let data = export_data::query_sheet3_data(
        &conn,
        &start,
        &end,
        subject_user_id,
        allowed_division_ids.as_deref(),
    )?;
    Ok(Json(ApiResponse::ok(data)))
}

// ========== Sheet 4: 实验室-汇总 ==========

async fn preview_sheet4(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<ExportQuery>,
) -> Result<Json<ApiResponse<Vec<export_data::LabSummaryRow>>>> {
    let (start, end) = resolve_date_range(&q);
    let conn = pool.get()?;
    let (subject_user_id, allowed_division_ids) = export_scope(&pool, &headers)?;
    let _scope = begin_export_scope(&conn, &q, allowed_division_ids.as_deref())?;
    let data = export_data::query_sheet4_data(
        &conn,
        &start,
        &end,
        subject_user_id,
        allowed_division_ids.as_deref(),
    )?;
    Ok(Json(ApiResponse::ok(data)))
}

// ========== Sheet 5: 人员-汇总（原始记录） ==========

async fn preview_sheet5(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<ExportQuery>,
) -> Result<Json<ApiResponse<Vec<export_data::AnalysisPersonRecordRow>>>> {
    let (start, end) = resolve_date_range(&q);
    let conn = pool.get()?;
    let (subject_user_id, allowed_division_ids) = export_scope(&pool, &headers)?;
    let _scope = begin_export_scope(&conn, &q, allowed_division_ids.as_deref())?;
    let data = export_data::query_sheet5_data(
        &conn,
        &start,
        &end,
        subject_user_id,
        allowed_division_ids.as_deref(),
    )?;
    Ok(Json(ApiResponse::ok(data)))
}

// ========== Sheet 6: 人员汇总表 ==========

async fn preview_sheet6(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<ExportQuery>,
) -> Result<Json<ApiResponse<Vec<export_data::PersonSummaryRow>>>> {
    let (start, end) = resolve_date_range(&q);
    let conn = pool.get()?;
    let (subject_user_id, allowed_division_ids) = export_scope(&pool, &headers)?;
    let _scope = begin_export_scope(&conn, &q, allowed_division_ids.as_deref())?;
    let data = export_data::query_sheet6_data(
        &conn,
        &start,
        &end,
        subject_user_id,
        allowed_division_ids.as_deref(),
    )?;
    Ok(Json(ApiResponse::ok(data)))
}

// ========== Sheet 13: 人员工作量汇总（按检测类型逐行） ==========

async fn preview_sheet13(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<ExportQuery>,
) -> Result<Json<ApiResponse<Vec<export_data::PersonTypeWorkloadRow>>>> {
    let (start, end) = resolve_date_range(&q);
    let conn = pool.get()?;
    let (subject_user_id, allowed_division_ids) = export_scope(&pool, &headers)?;
    let _scope = begin_export_scope(&conn, &q, allowed_division_ids.as_deref())?;
    let data = export_data::query_person_type_workload_data(
        &conn,
        &start,
        &end,
        subject_user_id,
        allowed_division_ids.as_deref(),
    )?;
    Ok(Json(ApiResponse::ok(data)))
}

// ========== Sheet 7: 实验室总表 ==========

async fn preview_sheet7(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<ExportQuery>,
) -> Result<Json<ApiResponse<Vec<export_data::LabTotalRow>>>> {
    let (start, end) = resolve_date_range(&q);
    let conn = pool.get()?;
    let (subject_user_id, allowed_division_ids) = export_scope(&pool, &headers)?;
    let _scope = begin_export_scope(&conn, &q, allowed_division_ids.as_deref())?;
    let data = export_data::query_sheet7_data(
        &conn,
        &start,
        &end,
        subject_user_id,
        allowed_division_ids.as_deref(),
    )?;
    Ok(Json(ApiResponse::ok(data)))
}

// ========== Sheet 8: 项目总表 ==========

async fn preview_sheet8(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<ExportQuery>,
) -> Result<Json<ApiResponse<Vec<export_data::ProjectTotalRow>>>> {
    let (start, end) = resolve_date_range(&q);
    let conn = pool.get()?;
    let (subject_user_id, allowed_division_ids) = export_scope(&pool, &headers)?;
    let _scope = begin_export_scope(&conn, &q, allowed_division_ids.as_deref())?;
    let data = export_data::query_sheet8_data(
        &conn,
        &start,
        &end,
        subject_user_id,
        allowed_division_ids.as_deref(),
    )?;
    Ok(Json(ApiResponse::ok(data)))
}

// ========== Sheet 9: 仪器汇总表 ==========

async fn preview_sheet9(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<ExportQuery>,
) -> Result<Json<ApiResponse<Vec<export_data::InstrumentSummaryRow>>>> {
    let (start, end) = resolve_date_range(&q);
    let conn = pool.get()?;
    let (subject_user_id, allowed_division_ids) = export_scope(&pool, &headers)?;
    let _scope = begin_export_scope(&conn, &q, allowed_division_ids.as_deref())?;
    let data = export_data::query_sheet9_data(
        &conn,
        &start,
        &end,
        subject_user_id,
        allowed_division_ids.as_deref(),
    )?;
    Ok(Json(ApiResponse::ok(data)))
}

// ========== Sheet 10: 理化汇总表 ==========

async fn preview_sheet10(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<ExportQuery>,
) -> Result<Json<ApiResponse<Vec<export_data::PhysChemRow>>>> {
    let (start, end) = resolve_date_range(&q);
    let conn = pool.get()?;
    let (subject_user_id, allowed_division_ids) = export_scope(&pool, &headers)?;
    let _scope = begin_export_scope(&conn, &q, allowed_division_ids.as_deref())?;
    let data = export_data::query_sheet10_data(
        &conn,
        &start,
        &end,
        subject_user_id,
        allowed_division_ids.as_deref(),
    )?;
    Ok(Json(ApiResponse::ok(data)))
}

// ========== Sheet 11: 检测部门-送样部门汇总 ==========

async fn preview_sheet11(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<ExportQuery>,
) -> Result<Json<ApiResponse<Vec<export_data::DepartmentPairSummaryRow>>>> {
    let (start, end) = resolve_date_range(&q);
    let conn = pool.get()?;
    let (subject_user_id, allowed_division_ids) = export_scope(&pool, &headers)?;
    let _scope = begin_export_scope(&conn, &q, allowed_division_ids.as_deref())?;
    let data = export_data::query_department_pair_summary_data(
        &conn,
        &start,
        &end,
        subject_user_id,
        allowed_division_ids.as_deref(),
    )?;
    Ok(Json(ApiResponse::ok(data)))
}

async fn filter_options(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<ExportQuery>,
) -> Result<Json<ApiResponse<export_data::ExportFilterOptions>>> {
    let (start, end) = resolve_date_range(&q);
    let conn = pool.get()?;
    let _scope = conn.unchecked_transaction()?;
    let (subject_user_id, allowed_division_ids) = export_scope(&pool, &headers)?;
    let data = export_data::query_filter_options(
        &conn,
        &start,
        &end,
        q.detection_division_ids.as_deref(),
        q.include_pending_ownership.unwrap_or(false),
        subject_user_id,
        allowed_division_ids.as_deref(),
    )?;
    Ok(Json(ApiResponse::ok(data)))
}
