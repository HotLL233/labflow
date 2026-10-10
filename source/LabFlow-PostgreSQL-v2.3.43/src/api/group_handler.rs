use crate::db::DbPool;
use crate::error::Result;
use crate::models::group::{GroupCreate, GroupResponse, GroupUpdate};
use crate::models::trash::DeleteReasonRequest;
use crate::models::ApiResponse;
use crate::repo::group_repo;
use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::get,
    Json, Router,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct PortalQuery {
    portal: Option<String>,
}

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/groups", get(list).post(create))
        .route("/api/groups/:id", axum::routing::put(update).delete(delete))
        .with_state(pool)
}

async fn list(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(query): Query<PortalQuery>,
) -> Result<Json<ApiResponse<Vec<GroupResponse>>>> {
    let ctx = crate::service::authz_service::authenticate(&pool, &headers)?;
    let mut items = group_repo::list_for_portal(&pool, query.portal.as_deref())?;
    if query.portal.as_deref() == Some("rd") {
        if let Some(allowed_ids) =
            crate::service::authz_service::rd_allowed_division_ids(&pool, &ctx)?
        {
            items.retain(|item| item.division_id.is_some_and(|id| allowed_ids.contains(&id)));
        }
    } else if query.portal.as_deref() == Some("work") {
        if let Some(allowed_ids) =
            crate::service::authz_service::work_allowed_division_ids(&pool, &ctx)?
        {
            items.retain(|item| item.division_id.is_some_and(|id| allowed_ids.contains(&id)));
        }
    }
    Ok(Json(ApiResponse::ok(items)))
}

fn require_manager(
    pool: &DbPool,
    headers: &HeaderMap,
) -> Result<crate::service::authz_service::AuthContext> {
    let ctx = crate::service::authz_service::authenticate(pool, headers)?;
    crate::service::authz_service::require_permission(&ctx, "manage:groups")?;
    Ok(ctx)
}

async fn create(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(body): Json<GroupCreate>,
) -> Result<Json<ApiResponse<GroupResponse>>> {
    let ctx = require_manager(&pool, &headers)?;
    let item = group_repo::create(&pool, &body, &ctx.user.username)?;
    Ok(Json(ApiResponse::ok(item)))
}

async fn update(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<GroupUpdate>,
) -> Result<Json<ApiResponse<GroupResponse>>> {
    let ctx = require_manager(&pool, &headers)?;
    let item = group_repo::update(&pool, id, &body, &ctx.user.username)?;
    Ok(Json(ApiResponse::ok(item)))
}

async fn delete(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(body): Query<DeleteReasonRequest>,
) -> Result<Json<ApiResponse<()>>> {
    let ctx = require_manager(&pool, &headers)?;
    let reason = body
        .reason
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("用户删除");
    group_repo::delete(&pool, id, &ctx.user.username, reason)?;
    Ok(Json(ApiResponse::ok_msg("删除成功")))
}
