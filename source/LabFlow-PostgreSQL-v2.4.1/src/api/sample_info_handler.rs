use crate::config::AppConfig;
use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::sample_info::{
    SampleInfoCreate, SampleInfoFilterOptions, SampleInfoFilterQuery, SampleInfoQuery,
    SampleInfoResponse, SampleInfoScopeFilter, SampleInfoStatusUpdate, SampleInfoUpdate,
};
use crate::models::trash::DeleteReasonRequest;
use crate::models::{ApiResponse, PaginatedResponse};
use crate::repo::sample_info_repo;
use crate::service::authz_service::{self, AuthContext};
use crate::service::notification_service;
use crate::service::sample_attachment_service;
use crate::service::sample_workload_mapping::{self, SampleWorkloadMapping};
use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use postgres_compat::OptionalExtension;
use serde::Serialize;

#[derive(Serialize)]
struct SampleInfoWorkloadInstrument {
    method_id: i64,
    instrument_id: i64,
    instrument: String,
    instrument_name: String,
    instrument_code: String,
    instrument_type: String,
    method_name: String,
    method_short_name: String,
    method_code: String,
    method_types: Vec<String>,
    coefficient: f64,
    multiplier: f64,
    /// 推荐来源：管理员映射、记录填写值、检测类型或其他可用方法。
    match_source: String,
}

#[derive(Serialize)]
struct SampleInfoWorkloadPreview {
    source_record_id: i64,
    operator_username: String,
    department: String,
    laboratory: String,
    project: String,
    detection_type: String,
    quantity: i32,
    recorded_quantity: i32,
    remaining_quantity: i32,
    coefficient: f64,
    instruments: Vec<SampleInfoWorkloadInstrument>,
    recorded: bool,
    /// v2.3.13: 与研发送样保持一致的单一方法/仪器摘要，供弹窗直接展示。
    method: Option<String>,
    instrument: Option<String>,
    multiplier: Option<f64>,
    /// v2.3.13: 当没有候选方法/仪器时说明原因，供前端给出可操作的提示。
    reason: Option<String>,
    /// 自定义录入独立于方法库候选，但确认时仍必须选择有效核算目标。
    allow_custom: bool,
    source_summary: SampleInfoWorkloadSourceSummary,
    targets: Vec<SampleInfoWorkloadTarget>,
    selected_project_id: Option<i64>,
    selected_group_id: Option<i64>,
}

#[derive(Serialize)]
struct SampleInfoWorkloadSourceSummary {
    business_no: String,
    department: String,
    laboratory: String,
    project: String,
    detection_type: String,
    type_key: String,
    quantity: i64,
    project_id: Option<i64>,
    group_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
struct SampleInfoWorkloadTarget {
    project_id: i64,
    project_name: String,
    group_id: i64,
    laboratory: String,
    division_id: Option<i64>,
    department: String,
    coefficient: f64,
    #[serde(skip_serializing)]
    legacy_source: bool,
}

#[derive(Default, serde::Deserialize)]
struct SampleInfoWorkloadQuery {
    project_id: Option<i64>,
    group_id: Option<i64>,
}

#[derive(serde::Deserialize)]
struct SampleInfoWorkloadConfirm {
    #[serde(default)]
    project_id: Option<i64>,
    #[serde(default)]
    group_id: Option<i64>,
    /// v2.3.14: 走方法库时传选中的方法 id；自定义录入时留空。
    #[serde(default)]
    method_id: Option<i64>,
    quantity: i32,
    multiplier: f64,
    notes: Option<String>,
    #[serde(default)]
    recorded_at: Option<String>,
    /// 自定义录入的方法名称（必填）。
    #[serde(default)]
    custom_method_name: Option<String>,
    /// 自定义录入的仪器名称（可空）。
    #[serde(default)]
    custom_instrument_name: Option<String>,
}

fn effective_submitter_name(requested: &str, login_username: &str) -> String {
    let requested = requested.trim();
    if requested.is_empty() {
        login_username.to_string()
    } else {
        requested.to_string()
    }
}

fn ensure_status_permission(ctx: &AuthContext, target_status: &str) -> Result<String> {
    let required = match target_status {
        "待检测" => "sample-info:collect",
        "已检测" => "sample-info:complete",
        _ => return Err(AppError::Validation("无效的样品状态操作".into())),
    };
    if !ctx.has_permission(required) {
        return Err(AppError::Forbidden("无权执行该样品状态操作".into()));
    }
    Ok(ctx.user.username.clone())
}

/// v2.3.17: 工作量录入的准入条件只要求“已取样且未被撤回/退回”。
/// 之前限定 `status == "待检测"`，记录一旦点过“完成检测”（状态变为“已检测”），
/// 列表按钮和两个工作量接口会同时失效，无法再补录工作量。
fn ensure_workload_editable(status: &str, sampled: bool) -> Result<()> {
    if !sampled {
        return Err(AppError::Validation("请先完成取样后再录入工作量".into()));
    }
    if !matches!(status, "待检测" | "已检测") {
        return Err(AppError::Validation(format!(
            "当前状态为「{}」，只有已完成取样的记录可以录入工作量",
            status
        )));
    }
    Ok(())
}

fn ensure_record_access(
    pool: &DbPool,
    ctx: &AuthContext,
    record: &SampleInfoResponse,
) -> Result<()> {
    if authz_service::sample_info_record_allowed(pool, ctx, record)? {
        Ok(())
    } else {
        Err(AppError::Forbidden("当前角色无权访问该样品记录".into()))
    }
}

fn apply_role_data_scopes(
    pool: &DbPool,
    ctx: &AuthContext,
    query: &mut SampleInfoQuery,
) -> Result<bool> {
    if let Some(filters) = authz_service::sample_info_record_scope_filters(pool, ctx)? {
        query.scope_filters = filters;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn apply_record_scope(pool: &DbPool, ctx: &AuthContext, query: &mut SampleInfoQuery) -> Result<()> {
    if ctx.is_system_admin() {
        return Ok(());
    }
    if apply_role_data_scopes(pool, ctx, query)? {
        return Ok(());
    }
    if ctx.is_rd_leader() {
        query.group_id = Some(
            ctx.user
                .group_id
                .ok_or_else(|| AppError::Validation("研发送样组长尚未设置所属实验室".into()))?,
        );
    } else {
        query.scope_filters.push(SampleInfoScopeFilter {
            execution_department: false,
            division_ids: vec![],
            type_keys: vec![],
            created_by_user_id: Some(ctx.user.id),
            business_user_id: Some(ctx.user.id),
        });
    }
    Ok(())
}

pub(super) fn apply_statistics_scope(
    pool: &DbPool,
    ctx: &AuthContext,
    query: &mut SampleInfoQuery,
) -> Result<()> {
    authz_service::require_permission(ctx, "stats:portal:sample-info")?;
    query.include_pending_ownership.get_or_insert(false);
    query.created_by_user_id = None;
    query.group_id = None;
    if !ctx.is_system_admin() && apply_role_data_scopes(pool, ctx, query)? {
        return Ok(());
    }
    if ctx.is_system_admin() || ctx.has_permission("stats:sample-info:view-all") {
        return Ok(());
    }
    if ctx.has_permission("stats:sample-info:view-lab") {
        query.group_id = Some(
            ctx.user
                .group_id
                .ok_or_else(|| AppError::Validation("当前统计角色尚未设置所属实验室".into()))?,
        );
    } else {
        query.scope_filters.push(SampleInfoScopeFilter {
            created_by_user_id: Some(ctx.user.id),
            business_user_id: Some(ctx.user.id),
            ..Default::default()
        });
    }
    Ok(())
}

pub(super) fn apply_query_scope(
    pool: &DbPool,
    ctx: &AuthContext,
    query: &mut SampleInfoQuery,
) -> Result<()> {
    if query.scope.as_deref() == Some("statistics") {
        apply_statistics_scope(pool, ctx, query)
    } else {
        authz_service::require_permission(ctx, "entry:sample-info")?;
        apply_record_scope(pool, ctx, query)
    }
}

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/sample-info", get(list).post(create))
        .route(
            "/api/sample-info/with-attachments",
            post(create_with_attachments),
        )
        .route("/api/sample-info/stats", get(stats))
        .route("/api/sample-info/filter-options", get(filter_options))
        .route(
            "/api/sample-info/:id",
            axum::routing::put(update).delete(soft_delete),
        )
        .route(
            "/api/sample-info/:id/status",
            axum::routing::put(update_status),
        )
        .route("/api/sample-info/:id/sample", axum::routing::put(sample))
        .route(
            "/api/sample-info/:id/sample-workload",
            get(sample_workload_preview).post(sample_workload_confirm),
        )
        .route(
            "/api/sample-info/:id/withdraw",
            axum::routing::put(withdraw_sample),
        )
        .route("/api/sample-info/:id/restore", axum::routing::post(restore))
        .route(
            "/api/sample-info/:id/complete",
            axum::routing::put(complete),
        )
        .route(
            "/api/sample-info/:id/return",
            axum::routing::post(return_record),
        )
        .route(
            "/api/sample-info/:id/confirm-return",
            axum::routing::post(confirm_return),
        )
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .with_state(pool)
}

async fn list(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(mut q): Query<SampleInfoQuery>,
) -> Result<Json<ApiResponse<PaginatedResponse<SampleInfoResponse>>>> {
    tokio::task::spawn_blocking(move || {
        let ctx = authz_service::authenticate(&pool, &headers)?;
        apply_query_scope(&pool, &ctx, &mut q)?;
        let (items, total) = sample_info_repo::list(&pool, &q)?;
        let page = q.page.unwrap_or(1).max(1);
        let page_size = q.page_size.unwrap_or(20).clamp(1, 500);
        Ok(Json(ApiResponse::ok(PaginatedResponse {
            items,
            total,
            page,
            page_size,
        })))
    })
    .await
    .map_err(|error| AppError::Internal(error.to_string()))?
}

async fn filter_options(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(mut q): Query<SampleInfoFilterQuery>,
) -> Result<Json<ApiResponse<SampleInfoFilterOptions>>> {
    tokio::task::spawn_blocking(move || {
        let ctx = authz_service::authenticate(&pool, &headers)?;
        apply_query_scope(&pool, &ctx, &mut q.query)?;
        let options = sample_info_repo::filter_options(
            &pool,
            &q.query,
            &q.field,
            q.search.as_deref(),
            q.limit.unwrap_or(200),
        )?;
        Ok(Json(ApiResponse::ok(options)))
    })
    .await
    .map_err(|error| AppError::Internal(error.to_string()))?
}

async fn create(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(mut body): Json<SampleInfoCreate>,
) -> Result<Json<ApiResponse<SampleInfoResponse>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "sample-info:create")?;
    body.user_name = effective_submitter_name(&body.user_name, &ctx.user.username);
    validate_sample_info_bindings(&pool, &ctx, &mut body, false)?;
    body.submitted_at = None;
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

fn validate_sample_info_bindings(
    pool: &DbPool,
    ctx: &authz_service::AuthContext,
    body: &mut SampleInfoCreate,
    allow_historical_organization: bool,
) -> Result<()> {
    // A returned draft keeps the organization that was valid when it was
    // submitted. Do not re-resolve historical lab/project master data while
    // the submitter only edits sample content; changed organization fields
    // still enter the normal validation path below.
    if allow_historical_organization {
        return Ok(());
    }
    let conn = pool.get()?;
    let group: Option<(i64, Option<i64>, String)> = conn
        .query_row(
            "SELECT g.id,g.division_id,g.name FROM project_groups g LEFT JOIN divisions d ON d.id=g.division_id WHERE (CASE WHEN CAST(?1 AS BIGINT) IS NOT NULL THEN g.id=?1 ELSE g.name=?2 END) AND g.deleted_at IS NULL AND g.show_in_sample_info=1 AND (g.division_id IS NULL OR (d.deleted_at IS NULL AND d.show_in_sample_info=1)) ORDER BY g.id LIMIT 1",
            postgres_compat::params![body.group_id, &body.lab_name],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    if (body.group_id.is_some() || !body.lab_name.trim().is_empty()) && group.is_none() {
        return Err(AppError::Validation("所选实验室不存在或已停用".into()));
    }
    let group_id = group.as_ref().map(|value| value.0);
    if let Some((_, group_division_id, group_name)) = &group {
        if body.lab_name.trim().is_empty() {
            body.lab_name = group_name.clone();
        }
        body.group_id = group_id;
        match (body.division_id, *group_division_id) {
            (Some(selected), Some(actual)) if selected != actual => {
                return Err(AppError::Validation("所选实验室不属于当前部门".into()));
            }
            (None, Some(_)) => {
                return Err(AppError::Validation("请选择实验室所属部门".into()));
            }
            _ => {}
        }
    }
    if let Some(false) =
        authz_service::sample_info_scope_allows(pool, ctx, body.division_id, &body.type_key, None)?
    {
        return Err(AppError::Forbidden(
            "当前角色无权在该部门登记此类样品信息".into(),
        ));
    }
    if !allow_historical_organization && !ctx.is_system_admin() && !ctx.is_analysis_leader() {
        if let (Some(current_group), Some(selected_group)) = (ctx.user.group_id, group_id) {
            if current_group != selected_group {
                return Err(AppError::Forbidden("只能提交所属实验室的样品信息".into()));
            }
        }
    }
    if body.project_id.is_some() || !body.project_name.trim().is_empty() {
        let project: Option<(i64, String)> = conn
            .query_row(
                "SELECT id,name FROM projects WHERE (CASE WHEN CAST(?1 AS BIGINT) IS NOT NULL THEN id=?1 ELSE name=?2 END) AND is_active=1 AND deleted_at IS NULL AND show_in_sample_info=1 ORDER BY id LIMIT 1",
                postgres_compat::params![body.project_id, &body.project_name],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let (project_id, project_name) =
            project.ok_or_else(|| AppError::Validation("所选项目不存在或已归档".into()))?;
        body.project_id = Some(project_id);
        if body.project_name.trim().is_empty() {
            body.project_name = project_name;
        }
        if let Some(selected_group) = group_id {
            let linked: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM project_lab_links WHERE project_id=?1 AND group_id=?2)",
                postgres_compat::params![project_id, selected_group],
                |row| row.get(0),
            )?;
            if !linked {
                return Err(AppError::Validation("所选项目未关联当前实验室".into()));
            }
            authz_service::require_project_execution_group(pool, project_id, selected_group)?;
        }
    }
    Ok(())
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
    authz_service::require_permission(&ctx, "sample-info:create")?;

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
    body.user_name = effective_submitter_name(&body.user_name, &ctx.user.username);
    validate_sample_info_bindings(&pool, &ctx, &mut body, false)?;
    body.submitted_at = None;

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
    Json(mut body): Json<SampleInfoUpdate>,
) -> Result<Json<ApiResponse<SampleInfoResponse>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "sample-info:edit-own")?;
    let existing = sample_info_repo::get_by_id(&pool, id)?;
    ensure_record_access(&pool, &ctx, &existing)?;
    if !ctx.is_system_admin()
        && existing.created_by_user_id != Some(ctx.user.id)
        && existing.business_user_id != Some(ctx.user.id)
    {
        return Err(AppError::Forbidden("只能修改本人提交的样品信息".into()));
    }
    if existing.sampled_at.is_some() {
        return Err(AppError::Forbidden("该记录已取样，不可修改".into()));
    }
    if existing.status == "退回待修改" {
        let organization_changed = body
            .division_id
            .is_some_and(|value| Some(value) != existing.division_id)
            || body
                .lab_name
                .as_ref()
                .is_some_and(|value| value != &existing.lab_name)
            || body
                .project_name
                .as_ref()
                .is_some_and(|value| value != &existing.project_name)
            || body
                .group_id
                .is_some_and(|value| Some(value) != existing.group_id)
            || body
                .project_id
                .is_some_and(|value| Some(value) != existing.project_id);
        if organization_changed {
            return Err(AppError::Validation(
                "退回待修改记录必须沿用原记录的所属部门、实验室和项目".into(),
            ));
        }
        // The resubmission draft inherits its organization from the returned
        // record. Ignore matching legacy-client values to avoid revalidating a
        // historical organization that is no longer selectable today.
        body.division_id = None;
        body.lab_name = None;
        body.project_name = None;
        body.group_id = None;
        body.project_id = None;
    }
    let mut candidate = SampleInfoCreate {
        batch_no: body
            .batch_no
            .clone()
            .unwrap_or_else(|| existing.batch_no.clone()),
        user_name: body
            .user_name
            .clone()
            .unwrap_or_else(|| existing.user_name.clone()),
        lab_name: body
            .lab_name
            .clone()
            .unwrap_or_else(|| existing.lab_name.clone()),
        project_name: body
            .project_name
            .clone()
            .unwrap_or_else(|| existing.project_name.clone()),
        project_id: body.project_id.or_else(|| {
            (!body
                .project_name
                .as_ref()
                .is_some_and(|name| name != &existing.project_name))
            .then_some(existing.project_id)
            .flatten()
        }),
        group_id: body.group_id.or_else(|| {
            (!body
                .lab_name
                .as_ref()
                .is_some_and(|name| name != &existing.lab_name))
            .then_some(existing.group_id)
            .flatten()
        }),
        submitted_at: body
            .submitted_at
            .clone()
            .or_else(|| Some(existing.submitted_at.clone())),
        detection_date: body
            .detection_date
            .clone()
            .or_else(|| Some(existing.detection_date.clone())),
        main_components: body
            .main_components
            .clone()
            .unwrap_or_else(|| existing.main_components.clone()),
        detection_type: existing.detection_type.clone(),
        type_key: existing.type_key.clone(),
        division_id: body.division_id.or(existing.division_id),
        quantity: body.quantity.unwrap_or(existing.quantity),
        notes: body.notes.clone().or_else(|| Some(existing.notes.clone())),
        extra_fields: body.extra_fields.clone(),
    };
    // A returned draft has already passed organization validation when it was submitted.
    // Keep that historical organization valid while the original submitter edits content;
    // changing the department, lab, or project still uses the normal validation path.
    let organization_unchanged = candidate.division_id == existing.division_id
        && candidate.lab_name == existing.lab_name
        && candidate.project_name == existing.project_name
        && candidate.group_id == existing.group_id
        && candidate.project_id == existing.project_id;
    let keep_historical_organization = organization_unchanged
        && (existing.status == "退回待修改" || existing.business_user_id == Some(ctx.user.id));
    validate_sample_info_bindings(&pool, &ctx, &mut candidate, keep_historical_organization)?;
    if !keep_historical_organization {
        body.project_id = candidate.project_id;
        body.group_id = candidate.group_id;
    }
    body.submitted_at = None;
    let record = sample_info_repo::update(&pool, id, &body, &ctx.user.username)?;
    if existing.status == "退回待修改" && record.status == "待取样" {
        if let Err(error) = notification_service::enqueue_sample_submitted(&pool, &record) {
            tracing::warn!(record_id = id, %error, "样品登记退回后重新提交通知入队失败");
        }
    }
    Ok(Json(ApiResponse::ok(record)))
}

async fn soft_delete(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(body): Query<DeleteReasonRequest>,
) -> Result<Json<ApiResponse<()>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "sample-info:edit-own")?;
    let existing = sample_info_repo::get_by_id(&pool, id)?;
    ensure_record_access(&pool, &ctx, &existing)?;
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
    authz_service::require_permission(&ctx, "entry:sample-info")?;
    let existing = sample_info_repo::get_by_id(&pool, id)?;
    ensure_record_access(&pool, &ctx, &existing)?;
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
    tokio::task::spawn_blocking(move || {
        let ctx = authz_service::authenticate(&pool, &headers)?;
        let existing = sample_info_repo::get_by_id(&pool, id)?;
        ensure_record_access(&pool, &ctx, &existing)?;
        let user_name = ensure_status_permission(&ctx, &body.status)?;
        let record = sample_info_repo::update_status(&pool, id, &body.status, &user_name)?;
        Ok(Json(ApiResponse::ok(record)))
    })
    .await
    .map_err(|error| AppError::Internal(error.to_string()))?
}

async fn sample(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<SampleInfoResponse>>> {
    tokio::task::spawn_blocking(move || {
        let ctx = authz_service::authenticate(&pool, &headers)?;
        let existing = sample_info_repo::get_by_id(&pool, id)?;
        ensure_record_access(&pool, &ctx, &existing)?;
        let user_name = ensure_status_permission(&ctx, "待检测")?;
        Ok(Json(ApiResponse::ok(sample_info_repo::update_status(
            &pool,
            id,
            "待检测",
            &user_name,
        )?)))
    })
    .await
    .map_err(|error| AppError::Internal(error.to_string()))?
}

#[derive(Debug)]
struct WorkloadMethodCandidate {
    method_id: i64,
    instrument_id: i64,
    instrument: String,
    method_name: String,
    method_short_name: String,
    instrument_code: String,
    instrument_name: String,
    instrument_type: String,
    method_code: String,
    coefficient: f64,
    multiplier: f64,
    method_types: Vec<String>,
    method_type_ids: Vec<i64>,
}

fn configured_workload_options(source: &str) -> Vec<String> {
    let source = source.trim();
    if source.is_empty() {
        return Vec::new();
    }
    if let Ok(values) = serde_json::from_str::<Vec<String>>(source) {
        return values
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect();
    }
    source
        .split(|ch| matches!(ch, ',' | '，' | ';' | '；' | '\n'))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn workload_record_values(
    conn: &postgres_compat::Connection,
    record: &SampleInfoResponse,
    mapping: &SampleWorkloadMapping,
) -> Result<(Vec<String>, std::collections::BTreeSet<i64>)> {
    let fields = record
        .extra_fields
        .as_ref()
        .and_then(|value| value.as_object());
    let source_fields =
        serde_json::to_value(record).map_err(|error| AppError::Internal(error.to_string()))?;
    // 下拉选项保留历史推荐逻辑；文本列只有管理员明确映射时才参与，不能扫描备注等自由文本。
    let mut stmt = conn.prepare(
        "SELECT c.field_key,COALESCE(c.options,''),c.data_type
         FROM sample_info_columns c
         WHERE c.deleted_at IS NULL AND c.is_active=1
           AND c.data_type IN ('text','textarea','select','multi_select','multiselect')
           AND COALESCE((SELECT v.is_visible FROM sample_info_column_visibility v
                         WHERE v.column_id=c.id AND v.type_key=?1),
                        CASE WHEN c.type_key=?1 THEN 1 ELSE 0 END)=1",
    )?;
    let rows = stmt.query_map(postgres_compat::params![&record.type_key], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    let mut values = Vec::new();
    let mut mapped_methods = std::collections::BTreeSet::new();
    for row in rows {
        let (field_key, options, data_type) = row?;
        let explicitly_mapped = mapping
            .field_mappings
            .iter()
            .any(|item| item.type_key == record.type_key && item.field_key == field_key);
        let value = fields
            .and_then(|fields| fields.get(&field_key))
            .or_else(|| {
                explicitly_mapped
                    .then(|| source_fields.get(&field_key))
                    .flatten()
            });
        let Some(value) = value else {
            continue;
        };
        let configured = configured_workload_options(&options);
        let selected: Vec<&str> = match value {
            serde_json::Value::String(text) => vec![text.as_str()],
            serde_json::Value::Array(items) => {
                items.iter().filter_map(|item| item.as_str()).collect()
            }
            _ => Vec::new(),
        };
        for text in selected {
            let text = text.trim();
            if matches!(
                data_type.as_str(),
                "select" | "multi_select" | "multiselect"
            ) && configured.iter().any(|option| option.trim() == text)
            {
                values.push(text.to_string());
            }
            mapped_methods.extend(
                mapping
                    .field_mappings
                    .iter()
                    .filter(|item| {
                        item.type_key == record.type_key
                            && item.field_key == field_key
                            && item.value.to_lowercase() == text.to_lowercase()
                    })
                    .map(|item| item.method_id),
            );
        }
    }
    Ok((values, mapped_methods))
}

fn normalized_workload_text(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|ch| {
            !ch.is_whitespace() && !matches!(ch, '-' | '_' | '·' | '/' | '（' | '）' | '(' | ')')
        })
        .flat_map(char::to_lowercase)
        .collect()
}

fn record_value_matches_candidate(value: &str, candidate: &WorkloadMethodCandidate) -> bool {
    let value = normalized_workload_text(value);
    if value.is_empty() {
        return false;
    }
    let method = normalized_workload_text(&candidate.method_name);
    let short_method = normalized_workload_text(&candidate.method_short_name);
    let instrument_code = normalized_workload_text(if candidate.instrument_code.is_empty() {
        &candidate.instrument_name
    } else {
        &candidate.instrument_code
    });
    let common_method = short_method.strip_prefix("通用").unwrap_or(&short_method);
    method == value
        || short_method == value
        || instrument_code == value
        || common_method == value
        || (value.chars().count() >= 3 && short_method.starts_with(&value))
}

fn workload_method_type_names(type_key: &str, detection_type: &str) -> Vec<String> {
    let mut names: Vec<String> = match type_key.trim().to_ascii_lowercase().as_str() {
        "icp" => vec!["ICP".into()],
        "thermal" => vec!["热分析".into()],
        "mass" => vec!["质谱".into()],
        "zzzb" => vec!["杂质制备".into()],
        "ffkf" => vec!["方法开发".into()],
        "other" => vec!["其它".into(), "其他".into()],
        _ => Vec::new(),
    };
    let detection_type = detection_type.trim();
    if !detection_type.is_empty() && !names.iter().any(|name| name == detection_type) {
        names.push(detection_type.to_string());
    }
    names
}

fn source_workload_group_id(
    conn: &postgres_compat::Connection,
    record: &SampleInfoResponse,
) -> Result<Option<i64>> {
    if let Some(id) = record.execution_group_id.or(record.group_id) {
        // A stored ID remains authoritative after a rename; never widen it with an OR name match.
        return Ok(Some(id));
    }
    conn.query_row(
        "SELECT id FROM project_groups WHERE name=?1 AND deleted_at IS NULL",
        [&record.lab_name],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
}

fn workload_target_allowed(
    target: &SampleInfoWorkloadTarget,
    is_admin: bool,
    source_group_id: Option<i64>,
    work_division_ids: &[i64],
) -> bool {
    is_admin
        || source_group_id == Some(target.group_id)
        || target
            .division_id
            .is_some_and(|id| work_division_ids.contains(&id))
}

fn workload_targets(
    pool: &DbPool,
    ctx: &AuthContext,
    source: &SampleInfoResponse,
    source_group_id: Option<i64>,
) -> Result<Vec<SampleInfoWorkloadTarget>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT p.id,p.name,g.id,g.name,d.id,d.name,p.coefficient
         FROM projects p JOIN project_lab_links pll ON pll.project_id=p.id
         JOIN project_groups g ON g.id=pll.group_id AND g.deleted_at IS NULL
         JOIN divisions d ON d.id=g.division_id AND d.deleted_at IS NULL AND d.is_active=1
         WHERE p.deleted_at IS NULL AND p.is_active=1 AND COALESCE(p.project_status,'ongoing')='ongoing'
           AND (COALESCE(p.project_division_id,(SELECT division_id FROM project_groups WHERE id=p.group_id))=d.id
                OR EXISTS(SELECT 1 FROM project_collaboration_divisions pcd WHERE pcd.project_id=p.id AND pcd.division_id=d.id))
         ORDER BY d.sort_order,d.id,g.sort_order,g.id,p.name,p.id",
    )?;
    let mut candidates = stmt
        .query_map([], |row| {
            Ok(SampleInfoWorkloadTarget {
                project_id: row.get(0)?,
                project_name: row.get(1)?,
                group_id: row.get(2)?,
                laboratory: row.get(3)?,
                division_id: row.get(4)?,
                department: row.get(5)?,
                coefficient: row.get(6)?,
                legacy_source: false,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    // Preserve the previously authorized source combination when historical organization
    // configuration is incomplete. This exception cannot recommend another project or lab.
    if let Some(group_id) = source_group_id {
        let legacy: Option<SampleInfoWorkloadTarget> = conn
            .query_row(
                "SELECT p.id,p.name,g.id,g.name,g.division_id,COALESCE(d.name,''),p.coefficient
             FROM projects p JOIN project_lab_links pll ON pll.project_id=p.id
             JOIN project_groups g ON g.id=pll.group_id AND g.deleted_at IS NULL
             LEFT JOIN divisions d ON d.id=g.division_id
             WHERE p.deleted_at IS NULL AND p.is_active=1 AND g.id=?3
               AND (CASE WHEN CAST(?1 AS BIGINT) IS NOT NULL THEN p.id=?1 ELSE p.name=?2 END)",
                postgres_compat::params![source.project_id, &source.project_name, group_id],
                |row| {
                    Ok(SampleInfoWorkloadTarget {
                        project_id: row.get(0)?,
                        project_name: row.get(1)?,
                        group_id: row.get(2)?,
                        laboratory: row.get(3)?,
                        division_id: row.get(4)?,
                        department: row.get(5)?,
                        coefficient: row.get(6)?,
                        legacy_source: true,
                    })
                },
            )
            .optional()?;
        if let Some(legacy) = legacy {
            if !candidates.iter().any(|item| {
                item.project_id == legacy.project_id && item.group_id == legacy.group_id
            }) {
                candidates.push(legacy);
            }
        }
    }
    let work_division_ids =
        authz_service::work_allowed_division_ids(pool, ctx)?.unwrap_or_default();
    // Source access has already been checked. Existing sample-only roles keep their source lab;
    // only an explicit analysis scope can add other labs. There is no new portal permission gate.
    Ok(candidates
        .into_iter()
        .filter(|target| {
            workload_target_allowed(
                target,
                ctx.is_system_admin(),
                source_group_id,
                &work_division_ids,
            )
        })
        .collect())
}

fn select_workload_target<'a>(
    targets: &'a [SampleInfoWorkloadTarget],
    source: &SampleInfoWorkloadSourceSummary,
    mapping: &SampleWorkloadMapping,
    query: &SampleInfoWorkloadQuery,
) -> Result<Option<&'a SampleInfoWorkloadTarget>> {
    let unique = |items: Vec<&'a SampleInfoWorkloadTarget>| (items.len() == 1).then(|| items[0]);
    if query.project_id.is_some() || query.group_id.is_some() {
        let selected: Vec<_> = targets
            .iter()
            .filter(|target| {
                query.project_id.is_none_or(|id| id == target.project_id)
                    && query.group_id.is_none_or(|id| id == target.group_id)
            })
            .collect();
        return unique(selected).map(Some).ok_or_else(|| {
            AppError::Validation("请选择有权限且有效的唯一项目和实验室组合".into())
        });
    }
    let source_lab = |target: &&SampleInfoWorkloadTarget| match source.group_id {
        Some(id) => target.group_id == id,
        None => !source.laboratory.trim().is_empty() && target.laboratory == source.laboratory,
    };
    if let Some(id) = source.project_id {
        return Ok(unique(
            targets
                .iter()
                .filter(source_lab)
                .filter(|target| target.project_id == id)
                .collect(),
        ));
    }
    let canonical: Vec<_> = targets
        .iter()
        .filter(source_lab)
        .filter(|target| !source.project.trim().is_empty() && target.project_name == source.project)
        .collect();
    if let Some(target) = unique(canonical) {
        return Ok(Some(target));
    }
    // A configured alias recommends only one currently authorized target. It does not mutate the source.
    let aliases: Vec<_> = targets
        .iter()
        .filter(source_lab)
        .filter(|target| {
            mapping.project_aliases.iter().any(|alias| {
                alias.alias.to_lowercase() == source.project.trim().to_lowercase()
                    && alias.project_id == target.project_id
                    && alias.group_id.is_none_or(|id| id == target.group_id)
            })
        })
        .collect();
    Ok(unique(aliases))
}

fn workload_instruments(
    conn: &postgres_compat::Connection,
    record: &SampleInfoResponse,
    target: &SampleInfoWorkloadTarget,
    mapping: &SampleWorkloadMapping,
) -> Result<Vec<SampleInfoWorkloadInstrument>> {
    // 样品工作量允许选择全库有效方法；核算目标授权由 workload_targets 独立校验。
    // 项目关联、通用方法部门配置与页面显示开关不缩小此弹窗的方法候选。
    let mut stmt = conn.prepare(
        "SELECT DISTINCT m.id,
                COALESCE(i.id,0),
                CASE WHEN COALESCE(i.id,0)=0
                     THEN COALESCE(NULLIF(m.full_name,''),m.name)
                     ELSE COALESCE(NULLIF(i.name,''),'未命名仪器') || ' · ' || COALESCE(NULLIF(m.full_name,''),m.name)
                END,
                COALESCE(m.coefficient,1.0),
                COALESCE(m.multiplier,1.0),
                COALESCE(NULLIF(m.full_name,''),m.name),
                m.name,
                COALESCE(i.code,''),
                COALESCE((SELECT string_agg(DISTINCT mt.name, ',')
                          FROM method_type_links mtl
                          JOIN method_types mt ON mt.id=mtl.method_type_id AND mt.deleted_at IS NULL
                          WHERE mtl.method_id=m.id),''),
                COALESCE((SELECT string_agg(DISTINCT CAST(mt.id AS TEXT), ',')
                          FROM method_type_links mtl
                          JOIN method_types mt ON mt.id=mtl.method_type_id AND mt.deleted_at IS NULL
                          WHERE mtl.method_id=m.id),''),
                COALESCE(i.name,''),
                COALESCE(i.instrument_type,''),
                COALESCE(m.method_code,'')
         FROM projects p
         JOIN methods m ON m.deleted_at IS NULL AND m.is_active=1
         LEFT JOIN instruments i ON i.id=m.instrument_id AND i.deleted_at IS NULL AND i.is_active=1
         WHERE p.id=?1
           AND p.deleted_at IS NULL AND p.is_active=1
           AND (m.instrument_id IS NULL OR i.id IS NOT NULL)
         ORDER BY m.id",
    )?;
    let rows = stmt.query_map([target.project_id], |row| {
        let types = row.get::<_, String>(8).unwrap_or_default();
        Ok(WorkloadMethodCandidate {
            method_id: row.get(0)?,
            instrument_id: row.get(1)?,
            instrument: row.get(2)?,
            coefficient: row.get::<_, f64>(3).unwrap_or(1.0),
            multiplier: row.get::<_, f64>(4).unwrap_or(1.0),
            method_name: row.get(5)?,
            method_short_name: row.get(6)?,
            instrument_code: row.get(7)?,
            instrument_name: row.get(10)?,
            instrument_type: row.get(11)?,
            method_code: row.get(12)?,
            method_types: types
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect(),
            method_type_ids: row
                .get::<_, String>(9)?
                .split(',')
                .filter_map(|value| value.parse().ok())
                .collect(),
        })
    })?;
    let mut candidates = rows.collect::<std::result::Result<Vec<_>, _>>()?;
    candidates.retain(|candidate| {
        crate::repo::record_repo::validate_workload_coefficient(candidate.coefficient).is_ok()
    });
    let (record_values, mapped_methods) = workload_record_values(conn, record, mapping)?;
    let type_names = workload_method_type_names(&record.type_key, &record.detection_type);
    let type_ids: Vec<i64> = mapping
        .type_mappings
        .iter()
        .filter(|item| item.type_key == record.type_key)
        .flat_map(|item| item.method_type_ids.iter().copied())
        .collect();
    Ok(rank_workload_candidates(
        candidates,
        &record_values,
        &mapped_methods,
        &type_names,
        &type_ids,
    ))
}

fn rank_workload_candidates(
    candidates: Vec<WorkloadMethodCandidate>,
    record_values: &[String],
    mapped_methods: &std::collections::BTreeSet<i64>,
    type_names: &[String],
    type_ids: &[i64],
) -> Vec<SampleInfoWorkloadInstrument> {
    let mut ranked: Vec<_> = candidates
        .into_iter()
        .map(|candidate| {
            let (rank, match_source) = if mapped_methods.contains(&candidate.method_id) {
                (0, "mapping")
            } else if record_values
                .iter()
                .any(|value| record_value_matches_candidate(value, &candidate))
            {
                (1, "record")
            } else if candidate
                .method_type_ids
                .iter()
                .any(|id| type_ids.contains(id))
                || candidate.method_types.iter().any(|method_type| {
                    type_names
                        .iter()
                        .any(|name| method_type.eq_ignore_ascii_case(name))
                })
            {
                (2, "type")
            } else {
                (3, "available")
            };
            (
                rank,
                SampleInfoWorkloadInstrument {
                    method_id: candidate.method_id,
                    instrument_id: candidate.instrument_id,
                    instrument: candidate.instrument,
                    instrument_name: candidate.instrument_name,
                    instrument_code: candidate.instrument_code,
                    instrument_type: candidate.instrument_type,
                    method_name: candidate.method_name,
                    method_short_name: candidate.method_short_name,
                    method_code: candidate.method_code,
                    method_types: candidate.method_types,
                    coefficient: candidate.coefficient,
                    multiplier: candidate.multiplier,
                    match_source: match_source.into(),
                },
            )
        })
        .collect();
    ranked.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then(left.1.instrument.cmp(&right.1.instrument))
            .then(left.1.method_id.cmp(&right.1.method_id))
    });
    ranked.into_iter().map(|(_, candidate)| candidate).collect()
}

fn source_workload_summary(
    conn: &postgres_compat::Connection,
    record: &SampleInfoResponse,
) -> Result<SampleInfoWorkloadSourceSummary> {
    Ok(SampleInfoWorkloadSourceSummary {
        business_no: record.business_no.clone(),
        department: if record.execution_division_name_snapshot.is_empty() {
            record.division_name.clone().unwrap_or_default()
        } else {
            record.execution_division_name_snapshot.clone()
        },
        laboratory: record.lab_name.clone(),
        project: record.project_name.clone(),
        detection_type: record.detection_type.clone(),
        type_key: record.type_key.clone(),
        quantity: record.quantity,
        project_id: record.project_id,
        group_id: source_workload_group_id(conn, record)?,
    })
}

fn workload_source_quantity(quantity: i64) -> Result<i32> {
    i32::try_from(quantity)
        .map_err(|_| AppError::Validation("样品数量超出工作量可录入范围，请联系管理员核对".into()))
}

async fn sample_workload_preview(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(query): Query<SampleInfoWorkloadQuery>,
) -> Result<Json<ApiResponse<SampleInfoWorkloadPreview>>> {
    tokio::task::spawn_blocking(move || {
        let ctx = authz_service::authenticate(&pool, &headers)?;
        authz_service::require_permission(&ctx, "sample-info:record-workload")?;
        let record = sample_info_repo::get_by_id(&pool, id)?;
        ensure_record_access(&pool, &ctx, &record)?;
        if record.deleted_at.is_some() {
            return Err(AppError::Validation(
                "该样品记录已删除，不能录入工作量".into(),
            ));
        }
        ensure_workload_editable(&record.status, record.sampled_at.is_some())?;
        let conn = pool.get()?;
        let mapping = sample_workload_mapping::load(&conn)?;
        let source_summary = source_workload_summary(&conn, &record)?;
        let targets = workload_targets(&pool, &ctx, &record, source_summary.group_id)?;
        let selected = select_workload_target(&targets, &source_summary, &mapping, &query)?;
        let instruments = match selected {
            Some(target) => workload_instruments(&conn, &record, target, &mapping)?,
            None => Vec::new(),
        };
        let reason = if targets.is_empty() {
            Some("没有有权限且有效的核算项目与实验室，请管理员配置项目、实验室关联和数据范围。来源记录已保留。".into())
        } else if selected.is_none() {
            Some("来源记录无法唯一关联核算项目，请选择本次工作量的项目和实验室。来源记录不会改变。".into())
        } else if selected.is_some_and(|target| target.legacy_source) {
            Some("本次沿用来源记录原有的有效项目和实验室关联，兼容历史配置；其他核算目标仍须完整配置。".into())
        } else if instruments.is_empty() {
            Some("全库暂无可用标准方法，可直接使用自定义方法与仪器录入。".into())
        } else {
            None
        };
        let first = instruments.first();
        let quantity = workload_source_quantity(record.quantity)?;
        let recorded_quantity = crate::repo::record_repo::sample_info_source_quantity(&pool, id)?;
        let selected_project_id = selected.map(|target| target.project_id);
        let selected_group_id = selected.map(|target| target.group_id);
        Ok(Json(ApiResponse::ok(SampleInfoWorkloadPreview {
            source_record_id: id,
            operator_username: ctx.user.username.clone(),
            department: selected.map(|target| target.department.clone()).unwrap_or_else(|| source_summary.department.clone()),
            laboratory: selected.map(|target| target.laboratory.clone()).unwrap_or_else(|| source_summary.laboratory.clone()),
            project: selected.map(|target| target.project_name.clone()).unwrap_or_else(|| source_summary.project.clone()),
            detection_type: record.detection_type.clone(),
            quantity,
            recorded_quantity,
            remaining_quantity: (quantity - recorded_quantity).max(0),
            coefficient: selected.map(|target| target.coefficient).unwrap_or(1.0),
            method: first.map(|item| item.method_name.clone()),
            instrument: first.map(|item| item.instrument.clone()),
            multiplier: first.map(|item| item.multiplier),
            instruments,
            recorded: recorded_quantity >= quantity,
            reason,
            allow_custom: true,
            selected_project_id,
            selected_group_id,
            source_summary,
            targets,
        })))
    })
    .await
    .map_err(|error| AppError::Internal(error.to_string()))?
}

async fn sample_workload_confirm(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<SampleInfoWorkloadConfirm>,
) -> Result<Json<ApiResponse<crate::models::record::RecordResponse>>> {
    tokio::task::spawn_blocking(move || {
        let ctx = authz_service::authenticate(&pool, &headers)?;
        authz_service::require_permission(&ctx, "sample-info:record-workload")?;
        let record = sample_info_repo::get_by_id(&pool, id)?;
        ensure_record_access(&pool, &ctx, &record)?;
        if record.deleted_at.is_some() {
            return Err(AppError::Validation(
                "该样品记录已删除，不能录入工作量".into(),
            ));
        }
        ensure_workload_editable(&record.status, record.sampled_at.is_some())?;
        if body.quantity <= 0 {
            return Err(AppError::Validation("本次录入数量必须大于 0".into()));
        }
        let recorded_quantity = crate::repo::record_repo::sample_info_source_quantity(&pool, id)?;
        let remaining_quantity =
            (workload_source_quantity(record.quantity)? - recorded_quantity).max(0);
        if body.quantity > remaining_quantity {
            return Err(AppError::Validation(format!(
                "本次录入数量不能超过剩余数量 {}",
                remaining_quantity
            )));
        }
        let conn = pool.get()?;
        let mapping = sample_workload_mapping::load(&conn)?;
        let source_summary = source_workload_summary(&conn, &record)?;
        let targets = workload_targets(&pool, &ctx, &record, source_summary.group_id)?;
        let query = SampleInfoWorkloadQuery {
            project_id: body.project_id,
            group_id: body.group_id,
        };
        let target = select_workload_target(&targets, &source_summary, &mapping, &query)?
            .ok_or_else(|| AppError::Validation("请选择本次工作量的有效核算项目和实验室".into()))?;
        let candidates = workload_instruments(&conn, &record, target, &mapping)?;
        let mut source_instrument_id = 0_i64;
        let mut match_source = "custom";
        let method_id = body.method_id;
        let custom_method_name = match method_id {
            Some(selected) => {
                let candidate = candidates
                    .iter()
                    .find(|item| item.method_id == selected)
                    .ok_or_else(|| {
                        AppError::Validation("所选方法不存在、已停用或绑定仪器不可用".into())
                    })?;
                source_instrument_id = candidate.instrument_id;
                match_source = &candidate.match_source;
                None
            }
            None => Some(
                body.custom_method_name
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| AppError::Validation("请填写自定义方法名称".into()))?
                    .to_string(),
            ),
        };
        let custom_instrument_name = if method_id.is_none() {
            body.custom_instrument_name
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        } else {
            None
        };
        let workload = crate::models::record::RecordCreate {
            project_id: target.project_id,
            method_id,
            // 工作量归属到实际点击“录入工作量”的登录账号，而非取样执行人。
            user_name: ctx.user.username.clone(),
            sender_user_id: Some(ctx.user.id),
            quantity: body.quantity,
            recorded_at: crate::service::record_service::sample_workload_recorded_at(
                body.recorded_at.as_deref(),
            )?,
            group_id: Some(target.group_id),
            multiplier: Some(body.multiplier),
            high_item: None,
            division_id: target.division_id,
            extra_fields: Some(serde_json::json!({
                "source_business_no": record.business_no,
                "source_notes": body.notes.unwrap_or_default(),
                "source_instrument_id": source_instrument_id,
                "source_custom_method": custom_method_name.is_some(),
                "source_record_notes": record.notes,
                "source_summary": source_summary,
                "source_match_source": match_source,
                "source_sending": {
                    "division_id": record.execution_division_id.or(record.division_id),
                    "division_name": source_summary.department,
                    "group_id": source_summary.group_id,
                    "group_name": record.lab_name,
                },
                "source_access_snapshot": {
                    "division_id": record.division_id,
                    "execution_division_id": record.execution_division_id,
                    "type_key": record.type_key,
                    "group_id": record.group_id,
                    "execution_group_id": record.execution_group_id,
                },
            })),
            source_type: Some("sample_info_sample".into()),
            source_record_id: Some(id),
        };
        // v2.3.14: 自定义录入时把手工填写的方法/仪器名称写入快照。
        let custom = custom_method_name
            .as_deref()
            .map(|name| (name, custom_instrument_name.as_deref()));
        Ok(Json(ApiResponse::ok(
            crate::service::record_service::create_sample_workload_record(
                &pool,
                &workload,
                &ctx.user.username,
                custom,
            )?,
        )))
    })
    .await
    .map_err(|error| AppError::Internal(error.to_string()))?
}

async fn withdraw_sample(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<SampleInfoResponse>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "sample-info:withdraw")?;
    let existing = sample_info_repo::get_by_id(&pool, id)?;
    ensure_record_access(&pool, &ctx, &existing)?;
    if crate::repo::record_repo::exists_sample_info_source(&pool, id)? {
        return Err(AppError::Validation(
            "该取样已录入工作量，请先按工作量记录流程处理".into(),
        ));
    }
    let is_sampler = existing.sampled_by == ctx.user.username;
    if !ctx.is_system_admin() && !ctx.is_analysis_leader() && !is_sampler {
        return Err(AppError::Forbidden("只能撤回本人执行的取样操作".into()));
    }
    Ok(Json(ApiResponse::ok(sample_info_repo::withdraw_sample(
        &pool,
        id,
        &ctx.user.username,
    )?)))
}

async fn complete(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<SampleInfoResponse>>> {
    tokio::task::spawn_blocking(move || {
        let ctx = authz_service::authenticate(&pool, &headers)?;
        let existing = sample_info_repo::get_by_id(&pool, id)?;
        ensure_record_access(&pool, &ctx, &existing)?;
        let user_name = ensure_status_permission(&ctx, "已检测")?;
        Ok(Json(ApiResponse::ok(sample_info_repo::update_status(
            &pool,
            id,
            "已检测",
            &user_name,
        )?)))
    })
    .await
    .map_err(|error| AppError::Internal(error.to_string()))?
}

#[derive(serde::Deserialize)]
struct ReturnRecordRequest {
    reason: String,
}

async fn return_record(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<ReturnRecordRequest>,
) -> Result<Json<ApiResponse<SampleInfoResponse>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "sample-info:return")?;
    let existing = sample_info_repo::get_by_id(&pool, id)?;
    ensure_record_access(&pool, &ctx, &existing)?;
    let record = sample_info_repo::return_record(&pool, id, &body.reason, &ctx.user.username)?;
    Ok(Json(ApiResponse::ok(record)))
}

async fn confirm_return(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<SampleInfoResponse>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    let existing = sample_info_repo::get_by_id(&pool, id)?;
    ensure_record_access(&pool, &ctx, &existing)?;
    if !ctx.has_permission("sample-info:return-confirm")
        || (!ctx.is_system_admin() && existing.created_by_user_id != Some(ctx.user.id))
    {
        return Err(AppError::Forbidden("仅提交账号可以确认退回".into()));
    }
    let record = sample_info_repo::confirm_return(&pool, id, &ctx.user.username)?;
    Ok(Json(ApiResponse::ok(record)))
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

#[cfg(test)]
fn stats_where(q: &SampleInfoQuery, table_alias: Option<&str>) -> Result<(String, Vec<String>)> {
    stats_where_columns(q, table_alias, &[])
}

fn stats_where_columns(
    q: &SampleInfoQuery,
    table_alias: Option<&str>,
    columns: &[crate::models::sample_info_column::SampleInfoColumn],
) -> Result<(String, Vec<String>)> {
    let mut query = q.clone();
    query.include_pending_ownership.get_or_insert(false);
    let (where_sql, params) = sample_info_repo::filtered_where(&query, columns, None)?;
    let where_sql = match table_alias {
        Some("sir") => format!("sir.id IN (SELECT id FROM sample_info_records WHERE {where_sql})"),
        None => where_sql,
        _ => return Err(AppError::Internal("未知样品信息查询别名".into())),
    };
    Ok((where_sql, params))
}
async fn stats(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(mut q): Query<SampleInfoQuery>,
) -> Result<Json<ApiResponse<SampleInfoStats>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    apply_statistics_scope(&pool, &ctx, &mut q)?;
    let conn = pool.get()?;
    let columns = crate::repo::sample_info_column_repo::list_all(&pool)?;
    let (wc, params) = stats_where_columns(&q, None, &columns)?;
    let (wc_sir, _) = stats_where_columns(&q, Some("sir"), &columns)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn target(project_id: i64, group_id: i64, division_id: i64) -> SampleInfoWorkloadTarget {
        SampleInfoWorkloadTarget {
            project_id,
            group_id,
            division_id: Some(division_id),
            project_name: format!("project-{project_id}"),
            laboratory: format!("lab-{group_id}"),
            department: format!("dept-{division_id}"),
            coefficient: 2.5,
            legacy_source: false,
        }
    }

    #[test]
    fn target_selection_uses_ids_before_old_names_and_never_guesses_unmatched_text() {
        let mut first = target(10, 20, 30);
        first.project_name = "renamed project".into();
        first.laboratory = "renamed lab".into();
        let mut second = target(11, 21, 31);
        second.project_name = "old project".into();
        second.laboratory = "old lab".into();
        let targets = vec![first, second];
        let mut source = SampleInfoWorkloadSourceSummary {
            business_no: "SI-test".into(),
            department: "source dept".into(),
            laboratory: "old lab".into(),
            project: "old project".into(),
            detection_type: "ICP".into(),
            type_key: "icp".into(),
            quantity: 2,
            project_id: Some(10),
            group_id: Some(20),
        };
        let mapping = SampleWorkloadMapping::default();
        assert_eq!(
            select_workload_target(
                &targets,
                &source,
                &mapping,
                &SampleInfoWorkloadQuery::default()
            )
            .unwrap()
            .unwrap()
            .project_id,
            10
        );
        source.project_id = None;
        source.project = "unmatched source text".into();
        assert!(select_workload_target(
            &targets,
            &source,
            &mapping,
            &SampleInfoWorkloadQuery::default()
        )
        .unwrap()
        .is_none());
        let explicit = SampleInfoWorkloadQuery {
            project_id: Some(10),
            group_id: Some(20),
        };
        assert_eq!(
            select_workload_target(&targets, &source, &mapping, &explicit)
                .unwrap()
                .unwrap()
                .project_id,
            10
        );
        let mut mapping = mapping;
        mapping
            .project_aliases
            .push(sample_workload_mapping::ProjectAlias {
                alias: source.project.clone(),
                project_id: 10,
                group_id: Some(20),
            });
        assert!(select_workload_target(
            &targets,
            &source,
            &mapping,
            &SampleInfoWorkloadQuery::default()
        )
        .unwrap()
        .is_some());
        source.group_id = Some(99);
        assert!(select_workload_target(
            &targets,
            &source,
            &mapping,
            &SampleInfoWorkloadQuery::default()
        )
        .unwrap()
        .is_none());
    }

    #[test]
    fn source_lab_permission_is_retained_without_opening_other_labs() {
        let source = target(10, 20, 30);
        let other = target(11, 21, 31);
        assert!(workload_target_allowed(&source, false, Some(20), &[]));
        assert!(!workload_target_allowed(&other, false, Some(20), &[]));
        assert!(workload_target_allowed(&other, false, Some(20), &[31]));
        assert!(workload_target_allowed(&other, true, None, &[]));
        assert!(!workload_target_allowed(&source, false, None, &[]));
    }

    #[test]
    fn source_quantity_overflow_is_rejected_instead_of_truncated() {
        assert_eq!(workload_source_quantity(100).unwrap(), 100);
        assert!(workload_source_quantity(3_000_000_000).is_err());
    }

    struct WorkloadFixture {
        pool: DbPool,
        headers: HeaderMap,
        sample_id: i64,
        project_id: i64,
        group_id: i64,
        other_project_id: i64,
        other_group_id: i64,
        last_method_id: i64,
    }

    fn workload_fixture() -> WorkloadFixture {
        let pool = crate::db::init_pool("postgres-test");
        crate::db::test_migrations::run(&pool.get().unwrap()).unwrap();
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO roles(name,is_system) VALUES('sample-only-workload-test',0)",
            [],
        )
        .unwrap();
        let role = conn.last_insert_rowid();
        conn.execute("INSERT INTO role_permissions(role_id,permission_key) VALUES(?1,'sample-info:record-workload')", [role]).unwrap();
        conn.execute("INSERT INTO users(username,password,role_id) VALUES('sample-operator-v53','unused',?1)", [role]).unwrap();
        let user_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO user_roles(user_id,role_id) VALUES(?1,?2)",
            postgres_compat::params![user_id, role],
        )
        .unwrap();
        let mut pairs = Vec::new();
        for suffix in ["source", "other"] {
            conn.execute(
                "INSERT INTO divisions(name) VALUES(?1)",
                [format!("v53-{suffix}-dept")],
            )
            .unwrap();
            let division = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO project_groups(name,division_id) VALUES(?1,?2)",
                postgres_compat::params![format!("v53-{suffix}-lab"), division],
            )
            .unwrap();
            let group = conn.last_insert_rowid();
            conn.execute("INSERT INTO projects(name,group_id,project_division_id,coefficient) VALUES(?1,?2,?3,2.5)", postgres_compat::params![format!("v53-{suffix}-project"),group,division]).unwrap();
            let project = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO project_lab_links(project_id,group_id) VALUES(?1,?2)",
                postgres_compat::params![project, group],
            )
            .unwrap();
            pairs.push((project, group, division));
        }
        let (project_id, group_id, division_id) = pairs[0];
        let (other_project_id, other_group_id, _) = pairs[1];
        conn.execute("INSERT INTO instruments(name,code,instrument_type) VALUES('v53-instrument','V53','thermal')", []).unwrap();
        let instrument = conn.last_insert_rowid();
        let mut last_method_id = 0;
        for number in 0..205 {
            conn.execute("INSERT INTO methods(name,instrument_id,coefficient,multiplier) VALUES(?1,?2,7.0,1.5)", postgres_compat::params![format!("v53-method-{number:03}"),instrument]).unwrap();
            last_method_id = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO project_method_links(project_id,method_id) VALUES(?1,?2)",
                postgres_compat::params![project_id, last_method_id],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO instruments(name,code,instrument_type,is_active) VALUES('v53-disabled-instrument','V53-disabled','thermal',0)",
            [],
        )
        .unwrap();
        let disabled_instrument = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO methods(name,instrument_id) VALUES('v53-disabled-method',?1)",
            [disabled_instrument],
        )
        .unwrap();
        let disabled_method = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO project_method_links(project_id,method_id) VALUES(?1,?2)",
            postgres_compat::params![project_id, disabled_method],
        )
        .unwrap();
        let body: SampleInfoCreate = serde_json::from_value(serde_json::json!({
            "batch_no":"v53-batch", "user_name":"sample-operator-v53", "lab_name":"v53-source-lab",
            "project_name":"v53-source-project", "project_id":project_id, "group_id":group_id,
            "main_components":"explicit mapped text", "detection_type":"ICP", "type_key":"icp",
            "division_id":division_id, "quantity":10
        }))
        .unwrap();
        let sample = sample_info_repo::create(&pool, &body, "sample-operator-v53").unwrap();
        sample_info_repo::update_status(&pool, sample.id, "待检测", "different-sampler").unwrap();
        conn.execute(
            "UPDATE projects SET name='v53-renamed-project' WHERE id=?1",
            [project_id],
        )
        .unwrap();
        conn.execute(
            "UPDATE project_groups SET name='v53-renamed-lab' WHERE id=?1",
            [group_id],
        )
        .unwrap();
        let user = crate::repo::user_repo::find_by_id(&pool, user_id)
            .unwrap()
            .unwrap();
        let login = crate::service::auth_service::create_login_response(
            &pool,
            user,
            false,
            Some("v53-test-device"),
            Some("test"),
        )
        .unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            format!("Bearer {}", login.token).parse().unwrap(),
        );
        WorkloadFixture {
            pool,
            headers,
            sample_id: sample.id,
            project_id,
            group_id,
            other_project_id,
            other_group_id,
            last_method_id,
        }
    }

    #[tokio::test]
    async fn extra_lab_allows_global_common_methods_but_still_requires_target_scope() {
        let f = workload_fixture();
        let conn = f.pool.get().unwrap();
        let ctx = authz_service::authenticate(&f.pool, &f.headers).unwrap();
        let role = ctx.user.role_ids[0];
        let other_division: i64 = conn
            .query_row(
                "SELECT division_id FROM project_groups WHERE id=?1",
                [f.other_group_id],
                |row| row.get(0),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO role_work_division_scopes(role_id,division_id) VALUES(?1,?2)",
            postgres_compat::params![role, other_division],
        )
        .unwrap();
        conn.execute(
            "UPDATE methods SET is_common=1 WHERE id=?1",
            [f.last_method_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO divisions(name) VALUES('v53-common-only-third-dept')",
            [],
        )
        .unwrap();
        let third_division = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO common_method_division_scopes(method_id,division_id) VALUES(?1,?2)",
            postgres_compat::params![f.last_method_id, third_division],
        )
        .unwrap();
        conn.execute(
            "DELETE FROM project_method_links WHERE method_id=?1",
            [f.last_method_id],
        )
        .unwrap();
        let outside = SampleInfoWorkloadQuery {
            project_id: Some(f.other_project_id),
            group_id: Some(f.other_group_id),
        };
        let preview = sample_workload_preview(
            State(f.pool.clone()),
            f.headers.clone(),
            Path(f.sample_id),
            Query(outside),
        )
        .await
        .unwrap()
        .0
        .data
        .unwrap();
        assert_eq!(
            preview.selected_project_id,
            Some(f.other_project_id),
            "formal work scope grants the target; sample methods are selected independently"
        );
        assert_eq!(preview.instruments.len(), 205);
        assert!(preview
            .instruments
            .iter()
            .any(|method| method.method_id == f.last_method_id));
        assert!(
            !authz_service::work_method_allowed_for_division(
                &f.pool,
                &ctx,
                f.last_method_id,
                Some(other_division)
            )
            .unwrap(),
            "mainstream common-method department restrictions are unchanged"
        );
        let outside_body: SampleInfoWorkloadConfirm = serde_json::from_value(serde_json::json!({"project_id":f.other_project_id,"group_id":f.other_group_id,"method_id":f.last_method_id,"quantity":1,"multiplier":1.0})).unwrap();
        let outside_recorded = sample_workload_confirm(
            State(f.pool.clone()),
            f.headers.clone(),
            Path(f.sample_id),
            Json(outside_body),
        )
        .await
        .unwrap()
        .0
        .data
        .unwrap();
        assert_eq!(outside_recorded.project_id, f.other_project_id);
        assert_eq!(outside_recorded.user_name, "sample-operator-v53");
        assert_eq!(outside_recorded.coefficient_snapshot, 7.0);

        let source = sample_workload_preview(
            State(f.pool.clone()),
            f.headers.clone(),
            Path(f.sample_id),
            Query(SampleInfoWorkloadQuery::default()),
        )
        .await
        .unwrap()
        .0
        .data
        .unwrap();
        assert!(
            source
                .instruments
                .iter()
                .any(|method| method.method_id == f.last_method_id),
            "the original source lab keeps its existing sample-workload authorization"
        );
        let source_body: SampleInfoWorkloadConfirm = serde_json::from_value(
            serde_json::json!({"method_id":f.last_method_id,"quantity":1,"multiplier":1.0}),
        )
        .unwrap();
        let recorded = sample_workload_confirm(
            State(f.pool.clone()),
            f.headers.clone(),
            Path(f.sample_id),
            Json(source_body),
        )
        .await
        .unwrap()
        .0
        .data
        .unwrap();
        assert_eq!(recorded.project_id, f.project_id);
        conn.execute(
            "DELETE FROM role_work_division_scopes WHERE role_id=?1",
            [role],
        )
        .unwrap();
        let outside = SampleInfoWorkloadQuery {
            project_id: Some(f.other_project_id),
            group_id: Some(f.other_group_id),
        };
        assert!(sample_workload_preview(
            State(f.pool.clone()),
            f.headers.clone(),
            Path(f.sample_id),
            Query(outside),
        )
        .await
        .is_err());
        let forbidden: SampleInfoWorkloadConfirm = serde_json::from_value(serde_json::json!({"project_id":f.other_project_id,"group_id":f.other_group_id,"method_id":f.last_method_id,"quantity":1,"multiplier":1.0})).unwrap();
        assert!(sample_workload_confirm(
            State(f.pool.clone()),
            f.headers.clone(),
            Path(f.sample_id),
            Json(forbidden)
        )
        .await
        .is_err());
    }

    #[tokio::test]
    async fn historical_source_target_keeps_legacy_recording_and_rejects_other_invalid_targets() {
        let f = workload_fixture();
        let conn = f.pool.get().unwrap();
        let source = sample_info_repo::get_by_id(&f.pool, f.sample_id).unwrap();
        let source_division = source.execution_division_id.unwrap();
        conn.execute(
            "UPDATE projects SET project_status='archived' WHERE id IN (?1,?2)",
            postgres_compat::params![f.project_id, f.other_project_id],
        )
        .unwrap();
        let mut admin = authz_service::authenticate(&f.pool, &f.headers).unwrap();
        admin.user.is_admin = true;
        let allowed = workload_targets(&f.pool, &admin, &source, Some(f.group_id)).unwrap();
        assert!(allowed
            .iter()
            .any(|item| item.project_id == f.project_id && item.legacy_source));
        assert!(
            !allowed
                .iter()
                .any(|item| item.project_id == f.other_project_id),
            "legacy exception is limited to the exact source pair, even for an admin"
        );
        for condition in [
            "archived",
            "missing-department",
            "inactive-department-and-owner-gap",
        ] {
            match condition {
                "missing-department" => {
                    conn.execute(
                        "UPDATE project_groups SET division_id=NULL WHERE id=?1",
                        [f.group_id],
                    )
                    .unwrap();
                }
                "inactive-department-and-owner-gap" => {
                    conn.execute(
                        "UPDATE project_groups SET division_id=?1 WHERE id=?2",
                        postgres_compat::params![source_division, f.group_id],
                    )
                    .unwrap();
                    conn.execute(
                        "UPDATE divisions SET is_active=0 WHERE id=?1",
                        [source_division],
                    )
                    .unwrap();
                    conn.execute(
                        "UPDATE projects SET project_division_id=NULL,group_id=?2 WHERE id=?1",
                        postgres_compat::params![f.project_id, f.other_group_id],
                    )
                    .unwrap();
                }
                _ => {}
            }
            let preview = sample_workload_preview(
                State(f.pool.clone()),
                f.headers.clone(),
                Path(f.sample_id),
                Query(SampleInfoWorkloadQuery::default()),
            )
            .await
            .unwrap()
            .0
            .data
            .unwrap();
            assert_eq!(
                preview.selected_project_id,
                Some(f.project_id),
                "{condition}"
            );
            assert_eq!(preview.selected_group_id, Some(f.group_id), "{condition}");
            assert!(preview.reason.is_some());
            if condition == "missing-department" {
                assert_eq!(preview.targets[0].division_id, None);
            }
            let body: SampleInfoWorkloadConfirm = serde_json::from_value(serde_json::json!({"quantity":1,"multiplier":1.0,"custom_method_name":format!("legacy-{condition}")})).unwrap();
            let recorded = sample_workload_confirm(
                State(f.pool.clone()),
                f.headers.clone(),
                Path(f.sample_id),
                Json(body),
            )
            .await
            .unwrap()
            .0
            .data
            .unwrap();
            assert_eq!(recorded.project_id, f.project_id);
            assert_eq!(recorded.user_name, "sample-operator-v53");
        }
        assert_eq!(
            crate::repo::record_repo::sample_info_source_quantity(&f.pool, f.sample_id).unwrap(),
            3
        );
        conn.execute(
            "UPDATE projects SET is_active=0 WHERE id=?1",
            [f.project_id],
        )
        .unwrap();
        let preview = sample_workload_preview(
            State(f.pool.clone()),
            f.headers.clone(),
            Path(f.sample_id),
            Query(SampleInfoWorkloadQuery::default()),
        )
        .await
        .unwrap()
        .0
        .data
        .unwrap();
        assert!(
            preview.targets.is_empty(),
            "active/deletion and project-lab linkage are never waived"
        );
        assert_eq!(preview.source_summary.project, "v53-source-project");
        let body: SampleInfoWorkloadConfirm = serde_json::from_value(
            serde_json::json!({"quantity":1,"multiplier":1.0,"custom_method_name":"disabled"}),
        )
        .unwrap();
        assert!(sample_workload_confirm(
            State(f.pool.clone()),
            f.headers.clone(),
            Path(f.sample_id),
            Json(body)
        )
        .await
        .is_err());
    }

    #[tokio::test]
    async fn sample_only_user_can_preview_all_methods_and_confirm_standard_or_custom_on_source_lab()
    {
        let f = workload_fixture();
        let ctx = authz_service::authenticate(&f.pool, &f.headers).unwrap();
        assert!(!ctx.has_permission("entry:workload"));
        assert_eq!(
            authz_service::work_allowed_division_ids(&f.pool, &ctx).unwrap(),
            Some(Vec::new())
        );
        let preview = sample_workload_preview(
            State(f.pool.clone()),
            f.headers.clone(),
            Path(f.sample_id),
            Query(SampleInfoWorkloadQuery::default()),
        )
        .await
        .unwrap()
        .0
        .data
        .unwrap();
        assert_eq!(
            (preview.selected_project_id, preview.selected_group_id),
            (Some(f.project_id), Some(f.group_id))
        );
        assert_eq!(preview.project, "v53-renamed-project");
        assert_eq!(preview.source_summary.project, "v53-source-project");
        assert_eq!(preview.laboratory, "v53-renamed-lab");
        assert_eq!(
            preview.instruments.len(),
            205,
            "no arbitrary 200-row cutoff or type-match filter"
        );
        assert!(preview
            .instruments
            .iter()
            .all(|method| method.coefficient == 7.0 && method.match_source == "available"));
        assert_eq!(preview.coefficient, 2.5);
        assert!(preview.allow_custom);
        assert!(preview
            .targets
            .iter()
            .all(|target| target.group_id == f.group_id));
        let forbidden = SampleInfoWorkloadQuery {
            project_id: Some(f.other_project_id),
            group_id: Some(f.other_group_id),
        };
        assert!(sample_workload_preview(
            State(f.pool.clone()),
            f.headers.clone(),
            Path(f.sample_id),
            Query(forbidden)
        )
        .await
        .is_err());

        let conn = f.pool.get().unwrap();
        let methods: Vec<i64> = conn.prepare("SELECT m.id FROM methods m JOIN project_method_links pml ON pml.method_id=m.id WHERE pml.project_id=?1 AND m.name LIKE 'v53-method-%' ORDER BY m.id LIMIT 4").unwrap()
            .query_map([f.project_id], |row| row.get(0)).unwrap()
            .collect::<std::result::Result<Vec<_>,_>>().unwrap();
        assert_eq!(methods.len(), 4);
        for (index, type_name) in ["液相", "气相", "理化"].into_iter().enumerate() {
            let code = format!("META-54-{index}");
            let name = format!("真实设备本名-{index}");
            conn.execute(
                "INSERT INTO instruments(code,name,instrument_type) VALUES(?1,?2,?3)",
                postgres_compat::params![&code, &name, type_name],
            )
            .unwrap();
            let instrument = conn.last_insert_rowid();
            conn.execute("UPDATE methods SET name=?1,full_name=?2,method_code=?3,instrument_id=?4 WHERE id=?5", postgres_compat::params![format!("lookup-short-{index}"),format!("实际完整方法本名-{index}"),format!("METHOD-54-{index}"),instrument,methods[index]]).unwrap();
            conn.execute("INSERT INTO method_type_links(method_id,method_type_id) SELECT ?1,id FROM method_types WHERE name=?2 AND deleted_at IS NULL", postgres_compat::params![methods[index],type_name]).unwrap();
            conn.execute(
                "DELETE FROM project_method_links WHERE method_id=?1",
                [methods[index]],
            )
            .unwrap();
        }
        conn.execute("UPDATE methods SET name='legacy-short',full_name='旧版无仪器完整方法',method_code='LEGACY-54',instrument_id=NULL WHERE id=?1", [methods[3]]).unwrap();
        conn.execute("INSERT INTO method_type_links(method_id,method_type_id) SELECT ?1,id FROM method_types WHERE name='理化' AND deleted_at IS NULL", [methods[3]]).unwrap();
        conn.execute("INSERT INTO methods(name,instrument_id) SELECT 'unlinked-to-source-v54',instrument_id FROM methods WHERE id=?1", [f.last_method_id]).unwrap();
        let unlinked = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO project_method_links(project_id,method_id) VALUES(?1,?2)",
            postgres_compat::params![f.other_project_id, unlinked],
        )
        .unwrap();
        let metadata = sample_workload_preview(
            State(f.pool.clone()),
            f.headers.clone(),
            Path(f.sample_id),
            Query(SampleInfoWorkloadQuery::default()),
        )
        .await
        .unwrap()
        .0
        .data
        .unwrap();
        assert_eq!(
            metadata.instruments.len(),
            206,
            "the complete valid method library includes methods without this project's links"
        );
        let serialized = serde_json::to_value(&metadata).unwrap();
        let serialized_candidates = serialized["instruments"].as_array().unwrap();
        for (index, type_name) in ["液相", "气相", "理化"].into_iter().enumerate() {
            let item = serialized_candidates
                .iter()
                .find(|item| item["method_id"] == methods[index])
                .unwrap();
            assert_eq!(item["instrument_name"], format!("真实设备本名-{index}"));
            assert_eq!(item["instrument_code"], format!("META-54-{index}"));
            assert_eq!(item["instrument_type"], type_name);
            assert_eq!(item["method_name"], format!("实际完整方法本名-{index}"));
            assert_eq!(item["method_short_name"], format!("lookup-short-{index}"));
            assert_eq!(item["method_code"], format!("METHOD-54-{index}"));
            assert_eq!(item["method_types"], serde_json::json!([type_name]));
            assert_eq!(
                item["instrument"],
                format!("真实设备本名-{index} · 实际完整方法本名-{index}")
            );
        }
        let legacy = metadata
            .instruments
            .iter()
            .find(|item| item.method_id == methods[3])
            .unwrap();
        assert_eq!(legacy.instrument_id, 0);
        assert_eq!(legacy.method_types, vec!["理化"]);
        assert!(
            legacy.instrument_name.is_empty()
                && legacy.instrument_code.is_empty()
                && legacy.instrument_type.is_empty()
        );
        assert_eq!(legacy.method_short_name, "legacy-short");
        assert!(metadata
            .instruments
            .iter()
            .any(|item| item.method_id == unlinked));
        conn.execute("INSERT INTO sample_info_columns(field_key,label,data_type,type_key) VALUES('main_components','主要成分','text','icp') ON CONFLICT(field_key) DO UPDATE SET data_type='text',is_active=1,deleted_at=NULL", []).unwrap();
        let column: i64 = conn
            .query_row(
                "SELECT id FROM sample_info_columns WHERE field_key='main_components'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        conn.execute("INSERT INTO sample_info_column_visibility(column_id,type_key,is_visible) VALUES(?1,'icp',1) ON CONFLICT(column_id,type_key) DO UPDATE SET is_visible=1", [column]).unwrap();
        crate::repo::settings_repo::upsert(&f.pool,sample_workload_mapping::SETTING_KEY,&serde_json::json!({
            "field_mappings":[{"type_key":"icp","field_key":"main_components","value":"explicit mapped text","method_id":f.last_method_id}]
        }).to_string()).unwrap();
        let mapped = sample_workload_preview(
            State(f.pool.clone()),
            f.headers.clone(),
            Path(f.sample_id),
            Query(SampleInfoWorkloadQuery::default()),
        )
        .await
        .unwrap()
        .0
        .data
        .unwrap();
        assert_eq!(mapped.instruments.len(), 206);
        assert_eq!(
            (
                mapped.instruments[0].method_id,
                mapped.instruments[0].match_source.as_str()
            ),
            (f.last_method_id, "mapping")
        );
        let standard: SampleInfoWorkloadConfirm = serde_json::from_value(serde_json::json!({"method_id":f.last_method_id,"quantity":1,"multiplier":1.5,"notes":"operator note"})).unwrap();
        let recorded = sample_workload_confirm(
            State(f.pool.clone()),
            f.headers.clone(),
            Path(f.sample_id),
            Json(standard),
        )
        .await
        .unwrap()
        .0
        .data
        .unwrap();
        assert_eq!(recorded.user_name, "sample-operator-v53");
        assert_eq!(recorded.coefficient_snapshot, 7.0);
        let stored: String = conn
            .query_row(
                "SELECT extra_fields FROM work_records WHERE id=?1",
                [recorded.id],
                |row| row.get(0),
            )
            .unwrap();
        let stored: serde_json::Value = serde_json::from_str(&stored).unwrap();
        assert_eq!(stored["source_summary"]["project"], "v53-source-project");
        assert_eq!(stored["source_match_source"], "mapping");
        assert_eq!(stored["source_notes"], "operator note");
        let custom: SampleInfoWorkloadConfirm = serde_json::from_value(serde_json::json!({"quantity":1,"multiplier":1.0,"custom_method_name":"custom despite candidates","custom_instrument_name":"custom instrument"})).unwrap();
        let recorded = sample_workload_confirm(
            State(f.pool.clone()),
            f.headers.clone(),
            Path(f.sample_id),
            Json(custom),
        )
        .await
        .unwrap()
        .0
        .data
        .unwrap();
        assert_eq!(recorded.method_id, None);
        assert_eq!(recorded.coefficient_snapshot, 2.5);
        assert_eq!(
            recorded.method_name.as_deref(),
            Some("custom despite candidates")
        );
        assert_eq!(recorded.user_name, "sample-operator-v53");
        let forbidden: SampleInfoWorkloadConfirm = serde_json::from_value(serde_json::json!({"project_id":f.other_project_id,"group_id":f.other_group_id,"quantity":1,"multiplier":1.0,"custom_method_name":"forbidden"})).unwrap();
        assert!(sample_workload_confirm(
            State(f.pool.clone()),
            f.headers.clone(),
            Path(f.sample_id),
            Json(forbidden)
        )
        .await
        .is_err());
        let unbound: SampleInfoWorkloadConfirm = serde_json::from_value(
            serde_json::json!({"method_id":methods[3],"quantity":1,"multiplier":1.0}),
        )
        .unwrap();
        let recorded = sample_workload_confirm(
            State(f.pool.clone()),
            f.headers.clone(),
            Path(f.sample_id),
            Json(unbound),
        )
        .await
        .unwrap()
        .0
        .data
        .unwrap();
        assert_eq!(recorded.method_id, Some(methods[3]));
        assert_eq!(recorded.instrument_code, "");
        assert_eq!(recorded.method_type.as_deref(), Some("理化"));
        let mut liquid_record_id = 0;
        for (index, type_name) in ["液相", "气相", "理化"].into_iter().enumerate() {
            let global: SampleInfoWorkloadConfirm = serde_json::from_value(
                serde_json::json!({"method_id":methods[index],"quantity":1,"multiplier":1.0}),
            )
            .unwrap();
            let recorded = sample_workload_confirm(
                State(f.pool.clone()),
                f.headers.clone(),
                Path(f.sample_id),
                Json(global),
            )
            .await
            .unwrap()
            .0
            .data
            .unwrap();
            assert_eq!(recorded.method_id, Some(methods[index]));
            assert_eq!(recorded.user_name, "sample-operator-v53");
            assert_eq!(recorded.coefficient_snapshot, 7.0);
            assert_eq!(recorded.method_type.as_deref(), Some(type_name));
            let snapshot: String = conn
                .query_row(
                    "SELECT detection_type_snapshot FROM work_records WHERE id=?1",
                    [recorded.id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(snapshot, type_name);
            if index == 0 {
                liquid_record_id = recorded.id;
            }
        }
        let global: SampleInfoWorkloadConfirm = serde_json::from_value(
            serde_json::json!({"method_id":unlinked,"quantity":1,"multiplier":1.0}),
        )
        .unwrap();
        sample_workload_confirm(
            State(f.pool.clone()),
            f.headers.clone(),
            Path(f.sample_id),
            Json(global),
        )
        .await
        .unwrap();

        // 工作量元数据编辑仍须原有 entry:workload 与当前范围授权。
        let metadata_edit: crate::models::record::RecordUpdate = serde_json::from_value(serde_json::json!({"quantity":2,"multiplier":3.0,"recorded_at":"2026-10-11T10:00:00","project_id":f.project_id,"method_id":methods[0],"group_id":f.group_id})).unwrap();
        assert!(crate::api::record_handler::update(
            State(f.pool.clone()),
            f.headers.clone(),
            Path(liquid_record_id),
            Json(metadata_edit)
        )
        .await
        .is_err());
        let role = ctx.user.role_ids[0];
        let source_division: i64 = conn
            .query_row(
                "SELECT division_id FROM project_groups WHERE id=?1",
                [f.group_id],
                |row| row.get(0),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO role_permissions(role_id,permission_key) VALUES(?1,'entry:workload')",
            [role],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO role_work_division_scopes(role_id,division_id) VALUES(?1,?2)",
            postgres_compat::params![role, source_division],
        )
        .unwrap();
        let metadata_edit: crate::models::record::RecordUpdate = serde_json::from_value(serde_json::json!({"quantity":2,"multiplier":3.0,"recorded_at":"2026-10-11T10:00:00","project_id":f.project_id,"method_id":methods[0],"group_id":f.group_id,"division_id":source_division})).unwrap();
        let edited = crate::api::record_handler::update(
            State(f.pool.clone()),
            f.headers.clone(),
            Path(liquid_record_id),
            Json(metadata_edit),
        )
        .await
        .unwrap()
        .0
        .data
        .unwrap();
        assert_eq!(
            (
                edited.quantity,
                edited.multiplier,
                edited.coefficient_snapshot
            ),
            (2, 3.0, 7.0)
        );
        assert_eq!(edited.method_id, Some(methods[0]));
        assert_eq!(edited.user_name, "sample-operator-v53");
        assert_eq!(edited.recorded_at, "2026-10-11T10:00:00");
        let snapshot: String = conn
            .query_row(
                "SELECT detection_type_snapshot FROM work_records WHERE id=?1",
                [liquid_record_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(snapshot, "液相");
        for payload in [
            serde_json::json!({"method_id":unlinked,"quantity":2}),
            serde_json::json!({"group_id":f.other_group_id,"quantity":2}),
            serde_json::json!({"project_id":f.other_project_id,"quantity":2}),
            serde_json::json!({"division_id":source_division+10000,"quantity":2}),
            serde_json::json!({"quantity":100}),
        ] {
            let edit = serde_json::from_value(payload).unwrap();
            assert!(crate::api::record_handler::update(
                State(f.pool.clone()),
                f.headers.clone(),
                Path(liquid_record_id),
                Json(edit)
            )
            .await
            .is_err());
        }
        conn.execute(
            "DELETE FROM role_work_division_scopes WHERE role_id=?1",
            [role],
        )
        .unwrap();
        let edit = serde_json::from_value(serde_json::json!({"quantity":3})).unwrap();
        assert!(crate::api::record_handler::update(
            State(f.pool.clone()),
            f.headers.clone(),
            Path(liquid_record_id),
            Json(edit)
        )
        .await
        .is_err());

        for disabled_method in [methods[0], methods[1]] {
            if disabled_method == methods[0] {
                conn.execute(
                    "UPDATE methods SET is_active=0 WHERE id=?1",
                    [disabled_method],
                )
                .unwrap();
            } else {
                conn.execute("UPDATE instruments SET is_active=0 WHERE id=(SELECT instrument_id FROM methods WHERE id=?1)",[disabled_method]).unwrap();
            }
            let invalid: SampleInfoWorkloadConfirm = serde_json::from_value(
                serde_json::json!({"method_id":disabled_method,"quantity":1,"multiplier":1.0}),
            )
            .unwrap();
            assert!(sample_workload_confirm(
                State(f.pool.clone()),
                f.headers.clone(),
                Path(f.sample_id),
                Json(invalid)
            )
            .await
            .is_err());
        }
        assert_eq!(
            crate::repo::record_repo::sample_info_source_quantity(&f.pool, f.sample_id).unwrap(),
            8
        );

        conn.execute("UPDATE sample_info_records SET project_id=NULL,project_name='unmatched historical free text' WHERE id=?1", [f.sample_id]).unwrap();
        let unmatched = sample_workload_preview(
            State(f.pool.clone()),
            f.headers.clone(),
            Path(f.sample_id),
            Query(SampleInfoWorkloadQuery::default()),
        )
        .await
        .unwrap()
        .0
        .data
        .unwrap();
        assert_eq!(unmatched.selected_project_id, None);
        assert!(!unmatched.targets.is_empty());
        let explicit = SampleInfoWorkloadQuery {
            project_id: Some(f.project_id),
            group_id: Some(f.group_id),
        };
        let selected = sample_workload_preview(
            State(f.pool.clone()),
            f.headers.clone(),
            Path(f.sample_id),
            Query(explicit),
        )
        .await
        .unwrap()
        .0
        .data
        .unwrap();
        assert_eq!(selected.selected_project_id, Some(f.project_id));
        assert_eq!(
            selected.source_summary.project,
            "unmatched historical free text"
        );
    }

    #[test]
    fn sample_workload_recorded_at_is_optional_and_preserved_in_sample_info_confirm() {
        for value in [
            None,
            Some(serde_json::Value::Null),
            Some(serde_json::json!("2026-09-30T23:59:59")),
        ] {
            let mut payload = serde_json::json!({ "quantity": 1, "multiplier": 1.0 });
            if let Some(value) = value {
                payload["recorded_at"] = value;
            }
            let body: SampleInfoWorkloadConfirm = serde_json::from_value(payload.clone()).unwrap();
            assert_eq!(body.recorded_at.as_deref(), payload["recorded_at"].as_str());
        }
    }

    fn candidate(method_name: &str, instrument_code: &str) -> WorkloadMethodCandidate {
        WorkloadMethodCandidate {
            method_id: 1,
            instrument_id: 1,
            instrument: format!("{} · {}", instrument_code, method_name),
            method_name: method_name.into(),
            method_short_name: method_name.into(),
            instrument_code: instrument_code.into(),
            instrument_name: String::new(),
            instrument_type: String::new(),
            method_code: String::new(),
            coefficient: 1.0,
            multiplier: 1.0,
            method_types: Vec::new(),
            method_type_ids: Vec::new(),
        }
    }

    #[test]
    fn raw_instrument_metadata_keeps_empty_code_recommendation_fallback() {
        let mut item = candidate("unrelated method", "");
        item.instrument_name = "ICP".into();
        assert!(record_value_matches_candidate("ICP", &item));
        assert!(
            item.instrument_code.is_empty(),
            "metadata exposes the actual code rather than a substituted name"
        );
    }

    #[test]
    fn entered_submitter_is_preserved_and_blank_falls_back_to_login() {
        assert_eq!(
            effective_submitter_name("代送样人", "login_user"),
            "代送样人"
        );
        assert_eq!(effective_submitter_name("  ", "login_user"), "login_user");
    }

    #[test]
    fn workload_entry_is_allowed_after_detection_completed() {
        assert!(ensure_workload_editable("待检测", true).is_ok());
        // v2.3.17: 完成检测（已检测）后仍可补录工作量。
        assert!(ensure_workload_editable("已检测", true).is_ok());
        // 未取样、被撤回或被退回的记录不能录入工作量。
        assert!(ensure_workload_editable("待取样", false).is_err());
        assert!(ensure_workload_editable("已退回", false).is_err());
        assert!(ensure_workload_editable("已退回已确认", false).is_err());
        assert!(ensure_workload_editable("退回待修改", false).is_err());
    }

    #[test]
    fn workload_record_values_match_methods_and_instruments_precisely() {
        assert!(record_value_matches_candidate(
            "HPLC-MS",
            &candidate("HPLC-MS", "HPLC-MS-01")
        ));
        assert!(record_value_matches_candidate(
            "ICP-OES",
            &candidate("ICP-OES", "ICP-OES-01")
        ));
        assert!(record_value_matches_candidate(
            "ARC",
            &candidate("ARC01", "ARC-01")
        ));
        assert!(record_value_matches_candidate(
            "气相",
            &candidate("通用气相", "GC")
        ));
        assert!(!record_value_matches_candidate(
            "HPLC-MS",
            &candidate("通用气相", "GC")
        ));
    }

    #[test]
    fn workload_type_keys_map_to_method_type_vocabulary() {
        assert_eq!(workload_method_type_names("icp", "ICP"), vec!["ICP"]);
        assert_eq!(
            workload_method_type_names("thermal", "热稳定性"),
            vec!["热分析", "热稳定性"]
        );
        assert_eq!(workload_method_type_names("mass", "质谱"), vec!["质谱"]);
        assert_eq!(
            workload_method_type_names("zzzb", "杂质制备"),
            vec!["杂质制备"]
        );
        assert_eq!(
            workload_method_type_names("ffkf", "方法开发"),
            vec!["方法开发"]
        );
        assert_eq!(
            workload_method_type_names("other", "其他检测"),
            vec!["其它", "其他", "其他检测"]
        );
    }
    #[test]
    fn statistics_scope_uses_execution_and_includes_business_owner() {
        use crate::models::sample_info::{SampleInfoQuery, SampleInfoScopeFilter};
        let conn = postgres_compat::Connection::open_test_database().unwrap();
        let query = SampleInfoQuery {
            ownership_basis: Some("project".into()),
            scope_filters: vec![
                SampleInfoScopeFilter {
                    execution_department: true,
                    division_ids: vec![2],
                    type_keys: vec!["normal".into()],
                    ..Default::default()
                },
                SampleInfoScopeFilter {
                    created_by_user_id: Some(99),
                    business_user_id: Some(99),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let (where_sql, params) = super::stats_where(&query, None).unwrap();
        let sql = format!("WITH sample_info_records(id,division_id,execution_division_id,type_key,created_by_user_id,business_user_id,deleted_at,ownership_status) AS (VALUES (1,1,2,'normal',10,10,NULL::text,'confirmed'),(2,2,1,'normal',10,10,NULL::text,'confirmed'),(3,1,1,'normal',10,99,NULL::text,'confirmed'),(4,1,2,'other',10,10,NULL::text,'confirmed')) SELECT id FROM sample_info_records WHERE {where_sql} ORDER BY id");
        let ids = conn
            .prepare(&sql)
            .unwrap()
            .query_map(postgres_compat::params_from_iter(params.iter()), |row| {
                row.get::<_, i64>(0)
            })
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(ids, vec![1, 3]);
    }
}
