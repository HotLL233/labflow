use axum::{
    extract::State,
    http::HeaderMap,
    routing::{delete, get},
    Json, Router,
};
use serde::Serialize;

use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::ApiResponse;

#[derive(Debug, Serialize)]
pub struct SessionView {
    pub id: i64,
    pub user_id: i64,
    pub username: String,
    pub created_at: String,
    pub expires_at: String,
    pub device_id: String,
    pub device_name: String,
    pub is_expired: bool,
}

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/sessions", get(list))
        .route("/api/sessions/expired", delete(cleanup_expired))
        .with_state(pool)
}

fn require_admin(pool: &DbPool, headers: &HeaderMap) -> Result<()> {
    let ctx = crate::service::authz_service::authenticate(pool, headers)?;
    if !ctx.is_system_admin() {
        return Err(AppError::Forbidden("仅管理员可管理登录会话".into()));
    }
    Ok(())
}

async fn list(
    State(pool): State<DbPool>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<SessionView>>>> {
    require_admin(&pool, &headers)?;
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT s.id, s.user_id, u.username, s.created_at, s.expires_at,
                COALESCE(s.device_id,''), COALESCE(s.device_name,''),
                CASE WHEN datetime(s.expires_at) <= datetime('now') THEN 1 ELSE 0 END
         FROM user_sessions s
         JOIN users u ON u.id = s.user_id
         ORDER BY s.created_at DESC",
    )?;
    let sessions = stmt
        .query_map([], |row| {
            Ok(SessionView {
                id: row.get(0)?,
                user_id: row.get(1)?,
                username: row.get(2)?,
                created_at: row.get(3)?,
                expires_at: row.get(4)?,
                device_id: row.get(5)?,
                device_name: row.get(6)?,
                is_expired: row.get::<_, i64>(7)? != 0,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(Json(ApiResponse::ok(sessions)))
}

async fn cleanup_expired(
    State(pool): State<DbPool>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<usize>>> {
    require_admin(&pool, &headers)?;
    let policy =
        crate::service::log_maintenance_service::get_policy(&pool).map_err(AppError::Internal)?;
    let deleted = crate::service::log_maintenance_service::cleanup_sessions(&pool, &policy)
        .map_err(AppError::Internal)?
        .affected as usize;
    Ok(Json(ApiResponse {
        code: 0,
        message: format!("已清理 {} 条过期会话", deleted),
        data: Some(deleted),
    }))
}
