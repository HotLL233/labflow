use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::record::{RecordCreate, RecordResponse, RecordUpdate};
use crate::models::trash::DeleteReasonRequest;
use crate::models::ApiResponse;
use crate::models::PaginatedResponse;
use crate::repo::record_repo;
use crate::service::authz_service::{self, RecordScope};
use crate::service::record_service;
use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::get,
    Json, Router,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct RecordQuery {
    pub project_id: Option<i64>,
    pub group_id: Option<i64>,
    pub user_name: Option<String>,
    pub division_id: Option<i64>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub include_deleted: Option<bool>,
}

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/records", get(list).post(create))
        .route("/api/records/users", get(users))
        .route(
            "/api/records/:id",
            axum::routing::put(update).delete(soft_delete),
        )
        .route("/api/records/restore/:id", axum::routing::post(restore))
        .route(
            "/api/records/by-user/:user_name",
            axum::routing::delete(delete_by_user),
        )
        .with_state(pool)
}

async fn list(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<RecordQuery>,
) -> Result<Json<ApiResponse<PaginatedResponse<RecordResponse>>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "entry:workload")?;
    let page = q.page.unwrap_or(1);
    let page_size = q.page_size.unwrap_or(50).min(500);
    let scoped_user = match ctx.workload_scope() {
        RecordScope::Global | RecordScope::AnalysisAll => q.user_name.as_deref(),
        _ => None,
    };
    let scoped_user_id = match ctx.workload_scope() {
        RecordScope::Global | RecordScope::AnalysisAll => None,
        _ => Some(ctx.user.id),
    };
    let (items, total) = record_repo::list(
        &pool,
        q.project_id,
        q.group_id,
        scoped_user_id,
        scoped_user,
        q.division_id,
        q.start.as_deref(),
        q.end.as_deref(),
        page,
        page_size,
        q.include_deleted.unwrap_or(false),
    )?;
    Ok(Json(ApiResponse::ok(PaginatedResponse {
        items,
        total,
        page,
        page_size,
    })))
}

async fn create(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(body): Json<RecordCreate>,
) -> Result<Json<ApiResponse<RecordResponse>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "entry:workload")?;
    let user_name = if matches!(
        ctx.workload_scope(),
        RecordScope::Global | RecordScope::AnalysisAll
    ) {
        body.user_name
    } else {
        ctx.user.username.clone()
    };
    let record = RecordCreate {
        project_id: body.project_id,
        method_id: body.method_id,
        user_name,
        quantity: body.quantity,
        recorded_at: body.recorded_at,
        group_id: body.group_id,
        multiplier: body.multiplier,
        division_id: body.division_id,
        high_item: body.high_item,
    };
    // Service layer: validates quantity > 0 and project existence
    Ok(Json(ApiResponse::ok(record_service::create_record(
        &pool,
        &record,
        &ctx.user.username,
    )?)))
}

async fn update(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<RecordUpdate>,
) -> Result<Json<ApiResponse<RecordResponse>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    let existing = record_repo::get_by_id(&pool, id)?;
    if !matches!(
        ctx.workload_scope(),
        RecordScope::Global | RecordScope::AnalysisAll
    ) && existing.subject_user_id != Some(ctx.user.id)
    {
        return Err(AppError::Forbidden("只能修改本人的分析检测记录".into()));
    }
    // Service layer: validates not-deleted, change detection
    Ok(Json(ApiResponse::ok(record_service::update_record(
        &pool,
        id,
        &body,
        &ctx.user.username,
    )?)))
}

async fn soft_delete(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(body): Query<DeleteReasonRequest>,
) -> Result<Json<ApiResponse<()>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    let existing = record_repo::get_by_id(&pool, id)?;
    if !matches!(
        ctx.workload_scope(),
        RecordScope::Global | RecordScope::AnalysisAll
    ) && existing.subject_user_id != Some(ctx.user.id)
    {
        return Err(AppError::Forbidden("只能删除本人的分析检测记录".into()));
    }
    // Service layer: validates record exists and not already deleted
    let reason = body
        .reason
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("用户删除");
    record_service::delete_record(&pool, id, &ctx.user.username, reason)?;
    Ok(Json(ApiResponse::ok_msg("删除成功")))
}

async fn restore(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<RecordResponse>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    let existing = record_repo::get_by_id(&pool, id)?;
    if !matches!(
        ctx.workload_scope(),
        RecordScope::Global | RecordScope::AnalysisAll
    ) && existing.subject_user_id != Some(ctx.user.id)
    {
        return Err(AppError::Forbidden("只能恢复本人的分析检测记录".into()));
    }
    Ok(Json(ApiResponse::ok(record_repo::restore(
        &pool,
        id,
        &ctx.user.username,
    )?)))
}

#[derive(Deserialize)]
pub struct UsersQuery {
    pub start: Option<String>,
    pub end: Option<String>,
}

async fn users(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<UsersQuery>,
) -> Result<Json<ApiResponse<Vec<String>>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    if !matches!(
        ctx.workload_scope(),
        RecordScope::Global | RecordScope::AnalysisAll
    ) {
        return Ok(Json(ApiResponse::ok(vec![ctx.user.username])));
    }
    let conn = pool.get()?;
    let mut sql =
        String::from("SELECT DISTINCT user_name FROM work_records WHERE deleted_at IS NULL");
    let mut params: Vec<Box<dyn postgres_compat::types::ToSql>> = vec![];
    if let Some(ref s) = q.start {
        let idx = params.len() + 1;
        sql.push_str(&format!(" AND recorded_at>=?{}", idx));
        params.push(Box::new(s.to_string()));
    }
    if let Some(ref e) = q.end {
        let idx = params.len() + 1;
        sql.push_str(&format!(" AND recorded_at<=?{}", idx));
        params.push(Box::new(format!("{}T23:59:59", e)));
    }
    sql.push_str(" ORDER BY user_name");

    let mut stmt = conn.prepare(&sql)?;
    let users: Vec<String> = stmt
        .query_map(
            postgres_compat::params_from_iter(params.iter().map(|p| p.as_ref())),
            |row| row.get::<_, String>(0),
        )?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(Json(ApiResponse::ok(users)))
}

#[derive(Deserialize)]
pub struct DeleteByUserQuery {
    pub start: Option<String>,
    pub end: Option<String>,
    pub reason: Option<String>,
}

async fn delete_by_user(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(user_name): Path<String>,
    Query(q): Query<DeleteByUserQuery>,
) -> Result<Json<ApiResponse<serde_json::Value>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    if !matches!(
        ctx.workload_scope(),
        RecordScope::Global | RecordScope::AnalysisAll
    ) && user_name != ctx.user.username
    {
        return Err(AppError::Forbidden("只能删除本人的分析检测记录".into()));
    }
    let count = record_repo::delete_by_user(
        &pool,
        &user_name,
        q.start.as_deref(),
        q.end.as_deref(),
        &ctx.user.username,
        q.reason
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("批量删除"),
    )?;
    Ok(Json(ApiResponse::ok(
        serde_json::json!({"deleted_count": count}),
    )))
}
