use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::help::{HelpDocUpdateRequest, HelpDocument};
use crate::models::trash::DeleteReasonRequest;
use crate::models::ApiResponse;
use crate::repo::help_repo;
use crate::service::authz_service;
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::Response,
    Json, Router,
};
use serde::Deserialize;

fn require_help(
    pool: &DbPool,
    headers: &HeaderMap,
    edit: bool,
) -> Result<crate::service::authz_service::AuthContext> {
    let ctx = authz_service::authenticate(pool, headers)?;
    authz_service::require_permission(&ctx, if edit { "help:edit" } else { "help:view" })?;
    if edit && !ctx.is_system_admin() {
        return Err(AppError::Forbidden("仅系统管理员可管理教程内容".into()));
    }
    Ok(ctx)
}

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/help-documents", axum::routing::get(list).post(upload))
        .route(
            "/api/help-documents/:id",
            axum::routing::put(update).delete(delete),
        )
        .route("/api/help-documents/sort", axum::routing::put(reorder_docs))
        .route("/api/help-documents/:id/file", axum::routing::get(get_file))
        .route(
            "/api/help-documents/:id/images/:filename",
            axum::routing::get(get_docx_image),
        )
        .route(
            "/api/help-documents/:id/pages/:page",
            axum::routing::get(get_page),
        )
        .with_state(pool)
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
}

#[derive(Deserialize)]
struct HelpDocQuery {
    visible_only: Option<bool>,
}

/// GET /api/help-documents?visible_only=true
async fn list(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<HelpDocQuery>,
) -> Result<Json<ApiResponse<Vec<HelpDocument>>>> {
    require_help(&pool, &headers, false)?;
    let visible_only = q.visible_only.unwrap_or(false);
    let items = help_repo::list(&pool, visible_only)?;
    Ok(Json(ApiResponse::ok(items)))
}

/// POST /api/help-documents — multipart upload (file + title)
async fn upload(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    mut mp: Multipart,
) -> Result<Json<ApiResponse<HelpDocument>>> {
    let ctx = require_help(&pool, &headers, true)?;
    let mut title = String::new();
    let mut file_data: Vec<u8> = Vec::new();
    let mut original_filename = String::new();

    while let Ok(Some(field)) = mp.next_field().await {
        match field.name() {
            Some("title") => {
                title = field
                    .text()
                    .await
                    .map_err(|e| AppError::Validation(format!("读取 title 失败: {}", e)))?;
            }
            Some("file") => {
                original_filename = field.file_name().unwrap_or("unknown").to_string();
                file_data = field
                    .bytes()
                    .await
                    .map_err(|e| AppError::Validation(format!("读取文件失败: {}", e)))?
                    .to_vec();
            }
            _ => {}
        }
    }

    if file_data.is_empty() {
        return Err(AppError::Validation("未收到文件".into()));
    }
    if title.trim().is_empty() {
        title = original_filename.clone();
    }

    // 确定文件扩展名
    let ext = std::path::Path::new(&original_filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin")
        .to_ascii_lowercase();

    // 生成唯一存储名
    let stored_name = format!("{}_{}", uuid::Uuid::new_v4(), original_filename);

    // 确定存储目录（相对于 exe 所在目录）
    let help_dir = crate::config::AppConfig::load()
        .help_docs_dir()
        .join("source");
    std::fs::create_dir_all(&help_dir)
        .map_err(|e| AppError::Internal(format!("创建目录失败: {}", e)))?;

    let file_path = help_dir.join(&stored_name);
    std::fs::write(&file_path, &file_data)
        .map_err(|e| AppError::Internal(format!("写入文件失败: {}", e)))?;

    let file_size = file_data.len() as i64;

    // 存储相对路径
    let relative_path = format!("help_docs/source/{}", stored_name);

    // 先写 DB 获取 id
    let doc = help_repo::create(
        &pool,
        &title,
        &original_filename,
        &relative_path,
        &ext,
        file_size,
        &ctx.user.username,
    )?;

    // PDF and Office documents use the same source-preserving page preview as sample attachments.
    let page_count: Option<i64> = if ext == "pdf" {
        let pages_dir = crate::config::AppConfig::load()
            .help_docs_dir()
            .join("preview")
            .join(format!("pages_{}", doc.id));
        #[cfg(target_os = "windows")]
        {
            match crate::api::pdf_render::pdf_to_pngs(&file_path, &pages_dir) {
                Ok(n) => {
                    tracing::info!("PDF 渲染完成，{} 页", n);
                    Some(n as i64)
                }
                Err(e) => {
                    tracing::warn!("PDF 转 PNG 失败: {}", e);
                    None
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            tracing::info!("Linux: PDF 页面渲染跳过（仅支持 Windows），文本提取不受影响");
            None
        }
    } else if ["doc", "docx", "xls", "xlsx"].contains(&ext.as_str()) {
        let pages_dir = crate::config::AppConfig::load()
            .help_docs_dir()
            .join("preview")
            .join(format!("pages_{}", doc.id));
        let office_source = pages_dir.join(format!("source.{ext}"));
        std::fs::create_dir_all(&pages_dir)
            .map_err(|e| AppError::Internal(format!("创建教程预览目录失败: {e}")))?;
        std::fs::copy(&file_path, &office_source)
            .map_err(|e| AppError::Internal(format!("准备教程预览文件失败: {e}")))?;
        match crate::api::sample_info_attachment_handler::convert_office_file_to_pdf(
            &office_source,
            &pages_dir,
        )
        .and_then(|pdf| {
            crate::api::pdf_render::pdf_to_pngs(&pdf, &pages_dir)
                .map(|n| n as i64)
                .map_err(AppError::Validation)
        }) {
            Ok(n) => Some(n),
            Err(e) => {
                tracing::warn!("教程 Office 原格式预览失败: {e}");
                None
            }
        }
    } else {
        None
    };

    if let Some(pc) = page_count {
        help_repo::set_page_count(&pool, doc.id, pc)?;
    }

    // Word/PDF/Excel → 结构化文章
    if ext == "docx" || ext == "pdf" || ext == "xlsx" || ext == "xls" {
        let article_title = if title.trim().is_empty() {
            &original_filename
        } else {
            &title
        };
        let source = Some(format!("{} (导入自 {})", article_title, ext.to_uppercase()));
        if ext == "docx" {
            let image_dir = crate::config::AppConfig::load()
                .help_docs_dir()
                .join("preview")
                .join(format!("docx_images_{}", doc.id));
            let image_url_prefix = format!("/api/help-documents/{}/images", doc.id);
            if let Some(result) = crate::api::docx_parser::parse_docx(
                &file_data,
                crate::api::docx_parser::DocxImageOptions {
                    output_dir: &image_dir,
                    image_url_prefix: &image_url_prefix,
                },
            )
            .map_err(|e| tracing::warn!("Word解析: {}", e))
            .ok()
            {
                let toc = serde_json::to_string(&result.toc).unwrap_or_default();
                if let Err(e) = crate::repo::article_repo::create(
                    &pool,
                    article_title,
                    &result.html,
                    Some(&toc),
                    source.as_deref(),
                    &ctx.user.username,
                ) {
                    tracing::warn!("保存文章失败: {}", e);
                }
            }
        } else if ext == "xlsx" || ext == "xls" {
            if let Some(result) = crate::api::xlsx_parser::parse_xlsx(&file_data)
                .map_err(|e| tracing::warn!("Excel解析: {}", e))
                .ok()
            {
                if let Err(e) = crate::repo::article_repo::create(
                    &pool,
                    article_title,
                    &result.html,
                    None,
                    source.as_deref(),
                    &ctx.user.username,
                ) {
                    tracing::warn!("保存文章失败: {}", e);
                }
            }
        } else {
            if let Some(result) = crate::api::pdf_parser::parse_pdf(&file_data)
                .map_err(|e| tracing::warn!("PDF文字提取: {}", e))
                .ok()
            {
                if let Err(e) = crate::repo::article_repo::create(
                    &pool,
                    article_title,
                    &result.html,
                    None,
                    source.as_deref(),
                    &ctx.user.username,
                ) {
                    tracing::warn!("保存文章失败: {}", e);
                }
            }
        }
    }

    Ok(Json(ApiResponse::ok(help_repo::get_by_id(&pool, doc.id)?)))
}

/// PUT /api/help-documents/:id — 编辑标题/显隐/排序
async fn update(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<HelpDocUpdateRequest>,
) -> Result<Json<ApiResponse<HelpDocument>>> {
    let ctx = require_help(&pool, &headers, true)?;
    let item = help_repo::update(&pool, id, &body, &ctx.user.username)?;
    Ok(Json(ApiResponse::ok(item)))
}

/// PUT /api/help-documents/sort — 批量排序
#[derive(serde::Deserialize)]
struct DocReorderItem {
    id: i64,
    sort_order: i64,
}
#[derive(serde::Deserialize)]
struct DocReorderBody {
    ids: Vec<DocReorderItem>,
}
async fn reorder_docs(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(body): Json<DocReorderBody>,
) -> Result<Json<ApiResponse<()>>> {
    let ctx = require_help(&pool, &headers, true)?;
    let items: Vec<(i64, i64)> = body.ids.iter().map(|x| (x.id, x.sort_order)).collect();
    help_repo::reorder_documents(&pool, &items, &ctx.user.username)?;
    Ok(Json(ApiResponse::ok(())))
}

/// DELETE /api/help-documents/:id — 删除 DB 记录 + 磁盘文件
async fn delete(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(body): Query<DeleteReasonRequest>,
) -> Result<Json<ApiResponse<()>>> {
    let ctx = require_help(&pool, &headers, true)?;
    let reason = body
        .reason
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("用户删除");
    help_repo::delete(&pool, id, &ctx.user.username, reason)?;

    Ok(Json(ApiResponse::ok_msg("已移入回收站")))
}

/// GET /api/help-documents/:id/file — 下载/查看文件
#[derive(Deserialize)]
struct FileQuery {
    raw: Option<u8>,
}

async fn get_file(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(q): Query<FileQuery>,
) -> Result<Response> {
    // Source documents are readable by all help viewers; management remains edit-only.
    require_help(&pool, &headers, false)?;
    let doc = help_repo::get_by_id(&pool, id)?;

    let file_abs = if doc.file_path.starts_with("help_docs/") {
        crate::config::AppConfig::load()
            .data_dir()
            .join(&doc.file_path)
    } else {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join(&doc.file_path)))
            .unwrap_or_else(|| std::path::PathBuf::from(&doc.file_path))
    };

    let data = tokio::fs::read(&file_abs)
        .await
        .map_err(|_| AppError::NotFound("文件不存在或已被删除".into()))?;

    // ?raw=1 → JSON Base64（绕下载工具）
    if q.raw == Some(1) {
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        let b64 = STANDARD.encode(&data);
        let body = serde_json::json!({ "data": b64, "filename": doc.filename });
        return Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap());
    }

    let content_type = mime_type(&doc.file_type);

    let mut response = Response::new(Body::from(data));
    *response.status_mut() = StatusCode::OK;
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, content_type.parse().unwrap());
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        format!("inline; filename=\"{}\"", doc.filename)
            .parse()
            .unwrap(),
    );

    Ok(response)
}

/// GET /api/help-documents/:id/pages/:page — PDF 逐页 PNG
async fn get_page(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path((id, page)): Path<(i64, u32)>,
) -> Result<Response> {
    require_help(&pool, &headers, false)?;
    // validate doc exists
    help_repo::get_by_id(&pool, id)?;

    let png_path = crate::config::AppConfig::load()
        .help_docs_dir()
        .join("preview")
        .join(format!("pages_{}", id))
        .join(format!("page_{}.png", page));

    let data = match tokio::fs::read(&png_path).await {
        Ok(data) => data,
        Err(_) => {
            let legacy = std::env::current_exe()
                .ok()
                .and_then(|path| {
                    path.parent().map(|dir| {
                        dir.join("data")
                            .join("help_docs")
                            .join(format!("pages_{}", id))
                            .join(format!("page_{}.png", page))
                    })
                })
                .ok_or_else(|| AppError::NotFound("页面不存在".into()))?;
            tokio::fs::read(legacy)
                .await
                .map_err(|_| AppError::NotFound("页面不存在".into()))?
        }
    };

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/png")
        .body(Body::from(data))
        .unwrap())
}

async fn get_docx_image(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path((id, filename)): Path<(i64, String)>,
) -> Result<Response> {
    require_help(&pool, &headers, false)?;
    let document = help_repo::get_by_id(&pool, id)?;
    if document.file_type.to_lowercase() != "docx" || filename.contains(['/', '\\']) {
        return Err(AppError::NotFound("教程图片不存在".into()));
    }
    let path = crate::config::AppConfig::load()
        .help_docs_dir()
        .join("preview")
        .join(format!("docx_images_{}", id))
        .join(&filename);
    let data = tokio::fs::read(&path)
        .await
        .map_err(|_| AppError::NotFound("教程图片不存在或已删除".into()))?;
    let extension = std::path::Path::new(&filename)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime_type(extension))
        .header(header::CACHE_CONTROL, "private, max-age=3600")
        .body(Body::from(data))
        .unwrap())
}

/// 简单扩展名 → MIME 映射
fn mime_type(ext: &str) -> &'static str {
    match ext.to_lowercase().as_str() {
        "pdf" => "application/pdf",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "txt" => "text/plain; charset=utf-8",
        "html" | "htm" => "text/html; charset=utf-8",
        "csv" => "text/csv; charset=utf-8",
        "zip" => "application/zip",
        "rar" => "application/vnd.rar",
        "7z" => "application/x-7z-compressed",
        _ => "application/octet-stream",
    }
}
