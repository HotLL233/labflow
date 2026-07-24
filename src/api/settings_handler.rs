use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::settings::{SettingUpdate, SystemSetting};
use crate::models::ApiResponse;
use crate::repo::settings_repo;
use crate::service::authz_service;
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json, Router,
};

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/settings", axum::routing::get(list))
        .route(
            "/api/settings/:key",
            axum::routing::get(get_by_key).put(upsert),
        )
        .with_state(pool)
}

/// 从 HeaderMap 中提取 JWT claims
/// GET /api/settings — 获取所有系统设置
async fn list(State(pool): State<DbPool>) -> Result<Json<ApiResponse<Vec<SystemSetting>>>> {
    let settings = settings_repo::get_all(&pool)?;
    Ok(Json(ApiResponse::ok(settings)))
}

/// GET /api/settings/:key — 获取单个系统设置
async fn get_by_key(
    State(pool): State<DbPool>,
    Path(key): Path<String>,
) -> Result<Json<ApiResponse<SystemSetting>>> {
    let setting = settings_repo::get(&pool, &key)?
        .ok_or_else(|| AppError::NotFound(format!("设置 '{}' 不存在", key)))?;
    Ok(Json(ApiResponse::ok(setting)))
}

/// PUT /api/settings/:key — 更新系统设置（需管理员权限）
async fn upsert(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(key): Path<String>,
    Json(body): Json<SettingUpdate>,
) -> Result<Json<ApiResponse<SystemSetting>>> {
    // 鉴权：检查管理员权限
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "manage:settings")?;

    let value_str = serde_json::to_string(&body.value)
        .map_err(|e| AppError::Internal(format!("JSON 序列化失败: {}", e)))?;

    let mut conn = pool.get().map_err(AppError::Pool)?;
    let before = settings_repo::get(&pool, &key)?
        .map(|setting| serde_json::json!({"key":setting.key,"value":setting.value}));
    let tx = conn.transaction().map_err(AppError::Database)?;
    settings_repo::upsert(&pool, &key, &value_str, Some(&tx))?;
    let after = serde_json::json!({"key":key,"value":value_str});
    crate::repo::audit_repo::log_structured_actor_on_conn(
        &tx,
        "update",
        "system_settings",
        Some(0),
        ctx.user.id,
        &ctx.user.username,
        &format!("更新系统设置: {}", key),
        "shared",
        &key,
        before.as_ref(),
        Some(&after),
        "management",
    )?;
    tx.commit().map_err(AppError::Database)?;

    let setting = settings_repo::get(&pool, &key)?
        .ok_or_else(|| AppError::Internal("保存后读取失败".into()))?;

    Ok(Json(ApiResponse::ok(setting)))
}
