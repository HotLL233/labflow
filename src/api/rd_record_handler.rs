use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::rd_record::{RdRecordCreate, RdRecordResponse};
use crate::models::record::{RecordCreate, RecordUpdate};
use crate::models::trash::DeleteReasonRequest;
use crate::models::ApiResponse;
use crate::models::PaginatedResponse;
use crate::repo::rd_record_repo;
use crate::service::authz_service::{self, RecordScope};
use crate::service::notification_service;
use crate::service::rd_record_service;
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
        .route("/api/rd-records", get(list).post(create))
        .route(
            "/api/rd-records/:id",
            axum::routing::put(update).delete(soft_delete),
        )
        .route("/api/rd-records/:id/sample", axum::routing::put(sample))
        .route("/api/rd-records/restore/:id", axum::routing::post(restore))
        .route(
            "/api/rd-records/by-user/:user_name",
            axum::routing::delete(delete_by_user),
        )
        .with_state(pool)
}

async fn list(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<RecordQuery>,
) -> Result<Json<ApiResponse<PaginatedResponse<RdRecordResponse>>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    if !ctx.has_permission("entry:sample")
        && !ctx.has_permission("records:rd:view-all")
        && !ctx.has_permission("records:rd:view-lab")
    {
        return Err(AppError::Forbidden("无研发送样记录查看权限".into()));
    }
    let page = q.page.unwrap_or(1);
    let page_size = q.page_size.unwrap_or(50).min(500);
    let scope = if q.include_deleted.unwrap_or(false)
        && ctx.is_analysis_member()
        && !ctx.is_analysis_leader()
        && !ctx.is_system_admin()
    {
        RecordScope::Own
    } else {
        ctx.rd_scope()?
    };
    let scoped_group = match scope {
        RecordScope::Lab(group_id) => Some(group_id),
        _ => q.group_id,
    };
    let scoped_user = match scope {
        RecordScope::Own => None,
        _ => q.user_name.as_deref(),
    };
    let scoped_user_id = if matches!(scope, RecordScope::Own) {
        Some(ctx.user.id)
    } else {
        None
    };
    let (items, total) = rd_record_repo::list(
        &pool,
        q.project_id,
        scoped_group,
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
    Json(body): Json<RdRecordCreate>,
) -> Result<Json<ApiResponse<RdRecordResponse>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "entry:sample")?;
    let record = RecordCreate {
        project_id: body.project_id,
        method_id: body.method_id,
        user_name: ctx.user.username.clone(),
        quantity: body.quantity,
        recorded_at: body.recorded_at,
        group_id: body.group_id,
        multiplier: None,
        division_id: body.division_id,
        high_item: None,
    };
    // Service layer: validates quantity > 0 and project existence
    let result = rd_record_service::create_record(
        &pool,
        &record,
        body.batch_no,
        body.notes,
        &ctx.user.username,
    )?;
    // Queue only after the business record is committed; notification failures never block sending.
    notification_service::enqueue_rd_submitted(&pool, &result)?;
    let notification_pool = pool.clone();
    tokio::task::spawn_blocking(move || {
        if let Err(error) = notification_service::process_pending(&notification_pool, 20) {
            tracing::warn!("rd sample notification processing failed: {error}");
        }
    });
    Ok(Json(ApiResponse::ok(result)))
}

async fn update(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<RecordUpdate>,
) -> Result<Json<ApiResponse<RdRecordResponse>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    let existing = rd_record_repo::get_by_id(&pool, id)?;
    if !ctx.is_system_admin() && existing.created_by_user_id != Some(ctx.user.id) {
        return Err(AppError::Forbidden("只能修改本人提交的研发送样记录".into()));
    }
    // v0.4.34: 已取样记录不可修改
    if rd_record_repo::is_sampled(&pool, id)? {
        return Err(crate::error::AppError::Forbidden(
            "该记录已取样，不可修改".to_string(),
        ));
    }
    let result = rd_record_service::update_record(&pool, id, &body, &ctx.user.username)?;
    Ok(Json(ApiResponse::ok(result)))
}

async fn soft_delete(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(body): Query<DeleteReasonRequest>,
) -> Result<Json<ApiResponse<()>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    let existing = rd_record_repo::get_by_id(&pool, id)?;
    if !ctx.is_system_admin() && existing.created_by_user_id != Some(ctx.user.id) {
        return Err(AppError::Forbidden("只能删除本人提交的研发送样记录".into()));
    }
    let reason = body
        .reason
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("用户删除");
    rd_record_service::delete_record(&pool, id, &ctx.user.username, reason)?;
    Ok(Json(ApiResponse::ok_msg("删除成功")))
}

async fn restore(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<RdRecordResponse>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    let existing = rd_record_repo::get_by_id(&pool, id)?;
    let can_restore = ctx.is_system_admin()
        || ctx.is_analysis_leader()
        || existing.created_by_user_id == Some(ctx.user.id)
        || (ctx.is_rd_leader()
            && existing.group_id.is_some()
            && existing.group_id == ctx.user.group_id);
    if !can_restore {
        return Err(AppError::Forbidden("只能恢复本人提交的研发送样记录".into()));
    }
    Ok(Json(ApiResponse::ok(rd_record_repo::restore(
        &pool,
        id,
        &ctx.user.username,
    )?)))
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
    if !ctx.is_system_admin() && user_name != ctx.user.username {
        return Err(AppError::Forbidden("只能批量删除本人提交的记录".into()));
    }
    let count = rd_record_repo::delete_by_user(
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

async fn sample(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<RdRecordResponse>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "sample:collect")?;
    let result = rd_record_service::sample(&pool, id, &ctx.user.username)?;
    Ok(Json(ApiResponse::ok(result)))
}
