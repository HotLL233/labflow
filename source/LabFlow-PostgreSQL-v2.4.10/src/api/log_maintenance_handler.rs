use axum::{
    extract::{Path, Query, Request, State},
    http::{header, HeaderMap},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower::Service;
use tower_http::services::ServeFile;

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

#[derive(Deserialize)]
struct ArchiveQuery {
    keyword: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
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
        .route("/api/log-maintenance/archives/page", get(archives_page))
        .route(
            "/api/log-maintenance/archives/:id/verify",
            post(verify_archive),
        )
        .route(
            "/api/log-maintenance/archives/:id/download",
            get(download_archive),
        )
        .route(
            "/api/log-maintenance/archives/:id/entries",
            get(archive_entries),
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
    if let Err(error) = crate::repo::audit_repo::log_actor(
        pool,
        "maintenance",
        "log_maintenance",
        None,
        ctx.user.id,
        &ctx.user.username,
        detail,
        "shared",
    ) {
        tracing::error!(%error, operator_user_id=ctx.user.id, "维护操作已完成，但审计写入失败");
    }
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
    let previous = current_config(&base);
    let mut config = previous.clone();
    config.runtime_log_enabled = body.runtime_log_enabled;
    config.runtime_log_max_size_mb = body.runtime_log_max_size_mb.clamp(1, 1024);
    config.try_save().map_err(AppError::Internal)?;
    let policy = match maintenance::save_policy(&pool, body.policy) {
        Ok(policy) => policy,
        Err(error) => {
            if let Err(rollback_error) = previous.try_save() {
                tracing::error!(%rollback_error, "日志策略保存失败，服务器配置回滚失败");
            }
            return Err(AppError::Internal(error));
        }
    };
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

async fn archives_page(
    State((_, pool)): State<(Arc<AppConfig>, DbPool)>,
    headers: HeaderMap,
    Query(query): Query<ArchiveQuery>,
) -> Result<Json<ApiResponse<crate::models::PaginatedResponse<maintenance::ArchiveBatch>>>> {
    require_system_admin(&pool, &headers)?;
    let result = tokio::task::spawn_blocking(move || {
        maintenance::archives_page(
            &pool,
            query.keyword.as_deref().unwrap_or(""),
            query.page.unwrap_or(1),
            query.page_size.unwrap_or(50),
        )
    })
    .await
    .map_err(|error| AppError::Internal(error.to_string()))?
    .map_err(AppError::Validation)?;
    Ok(Json(ApiResponse::ok(result)))
}

async fn verify_archive(
    State((base, pool)): State<(Arc<AppConfig>, DbPool)>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<maintenance::ArchiveBatch>>> {
    let ctx = require_system_admin(&pool, &headers)?;
    let task_pool = pool.clone();
    let config = current_config(&base);
    let result =
        tokio::task::spawn_blocking(move || maintenance::verify_archive(&task_pool, &config, id))
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?
            .map_err(AppError::Internal)?;
    audit_manual_action(&pool, &ctx, &format!("verified log archive #{}", id));
    Ok(Json(ApiResponse::ok(result)))
}

async fn download_archive(
    State((base, pool)): State<(Arc<AppConfig>, DbPool)>,
    Path(id): Path<i64>,
    request: Request,
) -> Result<Response> {
    let ctx = require_system_admin(&pool, request.headers())?;
    let config = current_config(&base);
    let task_pool = pool.clone();
    let (_, path) =
        tokio::task::spawn_blocking(move || maintenance::verified_archive(&task_pool, &config, id))
            .await
            .map_err(|error| AppError::Internal(error.to_string()))?
            .map_err(AppError::Validation)?;
    let response = ServeFile::new(path)
        .call(request)
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;
    let mut response = response.into_response();
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"labflow-log-archive-{id}.zip\"")
            .parse()
            .map_err(|_| AppError::Internal("无法生成归档文件名".into()))?,
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "private, no-store".parse().unwrap());
    audit_manual_action(&pool, &ctx, &format!("downloaded log archive #{id}"));
    Ok(response)
}

async fn archive_entries(
    State((base, pool)): State<(Arc<AppConfig>, DbPool)>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(query): Query<ArchiveQuery>,
) -> Result<
    Json<ApiResponse<crate::models::PaginatedResponse<crate::models::audit::AuditLogResponse>>>,
> {
    require_system_admin(&pool, &headers)?;
    let config = current_config(&base);
    let result = tokio::task::spawn_blocking(move || {
        maintenance::archive_entries(
            &pool,
            &config,
            id,
            query.keyword.as_deref().unwrap_or(""),
            query
                .start_date
                .as_deref()
                .filter(|value| !value.is_empty()),
            query.end_date.as_deref().filter(|value| !value.is_empty()),
            query.page.unwrap_or(1),
            query.page_size.unwrap_or(50),
        )
    })
    .await
    .map_err(|error| AppError::Internal(error.to_string()))?
    .map_err(AppError::Validation)?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn archive_download_and_search_reject_non_administrators() {
        let pool = crate::db::init_pool("log-archive-permissions");
        let conn = pool.get().unwrap();
        crate::db::test_migrations::run(&conn).unwrap();
        conn.execute("INSERT INTO users(username,password,is_admin,is_active) VALUES('archive-reader','unused',0,1)", []).unwrap();
        let id = conn.last_insert_rowid();
        drop(conn);
        let user = crate::repo::user_repo::find_by_id(&pool, id)
            .unwrap()
            .unwrap();
        let login = crate::service::auth_service::create_login_response(
            &pool,
            user,
            false,
            Some("archive-permissions"),
            Some("test"),
        )
        .unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            format!("Bearer {}", login.token).parse().unwrap(),
        );
        let config = Arc::new(AppConfig::default());
        let batches = archives_page(
            State((config.clone(), pool.clone())),
            headers.clone(),
            Query(ArchiveQuery {
                keyword: None,
                start_date: None,
                end_date: None,
                page: None,
                page_size: None,
            }),
        )
        .await;
        assert!(matches!(batches, Err(AppError::Forbidden(_))));
        let search = archive_entries(
            State((config.clone(), pool.clone())),
            headers,
            Path(1),
            Query(ArchiveQuery {
                keyword: None,
                start_date: None,
                end_date: None,
                page: None,
                page_size: None,
            }),
        )
        .await;
        assert!(matches!(search, Err(AppError::Forbidden(_))));
        let request = Request::builder()
            .header(header::AUTHORIZATION, format!("Bearer {}", login.token))
            .body(axum::body::Body::empty())
            .unwrap();
        let download = download_archive(State((config, pool)), Path(1), request).await;
        assert!(matches!(download, Err(AppError::Forbidden(_))));
    }
}
