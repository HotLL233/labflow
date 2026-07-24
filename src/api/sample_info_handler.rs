use crate::config::AppConfig;
use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::sample_info::{
    SampleInfoCreate, SampleInfoQuery, SampleInfoResponse, SampleInfoStatusUpdate, SampleInfoUpdate,
};
use crate::models::trash::DeleteReasonRequest;
use crate::models::{ApiResponse, PaginatedResponse};
use crate::repo::sample_info_repo;
use crate::service::authz_service::{self, AuthContext};
use crate::service::notification_service;
use crate::service::sample_attachment_service;
use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use serde::Serialize;

fn ensure_status_permission(
    pool: &DbPool,
    headers: &HeaderMap,
    target_status: &str,
) -> Result<String> {
    let ctx = authz_service::authenticate(pool, headers)?;
    let required = match target_status {
        "待检测" => "sample-info:collect",
        "已检测" => "sample-info:complete",
        _ => return Err(AppError::Validation("无效的样品状态操作".into())),
    };
    if !ctx.has_permission(required) {
        return Err(AppError::Forbidden("无权执行该样品状态操作".into()));
    }
    Ok(ctx.user.username)
}

fn apply_record_scope(ctx: &AuthContext, query: &mut SampleInfoQuery) -> Result<()> {
    if ctx.is_system_admin()
        || (ctx.is_analysis_member()
            && (!query.include_deleted.unwrap_or(false) || ctx.is_analysis_leader()))
    {
        return Ok(());
    }
    if ctx.is_rd_leader() {
        query.group_id = Some(
            ctx.user
                .group_id
                .ok_or_else(|| AppError::Validation("研发送样组长尚未设置所属实验室".into()))?,
        );
    } else {
        query.created_by_user_id = Some(ctx.user.id);
    }
    Ok(())
}

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/sample-info", get(list).post(create))
        .route(
            "/api/sample-info/with-attachments",
            post(create_with_attachments),
        )
        .route("/api/sample-info/stats", get(stats))
        .route(
            "/api/sample-info/:id",
            axum::routing::put(update).delete(soft_delete),
        )
        .route(
            "/api/sample-info/:id/status",
            axum::routing::put(update_status),
        )
        .route("/api/sample-info/:id/sample", axum::routing::put(sample))
        .route("/api/sample-info/:id/restore", axum::routing::post(restore))
        .route(
            "/api/sample-info/:id/complete",
            axum::routing::put(complete),
        )
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .with_state(pool)
}

async fn list(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(mut q): Query<SampleInfoQuery>,
) -> Result<Json<ApiResponse<PaginatedResponse<SampleInfoResponse>>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "entry:sample-info")?;
    apply_record_scope(&ctx, &mut q)?;
    let (items, total) = sample_info_repo::list(&pool, &q)?;
    let page = q.page.unwrap_or(1).max(1);
    let page_size = q.page_size.unwrap_or(20).min(500);
    Ok(Json(ApiResponse::ok(PaginatedResponse {
        items,
        total,
        page,
        page_size,
    })))
}

async fn create(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(mut body): Json<SampleInfoCreate>,
) -> Result<Json<ApiResponse<SampleInfoResponse>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "entry:sample-info")?;
    body.user_name = ctx.user.username.clone();
    let record = sample_info_repo::create(&pool, &body, &ctx.user.username)?;
    // The notification is queued only after the sample record has committed. Delivery failures never block registration.
    notification_service::enqueue_sample_submitted(&pool, &record)?;
    let notification_pool = pool.clone();
    tokio::task::spawn_blocking(move || {
        if let Err(error) = notification_service::process_pending(&notification_pool, 20) {
            tracing::warn!("sample submission notification processing failed: {error}");
        }
    });
    Ok(Json(ApiResponse::ok(record)))
}

struct PendingAttachment {
    file_name: String,
    file_type: String,
    file_data: Vec<u8>,
}

fn rollback_failed_attachment_submission(
    pool: &DbPool,
    record_id: i64,
    saved_files: &[std::path::PathBuf],
) {
    for path in saved_files {
        let _ = std::fs::remove_file(path);
    }
    if let Ok(conn) = pool.get() {
        // This record was never submitted successfully, so do not retain it in the recycle bin.
        let _ = conn.execute("DELETE FROM sample_info_records WHERE id=?1", [record_id]);
    }
}

/// POST /api/sample-info/with-attachments
/// Receives one record payload plus all selected attachments, then queues the notification only
/// after every attachment has been persisted.
async fn create_with_attachments(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<ApiResponse<SampleInfoResponse>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "entry:sample-info")?;

    let mut payload_json: Option<String> = None;
    let mut attachments = Vec::new();
    let mut total_size = 0usize;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| AppError::Validation(format!("读取提交内容失败: {error}")))?
    {
        match field.name().unwrap_or("") {
            "payload" => {
                if payload_json.is_some() {
                    return Err(AppError::Validation("提交数据重复".into()));
                }
                payload_json =
                    Some(field.text().await.map_err(|error| {
                        AppError::Validation(format!("读取记录数据失败: {error}"))
                    })?);
            }
            "files" => {
                let file_name = field.file_name().unwrap_or("unknown").to_string();
                let file_type = field
                    .content_type()
                    .unwrap_or("application/octet-stream")
                    .to_string();
                let file_data = field
                    .bytes()
                    .await
                    .map_err(|error| AppError::Validation(format!("读取附件失败: {error}")))?
                    .to_vec();
                total_size = total_size.saturating_add(file_data.len());
                sample_attachment_service::validate_upload(&file_name, &file_data)?;
                attachments.push(PendingAttachment {
                    file_name,
                    file_type,
                    file_data,
                });
            }
            _ => {}
        }
    }
    if total_size > 100 * 1024 * 1024 {
        return Err(AppError::Validation(
            "本次提交的附件总大小不能超过 100MB".into(),
        ));
    }

    let payload_json = payload_json.ok_or_else(|| AppError::Validation("缺少记录数据".into()))?;
    let mut body: SampleInfoCreate = serde_json::from_str(&payload_json)
        .map_err(|error| AppError::Validation(format!("记录数据格式无效: {error}")))?;
    body.user_name = ctx.user.username.clone();

    let record = sample_info_repo::create(&pool, &body, &ctx.user.username)?;
    let attachment_dir = AppConfig::load().attachments_dir();
    let mut saved_files = Vec::with_capacity(attachments.len());
    for attachment in attachments {
        match sample_attachment_service::save_upload(
            &pool,
            &attachment_dir,
            record.id,
            &attachment.file_name,
            &attachment.file_type,
            &attachment.file_data,
        ) {
            Ok((_, path)) => saved_files.push(path),
            Err(error) => {
                rollback_failed_attachment_submission(&pool, record.id, &saved_files);
                return Err(error);
            }
        }
    }

    notification_service::enqueue_sample_submitted(&pool, &record)?;
    let notification_pool = pool.clone();
    tokio::task::spawn_blocking(move || {
        if let Err(error) = notification_service::process_pending(&notification_pool, 20) {
            tracing::warn!("sample submission notification processing failed: {error}");
        }
    });
    Ok(Json(ApiResponse::ok(record)))
}

async fn update(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<SampleInfoUpdate>,
) -> Result<Json<ApiResponse<SampleInfoResponse>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    let existing = sample_info_repo::get_by_id(&pool, id)?;
    if !ctx.is_system_admin() && existing.created_by_user_id != Some(ctx.user.id) {
        return Err(AppError::Forbidden("只能修改本人提交的样品信息".into()));
    }
    if existing.sampled_at.is_some() {
        return Err(AppError::Forbidden("该记录已取样，不可修改".into()));
    }
    let record = sample_info_repo::update(&pool, id, &body, &ctx.user.username)?;
    Ok(Json(ApiResponse::ok(record)))
}

async fn soft_delete(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(body): Query<DeleteReasonRequest>,
) -> Result<Json<ApiResponse<()>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    let existing = sample_info_repo::get_by_id(&pool, id)?;
    if !ctx.is_system_admin() && existing.created_by_user_id != Some(ctx.user.id) {
        return Err(AppError::Forbidden("只能删除本人提交的样品信息".into()));
    }
    let reason = body
        .reason
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("用户删除");
    sample_info_repo::soft_delete(&pool, id, &ctx.user.username, reason)?;
    Ok(Json(ApiResponse::ok_msg("删除成功")))
}

async fn restore(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<SampleInfoResponse>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    let existing = sample_info_repo::get_by_id(&pool, id)?;
    let can_restore = ctx.is_system_admin()
        || ctx.is_analysis_leader()
        || existing.created_by_user_id == Some(ctx.user.id)
        || (ctx.is_rd_leader()
            && existing.group_id.is_some()
            && existing.group_id == ctx.user.group_id);
    if !can_restore {
        return Err(AppError::Forbidden("只能恢复本人提交的样品信息".into()));
    }
    Ok(Json(ApiResponse::ok(sample_info_repo::restore(
        &pool,
        id,
        &ctx.user.username,
    )?)))
}

async fn update_status(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<SampleInfoStatusUpdate>,
) -> Result<Json<ApiResponse<SampleInfoResponse>>> {
    let user_name = ensure_status_permission(&pool, &headers, &body.status)?;
    let record = sample_info_repo::update_status(&pool, id, &body.status, &user_name)?;
    Ok(Json(ApiResponse::ok(record)))
}

async fn sample(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<SampleInfoResponse>>> {
    let user_name = ensure_status_permission(&pool, &headers, "待检测")?;
    Ok(Json(ApiResponse::ok(sample_info_repo::update_status(
        &pool,
        id,
        "待检测",
        &user_name,
    )?)))
}

async fn complete(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<SampleInfoResponse>>> {
    let user_name = ensure_status_permission(&pool, &headers, "已检测")?;
    Ok(Json(ApiResponse::ok(sample_info_repo::update_status(
        &pool,
        id,
        "已检测",
        &user_name,
    )?)))
}

// ========== 独立统计（不接分析检测 /stats） ==========

#[derive(Serialize)]
pub struct NameCount {
    pub name: String,
    pub count: i64,
}

#[derive(Serialize)]
pub struct TypeCount {
    pub type_key: String,
    pub label: String,
    pub count: i64,
}

#[derive(Serialize)]
pub struct MonthCount {
    pub month: String,
    pub count: i64,
}

#[derive(Serialize)]
pub struct SampleInfoStats {
    pub total: i64,
    pub by_status: Vec<NameCount>,
    pub by_type: Vec<TypeCount>,
    pub by_lab: Vec<NameCount>,
    pub by_project: Vec<NameCount>,
    pub by_user: Vec<NameCount>,
    pub by_month: Vec<MonthCount>,
}

fn stats_where(q: &SampleInfoQuery, table_alias: Option<&str>) -> (String, Vec<String>) {
    let prefix = table_alias
        .map(|value| format!("{value}."))
        .unwrap_or_default();
    let mut clauses: Vec<String> = vec![format!("{}deleted_at IS NULL", prefix)];
    let mut params: Vec<String> = vec![];
    if let Some(tk) = &q.type_key {
        if !tk.is_empty() {
            let i = params.len() + 1;
            clauses.push(format!("{}type_key=?{}", prefix, i));
            params.push(tk.clone());
        }
    }
    if let Some(s) = &q.status {
        if !s.is_empty() && s != "全部" {
            let i = params.len() + 1;
            clauses.push(format!("{}status=?{}", prefix, i));
            params.push(s.clone());
        }
    }
    if let Some(st) = &q.start {
        let i = params.len() + 1;
        clauses.push(format!("{}submitted_at>=?{}", prefix, i));
        params.push(st.clone());
    }
    if let Some(e) = &q.end {
        let i = params.len() + 1;
        clauses.push(format!("{}submitted_at<=?{}", prefix, i));
        params.push(format!("{}T23:59:59", e));
    }
    if let Some(user_name) = &q.user_name {
        let i = params.len() + 1;
        clauses.push(format!("{}user_name=?{}", prefix, i));
        params.push(user_name.clone());
    }
    if let Some(user_id) = q.created_by_user_id {
        let i = params.len() + 1;
        clauses.push(format!("{}created_by_user_id=?{}", prefix, i));
        params.push(user_id.to_string());
    }
    if let Some(lab_name) = &q.lab_name {
        let i = params.len() + 1;
        clauses.push(format!("{}lab_name=?{}", prefix, i));
        params.push(lab_name.clone());
    }
    if let Some(group_id) = q.group_id {
        let i = params.len() + 1;
        clauses.push(format!("{}group_id=?{}", prefix, i));
        params.push(group_id.to_string());
    }
    (clauses.join(" AND "), params)
}

async fn stats(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(mut q): Query<SampleInfoQuery>,
) -> Result<Json<ApiResponse<SampleInfoStats>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "stats:portal:sample-info")?;
    if ctx.is_system_admin() || ctx.has_permission("stats:sample-info:view-all") {
        q.created_by_user_id = None;
        q.group_id = None;
    } else if ctx.has_permission("stats:sample-info:view-lab") {
        q.created_by_user_id = None;
        q.group_id = Some(
            ctx.user
                .group_id
                .ok_or_else(|| AppError::Validation("当前统计角色尚未设置所属实验室".into()))?,
        );
    } else {
        q.created_by_user_id = Some(ctx.user.id);
    }
    let conn = pool.get()?;
    let (wc, params) = stats_where(&q, None);
    let (wc_sir, _) = stats_where(&q, Some("sir"));
    let param_refs: Vec<&dyn postgres_compat::types::ToSql> = params
        .iter()
        .map(|s| s as &dyn postgres_compat::types::ToSql)
        .collect();

    let total: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM sample_info_records WHERE {}", wc),
        postgres_compat::params_from_iter(param_refs.iter()),
        |r| r.get(0),
    )?;

    let mut q_status = conn.prepare(
        &format!("SELECT status, COUNT(*) FROM sample_info_records WHERE {} GROUP BY status ORDER BY COUNT(*) DESC", wc),
    )?;
    let by_status: Vec<NameCount> = q_status
        .query_map(
            postgres_compat::params_from_iter(param_refs.iter()),
            |row| {
                Ok(NameCount {
                    name: row.get(0)?,
                    count: row.get(1)?,
                })
            },
        )?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut q_type = conn.prepare(
        &format!("SELECT COALESCE(sit.label, sir.detection_type) AS t, sir.type_key, COUNT(*) \
                  FROM sample_info_records sir LEFT JOIN sample_info_types sit ON sit.type_key = sir.type_key \
                  WHERE {} GROUP BY COALESCE(sit.label, sir.detection_type), sir.type_key ORDER BY COUNT(*) DESC", wc_sir),
    )?;
    let by_type: Vec<TypeCount> = q_type
        .query_map(
            postgres_compat::params_from_iter(param_refs.iter()),
            |row| {
                Ok(TypeCount {
                    label: row.get(0)?,
                    type_key: row.get::<_, String>(1).unwrap_or_default(),
                    count: row.get(2)?,
                })
            },
        )?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut q_lab = conn.prepare(
        &format!("SELECT lab_name, COUNT(*) FROM sample_info_records WHERE {} GROUP BY lab_name ORDER BY COUNT(*) DESC", wc),
    )?;
    let by_lab: Vec<NameCount> = q_lab
        .query_map(
            postgres_compat::params_from_iter(param_refs.iter()),
            |row| {
                Ok(NameCount {
                    name: row.get(0)?,
                    count: row.get(1)?,
                })
            },
        )?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut q_proj = conn.prepare(
        &format!("SELECT project_name, COUNT(*) FROM sample_info_records WHERE {} GROUP BY project_name ORDER BY COUNT(*) DESC", wc),
    )?;
    let by_project: Vec<NameCount> = q_proj
        .query_map(
            postgres_compat::params_from_iter(param_refs.iter()),
            |row| {
                Ok(NameCount {
                    name: row.get(0)?,
                    count: row.get(1)?,
                })
            },
        )?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut q_user = conn.prepare(
        &format!("SELECT COALESCE(MAX(u.username),MAX(sir.user_name)), COUNT(*)
                  FROM sample_info_records sir LEFT JOIN users u ON u.id=sir.created_by_user_id
                  WHERE {} GROUP BY COALESCE(CAST(sir.created_by_user_id AS TEXT),'legacy:' || sir.user_name)
                  ORDER BY COUNT(*) DESC", wc_sir),
    )?;
    let by_user: Vec<NameCount> = q_user
        .query_map(
            postgres_compat::params_from_iter(param_refs.iter()),
            |row| {
                Ok(NameCount {
                    name: row.get(0)?,
                    count: row.get(1)?,
                })
            },
        )?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut q_month = conn.prepare(
        &format!("SELECT LEFT(submitted_at, 7) AS m, COUNT(*) FROM sample_info_records WHERE {} GROUP BY LEFT(submitted_at, 7) ORDER BY LEFT(submitted_at, 7) ASC", wc),
    )?;
    let by_month: Vec<MonthCount> = q_month
        .query_map(
            postgres_compat::params_from_iter(param_refs.iter()),
            |row| {
                Ok(MonthCount {
                    month: row.get::<_, String>(0).unwrap_or_default(),
                    count: row.get(1)?,
                })
            },
        )?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(Json(ApiResponse::ok(SampleInfoStats {
        total,
        by_status,
        by_type,
        by_lab,
        by_project,
        by_user,
        by_month,
    })))
}
