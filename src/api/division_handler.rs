use crate::db::DbPool;
use crate::error::Result;
use crate::models::division::{Division, DivisionCreate, DivisionResponse, DivisionUpdate};
use crate::models::trash::DeleteReasonRequest;
use crate::models::ApiResponse;
use crate::repo::audit_repo;
use crate::repo::division_repo;
use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::get,
    Json, Router,
};
use serde::Deserialize;

fn manager(
    pool: &DbPool,
    headers: &HeaderMap,
) -> Result<crate::service::authz_service::AuthContext> {
    let ctx = crate::service::authz_service::authenticate(pool, headers)?;
    crate::service::authz_service::require_permission(&ctx, "manage:divisions")?;
    Ok(ctx)
}

#[derive(Deserialize)]
pub struct SetLabsRequest {
    pub group_ids: Vec<i64>,
}

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/divisions", get(list).post(create))
        .route(
            "/api/divisions/:id",
            axum::routing::put(update).delete(delete),
        )
        .route("/api/divisions/:id/labs", axum::routing::put(set_labs))
        .with_state(pool)
}

async fn list(State(pool): State<DbPool>) -> Result<Json<ApiResponse<Vec<DivisionResponse>>>> {
    let items = division_repo::list(&pool)?;
    Ok(Json(ApiResponse::ok(items)))
}

async fn create(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(body): Json<DivisionCreate>,
) -> Result<Json<ApiResponse<Division>>> {
    let user_name = manager(&pool, &headers)?.user.username;
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let item = division_repo::create_on_conn(&tx, &body)?;
    let after = serde_json::to_value(&item).unwrap_or(serde_json::Value::Null);
    audit_repo::log_structured_on_conn(
        &tx,
        "create",
        "divisions",
        Some(item.id),
        &user_name,
        &format!("创建事业部「{}」", item.name),
        "shared",
        "",
        None,
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(Json(ApiResponse::ok(item)))
}

async fn update(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<DivisionUpdate>,
) -> Result<Json<ApiResponse<Division>>> {
    let user_name = manager(&pool, &headers)?.user.username;
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let current = division_repo::get_by_id_on_conn(&tx, id)?;
    let before = serde_json::to_value(&current).unwrap_or(serde_json::Value::Null);
    let item = division_repo::update_on_conn(&tx, id, &body)?;
    let after = serde_json::to_value(&item).unwrap_or(serde_json::Value::Null);
    let detail = body.name.clone().map_or_else(
        || "更新事业部".to_string(),
        |n| format!("更新事业部「{}」", n),
    );
    audit_repo::log_structured_on_conn(
        &tx,
        "update",
        "divisions",
        Some(id),
        &user_name,
        &detail,
        "shared",
        "",
        Some(&before),
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(Json(ApiResponse::ok(item)))
}

async fn delete(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(body): Query<DeleteReasonRequest>,
) -> Result<Json<ApiResponse<()>>> {
    let ctx = manager(&pool, &headers)?;
    let reason = body
        .reason
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("用户删除");
    division_repo::delete(&pool, id, &ctx.user.username, reason)?;
    // 审计在 repo.delete 的事务内完成
    Ok(Json(ApiResponse::ok_msg("删除成功")))
}

/// PUT /api/divisions/:id/labs — 批量设置事业部关联的实验室
async fn set_labs(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<SetLabsRequest>,
) -> Result<Json<ApiResponse<()>>> {
    let user_name = manager(&pool, &headers)?.user.username;
    division_repo::set_division_labs(&pool, id, &body.group_ids, &user_name)?;
    Ok(Json(ApiResponse::ok_msg("实验室关联更新成功")))
}
