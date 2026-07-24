use crate::config::AppConfig;
use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::trash::{PurgeRequest, TrashEntry, TrashPrecheck, TrashQuery};
use crate::models::{ApiResponse, PaginatedResponse};
use crate::repo::{trash_repo, user_repo};
use crate::service::{auth_service, authz_service, backup_service};
use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use std::sync::Arc;

type TrashState = (DbPool, Arc<AppConfig>);

pub fn router(pool: DbPool, config: Arc<AppConfig>) -> Router {
    Router::new()
        .route("/api/trash", get(list))
        .route("/api/trash/precheck/:table/:id", get(precheck))
        .route("/api/trash/:id/restore", post(restore))
        .route("/api/trash/:id/purge", post(purge))
        .with_state((pool, config))
}

fn scope_for(ctx: &authz_service::AuthContext) -> trash_repo::TrashScope {
    if ctx.is_system_admin() || ctx.is_analysis_leader() {
        trash_repo::TrashScope::Global
    } else if ctx.is_rd_leader() {
        ctx.user
            .group_id
            .map(trash_repo::TrashScope::Group)
            .unwrap_or(trash_repo::TrashScope::User(ctx.user.id))
    } else {
        trash_repo::TrashScope::User(ctx.user.id)
    }
}

fn visible_to(ctx: &authz_service::AuthContext, entry: &TrashEntry) -> bool {
    match scope_for(ctx) {
        trash_repo::TrashScope::Global => true,
        trash_repo::TrashScope::Group(group_id) => {
            entry.owner_group_id == Some(group_id) || entry.deleted_by_user_id == Some(ctx.user.id)
        }
        trash_repo::TrashScope::User(user_id) => {
            entry.owner_user_id == Some(user_id) || entry.deleted_by_user_id == Some(user_id)
        }
    }
}

async fn list(
    State((pool, _)): State<TrashState>,
    headers: HeaderMap,
    Query(query): Query<TrashQuery>,
) -> Result<Json<ApiResponse<PaginatedResponse<TrashEntry>>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "manage:trash")?;
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(50).clamp(1, 200);
    let (items, total) = trash_repo::list(
        &pool,
        scope_for(&ctx),
        query.category.as_deref(),
        query.module.as_deref(),
        query.keyword.as_deref(),
        page,
        page_size,
    )?;
    Ok(Json(ApiResponse::ok(PaginatedResponse {
        items,
        total,
        page,
        page_size,
    })))
}

async fn precheck(
    State((pool, _)): State<TrashState>,
    headers: HeaderMap,
    Path((table, id)): Path<(String, i64)>,
) -> Result<Json<ApiResponse<TrashPrecheck>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "manage:trash")?;
    Ok(Json(ApiResponse::ok(trash_repo::precheck(
        &pool, &table, id,
    )?)))
}

async fn restore(
    State((pool, config)): State<TrashState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<TrashEntry>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "manage:trash")?;
    let entry = trash_repo::get_active(&pool, id)?;
    if !visible_to(&ctx, &entry) {
        return Err(AppError::Forbidden("无权恢复该回收站数据".into()));
    }
    let current_config = if AppConfig::config_path().exists() {
        AppConfig::load()
    } else {
        (*config).clone()
    };
    let backup_paths = if entry.table_name == "backup_files" {
        let original = entry
            .snapshot
            .get("original_name")
            .and_then(|value| value.as_str())
            .ok_or_else(|| AppError::Validation("备份回收站条目缺少原文件名".into()))?;
        let stored = entry
            .snapshot
            .get("stored_name")
            .and_then(|value| value.as_str())
            .ok_or_else(|| AppError::Validation("备份回收站条目缺少存储文件名".into()))?;
        let source = current_config.backup_dir().join(".trash").join(stored);
        let destination = current_config.backup_dir().join(original);
        if destination.exists() {
            return Err(AppError::Conflict(
                "备份目录中已有同名文件，请先处理同名文件".into(),
            ));
        }
        std::fs::rename(&source, &destination)
            .map_err(|error| AppError::Internal(format!("恢复备份文件失败：{error}")))?;
        Some((source, destination))
    } else {
        None
    };
    let restored = trash_repo::restore(&pool, id, ctx.user.id, &ctx.user.username);
    if restored.is_err() {
        if let Some((source, destination)) = backup_paths {
            std::fs::rename(destination, source).ok();
        }
    }
    Ok(Json(ApiResponse::ok(restored?)))
}

fn verify_admin(pool: &DbPool, body: &PurgeRequest) -> Result<()> {
    let user = user_repo::find_by_username(pool, body.admin_username.trim())?
        .ok_or_else(|| AppError::Forbidden("管理员账号或密码错误".into()))?;
    let is_admin = user.is_admin
        || user
            .role_names
            .iter()
            .any(|name| name == authz_service::ROLE_SYSTEM_ADMIN);
    if !is_admin
        || !user.is_active
        || !auth_service::verify_password(&body.admin_password, &user.password)
    {
        return Err(AppError::Forbidden("管理员账号或密码错误".into()));
    }
    Ok(())
}

async fn purge(
    State((pool, config)): State<TrashState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<PurgeRequest>,
) -> Result<Json<ApiResponse<TrashEntry>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    if !ctx.is_system_admin() {
        return Err(AppError::Forbidden(
            "只有系统管理员可以永久清理回收站".into(),
        ));
    }
    verify_admin(&pool, &body)?;
    let entry = trash_repo::get_active(&pool, id)?;
    let current_config = if AppConfig::config_path().exists() {
        AppConfig::load()
    } else {
        (*config).clone()
    };
    backup_service::create_backup(&current_config, false).map_err(|error| {
        AppError::Internal(format!("永久清理前全量备份失败，操作已取消：{error}"))
    })?;

    let attachment_files: Vec<String> = if entry.table_name == "sample_info_records" {
        let conn = pool.get()?;
        let mut stmt =
            conn.prepare("SELECT stored_name FROM sample_info_attachments WHERE record_id=?1")?;
        let files = stmt
            .query_map([entry.record_id], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        files
    } else {
        Vec::new()
    };
    let backup_trash_file = if entry.table_name == "backup_files" {
        entry
            .snapshot
            .get("stored_name")
            .and_then(|value| value.as_str())
            .map(|name| current_config.backup_dir().join(".trash").join(name))
    } else {
        None
    };
    let purged = trash_repo::purge(&pool, id, ctx.user.id, &ctx.user.username)?;

    if purged.table_name == "sample_info_attachments" {
        if let Some(name) = purged
            .snapshot
            .get("stored_name")
            .and_then(|value| value.as_str())
        {
            std::fs::remove_file(current_config.attachments_dir().join(name)).ok();
        }
    }
    for name in attachment_files {
        std::fs::remove_file(current_config.attachments_dir().join(name)).ok();
    }
    if let Some(path) = backup_trash_file {
        std::fs::remove_file(path).ok();
    }
    if purged.table_name == "help_documents" {
        if let Some(path) = purged
            .snapshot
            .get("file_path")
            .and_then(|value| value.as_str())
        {
            let source_path = if path.starts_with("help_docs/") {
                current_config.data_dir().join(path)
            } else {
                std::env::current_exe()
                    .ok()
                    .and_then(|exe_path| exe_path.parent().map(|value| value.join(path)))
                    .unwrap_or_default()
            };
            std::fs::remove_file(source_path).ok();
        }
        std::fs::remove_dir_all(
            current_config
                .help_docs_dir()
                .join("preview")
                .join(format!("pages_{}", purged.record_id)),
        )
        .ok();
        std::fs::remove_dir_all(
            current_config
                .help_docs_dir()
                .join("preview")
                .join(format!("docx_images_{}", purged.record_id)),
        )
        .ok();
    }
    Ok(Json(ApiResponse::ok(purged)))
}
