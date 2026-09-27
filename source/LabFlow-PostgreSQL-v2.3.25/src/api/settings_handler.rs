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

fn setting_save_error(stage: &str, error: impl std::fmt::Display) -> AppError {
    tracing::error!(stage, detail = %error, "system setting save failed");
    AppError::Internal(format!(
        "保存系统配置失败（{}），请重试；如持续出现请联系管理员。",
        stage
    ))
}

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/settings", axum::routing::get(list))
        .route(
            "/api/settings/:key",
            axum::routing::get(get_by_key).put(upsert),
        )
        .with_state(pool)
}

/// 登录页需要在登录前读取主题配色，只放行这一组键；其余系统设置必须登录后读取。
/// v2.3.19：此前 list 与 get_by_key 完全没有鉴权，任何人都能读取全部系统设置。
const PUBLIC_SETTING_KEYS: &[&str] = &["theme"];

/// 从 HeaderMap 中提取 JWT claims
/// GET /api/settings — 获取所有系统设置
async fn list(
    State(pool): State<DbPool>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<SystemSetting>>>> {
    authz_service::authenticate(&pool, &headers)?;
    let settings = settings_repo::get_all(&pool)?;
    Ok(Json(ApiResponse::ok(settings)))
}

/// GET /api/settings/:key — 获取单个系统设置
async fn get_by_key(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(key): Path<String>,
) -> Result<Json<ApiResponse<SystemSetting>>> {
    if !PUBLIC_SETTING_KEYS.contains(&key.as_str()) {
        authz_service::authenticate(&pool, &headers)?;
    }
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

    let mut conn = pool
        .get()
        .map_err(|error| setting_save_error("获取数据库连接", error))?;
    let tx = conn
        .transaction()
        .map_err(|error| setting_save_error("开启事务", error))?;
    let before = settings_repo::get_on_conn(&tx, &key)
        .map_err(|error| setting_save_error("读取当前配置", error))?
        .map(|setting| serde_json::json!({"key":setting.key,"value":setting.value}));
    settings_repo::upsert_on_conn(&tx, &key, &value_str)
        .map_err(|error| setting_save_error("写入配置", error))?;
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
    )
    .map_err(|error| setting_save_error("写入审计日志", error))?;
    tx.commit()
        .map_err(|error| setting_save_error("提交事务", error))?;

    let setting = settings_repo::get(&pool, &key)
        .map_err(|error| setting_save_error("读取保存结果", error))?
        .ok_or_else(|| AppError::Internal("保存系统配置失败（读取保存结果）。".into()))?;

    Ok(Json(ApiResponse::ok(setting)))
}
