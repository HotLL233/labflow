use crate::db::DbPool;
use crate::error::Result;
use crate::models::audit::AuditLogResponse;
use crate::models::ApiResponse;
use crate::models::PaginatedResponse;
use crate::repo::audit_repo;
use axum::{
    extract::{Query, State},
    http::HeaderMap,
    routing::get,
    Json, Router,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct AuditQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    /// 模块过滤：work=分析检测, rd=研发送样, shared=仅主数据, 不传=全部
    pub module: Option<String>,
    pub action: Option<String>,
    pub user_name: Option<String>,
    pub business_no: Option<String>,
}

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/audit-logs", get(list))
        .with_state(pool)
}

async fn list(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<AuditQuery>,
) -> Result<Json<ApiResponse<PaginatedResponse<AuditLogResponse>>>> {
    let ctx = crate::service::authz_service::authenticate(&pool, &headers)?;
    crate::service::authz_service::require_permission(&ctx, "manage:audit")?;
    let page = q.page.unwrap_or(1);
    let page_size = q.page_size.unwrap_or(50).min(500);
    let (items, total) = audit_repo::list(
        &pool,
        page,
        page_size,
        q.module.as_deref(),
        q.action.as_deref(),
        q.user_name.as_deref(),
        q.business_no.as_deref(),
    )?;
    Ok(Json(ApiResponse::ok(PaginatedResponse {
        items,
        total,
        page,
        page_size,
    })))
}
