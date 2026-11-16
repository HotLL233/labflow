use crate::db::DbPool;
use crate::error::Result;
use crate::models::trash::DeleteReasonRequest;
use crate::models::{instrument::*, ApiResponse};
use crate::repo::instrument_repo;
use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::get,
    Json, Router,
};

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/instruments", get(list).post(create))
        .route(
            "/api/instruments/:id",
            axum::routing::put(update).delete(delete),
        )
        .with_state(pool)
}

/// v2.3.19：补登录校验。此前仪器台账未登录即可读取。
async fn list(
    State(pool): State<DbPool>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<InstrumentResponse>>>> {
    crate::service::authz_service::authenticate(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(instrument_repo::list(&pool)?)))
}

fn require_manager(
    pool: &DbPool,
    headers: &HeaderMap,
) -> Result<crate::service::authz_service::AuthContext> {
    let ctx = crate::service::authz_service::authenticate(pool, headers)?;
    crate::service::authz_service::require_permission(&ctx, "manage:instruments")?;
    Ok(ctx)
}

async fn create(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(body): Json<InstrumentCreate>,
) -> Result<Json<ApiResponse<InstrumentResponse>>> {
    let ctx = require_manager(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(instrument_repo::create(
        &pool,
        &body,
        &ctx.user.username,
    )?)))
}

async fn update(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<InstrumentUpdate>,
) -> Result<Json<ApiResponse<InstrumentResponse>>> {
    let ctx = require_manager(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(instrument_repo::update(
        &pool,
        id,
        &body,
        &ctx.user.username,
    )?)))
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
    instrument_repo::delete(&pool, id, &ctx.user.username, reason)?;
    Ok(Json(ApiResponse::ok_msg("删除成功")))
}
