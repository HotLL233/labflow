use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::config::AppConfig;
use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::ApiResponse;
use crate::service::{authz_service, log_maintenance_service as maintenance};

#[derive(Serialize)]
struct PolicyView {
    policy: maintenance::LogMaintenancePolicy,
    runtime_log_enabled: bool,
    runtime_log_max_size_mb: u64,
}

#[derive(Deserialize)]
struct PolicyUpdate {
    policy: maintenance::LogMaintenancePolicy,
    runtime_log_enabled: bool,
    runtime_log_max_size_mb: u64,
}

pub fn router(config: Arc<AppConfig>, pool: DbPool) -> Router {
    Router::new()
        .route("/api/log-maintenance/status", get(status))
        .route(
            "/api/log-maintenance/policy",
            get(get_policy).put(update_policy),
        )
        .route(
            "/api/log-maintenance/run/session-cleanup",
            post(run_session_cleanup),
        )
        .route(
            "/api/log-maintenance/run/runtime-log-archive",
            post(run_runtime_log_archive),
        )
        .route(
            "/api/log-maintenance/run/audit-archive",
            post(run_audit_archive),
        )
        .route(
            "/api/log-maintenance/run/database-maintenance",
            post(run_database_maintenance),
        )
        .route("/api/log-maintenance/archives", get(list_archives))
        .route(
            "/api/log-maintenance/archives/:id/verify",
            post(verify_archive),
        )
        .with_state((config, pool))
}

fn require_system_admin(pool: &DbPool, headers: &HeaderMap) -> Result<authz_service::AuthContext> {
    let ctx = authz_service::authenticate(pool, headers)?;
    if !ctx.is_system_admin() {
        return Err(AppError::Forbidden(
            "Only system administrators can manage log retention and database maintenance".into(),
        ));
    }
    Ok(ctx)
}

fn audit_manual_action(pool: &DbPool, ctx: &authz_service::AuthContext, detail: &str) {
    let _ = crate::repo::audit_repo::log_actor(
        pool,
        "maintenance",
        "log_maintenance",
        None,
        ctx.user.id,
        &ctx.user.username,
        detail,
        "shared",
    );
}

fn current_config(base: &AppConfig) -> AppConfig {
    if AppConfig::config_path().exists() {
        AppConfig::load()
    } else {
        base.clone()
    }
}

async fn status(
    State((base, pool)): State<(Arc<AppConfig>, DbPool)>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<maintenance::MaintenanceStatus>>> {
    require_system_admin(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(
        maintenance::status(&pool, &current_config(&base)).map_err(AppError::Internal)?,
    )))
}

async fn get_policy(
    State((base, pool)): State<(Arc<AppConfig>, DbPool)>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<PolicyView>>> {
    require_system_admin(&pool, &headers)?;
    let config = current_config(&base);
    Ok(Json(ApiResponse::ok(PolicyView {
        policy: maintenance::get_policy(&pool).map_err(AppError::Internal)?,
        runtime_log_enabled: config.runtime_log_enabled,
        runtime_log_max_size_mb: config.runtime_log_max_size_mb,
    })))
}

async fn update_policy(
    State((base, pool)): State<(Arc<AppConfig>, DbPool)>,
    headers: HeaderMap,
    Json(body): Json<PolicyUpdate>,
) -> Result<Json<ApiResponse<PolicyView>>> {
    let ctx = require_system_admin(&pool, &headers)?;
    let policy = maintenance::save_policy(&pool, body.policy).map_err(AppError::Internal)?;
    let mut config = current_config(&base);
    config.runtime_log_enabled = body.runtime_log_enabled;
    config.runtime_log_max_size_mb = body.runtime_log_max_size_mb.clamp(1, 1024);
    config.save();
    audit_manual_action(
        &pool,
        &ctx,
        "updated log retention and database maintenance policy",
    );
    Ok(Json(ApiResponse::ok(PolicyView {
        policy,
        runtime_log_enabled: config.runtime_log_enabled,
        runtime_log_max_size_mb: config.runtime_log_max_size_mb,
    })))
}

async fn run_session_cleanup(
    State((_, pool)): State<(Arc<AppConfig>, DbPool)>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<maintenance::JobResult>>> {
    let ctx = require_system_admin(&pool, &headers)?;
    let policy = maintenance::get_policy(&pool).map_err(AppError::Internal)?;
    let task_pool = pool.clone();
    let result =
        tokio::task::spawn_blocking(move || maintenance::cleanup_sessions(&task_pool, &policy))
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?
            .map_err(AppError::Internal)?;
    audit_manual_action(
        &pool,
        &ctx,
        &format!("manual session cleanup: {}", result.detail),
    );
    Ok(Json(ApiResponse::ok(result)))
}

async fn run_runtime_log_archive(
    State((base, pool)): State<(Arc<AppConfig>, DbPool)>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<maintenance::JobResult>>> {
    let ctx = require_system_admin(&pool, &headers)?;
    let policy = maintenance::get_policy(&pool).map_err(AppError::Internal)?;
    let config = current_config(&base);
    let task_pool = pool.clone();
    let result = tokio::task::spawn_blocking(move || {
        maintenance::archive_runtime_logs(&task_pool, &config, &policy)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
    .map_err(AppError::Internal)?;
    audit_manual_action(
        &pool,
        &ctx,
        &format!("manual runtime log archive: {}", result.detail),
    );
    Ok(Json(ApiResponse::ok(result)))
}

async fn run_audit_archive(
    State((base, pool)): State<(Arc<AppConfig>, DbPool)>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<maintenance::JobResult>>> {
    let ctx = require_system_admin(&pool, &headers)?;
    let policy = maintenance::get_policy(&pool).map_err(AppError::Internal)?;
    let config = current_config(&base);
    let task_pool = pool.clone();
    let result = tokio::task::spawn_blocking(move || {
        maintenance::archive_audit(&task_pool, &config, &policy)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
    .map_err(AppError::Internal)?;
    audit_manual_action(
        &pool,
        &ctx,
        &format!("manual audit archive: {}", result.detail),
    );
    Ok(Json(ApiResponse::ok(result)))
}

async fn run_database_maintenance(
    State((base, pool)): State<(Arc<AppConfig>, DbPool)>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<maintenance::JobResult>>> {
    let ctx = require_system_admin(&pool, &headers)?;
    let config = current_config(&base);
    let task_pool = pool.clone();
    let result =
        tokio::task::spawn_blocking(move || maintenance::database_maintenance(&task_pool, &config))
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?
            .map_err(AppError::Internal)?;
    audit_manual_action(
        &pool,
        &ctx,
        &format!("manual database maintenance: {}", result.detail),
    );
    Ok(Json(ApiResponse::ok(result)))
}

async fn list_archives(
    State((_, pool)): State<(Arc<AppConfig>, DbPool)>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<maintenance::ArchiveBatch>>>> {
    require_system_admin(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(
        maintenance::list_archives(&pool).map_err(AppError::Internal)?,
    )))
}

async fn verify_archive(
    State((_, pool)): State<(Arc<AppConfig>, DbPool)>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<maintenance::ArchiveBatch>>> {
    let ctx = require_system_admin(&pool, &headers)?;
    let task_pool = pool.clone();
    let result = tokio::task::spawn_blocking(move || maintenance::verify_archive(&task_pool, id))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .map_err(AppError::Internal)?;
    audit_manual_action(&pool, &ctx, &format!("verified log archive #{}", id));
    Ok(Json(ApiResponse::ok(result)))
}
