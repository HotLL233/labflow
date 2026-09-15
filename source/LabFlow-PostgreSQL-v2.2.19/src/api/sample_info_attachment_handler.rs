use crate::config::AppConfig;
use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::sample_info_attachment::SampleInfoAttachment;
use crate::models::trash::DeleteReasonRequest;
use crate::models::ApiResponse;
use crate::repo::{sample_info_attachment_repo, sample_info_repo};
use crate::service::authz_service::{self, AuthContext};
use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::{header, HeaderMap},
    response::IntoResponse,
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::File,
    io::BufWriter,
    path::{Path as FsPath, PathBuf},
    process::Command,
    sync::{Arc, Mutex, OnceLock},
};
use tokio::sync::Semaphore;

#[derive(Serialize)]
struct DocxPreview {
    html: String,
}

#[derive(Serialize)]
struct ImagePreview {
    status: String,
    page_count: u32,
    first_page_ready: bool,
    error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PreviewManifest {
    status: String,
    page_count: u32,
    first_page_ready: bool,
    error: Option<String>,
}

impl PreviewManifest {
    fn queued() -> Self {
        Self {
            status: "queued".into(),
            page_count: 0,
            first_page_ready: false,
            error: None,
        }
    }

    fn generating() -> Self {
        Self {
            status: "generating".into(),
            ..Self::queued()
        }
    }

    fn first_page_ready(page_count: u32) -> Self {
        Self {
            status: "first_page_ready".into(),
            page_count,
            first_page_ready: true,
            error: None,
        }
    }

    fn ready(page_count: u32) -> Self {
        Self {
            status: "ready".into(),
            page_count,
            first_page_ready: page_count > 0,
            error: None,
        }
    }

    fn failed(message: String) -> Self {
        Self {
            status: "failed".into(),
            page_count: 0,
            first_page_ready: false,
            error: Some(message),
        }
    }

    fn response(&self) -> ImagePreview {
        ImagePreview {
            status: self.status.clone(),
            page_count: self.page_count,
            first_page_ready: self.first_page_ready,
            error: self.error.clone(),
        }
    }
}

static ACTIVE_PREVIEW_JOBS: OnceLock<Mutex<HashSet<i64>>> = OnceLock::new();
static PREVIEW_RENDER_SLOTS: OnceLock<Arc<Semaphore>> = OnceLock::new();
static WORD_CONVERSION_SLOT: OnceLock<Arc<Semaphore>> = OnceLock::new();

fn active_preview_jobs() -> &'static Mutex<HashSet<i64>> {
    ACTIVE_PREVIEW_JOBS.get_or_init(|| Mutex::new(HashSet::new()))
}

fn preview_render_slots() -> Arc<Semaphore> {
    PREVIEW_RENDER_SLOTS
        .get_or_init(|| Arc::new(Semaphore::new(2)))
        .clone()
}

fn word_conversion_slot() -> Arc<Semaphore> {
    WORD_CONVERSION_SLOT
        .get_or_init(|| Arc::new(Semaphore::new(1)))
        .clone()
}

fn attachment_content_disposition(file_name: &str, file_type: &str) -> String {
    let is_pdf = file_type.eq_ignore_ascii_case("application/pdf")
        || file_name.to_ascii_lowercase().ends_with(".pdf");
    let mode = if is_pdf { "inline" } else { "attachment" };
    let encoded_name = url_escape::encode_component(file_name);
    format!("{mode}; filename=\"attachment\"; filename*=UTF-8''{encoded_name}")
}

fn ensure_view_access(
    pool: &DbPool,
    ctx: &AuthContext,
    record_id: i64,
) -> Result<crate::models::sample_info::SampleInfoResponse> {
    let record = sample_info_repo::get_by_id(pool, record_id)?;
    if record.business_user_id == Some(ctx.user.id) {
        return Ok(record);
    }
    if let Some(allowed) = authz_service::sample_info_scope_allows(
        pool,
        ctx,
        record.division_id,
        &record.type_key,
        record.created_by_user_id,
    )? {
        if !allowed {
            return Err(AppError::Forbidden("无权查看该样品记录的附件".into()));
        }
        return Ok(record);
    }
    let analysis_scope_allowed = if ctx.is_analysis_member() {
        authz_service::work_division_allowed(pool, ctx, record.division_id)?
    } else {
        false
    };
    let allowed = ctx.is_system_admin()
        || analysis_scope_allowed
        || (ctx.is_rd_leader()
            && record.group_id.is_some()
            && record.group_id == ctx.user.group_id)
        || record.created_by_user_id == Some(ctx.user.id)
        || record.business_user_id == Some(ctx.user.id);
    if !allowed {
        return Err(AppError::Forbidden("无权查看该样品记录的附件".into()));
    }
    Ok(record)
}

fn ensure_modify_access(pool: &DbPool, ctx: &AuthContext, record_id: i64) -> Result<()> {
    let record = ensure_view_access(pool, ctx, record_id)?;
    if record.status == "已退回" || record.status == "已退回已确认" {
        return Err(AppError::Forbidden("退回原记录的附件不可修改".into()));
    }
    if ctx.is_system_admin() || ctx.is_analysis_member() {
        return Ok(());
    }
    if record.created_by_user_id != Some(ctx.user.id)
        && record.business_user_id != Some(ctx.user.id)
    {
        return Err(AppError::Forbidden("只能修改本人提交记录的附件".into()));
    }
    if record.sampled_at.is_some() {
        return Err(AppError::Forbidden("记录已取样，不能再修改附件".into()));
    }
    Ok(())
}

pub fn router(pool: DbPool) -> Router {
    let config = Arc::new(AppConfig::load());
    Router::new()
        .route("/api/sample-info/attachments/preview-failures/clear", axum::routing::post(clear_failed_preview_cache))
        .route(
            "/api/sample-info/:id/attachments",
            axum::routing::get(list_attachments).post(upload_attachment),
        )
        .route(
            "/api/sample-info/attachments/batch",
            axum::routing::get(batch_attachments),
        )
        .route(
            "/api/sample-info/attachments/:att_id/file",
            axum::routing::get(download_attachment),
        )
        .route(
            "/api/sample-info/attachments/:att_id/docx-preview",
            axum::routing::get(docx_preview),
        )
        .route(
            "/api/sample-info/attachments/:att_id/preview-pdf",
            axum::routing::get(preview_pdf),
        )
        .route(
            "/api/sample-info/attachments/:att_id/image-preview",
            axum::routing::get(image_preview),
        )
        .route(
            "/api/sample-info/attachments/:att_id/image-preview/:page",
            axum::routing::get(image_preview_page),
        )
        .route(
            "/api/sample-info/attachments/:att_id",
            axum::routing::delete(delete_attachment),
        )
        // v0.4.62: 提升 body 限制到 100MB（默认 2MB，之前小文件测试蒙蔽了）
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .with_state((pool, config))
}

async fn clear_failed_preview_cache(
    State((pool, config)): State<(DbPool, Arc<AppConfig>)>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<serde_json::Value>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    if !ctx.is_system_admin() { return Err(AppError::Forbidden("仅系统管理员可清理失败预览缓存".into())); }
    let root = config.attachment_preview_dir();
    let mut cleared = 0u64;
    if let Ok(entries) = std::fs::read_dir(&root) {
        for entry in entries.flatten() {
            let dir = entry.path();
            if dir.is_dir() && read_preview_manifest(&dir).status == "failed" {
                let _ = std::fs::remove_dir_all(&dir);
                cleared += 1;
            }
        }
    }
    Ok(Json(ApiResponse::ok(serde_json::json!({"cleared": cleared}))))
}

fn inline_docx_images(html: String, image_dir: &std::path::Path) -> Result<String> {
    let mut rendered = html;
    let entries = std::fs::read_dir(image_dir)
        .map_err(|error| AppError::Internal(format!("读取 Word 预览图片失败: {error}")))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        let bytes = std::fs::read(&path)
            .map_err(|error| AppError::Internal(format!("读取 Word 图片失败: {error}")))?;
        if bytes.len() > 10 * 1024 * 1024 {
            return Err(AppError::Validation(
                "Word 内嵌图片超过 10MB，无法网页预览".into(),
            ));
        }
        let mime = match path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            _ => "image/png",
        };
        let from = format!("data-help-image=\"inline-docx/{name}\"");
        let to = format!("src=\"data:{mime};base64,{}\"", BASE64.encode(bytes));
        rendered = rendered.replace(&from, &to);
    }
    Ok(rendered)
}

/// Render DOCX attachments in the browser. PDFs continue to use the browser's
/// native viewer; legacy .doc files remain download-only because they are not
/// a zip/XML format and cannot be parsed safely without Office automation.
async fn docx_preview(
    State((pool, config)): State<(DbPool, Arc<AppConfig>)>,
    headers: HeaderMap,
    Path(att_id): Path<i64>,
) -> Result<Json<ApiResponse<DocxPreview>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    let att = sample_info_attachment_repo::find_by_id(&pool, att_id)?;
    ensure_view_access(&pool, &ctx, att.record_id)?;
    if !att.file_name.to_ascii_lowercase().ends_with(".docx") {
        return Err(AppError::Validation(
            "网页预览仅支持 PDF 和 DOCX；DOC 文件请下载后查看".into(),
        ));
    }
    if att.file_size > 25 * 1024 * 1024 {
        return Err(AppError::Validation(
            "DOCX 附件超过 25MB，请下载后查看".into(),
        ));
    }
    let bytes = tokio::fs::read(config.attachments_dir().join(&att.stored_name))
        .await
        .map_err(|error| AppError::NotFound(format!("附件文件不存在: {error}")))?;
    let image_dir = config
        .attachments_dir()
        .join("previews")
        .join(format!("docx_{att_id}"));
    let _ = std::fs::remove_dir_all(&image_dir);
    std::fs::create_dir_all(&image_dir)
        .map_err(|error| AppError::Internal(format!("创建 Word 预览目录失败: {error}")))?;
    let parsed = crate::api::docx_parser::parse_docx(
        &bytes,
        crate::api::docx_parser::DocxImageOptions {
            output_dir: &image_dir,
            image_url_prefix: "inline-docx",
        },
    )
    .map_err(AppError::Validation)?;
    let html = inline_docx_images(parsed.html, &image_dir)?;
    Ok(Json(ApiResponse::ok(DocxPreview { html })))
}

fn preview_dir(preview_root: &FsPath, attachment_id: i64) -> PathBuf {
    preview_root.join(format!("attachment_{attachment_id}"))
}

fn find_soffice() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("LIBREOFFICE_PATH") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    let mut roots = Vec::new();
    for key in ["ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(value) = std::env::var_os(key) {
            let root = PathBuf::from(value);
            if !roots.iter().any(|item: &PathBuf| item == &root) {
                roots.push(root);
            }
        }
    }
    for root in roots {
        let program_dir = root.join("LibreOffice").join("program");
        for executable in ["soffice.com", "soffice.exe"] {
            let path = program_dir.join(executable);
            if path.is_file() {
                return Some(path);
            }
        }
    }
    None
}

fn convert_office_to_pdf(source: &FsPath, output_dir: &FsPath) -> Result<PathBuf> {
    let soffice = find_soffice().ok_or_else(|| {
        AppError::Validation(
            "未检测到 LibreOffice，无法保持 Word 原排版进行网页预览；请完成安装后重试。".into(),
        )
    })?;
    // LibreOffice locks its profile directory. A per-render profile prevents one
    // attachment preview (or another server instance during upgrade) from
    // blocking every subsequent Word preview.
    let profile_dir = output_dir.join(format!("libreoffice_profile_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&profile_dir)
        .map_err(|error| AppError::Internal(format!("创建 LibreOffice 预览目录失败: {error}")))?;
    let profile_url = format!(
        "file:///{}",
        profile_dir.to_string_lossy().replace('\\', "/")
    );
    let output = Command::new(soffice)
        .arg("--headless")
        .arg("--nologo")
        .arg("--nodefault")
        .arg("--nolockcheck")
        .arg("--nofirststartwizard")
        .arg(format!("-env:UserInstallation={profile_url}"))
        .arg("--convert-to")
        .arg("pdf:writer_pdf_Export")
        .arg("--outdir")
        .arg(output_dir)
        .arg(source)
        .output()
        .map_err(|error| AppError::Internal(format!("启动 LibreOffice 转换失败: {error}")))?;
    if !output.status.success() {
        let detail = [
            String::from_utf8_lossy(&output.stdout).trim().to_string(),
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("；");
        let _ = std::fs::remove_dir_all(&profile_dir);
        return Err(AppError::Validation(format!(
            "LibreOffice 转换失败{}",
            if detail.is_empty() {
                "".to_string()
            } else {
                format!("：{detail}")
            }
        )));
    }
    let expected = output_dir.join(
        source
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
            + ".pdf",
    );
    if expected.is_file() {
        let _ = std::fs::remove_dir_all(&profile_dir);
        return Ok(expected);
    }
    let fallback = std::fs::read_dir(output_dir)
        .map_err(|error| AppError::Internal(format!("读取转换结果失败: {error}")))?
        .flatten()
        .map(|entry| entry.path())
        .find(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|value| value.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"))
        })
        .ok_or_else(|| AppError::Validation("LibreOffice 未生成可预览的 PDF 文件".into()));
    let _ = std::fs::remove_dir_all(&profile_dir);
    fallback
}

/// Shared source-format preview conversion for help tutorials.
pub fn convert_office_file_to_pdf(source: &FsPath, output_dir: &FsPath) -> Result<PathBuf> {
    convert_office_to_pdf(source, output_dir)
}

/// Windows PDF APIs are sensitive to long and non-ASCII source paths. Keep the
/// original attachment untouched and render a short, stable cache copy instead.
fn copy_to_preview_source(
    source: &FsPath,
    render_dir: &FsPath,
    extension: &str,
) -> Result<PathBuf> {
    let extension = match extension {
        "pdf" => "pdf",
        "doc" => "doc",
        "docx" => "docx",
        _ => return Err(AppError::Validation("该附件类型暂不支持网页预览".into())),
    };
    let preview_source = render_dir.join(format!("source.{extension}"));
    std::fs::copy(source, &preview_source)
        .map_err(|error| AppError::Internal(format!("准备附件预览缓存失败: {error}")))?;
    Ok(preview_source)
}

fn supported_preview_extension(value: &str) -> Option<&'static str> {
    match value
        .trim()
        .trim_start_matches('.')
        .to_ascii_lowercase()
        .as_str()
    {
        "pdf" => Some("pdf"),
        "doc" => Some("doc"),
        "docx" => Some("docx"),
        _ => None,
    }
}

fn attachment_preview_extension(att: &SampleInfoAttachment) -> Option<&'static str> {
    for name in [&att.stored_name, &att.file_name] {
        if let Some(extension) = FsPath::new(name)
            .extension()
            .and_then(|value| value.to_str())
        {
            if let Some(extension) = supported_preview_extension(extension) {
                return Some(extension);
            }
        }
    }
    match att.file_type.trim().to_ascii_lowercase().as_str() {
        "application/pdf" => Some("pdf"),
        "application/msword" => Some("doc"),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => Some("docx"),
        _ => None,
    }
}

fn preview_manifest_path(render_dir: &FsPath) -> PathBuf {
    render_dir.join("preview-state.json")
}

fn count_preview_pages(render_dir: &FsPath) -> u32 {
    std::fs::read_dir(render_dir)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| {
            let path = entry.path();
            path.is_file()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        name.starts_with("page_")
                            && (name.ends_with(".webp") || name.ends_with(".png"))
                    })
        })
        .count() as u32
}

fn preview_page_path(render_dir: &FsPath, page: u32) -> Option<PathBuf> {
    for extension in ["webp", "png"] {
        let path = render_dir.join(format!("page_{page}.{extension}"));
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

fn read_preview_manifest(render_dir: &FsPath) -> PreviewManifest {
    let manifest_path = preview_manifest_path(render_dir);
    if let Ok(raw) = std::fs::read_to_string(&manifest_path) {
        if let Ok(manifest) = serde_json::from_str::<PreviewManifest>(&raw) {
            let cached_pages = count_preview_pages(render_dir);
            // Older builds marked a one-page PDF as failed after trying to
            // render page 2. Keep the already generated first page usable and
            // avoid sending that attachment back into the retry loop.
            if manifest.status == "failed"
                && manifest
                    .error
                    .as_deref()
                    .is_some_and(|error| error.contains("Wrong page range given"))
                && cached_pages > 0
            {
                return PreviewManifest::ready(cached_pages);
            }
            return manifest;
        }
    }
    // Preview caches produced by beta.8 did not carry a manifest. They remain
    // reusable rather than forcing every old attachment through LibreOffice again.
    let cached_pages = count_preview_pages(render_dir);
    if cached_pages > 0 {
        PreviewManifest::ready(cached_pages)
    } else {
        PreviewManifest::queued()
    }
}

fn write_preview_manifest(render_dir: &FsPath, manifest: &PreviewManifest) -> Result<()> {
    std::fs::create_dir_all(render_dir)
        .map_err(|error| AppError::Internal(format!("创建附件预览目录失败: {error}")))?;
    let path = preview_manifest_path(render_dir);
    let temp = render_dir.join("preview-state.tmp");
    let body = serde_json::to_vec(manifest)
        .map_err(|error| AppError::Internal(format!("序列化附件预览状态失败: {error}")))?;
    std::fs::write(&temp, body)
        .map_err(|error| AppError::Internal(format!("写入附件预览状态失败: {error}")))?;
    let _ = std::fs::remove_file(&path);
    std::fs::rename(&temp, &path)
        .map_err(|error| AppError::Internal(format!("保存附件预览状态失败: {error}")))?;
    Ok(())
}

fn convert_png_pages_to_webp(render_dir: &FsPath) -> Result<()> {
    let pages = std::fs::read_dir(render_dir)
        .map_err(|error| AppError::Internal(format!("读取附件预览图片失败: {error}")))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("page_") && name.ends_with(".png"))
        })
        .collect::<Vec<_>>();
    for png_path in pages {
        let webp_path = png_path.with_extension("webp");
        let image = image::ImageReader::open(&png_path)
            .map_err(|error| AppError::Internal(format!("读取附件预览图片失败: {error}")))?
            .decode()
            .map_err(|error| AppError::Internal(format!("解析附件预览图片失败: {error}")))?;
        let file = File::create(&webp_path)
            .map_err(|error| AppError::Internal(format!("创建 WebP 预览失败: {error}")))?;
        let mut output = BufWriter::new(file);
        image
            .write_to(&mut output, image::ImageFormat::WebP)
            .map_err(|error| AppError::Internal(format!("生成 WebP 预览失败: {error}")))?;
        drop(output);
        std::fs::remove_file(&png_path)
            .map_err(|error| AppError::Internal(format!("清理 PNG 中间预览失败: {error}")))?;
    }
    Ok(())
}

fn release_preview_job(attachment_id: i64) {
    if let Ok(mut jobs) = active_preview_jobs().lock() {
        jobs.remove(&attachment_id);
    }
}

fn build_preview_images(
    attachments_dir: &FsPath,
    preview_root: &FsPath,
    att: &SampleInfoAttachment,
) -> Result<()> {
    const MAX_RENDER_SIZE: i64 = 50 * 1024 * 1024;
    if att.file_size > MAX_RENDER_SIZE {
        return Err(AppError::Validation("附件超过 50MB，请下载后查看".into()));
    }
    let render_dir = preview_dir(preview_root, att.id);
    let _ = std::fs::remove_dir_all(&render_dir);
    std::fs::create_dir_all(&render_dir)
        .map_err(|error| AppError::Internal(format!("创建附件预览目录失败: {error}")))?;
    write_preview_manifest(&render_dir, &PreviewManifest::generating())?;

    let pdf_path = prepare_preview_pdf(attachments_dir, preview_root, att)?;
    let first_page_count = crate::api::pdf_render::pdf_page_to_png(&pdf_path, &render_dir, 1)
        .map_err(|error| AppError::Validation(format!("生成附件首页预览失败: {error}")))?;
    if first_page_count == 0 {
        return Err(AppError::Validation("附件没有可预览的页面".into()));
    }
    convert_png_pages_to_webp(&render_dir)?;
    let page_count = crate::api::pdf_render::pdf_page_count(&pdf_path)
        .map_err(|error| AppError::Validation(format!("读取附件页数失败: {error}")))?;
    write_preview_manifest(&render_dir, &PreviewManifest::first_page_ready(page_count))?;
    Ok(())
}

fn enqueue_image_preview(
    attachments_dir: PathBuf,
    preview_root: PathBuf,
    att: SampleInfoAttachment,
) -> ImagePreview {
    let render_dir = preview_dir(&preview_root, att.id);
    let current = read_preview_manifest(&render_dir);
    // A failed task must remain visible as failed until the user explicitly
    // retries it. Re-queueing it on every status request creates a tight
    // frontend polling loop and repeatedly launches the same broken task.
    let recoverable_legacy_failure = current.status == "failed"
        && current
            .error
            .as_deref()
            .is_some_and(|error| error.contains("该附件类型暂不支持网页预览"))
        && attachment_preview_extension(&att).is_some();
    if matches!(current.status.as_str(), "ready")
        || (current.status == "first_page_ready" && preview_page_path(&render_dir, 1).is_some())
        || (current.status == "failed" && !recoverable_legacy_failure)
    {
        return current.response();
    }
    if recoverable_legacy_failure {
        let _ = std::fs::remove_dir_all(&render_dir);
    }
    let claimed = active_preview_jobs()
        .lock()
        .map(|mut jobs| jobs.insert(att.id))
        .unwrap_or(false);
    if !claimed {
        return current.response();
    }
    if let Err(error) = write_preview_manifest(&render_dir, &PreviewManifest::queued()) {
        release_preview_job(att.id);
        return PreviewManifest::failed(error.to_string()).response();
    }
    let attachment_id = att.id;
    let is_word = matches!(attachment_preview_extension(&att), Some("doc" | "docx"));
    tokio::spawn(async move {
        let word_permit = if is_word {
            match word_conversion_slot().acquire_owned().await {
                Ok(permit) => Some(permit),
                Err(_) => {
                    let render_dir = preview_dir(&preview_root, attachment_id);
                    let _ = write_preview_manifest(
                        &render_dir,
                        &PreviewManifest::failed("Word 预览任务队列已关闭".into()),
                    );
                    release_preview_job(attachment_id);
                    return;
                }
            }
        } else {
            None
        };
        let render_permit = match preview_render_slots().acquire_owned().await {
            Ok(permit) => permit,
            Err(_) => {
                let render_dir = preview_dir(&preview_root, attachment_id);
                let _ = write_preview_manifest(
                    &render_dir,
                    &PreviewManifest::failed("附件预览任务队列已关闭".into()),
                );
                release_preview_job(attachment_id);
                return;
            }
        };
        let task_preview_root = preview_root.clone();
        let result = tokio::task::spawn_blocking(move || {
            build_preview_images(&attachments_dir, &task_preview_root, &att)
        })
        .await
        .map_err(|error| AppError::Internal(format!("附件预览任务异常: {error}")))
        .and_then(|result| result);
        drop(word_permit);
        drop(render_permit);
        if let Err(error) = result {
            let render_dir = preview_dir(&preview_root, attachment_id);
            let _ =
                write_preview_manifest(&render_dir, &PreviewManifest::failed(error.to_string()));
            tracing::warn!(
                attachment_id,
                "attachment preview generation failed: {error}"
            );
        }
        release_preview_job(attachment_id);
    });
    PreviewManifest::queued().response()
}

fn prepare_preview_pdf(
    attachments_dir: &FsPath,
    preview_root: &FsPath,
    att: &SampleInfoAttachment,
) -> Result<PathBuf> {
    const MAX_RENDER_SIZE: i64 = 50 * 1024 * 1024;
    if att.file_size > MAX_RENDER_SIZE {
        return Err(AppError::Validation("附件超过 50MB，请下载后查看".into()));
    }
    let render_dir = preview_dir(preview_root, att.id);
    std::fs::create_dir_all(&render_dir)
        .map_err(|error| AppError::Internal(format!("创建附件预览目录失败: {error}")))?;
    let preview_pdf = render_dir.join("preview.pdf");
    if preview_pdf.is_file() {
        return Ok(preview_pdf);
    }
    let source = attachments_dir.join(&att.stored_name);
    if !source.is_file() {
        return Err(AppError::NotFound("附件原文件不存在".into()));
    }
    let extension = attachment_preview_extension(att)
        .ok_or_else(|| AppError::Validation("该附件类型暂不支持网页预览".into()))?;
    let preview_source = copy_to_preview_source(&source, &render_dir, &extension)?;
    let produced_pdf = if extension == "pdf" {
        preview_source
    } else if extension == "doc" || extension == "docx" {
        convert_office_to_pdf(&preview_source, &render_dir)?
    } else {
        return Err(AppError::Validation("该附件类型暂不支持网页预览".into()));
    };
    if produced_pdf != preview_pdf {
        std::fs::copy(&produced_pdf, &preview_pdf)
            .map_err(|error| AppError::Internal(format!("保存附件预览 PDF 失败: {error}")))?;
    }
    Ok(preview_pdf)
}

async fn preview_pdf(
    State((pool, config)): State<(DbPool, Arc<AppConfig>)>,
    headers: HeaderMap,
    Path(att_id): Path<i64>,
) -> Result<impl IntoResponse> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    let att = sample_info_attachment_repo::find_by_id(&pool, att_id)?;
    ensure_view_access(&pool, &ctx, att.record_id)?;
    let attachments_dir = config.attachments_dir();
    let preview_root = config.attachment_preview_dir();
    let _ = enqueue_image_preview(attachments_dir, preview_root.clone(), att.clone());
    let preview_path = preview_dir(&preview_root, att_id).join("preview.pdf");
    if !preview_path.is_file() {
        return Err(AppError::Validation(
            "附件预览正在后台生成，请稍后重试".into(),
        ));
    }
    let bytes = tokio::fs::read(preview_path)
        .await
        .map_err(|error| AppError::NotFound(format!("附件预览文件不存在: {error}")))?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/pdf"),
            (header::CACHE_CONTROL, "private, max-age=86400"),
            (header::CONTENT_DISPOSITION, "inline; filename=preview.pdf"),
        ],
        bytes,
    ))
}

async fn image_preview(
    State((pool, config)): State<(DbPool, Arc<AppConfig>)>,
    headers: HeaderMap,
    Path(att_id): Path<i64>,
) -> Result<Json<ApiResponse<ImagePreview>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    let att = sample_info_attachment_repo::find_by_id(&pool, att_id)?;
    ensure_view_access(&pool, &ctx, att.record_id)?;
    let attachments_dir = config.attachments_dir();
    let preview_root = config.attachment_preview_dir();
    let preview = enqueue_image_preview(attachments_dir, preview_root, att);
    Ok(Json(ApiResponse::ok(preview)))
}

async fn image_preview_page(
    State((pool, config)): State<(DbPool, Arc<AppConfig>)>,
    headers: HeaderMap,
    Path((att_id, page)): Path<(i64, u32)>,
) -> Result<impl IntoResponse> {
    if page == 0 {
        return Err(AppError::Validation("预览页码无效".into()));
    }
    let ctx = authz_service::authenticate(&pool, &headers)?;
    let att = sample_info_attachment_repo::find_by_id(&pool, att_id)?;
    ensure_view_access(&pool, &ctx, att.record_id)?;
    let render_dir = preview_dir(&config.attachment_preview_dir(), att_id);
    if let Some(page_path) = preview_page_path(&render_dir, page) {
        return read_preview_page(&page_path).await;
    }

    let manifest = read_preview_manifest(&render_dir);
    if manifest.status == "failed" {
        return Err(AppError::Validation(
            manifest.error.unwrap_or_else(|| "附件预览生成失败".into()),
        ));
    }
    if manifest.page_count > 0 && page > manifest.page_count {
        return Err(AppError::Validation("预览页码超出附件页数".into()));
    }

    // The upload task prepares preview.pdf and page 1. If a user asks for a
    // later page before that task finishes, keep the request lightweight and
    // let the existing status poller expose the first page first.
    let preview_pdf = render_dir.join("preview.pdf");
    if !preview_pdf.is_file() {
        let _ = enqueue_image_preview(
            config.attachments_dir(),
            config.attachment_preview_dir(),
            att.clone(),
        );
        return Err(AppError::NotFound("附件首页仍在生成，请稍后重试".into()));
    }

    let permit = preview_render_slots()
        .acquire_owned()
        .await
        .map_err(|_| AppError::Internal("附件预览任务队列已关闭".into()))?;
    let render_dir_for_task = render_dir.clone();
    let preview_pdf_for_task = preview_pdf.clone();
    let result = tokio::task::spawn_blocking(move || {
        if preview_page_path(&render_dir_for_task, page).is_none() {
            crate::api::pdf_render::pdf_page_to_png(
                &preview_pdf_for_task,
                &render_dir_for_task,
                page,
            )
            .map_err(|error| AppError::Validation(format!("生成第 {page} 页预览失败: {error}")))?;
            convert_png_pages_to_webp(&render_dir_for_task)?;
        }
        preview_page_path(&render_dir_for_task, page)
            .ok_or_else(|| AppError::NotFound("预览图片生成失败，请稍后重试".into()))
    })
    .await
    .map_err(|error| AppError::Internal(format!("附件分页预览任务异常: {error}")))??;
    drop(permit);
    read_preview_page(&result).await
}

async fn read_preview_page(page_path: &FsPath) -> Result<impl IntoResponse> {
    let bytes = tokio::fs::read(&page_path)
        .await
        .map_err(|_| AppError::NotFound("预览图片不存在，请重新打开附件预览".into()))?;
    let content_type = match page_path.extension().and_then(|value| value.to_str()) {
        Some("webp") => "image/webp",
        _ => "image/png",
    };
    Ok((
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "private, max-age=86400"),
        ],
        bytes,
    ))
}

/// GET /api/sample-info/:id/attachments — 获取某条记录的所有附件
async fn list_attachments(
    State((pool, _config)): State<(DbPool, Arc<AppConfig>)>,
    headers: HeaderMap,
    Path(record_id): Path<i64>,
) -> Result<Json<ApiResponse<Vec<SampleInfoAttachment>>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    ensure_view_access(&pool, &ctx, record_id)?;
    let items = sample_info_attachment_repo::list_by_record(&pool, record_id)?;
    Ok(Json(ApiResponse::ok(items)))
}

/// POST /api/sample-info/:id/attachments — 上传附件（multipart, 限制 PDF/Word, max 100MB）
async fn upload_attachment(
    State((pool, config)): State<(DbPool, Arc<AppConfig>)>,
    headers: HeaderMap,
    Path(record_id): Path<i64>,
    mut multipart: Multipart,
) -> Result<Json<ApiResponse<SampleInfoAttachment>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    ensure_modify_access(&pool, &ctx, record_id)?;
    const MAX_SIZE: usize = 100 * 1024 * 1024; // 100 MB
                                               // v0.4.61: 只检查文件扩展名（MIME 类型不可靠，浏览器/系统差异大）
    let allowed_exts = ["pdf", "doc", "docx"];

    let mut file_name = String::new();
    let mut file_type = String::new();
    let mut file_data: Vec<u8> = Vec::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Internal(format!("上传错误: {}", e)))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" {
            file_name = field.file_name().unwrap_or("unknown").to_string();
            file_type = field
                .content_type()
                .unwrap_or("application/octet-stream")
                .to_string();

            // v0.4.61: 只用扩展名验证（MIME 不可靠）
            let ext = std::path::Path::new(&file_name)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if !allowed_exts.contains(&ext.as_str()) {
                return Err(AppError::Validation(format!(
                    "仅支持 PDF、Word 文档（.pdf/.doc/.docx），当前文件扩展名: .{}",
                    ext
                )));
            }

            file_data = field
                .bytes()
                .await
                .map_err(|e| AppError::Internal(format!("读取文件失败: {}", e)))?
                .to_vec();

            if file_data.len() > MAX_SIZE {
                return Err(AppError::Validation("文件大小不能超过 100MB".into()));
            }
        }
    }

    if file_data.is_empty() {
        return Err(AppError::Validation("未选择文件".into()));
    }

    // v0.4.28: 生成唯一存储文件名 seq_{序号}_{ID}_{时间戳}_{原名}
    let ext = std::path::Path::new(&file_name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin");
    let seq = sample_info_attachment_repo::next_seq_for_record(&pool, record_id)?;
    let now = chrono::Local::now().format("%Y%m%d%H%M%S");
    let stored_name = format!("seq_{}_{}_{}_{}.{}", seq, record_id, now, file_name, ext);

    // 确保附件目录存在
    let attachments_dir = config.attachments_dir();
    std::fs::create_dir_all(&attachments_dir)
        .map_err(|e| AppError::Internal(format!("创建附件目录失败: {}", e)))?;

    // 写入文件
    let file_path = attachments_dir.join(&stored_name);
    std::fs::write(&file_path, &file_data)
        .map_err(|e| AppError::Internal(format!("保存文件失败: {}", e)))?;

    let file_size = file_data.len() as i64;
    let att = sample_info_attachment_repo::create(
        &pool,
        record_id,
        &file_name,
        &stored_name,
        file_size,
        &file_type,
    )?;

    // The record is already durable. Create the preview asynchronously so the
    // submit action never waits for LibreOffice or PDF rendering.
    let preview_attachments_dir = attachments_dir;
    let preview_root = config.attachment_preview_dir();
    let preview_attachment = att.clone();
    let _ = enqueue_image_preview(preview_attachments_dir, preview_root, preview_attachment);

    Ok(Json(ApiResponse::ok(att)))
}

/// GET /api/sample-info/attachments/:att_id/file — 下载/预览附件
async fn download_attachment(
    State((pool, config)): State<(DbPool, Arc<AppConfig>)>,
    headers: HeaderMap,
    Path(att_id): Path<i64>,
) -> Result<impl IntoResponse> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    let att = sample_info_attachment_repo::find_by_id(&pool, att_id)?;
    ensure_view_access(&pool, &ctx, att.record_id)?;
    let file_path = config.attachments_dir().join(&att.stored_name);

    let bytes = tokio::fs::read(&file_path)
        .await
        .map_err(|e| AppError::NotFound(format!("附件文件不存在: {}", e)))?;

    let ct = if att.file_type.trim().is_empty() {
        "application/octet-stream".to_string()
    } else {
        att.file_type.clone()
    };
    let disposition = attachment_content_disposition(&att.file_name, &att.file_type);

    Ok((
        [
            (header::CONTENT_TYPE, ct),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        bytes,
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        attachment_content_disposition, attachment_preview_extension, build_preview_images,
        convert_png_pages_to_webp, copy_to_preview_source, count_preview_pages, preview_dir,
        preview_page_path, read_preview_manifest, write_preview_manifest, PreviewManifest,
    };
    use crate::models::sample_info_attachment::SampleInfoAttachment;

    #[test]
    fn pdf_opens_inline_and_word_uses_download_with_utf8_filename() {
        let pdf = attachment_content_disposition("检测报告.pdf", "application/pdf");
        assert!(pdf.starts_with("inline;"));
        assert!(pdf.contains("filename*=UTF-8''"));
        let word = attachment_content_disposition(
            "说明.docx",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        );
        assert!(word.starts_with("attachment;"));
        assert!(word.contains("filename*=UTF-8''"));
    }

    #[test]
    fn preview_uses_a_short_stable_cache_filename() {
        let root = std::env::temp_dir().join(format!(
            "workload-attachment-preview-test-{}",
            uuid::Uuid::new_v4()
        ));
        let source_dir = root.join("very-long-source-directory-name");
        let preview_dir = root.join("preview");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::create_dir_all(&preview_dir).unwrap();
        let source = source_dir.join("包含中文和特殊符号的超长附件名称.docx");
        std::fs::write(&source, b"attachment-preview").unwrap();

        let cached = copy_to_preview_source(&source, &preview_dir, "docx").unwrap();
        assert_eq!(
            cached.file_name().and_then(|name| name.to_str()),
            Some("source.docx")
        );
        assert_eq!(std::fs::read(cached).unwrap(), b"attachment-preview");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn old_attachment_uses_original_name_or_mime_when_storage_name_has_no_extension() {
        let mut attachment = SampleInfoAttachment {
            id: 1,
            record_id: 1,
            file_name: "旧检测报告.pdf".into(),
            stored_name: "20250818_123456".into(),
            file_size: 1,
            file_type: String::new(),
            created_at: String::new(),
        };
        assert_eq!(attachment_preview_extension(&attachment), Some("pdf"));

        attachment.file_name = "旧检测报告".into();
        attachment.file_type = "application/pdf".into();
        assert_eq!(attachment_preview_extension(&attachment), Some("pdf"));
    }

    #[test]
    fn preview_cache_uses_webp_and_persists_ready_state() {
        let root = std::env::temp_dir().join(format!(
            "workload-attachment-preview-webp-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        image::RgbaImage::from_pixel(8, 8, image::Rgba([34, 95, 166, 255]))
            .save(root.join("page_1.png"))
            .unwrap();

        convert_png_pages_to_webp(&root).unwrap();
        assert!(root.join("page_1.webp").is_file());
        assert!(!root.join("page_1.png").exists());
        assert_eq!(count_preview_pages(&root), 1);
        assert_eq!(
            preview_page_path(&root, 1)
                .and_then(|path| path.extension().map(|value| value.to_owned()))
                .and_then(|value| value.to_str().map(str::to_owned)),
            Some("webp".to_string())
        );

        write_preview_manifest(&root, &PreviewManifest::ready(1)).unwrap();
        let state = read_preview_manifest(&root);
        assert_eq!(state.status, "ready");
        assert!(state.first_page_ready);
        assert_eq!(state.page_count, 1);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn old_one_page_failure_cache_is_recovered_without_requeueing() {
        let root = std::env::temp_dir().join(format!(
            "workload-attachment-preview-one-page-recovery-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        image::RgbaImage::from_pixel(8, 8, image::Rgba([34, 95, 166, 255]))
            .save(root.join("page_1.webp"))
            .unwrap();
        write_preview_manifest(
            &root,
            &PreviewManifest::failed("验证失败: Wrong page range given: first page 2".into()),
        )
        .unwrap();

        let state = read_preview_manifest(&root);
        assert_eq!(state.status, "ready");
        assert_eq!(state.page_count, 1);
        assert!(state.first_page_ready);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn one_page_pdf_completes_preview_without_requesting_page_two() {
        let Ok(fixture) = std::env::var("WORKLOAD_ATTACHMENT_PREVIEW_ONE_PAGE_TEST_FILE") else {
            return;
        };
        let root = std::env::temp_dir().join(format!(
            "workload-attachment-one-page-preview-test-{}",
            uuid::Uuid::new_v4()
        ));
        let attachments = root.join("attachments");
        let previews = root.join("previews");
        std::fs::create_dir_all(&attachments).unwrap();
        let attachment = SampleInfoAttachment {
            id: 902,
            record_id: 1,
            file_name: "one-page.pdf".into(),
            stored_name: "one-page.pdf".into(),
            file_size: std::fs::metadata(&fixture).unwrap().len() as i64,
            file_type: "application/pdf".into(),
            created_at: String::new(),
        };
        std::fs::copy(&fixture, attachments.join(&attachment.stored_name)).unwrap();

        build_preview_images(&attachments, &previews, &attachment).unwrap();
        let cache = preview_dir(&previews, attachment.id);
        let state = read_preview_manifest(&cache);
        assert_eq!(state.status, "first_page_ready");
        assert_eq!(state.page_count, 1);
        assert!(cache.join("page_1.webp").is_file());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn background_pdf_preview_creates_reusable_first_page_cache_when_a_fixture_is_supplied() {
        let Ok(fixture) = std::env::var("WORKLOAD_ATTACHMENT_PREVIEW_TEST_FILE") else {
            return;
        };
        let root = std::env::temp_dir().join(format!(
            "workload-attachment-background-preview-test-{}",
            uuid::Uuid::new_v4()
        ));
        let attachments = root.join("attachments");
        let previews = root.join("previews");
        std::fs::create_dir_all(&attachments).unwrap();
        let attachment = SampleInfoAttachment {
            id: 901,
            record_id: 1,
            file_name: "fixture.pdf".into(),
            stored_name: "fixture.pdf".into(),
            file_size: std::fs::metadata(&fixture).unwrap().len() as i64,
            file_type: "application/pdf".into(),
            created_at: String::new(),
        };
        std::fs::copy(&fixture, attachments.join(&attachment.stored_name)).unwrap();

        build_preview_images(&attachments, &previews, &attachment).unwrap();
        let cache = preview_dir(&previews, attachment.id);
        let state = read_preview_manifest(&cache);
        assert_eq!(state.status, "first_page_ready");
        assert!(state.first_page_ready);
        assert!(state.page_count > 1);
        assert!(cache.join("preview.pdf").is_file());
        assert!(cache.join("page_1.webp").is_file());

        // Remaining pages are rendered only when a viewer requests them.
        assert_eq!(count_preview_pages(&cache), 1);
        crate::api::pdf_render::pdf_page_to_png(&cache.join("preview.pdf"), &cache, 2).unwrap();
        convert_png_pages_to_webp(&cache).unwrap();
        assert!(cache.join("page_2.webp").is_file());

        let _ = std::fs::remove_dir_all(root);
    }
}

/// DELETE /api/sample-info/attachments/:att_id — 删除附件
async fn delete_attachment(
    State((pool, _config)): State<(DbPool, Arc<AppConfig>)>,
    headers: HeaderMap,
    Path(att_id): Path<i64>,
    axum::extract::Query(body): axum::extract::Query<DeleteReasonRequest>,
) -> Result<Json<ApiResponse<()>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    let existing = sample_info_attachment_repo::find_by_id(&pool, att_id)?;
    ensure_modify_access(&pool, &ctx, existing.record_id)?;
    let reason = body
        .reason
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("用户删除");
    sample_info_attachment_repo::delete(&pool, att_id, &ctx.user.username, reason)?;

    Ok(Json(ApiResponse::ok_msg("已移入回收站")))
}

/// GET /api/sample-info/attachments/batch?record_ids=1,2,3 — 批量获取附件
/// 返回 { [record_id]: SampleInfoAttachment[] }
async fn batch_attachments(
    State((pool, _config)): State<(DbPool, Arc<AppConfig>)>,
    headers: HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<ApiResponse<std::collections::HashMap<i64, Vec<SampleInfoAttachment>>>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    let ids_str = params.get("record_ids").cloned().unwrap_or_default();
    if ids_str.is_empty() {
        return Ok(Json(ApiResponse::ok(std::collections::HashMap::new())));
    }
    let record_ids: Vec<i64> = ids_str
        .split(',')
        .filter_map(|s| s.trim().parse::<i64>().ok())
        .collect();

    let mut allowed_ids = Vec::new();
    for record_id in record_ids {
        if ensure_view_access(&pool, &ctx, record_id).is_ok() {
            allowed_ids.push(record_id);
        }
    }
    Ok(Json(ApiResponse::ok(
        sample_info_attachment_repo::list_by_records(&pool, &allowed_ids)?,
    )))
}
