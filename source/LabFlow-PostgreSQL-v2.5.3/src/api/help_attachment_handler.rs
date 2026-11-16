use crate::{
    config::AppConfig,
    db::DbPool,
    error::{AppError, Result},
    models::{
        help_attachment::{HelpAttachment, HelpAttachmentUpdate},
        ApiResponse,
    },
    repo::help_attachment_repo,
    service::authz_service,
};
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{header, HeaderMap},
    response::Response,
    Json, Router,
};
use serde::Deserialize;

fn auth(pool: &DbPool, headers: &HeaderMap, edit: bool) -> Result<authz_service::AuthContext> {
    let ctx = authz_service::authenticate(pool, headers)?;
    authz_service::require_permission(&ctx, if edit { "help:edit" } else { "help:view" })?;
    if edit && !ctx.is_system_admin() {
        return Err(AppError::Forbidden("仅系统管理员可管理教程附件".into()));
    }
    Ok(ctx)
}
#[derive(Deserialize)]
struct ListQuery {
    visible_only: Option<bool>,
}
pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route(
            "/api/help-attachments",
            axum::routing::get(list).post(upload),
        )
        .route(
            "/api/help-attachments/:id",
            axum::routing::put(update).delete(remove),
        )
        .route("/api/help-attachments/:id/file", axum::routing::get(file))
        .with_state(pool)
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
}
async fn list(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<ListQuery>,
) -> Result<Json<ApiResponse<Vec<HelpAttachment>>>> {
    auth(&pool, &headers, false)?;
    Ok(Json(ApiResponse::ok(help_attachment_repo::list(
        &pool,
        q.visible_only.unwrap_or(true),
    )?)))
}
async fn upload(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    mut mp: Multipart,
) -> Result<Json<ApiResponse<HelpAttachment>>> {
    auth(&pool, &headers, true)?;
    let mut title = String::new();
    let mut filename = String::new();
    let mut data = Vec::new();
    while let Some(field) = mp
        .next_field()
        .await
        .map_err(|e| AppError::Validation(format!("读取附件失败: {e}")))?
    {
        match field.name() {
            Some("title") => {
                title = field
                    .text()
                    .await
                    .map_err(|e| AppError::Validation(e.to_string()))?
            }
            Some("file") => {
                filename = field.file_name().unwrap_or("attachment").to_string();
                data = field
                    .bytes()
                    .await
                    .map_err(|e| AppError::Validation(e.to_string()))?
                    .to_vec();
            }
            _ => {}
        }
    }
    if data.is_empty() {
        return Err(AppError::Validation("未收到附件".into()));
    }
    if data.len() > 100 * 1024 * 1024 {
        return Err(AppError::Validation("附件不能超过100MB".into()));
    }
    if title.trim().is_empty() {
        title = filename.clone();
    }
    let safe_name = format!(
        "{}_{}",
        uuid::Uuid::new_v4(),
        std::path::Path::new(&filename)
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or("attachment")
    );
    let dir = AppConfig::load().help_docs_dir().join("attachments");
    std::fs::create_dir_all(&dir).map_err(|e| AppError::Internal(e.to_string()))?;
    std::fs::write(dir.join(&safe_name), &data).map_err(|e| AppError::Internal(e.to_string()))?;
    let ext = std::path::Path::new(&filename)
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("bin")
        .to_ascii_lowercase();
    let item = help_attachment_repo::create(
        &pool,
        &title,
        &filename,
        &format!("help_docs/attachments/{safe_name}"),
        &ext,
        data.len() as i64,
    )?;
    Ok(Json(ApiResponse::ok(item)))
}
async fn update(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<HelpAttachmentUpdate>,
) -> Result<Json<ApiResponse<HelpAttachment>>> {
    auth(&pool, &headers, true)?;
    Ok(Json(ApiResponse::ok(help_attachment_repo::update(
        &pool, id, &body,
    )?)))
}
async fn remove(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<()>>> {
    auth(&pool, &headers, true)?;
    let item = help_attachment_repo::delete(&pool, id)?;
    let path = AppConfig::load().data_dir().join(item.file_path);
    let _ = std::fs::remove_file(path);
    Ok(Json(ApiResponse::ok(())))
}
async fn file(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Response> {
    auth(&pool, &headers, false)?;
    let item = help_attachment_repo::get(&pool, id)?;
    let data = tokio::fs::read(AppConfig::load().data_dir().join(&item.file_path))
        .await
        .map_err(|_| AppError::NotFound("附件文件不存在".into()))?;
    let mut response = Response::new(Body::from(data));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        if item.file_type == "pdf" {
            "application/pdf"
        } else {
            "application/octet-stream"
        }
        .parse()
        .unwrap(),
    );
    let encoded = url_escape::encode_component(&item.filename);
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=attachment; filename*=UTF-8''{encoded}")
            .parse()
            .unwrap(),
    );
    Ok(response)
}
