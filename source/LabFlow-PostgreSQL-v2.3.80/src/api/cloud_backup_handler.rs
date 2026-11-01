use super::backup_handler::{current_config, require_backup_manager};
use crate::{
    config::AppConfig,
    db::DbPool,
    error::{AppError, Result},
    models::ApiResponse,
    repo::audit_repo,
    service::cloud_backup_service::{
        self as cloud, CloudStatus, RemoteBackup, SettingsInput, SettingsView,
    },
};
use axum::{
    extract::{Path, Request, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};
use tower::Service;
use tower_http::services::ServeFile;

type Context = (Arc<AppConfig>, DbPool);

struct DownloadGrant {
    name: String,
    authorization: axum::http::HeaderValue,
    expires_at: i64,
}

static DOWNLOADS: OnceLock<Mutex<HashMap<String, DownloadGrant>>> = OnceLock::new();

#[derive(Serialize)]
struct DownloadLink {
    url: String,
    filename: String,
}

pub fn router(config: Arc<AppConfig>, pool: DbPool) -> Router {
    Router::new()
        .route("/api/backup/cloud", get(status).put(configure))
        .route("/api/backup/cloud/test", post(test))
        .route("/api/backup/cloud/files", get(files))
        .route("/api/backup/cloud/upload/:fname", post(upload))
        .route("/api/backup/cloud/download/:fname", post(retrieve))
        .route("/api/backup/cloud/jobs/:id/retry", post(retry))
        .route("/api/backup/cloud/jobs/:id/cancel", post(cancel))
        .route("/api/backup/file/:fname/download-ticket", post(ticket))
        .route("/api/backup/latest-download-ticket", post(latest))
        .route("/api/backup/download/:ticket", get(download))
        .with_state((config, pool))
}

async fn status(
    State((base, pool)): State<Context>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<CloudStatus>>> {
    require_backup_manager(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(
        cloud::status(&current_config(&base)).map_err(AppError::Validation)?,
    )))
}

async fn configure(
    State((base, pool)): State<Context>,
    headers: HeaderMap,
    Json(input): Json<SettingsInput>,
) -> Result<Json<ApiResponse<SettingsView>>> {
    let actor = require_backup_manager(&pool, &headers)?;
    let result =
        cloud::update_settings(&current_config(&base), input).map_err(AppError::Validation)?;
    audit_repo::log_actor(
        &pool,
        "config",
        "backups",
        None,
        actor.user.id,
        &actor.user.username,
        "修改 WebDAV 备份配置（凭据不记录）",
        "shared",
    )?;
    Ok(Json(ApiResponse::ok(result)))
}

async fn test(
    State((base, pool)): State<Context>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<String>>> {
    let actor = require_backup_manager(&pool, &headers)?;
    let cfg = current_config(&base);
    tokio::task::spawn_blocking(move || cloud::test_connection(&cfg))
        .await
        .map_err(|_| AppError::Internal("云端连接测试异常".into()))?
        .map_err(AppError::Validation)?;
    audit_repo::log_actor(
        &pool,
        "test",
        "backups",
        None,
        actor.user.id,
        &actor.user.username,
        "WebDAV 小文件上传、回读及删除测试通过",
        "shared",
    )?;
    Ok(Json(ApiResponse::ok_msg(
        "小文件读写测试通过；实际备份仍受网盘单文件限制和配额约束",
    )))
}

async fn files(
    State((base, pool)): State<Context>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<RemoteBackup>>>> {
    require_backup_manager(&pool, &headers)?;
    let cfg = current_config(&base);
    let result = tokio::task::spawn_blocking(move || cloud::list_remote(&cfg))
        .await
        .map_err(|_| AppError::Internal("云端目录查询异常".into()))?
        .map_err(AppError::Validation)?;
    Ok(Json(ApiResponse::ok(result)))
}

fn enqueue(
    base: &AppConfig,
    pool: &DbPool,
    headers: &HeaderMap,
    name: &str,
    direction: &str,
) -> Result<Json<ApiResponse<String>>> {
    let actor = require_backup_manager(pool, headers)?;
    let cfg = current_config(base);
    let _guard = crate::service::backup_service::DATA_MAINTENANCE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| AppError::Internal("备份锁失败".into()))?;
    let id = cloud::enqueue(&cfg, name, direction).map_err(AppError::Validation)?;
    audit_repo::log_actor(
        pool,
        "transfer",
        "backups",
        None,
        actor.user.id,
        &actor.user.username,
        &format!("云端备份 {direction}: {name}，任务 {id}"),
        "shared",
    )?;
    Ok(Json(ApiResponse::ok_msg(
        "已加入后台传输任务，状态请查看传输列表",
    )))
}

async fn upload(
    State((base, pool)): State<Context>,
    headers: HeaderMap,
    Path(name): Path<String>,
) -> Result<Json<ApiResponse<String>>> {
    enqueue(&base, &pool, &headers, &name, "upload")
}

async fn retrieve(
    State((base, pool)): State<Context>,
    headers: HeaderMap,
    Path(name): Path<String>,
) -> Result<Json<ApiResponse<String>>> {
    enqueue(&base, &pool, &headers, &name, "download")
}

fn change(
    base: &AppConfig,
    pool: &DbPool,
    headers: &HeaderMap,
    id: &str,
    retry: bool,
) -> Result<Json<ApiResponse<String>>> {
    let actor = require_backup_manager(pool, headers)?;
    cloud::change_transfer(&current_config(base), id, retry).map_err(AppError::Validation)?;
    audit_repo::log_actor(
        pool,
        "transfer",
        "backups",
        None,
        actor.user.id,
        &actor.user.username,
        &format!("云端任务 {}: {id}", if retry { "重试" } else { "取消" }),
        "shared",
    )?;
    Ok(Json(ApiResponse::ok_msg("任务状态已更新")))
}

async fn retry(
    State((base, pool)): State<Context>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<String>>> {
    change(&base, &pool, &headers, &id, true)
}

async fn cancel(
    State((base, pool)): State<Context>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<String>>> {
    change(&base, &pool, &headers, &id, false)
}

fn issue(
    base: &AppConfig,
    pool: &DbPool,
    headers: &HeaderMap,
    name: String,
) -> Result<Json<ApiResponse<DownloadLink>>> {
    let actor = require_backup_manager(pool, headers)?;
    cloud::local_file(&current_config(base), &name).map_err(AppError::Validation)?;
    let authorization = headers
        .get(header::AUTHORIZATION)
        .cloned()
        .ok_or_else(|| AppError::Forbidden("请先登录".into()))?;
    let id = uuid::Uuid::new_v4().to_string();
    let mut values = DOWNLOADS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| AppError::Internal("下载状态锁失败".into()))?;
    let now = chrono::Utc::now().timestamp();
    values.retain(|_, value| value.expires_at > now);
    if values.len() >= 128 {
        return Err(AppError::Validation("下载任务过多，请稍后重试".into()));
    }
    values.insert(
        id.clone(),
        DownloadGrant {
            name: name.clone(),
            authorization,
            expires_at: now + 10 * 60,
        },
    );
    audit_repo::log_actor(
        pool,
        "download",
        "backups",
        None,
        actor.user.id,
        &actor.user.username,
        &format!("准备下载备份: {name}"),
        "shared",
    )?;
    Ok(Json(ApiResponse::ok(DownloadLink {
        url: format!("/api/backup/download/{id}"),
        filename: name,
    })))
}

async fn ticket(
    State((base, pool)): State<Context>,
    headers: HeaderMap,
    Path(name): Path<String>,
) -> Result<Json<ApiResponse<DownloadLink>>> {
    issue(&base, &pool, &headers, name)
}

async fn latest(
    State((base, pool)): State<Context>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<DownloadLink>>> {
    require_backup_manager(&pool, &headers)?;
    let cfg = current_config(&base);
    let mut files: Vec<_> = std::fs::read_dir(cfg.backup_dir())
        .map_err(|_| AppError::NotFound("尚无全量备份".into()))?
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            if !name.ends_with(".zip") || cloud::local_file(&cfg, &name).is_err() {
                return None;
            }
            Some((entry.metadata().ok()?.modified().ok()?, name))
        })
        .collect();
    files.sort_by_key(|(time, _)| std::cmp::Reverse(*time));
    let name = files
        .into_iter()
        .next()
        .map(|(_, name)| name)
        .ok_or_else(|| AppError::NotFound("尚无成功的全量备份，请先选择全量模式备份".into()))?;
    issue(&base, &pool, &headers, name)
}

async fn download(
    State((base, pool)): State<Context>,
    Path(id): Path<String>,
    request: Request,
) -> Response {
    let grant = DOWNLOADS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .ok()
        .and_then(|values| {
            values
                .get(&id)
                .filter(|value| value.expires_at > chrono::Utc::now().timestamp())
                .map(|value| (value.name.clone(), value.authorization.clone()))
        });
    let Some((name, authorization)) = grant else {
        return (StatusCode::FORBIDDEN, "下载链接已失效，请重新点击下载").into_response();
    };
    let mut headers = HeaderMap::new();
    headers.insert(header::AUTHORIZATION, authorization);
    if require_backup_manager(&pool, &headers).is_err() {
        return (StatusCode::FORBIDDEN, "登录或备份权限已失效").into_response();
    }
    let Ok(path) = cloud::local_file(&current_config(&base), &name) else {
        return (StatusCode::NOT_FOUND, "备份文件不存在").into_response();
    };
    match ServeFile::new(path).call(request).await {
        Ok(result) => {
            let mut result = result.into_response();
            result.headers_mut().insert(
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{name}\"").parse().unwrap(),
            );
            result
                .headers_mut()
                .insert(header::CACHE_CONTROL, "private, no-store".parse().unwrap());
            result.headers_mut().insert(
                header::CONTENT_TYPE,
                "application/octet-stream".parse().unwrap(),
            );
            result
                .headers_mut()
                .insert(header::X_CONTENT_TYPE_OPTIONS, "nosniff".parse().unwrap());
            result
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "备份文件读取失败").into_response(),
    }
}
