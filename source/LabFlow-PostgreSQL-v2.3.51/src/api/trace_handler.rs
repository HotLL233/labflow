use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::{trace::RecordEvent, ApiResponse};
use crate::repo::trace_repo;
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json, Router,
};

fn table_for_module(module: &str) -> Result<&'static str> {
    match module {
        "work" => Ok("work_records"),
        "rd" => Ok("rd_work_records"),
        "sample-info" => Ok("sample_info_records"),
        _ => Err(AppError::Validation("无效的溯源模块".into())),
    }
}

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/trace/:module/:id", axum::routing::get(list))
        .with_state(pool)
}

async fn list(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path((module, id)): Path<(String, i64)>,
) -> Result<Json<ApiResponse<Vec<RecordEvent>>>> {
    let ctx = crate::service::authz_service::authenticate(&pool, &headers)?;
    crate::service::authz_service::require_permission(&ctx, "manage:audit")?;
    Ok(Json(ApiResponse::ok(trace_repo::list(
        &pool,
        table_for_module(&module)?,
        id,
    )?)))
}
