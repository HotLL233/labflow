use crate::config::AppConfig;
use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::{
    sample_info_draft::{SampleInfoDraft, SampleInfoDraftAttachment, SampleInfoDraftInput},
    ApiResponse,
};
use crate::repo::sample_info_draft_repo;
use crate::service::{authz_service, sample_attachment_service};
use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::{header, HeaderMap},
    response::IntoResponse,
    routing::{delete, get, put},
    Json, Router,
};
use std::sync::Arc;
use uuid::Uuid;

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/sample-info/drafts", get(list).post(create))
        .route("/api/sample-info/drafts/:id", put(update).delete(remove))
        .route(
            "/api/sample-info/drafts/:id/attachments",
            get(list_attachments).post(upload_attachment),
        )
        .route(
            "/api/sample-info/drafts/attachments/:attachment_id/file",
            get(download_attachment),
        )
        .route(
            "/api/sample-info/drafts/attachments/:attachment_id",
            delete(delete_attachment),
        )
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .with_state((pool, Arc::new(AppConfig::load())))
}

async fn context(pool: &DbPool, headers: &HeaderMap) -> Result<authz_service::AuthContext> {
    let ctx = authz_service::authenticate(pool, headers)?;
    authz_service::require_permission(&ctx, "entry:sample-info")?;
    Ok(ctx)
}

async fn list(
    State((pool, _config)): State<(DbPool, Arc<AppConfig>)>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<SampleInfoDraft>>>> {
    let ctx = context(&pool, &headers).await?;
    Ok(Json(ApiResponse::ok(sample_info_draft_repo::list(
        &pool,
        ctx.user.id,
    )?)))
}

async fn create(
    State((pool, _config)): State<(DbPool, Arc<AppConfig>)>,
    headers: HeaderMap,
    Json(body): Json<SampleInfoDraftInput>,
) -> Result<Json<ApiResponse<SampleInfoDraft>>> {
    let ctx = context(&pool, &headers).await?;
    Ok(Json(ApiResponse::ok(sample_info_draft_repo::create(
        &pool,
        ctx.user.id,
        &ctx.user.username,
        &body,
    )?)))
}

async fn update(
    State((pool, _config)): State<(DbPool, Arc<AppConfig>)>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<SampleInfoDraftInput>,
) -> Result<Json<ApiResponse<SampleInfoDraft>>> {
    let ctx = context(&pool, &headers).await?;
    Ok(Json(ApiResponse::ok(sample_info_draft_repo::update(
        &pool,
        id,
        ctx.user.id,
        &body,
    )?)))
}

async fn remove(
    State((pool, config)): State<(DbPool, Arc<AppConfig>)>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<()>>> {
    let ctx = context(&pool, &headers).await?;
    let attachments = sample_info_draft_repo::list_attachments(&pool, id, ctx.user.id)?;
    sample_info_draft_repo::remove(&pool, id, ctx.user.id)?;
    for attachment in attachments {
        let _ = std::fs::remove_file(
            config
                .attachments_dir()
                .join("drafts")
                .join(attachment.stored_name),
        );
    }
    Ok(Json(ApiResponse::ok_msg("草稿已删除")))
}

async fn list_attachments(
    State((pool, _config)): State<(DbPool, Arc<AppConfig>)>,
    headers: HeaderMap,
    Path(draft_id): Path<i64>,
) -> Result<Json<ApiResponse<Vec<SampleInfoDraftAttachment>>>> {
    let ctx = context(&pool, &headers).await?;
    Ok(Json(ApiResponse::ok(
        sample_info_draft_repo::list_attachments(&pool, draft_id, ctx.user.id)?,
    )))
}

async fn upload_attachment(
    State((pool, config)): State<(DbPool, Arc<AppConfig>)>,
    headers: HeaderMap,
    Path(draft_id): Path<i64>,
    mut multipart: Multipart,
) -> Result<Json<ApiResponse<SampleInfoDraftAttachment>>> {
    let ctx = context(&pool, &headers).await?;
    sample_info_draft_repo::get(&pool, draft_id, ctx.user.id)?;
    let mut row_index: Option<i64> = None;
    let mut file_name = String::new();
    let mut file_type = String::new();
    let mut file_data = Vec::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Validation(format!("读取草稿附件失败: {e}")))?
    {
        match field.name().unwrap_or("") {
            "row_index" => {
                let value = field
                    .text()
                    .await
                    .map_err(|e| AppError::Validation(format!("读取行号失败: {e}")))?;
                row_index = Some(
                    value
                        .trim()
                        .parse::<i64>()
                        .map_err(|_| AppError::Validation("草稿附件行号无效".into()))?,
                );
            }
            "file" => {
                file_name = field.file_name().unwrap_or("unknown").to_string();
                file_type = field
                    .content_type()
                    .unwrap_or("application/octet-stream")
                    .to_string();
                file_data = field
                    .bytes()
                    .await
                    .map_err(|e| AppError::Validation(format!("读取草稿附件失败: {e}")))?
                    .to_vec();
            }
            _ => {}
        }
    }
    let row_index = row_index
        .filter(|value| *value >= 0)
        .ok_or_else(|| AppError::Validation("缺少有效的草稿附件行号".into()))?;
    sample_attachment_service::validate_upload(&file_name, &file_data)?;
    let ext = std::path::Path::new(&file_name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin");
    let stored_name = format!("draft_{}_{}.{}", draft_id, Uuid::new_v4().simple(), ext);
    let draft_dir = config.attachments_dir().join("drafts");
    std::fs::create_dir_all(&draft_dir)
        .map_err(|e| AppError::Internal(format!("创建草稿附件目录失败: {e}")))?;
    let file_path = draft_dir.join(&stored_name);
    std::fs::write(&file_path, &file_data)
        .map_err(|e| AppError::Internal(format!("保存草稿附件失败: {e}")))?;
    match sample_info_draft_repo::create_attachment(
        &pool,
        draft_id,
        ctx.user.id,
        row_index,
        &file_name,
        &stored_name,
        file_data.len() as i64,
        &file_type,
    ) {
        Ok(attachment) => Ok(Json(ApiResponse::ok(attachment))),
        Err(error) => {
            let _ = std::fs::remove_file(file_path);
            Err(error)
        }
    }
}

async fn download_attachment(
    State((pool, config)): State<(DbPool, Arc<AppConfig>)>,
    headers: HeaderMap,
    Path(attachment_id): Path<i64>,
) -> Result<impl IntoResponse> {
    let ctx = context(&pool, &headers).await?;
    let attachment = sample_info_draft_repo::find_attachment(&pool, attachment_id, ctx.user.id)?;
    let path = config
        .attachments_dir()
        .join("drafts")
        .join(&attachment.stored_name);
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|e| AppError::NotFound(format!("草稿附件文件不存在: {e}")))?;
    let content_type = if attachment.file_type.is_empty() {
        "application/octet-stream".to_string()
    } else {
        attachment.file_type.clone()
    };
    let disposition = format!(
        "inline; filename=\"{}\"",
        attachment.file_name.replace('"', "")
    );
    Ok((
        [
            (header::CONTENT_TYPE, content_type),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        bytes,
    ))
}

async fn delete_attachment(
    State((pool, config)): State<(DbPool, Arc<AppConfig>)>,
    headers: HeaderMap,
    Path(attachment_id): Path<i64>,
) -> Result<Json<ApiResponse<()>>> {
    let ctx = context(&pool, &headers).await?;
    let attachment = sample_info_draft_repo::delete_attachment(&pool, attachment_id, ctx.user.id)?;
    let _ = std::fs::remove_file(
        config
            .attachments_dir()
            .join("drafts")
            .join(attachment.stored_name),
    );
    Ok(Json(ApiResponse::ok_msg("草稿附件已删除")))
}
