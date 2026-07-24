use crate::db::DbPool;
use crate::error::Result;
use crate::models::project::*;
use crate::models::trash::DeleteReasonRequest;
use crate::models::ApiResponse;
use crate::repo::project_repo;
use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json, Router,
};
use serde::Deserialize;

fn manager(
    pool: &DbPool,
    headers: &HeaderMap,
) -> Result<crate::service::authz_service::AuthContext> {
    let ctx = crate::service::authz_service::authenticate(pool, headers)?;
    crate::service::authz_service::require_permission(&ctx, "manage:projects")?;
    Ok(ctx)
}

#[derive(Deserialize)]
pub struct ProjectQuery {
    pub group_id: Option<i64>,
    pub active_only: Option<bool>,
    pub method_type: Option<String>,
    pub status: Option<String>,
}

#[derive(Deserialize)]
pub struct BatchCoefficientPayload {
    pub group_id: i64,
    pub coefficient: f64,
}

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/projects", axum::routing::get(list).post(create))
        .route(
            "/api/projects/:id",
            axum::routing::put(update).delete(delete),
        )
        .route(
            "/api/projects/batch-coefficient",
            axum::routing::put(batch_coefficient),
        )
        .with_state(pool)
}

async fn list(
    State(pool): State<DbPool>,
    Query(q): Query<ProjectQuery>,
) -> Result<Json<ApiResponse<Vec<ProjectResponse>>>> {
    let items = project_repo::list(
        &pool,
        q.group_id,
        q.active_only.unwrap_or(false),
        q.method_type.as_deref(),
        q.status.as_deref(),
    )?;
    Ok(Json(ApiResponse::ok(items)))
}

async fn create(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(b): Json<ProjectCreate>,
) -> Result<Json<ApiResponse<ProjectResponse>>> {
    let ctx = manager(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(project_repo::create(
        &pool,
        &b,
        &ctx.user.username,
    )?)))
}

async fn update(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(b): Json<ProjectUpdate>,
) -> Result<Json<ApiResponse<ProjectResponse>>> {
    let ctx = manager(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(project_repo::update(
        &pool,
        id,
        &b,
        &ctx.user.username,
    )?)))
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
    project_repo::delete(&pool, id, &ctx.user.username, reason)?;
    Ok(Json(ApiResponse::ok_msg("删除成功")))
}

async fn batch_coefficient(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(b): Json<BatchCoefficientPayload>,
) -> Result<Json<ApiResponse<i64>>> {
    let ctx = manager(&pool, &headers)?;
    let c = project_repo::batch_coefficient(&pool, b.group_id, b.coefficient, &ctx.user.username)?;
    Ok(Json(ApiResponse::ok_msg(format!("已更新{}个项目系数", c))))
}
