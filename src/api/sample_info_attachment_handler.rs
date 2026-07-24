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
use std::sync::Arc;

fn ensure_view_access(
    pool: &DbPool,
    ctx: &AuthContext,
    record_id: i64,
) -> Result<crate::models::sample_info::SampleInfoResponse> {
    let record = sample_info_repo::get_by_id(pool, record_id)?;
    let allowed = ctx.is_system_admin()
        || ctx.is_analysis_member()
        || (ctx.is_rd_leader()
            && record.group_id.is_some()
            && record.group_id == ctx.user.group_id)
        || record.created_by_user_id == Some(ctx.user.id);
    if !allowed {
        return Err(AppError::Forbidden("无权查看该样品记录的附件".into()));
    }
    Ok(record)
}

fn ensure_modify_access(pool: &DbPool, ctx: &AuthContext, record_id: i64) -> Result<()> {
    let record = ensure_view_access(pool, ctx, record_id)?;
    if ctx.is_system_admin() || ctx.is_analysis_member() {
        return Ok(());
    }
    if record.created_by_user_id != Some(ctx.user.id) {
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
            "/api/sample-info/attachments/:att_id",
            axum::routing::delete(delete_attachment),
        )
        // v0.4.62: 提升 body 限制到 100MB（默认 2MB，之前小文件测试蒙蔽了）
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .with_state((pool, config))
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

    // PDF → 浏览器直接预览；其他 → 触发下载
    let ct = if att.file_type == "application/pdf" {
        "application/pdf".to_string()
    } else {
        "application/octet-stream".to_string()
    };

    let disposition = format!("attachment; filename=\"{}\"", att.file_name);

    Ok((
        [
            (header::CONTENT_TYPE, ct),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        bytes,
    ))
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
