use crate::config::AppConfig;
use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::sample_info::{
    SampleInfoCreate, SampleInfoQuery, SampleInfoResponse, SampleInfoScopeFilter,
    SampleInfoStatusUpdate, SampleInfoUpdate,
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
use postgres_compat::OptionalExtension;
use serde::Serialize;

#[derive(Serialize)]
struct SampleInfoWorkloadInstrument {
    method_id: i64,
    instrument_id: i64,
    instrument: String,
    method_name: String,
    multiplier: f64,
    /// v2.3.14: 该方法是否命中样品记录的检测类型。命中项在候选列表中优先展示。
    type_matched: bool,
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
    instruments: Vec<SampleInfoWorkloadInstrument>,
    recorded: bool,
    /// v2.3.13: 与研发送样保持一致的单一方法/仪器摘要，供弹窗直接展示。
    method: Option<String>,
    instrument: Option<String>,
    multiplier: Option<f64>,
    /// v2.3.13: 当没有候选方法/仪器时说明原因，供前端给出可操作的提示。
    reason: Option<String>,
    /// v2.3.14: 方法库检索不到候选时，允许手工填写方法与仪器后录入。
    allow_custom: bool,
}

#[derive(serde::Deserialize)]
struct SampleInfoWorkloadConfirm {
    /// v2.3.14: 走方法库时传选中的方法 id；自定义录入时留空。
    #[serde(default)]
    method_id: Option<i64>,
    quantity: i32,
    multiplier: f64,
    notes: Option<String>,
    /// v2.3.14: 方法库无候选时填写的方法名称（自定义录入必填）。
    #[serde(default)]
    custom_method_name: Option<String>,
    /// v2.3.14: 方法库无候选时填写的仪器名称（自定义录入可空）。
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

fn ensure_record_access(
    pool: &DbPool,
    ctx: &AuthContext,
    record: &SampleInfoResponse,
) -> Result<()> {
    if record.business_user_id == Some(ctx.user.id) {
        return Ok(());
    }
    if ctx.is_analysis_member() {
        let execution_division_id = record.execution_division_id.or(record.division_id);
        if authz_service::work_division_allowed(pool, ctx, execution_division_id)? {
            return Ok(());
        }
        if record.created_by_user_id == Some(ctx.user.id) {
            return Ok(());
        }
        return Err(AppError::Forbidden(
            "当前角色无权访问该检测部门的样品记录".into(),
        ));
    }
    if let Some(allowed) = authz_service::sample_info_scope_allows(
        pool,
        ctx,
        record.division_id,
        &record.type_key,
        record.created_by_user_id,
    )? {
        if allowed {
            return Ok(());
        }
        return Err(AppError::Forbidden("当前角色无权访问该样品记录".into()));
    }
    let analysis_scope_allowed = if ctx.is_analysis_member() {
        authz_service::work_division_allowed(pool, ctx, record.division_id)?
    } else {
        false
    };
    let allowed = ctx.is_system_admin()
        || analysis_scope_allowed
        || record.created_by_user_id == Some(ctx.user.id)
        || (ctx.is_rd_leader()
            && record.group_id.is_some()
            && record.group_id == ctx.user.group_id);
    if allowed {
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
    let scopes = authz_service::role_data_scopes(pool, ctx)?;
    if scopes.is_empty() {
        return Ok(false);
    }
    query.scope_filters = scopes
        .into_iter()
        .map(|scope| SampleInfoScopeFilter {
            division_ids: scope.division_ids,
            type_keys: scope.sample_info_type_keys,
            created_by_user_id: None,
            business_user_id: None,
        })
        .collect();
    // Keep the historical "I can always inspect my own submission" behavior without
    // granting a role's cross-department scope to another role.
    query.scope_filters.push(SampleInfoScopeFilter {
        division_ids: vec![],
        type_keys: vec![],
        created_by_user_id: Some(ctx.user.id),
        business_user_id: Some(ctx.user.id),
    });
    Ok(true)
}

fn apply_record_scope(pool: &DbPool, ctx: &AuthContext, query: &mut SampleInfoQuery) -> Result<()> {
    if ctx.is_system_admin() {
        return Ok(());
    }
    if ctx.is_analysis_member() {
        let analysis_scopes: Vec<_> = authz_service::role_data_scopes(pool, ctx)?
            .into_iter()
            .filter(|scope| !scope.work_division_ids.is_empty())
            .collect();
        if !analysis_scopes.is_empty() {
            query.ownership_basis = Some("execution".into());
            query.scope_filters = analysis_scopes
                .into_iter()
                .map(|scope| SampleInfoScopeFilter {
                    division_ids: scope.work_division_ids,
                    type_keys: scope.sample_info_type_keys,
                    created_by_user_id: None,
                    business_user_id: None,
                })
                .collect();
            query.scope_filters.push(SampleInfoScopeFilter {
                division_ids: vec![],
                type_keys: vec![],
                created_by_user_id: Some(ctx.user.id),
                business_user_id: Some(ctx.user.id),
            });
            return Ok(());
        }
        match authz_service::work_allowed_division_ids(pool, ctx)? {
            // Compatibility for legacy fixed roles that have not yet been
            // migrated to an explicit role data range.
            None => return Ok(()),
            Some(ids) if ids.is_empty() => {
                query.created_by_user_id = Some(ctx.user.id);
                return Ok(());
            }
            Some(ids) => {
                query.scope_filters.push(SampleInfoScopeFilter {
                    division_ids: ids,
                    type_keys: vec![],
                    created_by_user_id: None,
                    business_user_id: None,
                });
                query.scope_filters.push(SampleInfoScopeFilter {
                    division_ids: vec![],
                    type_keys: vec![],
                    created_by_user_id: Some(ctx.user.id),
                    business_user_id: Some(ctx.user.id),
                });
                return Ok(());
            }
        }
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
            division_ids: vec![],
            type_keys: vec![],
            created_by_user_id: Some(ctx.user.id),
            business_user_id: Some(ctx.user.id),
        });
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
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "entry:sample-info")?;
    apply_record_scope(&pool, &ctx, &mut q)?;
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
    authz_service::require_permission(&ctx, "sample-info:create")?;
    body.user_name = effective_submitter_name(&body.user_name, &ctx.user.username);
    validate_sample_info_bindings(&pool, &ctx, &body, false)?;
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
    body: &SampleInfoCreate,
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
    let group: Option<(i64, Option<i64>)> = conn
        .query_row(
            "SELECT g.id, g.division_id FROM project_groups g LEFT JOIN divisions d ON d.id=g.division_id WHERE g.name=?1 AND g.deleted_at IS NULL AND g.show_in_sample_info=1 AND (g.division_id IS NULL OR (d.deleted_at IS NULL AND d.show_in_sample_info=1)) ORDER BY g.id LIMIT 1",
            postgres_compat::params![&body.lab_name],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if !body.lab_name.trim().is_empty() && group.is_none() {
        return Err(AppError::Validation("所选实验室不存在或已停用".into()));
    }
    let group_id = group.map(|value| value.0);
    if let Some((_, group_division_id)) = group {
        match (body.division_id, group_division_id) {
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
    if !body.project_name.trim().is_empty() {
        let project_group: Option<i64> = conn
            .query_row(
                "SELECT id FROM projects WHERE name=?1 AND is_active=1 AND deleted_at IS NULL AND show_in_sample_info=1 ORDER BY id LIMIT 1",
                postgres_compat::params![&body.project_name],
                |row| row.get(0),
            )
            .optional()?;
        let project_id =
            project_group.ok_or_else(|| AppError::Validation("所选项目不存在或已归档".into()))?;
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
    validate_sample_info_bindings(&pool, &ctx, &body, false)?;
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
                .is_some_and(|value| value != &existing.project_name);
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
    }
    let candidate = SampleInfoCreate {
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
        && candidate.project_name == existing.project_name;
    let keep_historical_organization = organization_unchanged
        && (existing.status == "退回待修改" || existing.business_user_id == Some(ctx.user.id));
    validate_sample_info_bindings(&pool, &ctx, &candidate, keep_historical_organization)?;
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
    let ctx = authz_service::authenticate(&pool, &headers)?;
    let existing = sample_info_repo::get_by_id(&pool, id)?;
    ensure_record_access(&pool, &ctx, &existing)?;
    let user_name = ensure_status_permission(&pool, &headers, &body.status)?;
    let record = sample_info_repo::update_status(&pool, id, &body.status, &user_name)?;
    Ok(Json(ApiResponse::ok(record)))
}

async fn sample(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<SampleInfoResponse>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    let existing = sample_info_repo::get_by_id(&pool, id)?;
    ensure_record_access(&pool, &ctx, &existing)?;
    let user_name = ensure_status_permission(&pool, &headers, "待检测")?;
    Ok(Json(ApiResponse::ok(sample_info_repo::update_status(
        &pool,
        id,
        "待检测",
        &user_name,
    )?)))
}

fn workload_instruments(
    pool: &DbPool,
    record: &SampleInfoResponse,
) -> Result<Vec<SampleInfoWorkloadInstrument>> {
    let conn = pool.get()?;
    // v2.3.13: 不再要求「项目-实验室-方法-仪器」四层关联全部齐备。
    // 项目已关联的方法（含"通用方法"自动关联）优先；项目未配置方法关联时按检测类型回退。
    // 方法未绑定仪器（或仪器已停用）时仍返回候选，仪器名称显示为方法名。
    // v2.3.14: 检测类型不再作为硬过滤条件——`sample_info_records.detection_type` 取自
    // `sample_info_types.label`，与 `method_types.name` 是两套互相独立的词表，直接相等比较
    // 会把"通用方法"全部挡掉。现在类型命中只作为排序优先级：命中的排在最前，未命中的仍可选用，
    // 保证方法库始终可检索；方法库完全没有候选时前端再走自定义方法/仪器。
    let mut stmt = conn.prepare(
        "SELECT DISTINCT m.id,
                COALESCE(i.id,0),
                CASE WHEN COALESCE(i.id,0)=0
                     THEN COALESCE(NULLIF(m.full_name,''),m.name)
                     ELSE COALESCE(NULLIF(i.code,''),i.name) || ' · ' || COALESCE(NULLIF(m.full_name,''),m.name)
                END,
                COALESCE(m.multiplier,1.0),
                CASE WHEN COALESCE(i.id,0)=0 THEN '~' ELSE COALESCE(NULLIF(i.code,''),i.name) END AS instrument_sort_name,
                COALESCE(NULLIF(m.full_name,''),m.name),
                CASE WHEN (
                      NOT EXISTS(SELECT 1 FROM method_type_links x WHERE x.method_id=m.id)
                      OR EXISTS(SELECT 1 FROM method_type_links mtl
                                JOIN method_types mt ON mt.id=mtl.method_type_id AND mt.deleted_at IS NULL
                                WHERE mtl.method_id=m.id AND mt.name=?2)
                      OR p.method_type=?2
                     ) THEN 1 ELSE 0 END AS type_rank
         FROM projects p
         JOIN methods m ON m.deleted_at IS NULL AND m.is_active=1 AND m.show_in_sample_info=1
         LEFT JOIN instruments i ON i.id=m.instrument_id AND i.deleted_at IS NULL AND i.is_active=1
         WHERE p.name=?1
           AND p.deleted_at IS NULL AND p.is_active=1
           AND (
                 EXISTS(SELECT 1 FROM project_method_links pml WHERE pml.project_id=p.id AND pml.method_id=m.id)
                 OR NOT EXISTS(SELECT 1 FROM project_method_links pml WHERE pml.project_id=p.id)
               )
         ORDER BY type_rank DESC,instrument_sort_name,m.id
         LIMIT 100",
    )?;
    let rows = stmt.query_map(
        postgres_compat::params![&record.project_name, &record.detection_type],
        |row| {
            Ok(SampleInfoWorkloadInstrument {
                method_id: row.get(0)?,
                instrument_id: row.get(1)?,
                instrument: row.get(2)?,
                method_name: row.get(5)?,
                multiplier: row.get::<_, f64>(3).unwrap_or(1.0),
                type_matched: row.get::<_, i64>(6).unwrap_or(0) == 1,
            })
        },
    )?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

/// v2.3.13: 没有候选方法时给出可操作的原因说明。
/// v2.3.14: 方法库整体为空时才提示检查配置，并说明可以自定义录入。
fn workload_unavailable_reason(record: &SampleInfoResponse) -> Option<String> {
    if record.project_name.trim().is_empty() {
        return Some(
            "该样品记录没有填写项目名称，无法从方法库检索方法，请直接自定义方法名称。".into(),
        );
    }
    Some(format!(
        "项目「{}」在方法库中没有可用的检测方法（项目未关联方法、方法未启用或未在样品信息登记中显示）。可直接自定义方法与仪器后录入。",
        record.project_name
    ))
}

async fn sample_workload_preview(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<SampleInfoWorkloadPreview>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "sample-info:record-workload")?;
    let record = sample_info_repo::get_by_id(&pool, id)?;
    ensure_record_access(&pool, &ctx, &record)?;
    if record.status != "待检测" || record.sampled_at.is_none() {
        return Err(AppError::Validation("请先完成取样后再录入工作量".into()));
    }
    let department = record
        .division_id
        .and_then(|division_id| crate::repo::division_repo::get_by_id(&pool, division_id).ok())
        .map(|division| division.name)
        .unwrap_or_default();
    let instruments = workload_instruments(&pool, &record)?;
    let allow_custom = instruments.is_empty();
    let reason = if allow_custom {
        workload_unavailable_reason(&record)
    } else {
        None
    };
    let first = instruments.first();
    Ok(Json(ApiResponse::ok(SampleInfoWorkloadPreview {
        source_record_id: id,
        operator_username: ctx.user.username.clone(),
        department,
        laboratory: record.lab_name.clone(),
        project: record.project_name.clone(),
        detection_type: record.detection_type.clone(),
        quantity: record.quantity as i32,
        method: first.map(|item| item.method_name.clone()),
        instrument: first.map(|item| item.instrument.clone()),
        multiplier: first.map(|item| item.multiplier),
        instruments,
        recorded: crate::repo::record_repo::exists_sample_info_source(&pool, id)?,
        reason,
        allow_custom,
    })))
}

async fn sample_workload_confirm(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<SampleInfoWorkloadConfirm>,
) -> Result<Json<ApiResponse<crate::models::record::RecordResponse>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "sample-info:record-workload")?;
    let record = sample_info_repo::get_by_id(&pool, id)?;
    ensure_record_access(&pool, &ctx, &record)?;
    if record.status != "待检测" || record.sampled_at.is_none() {
        return Err(AppError::Validation("请先完成取样后再录入工作量".into()));
    }
    if crate::repo::record_repo::exists_sample_info_source(&pool, id)? {
        return Err(AppError::Validation("本次取样已录入工作量".into()));
    }
    // v2.3.14: 优先使用从方法库检索到的检测方法；方法库没有候选时，允许自定义方法与仪器。
    let candidates = workload_instruments(&pool, &record)?;
    let mut source_instrument_id = 0_i64;
    let method_id = body.method_id;
    let custom_method_name = match method_id {
        Some(selected) => {
            let candidate = candidates
                .iter()
                .find(|item| item.method_id == selected)
                .ok_or_else(|| {
                    AppError::Validation("所选方法不属于该项目、实验室和检测类型".into())
                })?;
            source_instrument_id = candidate.instrument_id;
            None
        }
        None => Some(
            body.custom_method_name
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    AppError::Validation("请选择方法库中的方法，或填写自定义方法名称".into())
                })?
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
    // v2.3.13: 项目解析放宽。优先使用项目已关联的实验室；项目未配置实验室关联时
    // 按名称匹配项目，实验室回退到记录的项目组/执行实验室（允许为空），
    // 让历史配置的样品信息也能补录工作量。
    let project_and_group: (i64, Option<i64>) = {
        let conn = pool.get()?;
        let linked = conn
            .query_row(
                "SELECT p.id,pg.id FROM projects p
                 JOIN project_lab_links pll ON pll.project_id=p.id
                 JOIN project_groups pg ON pg.id=pll.group_id
                 WHERE p.name=?1 AND (pll.group_id=?2 OR pg.name=?3) AND p.deleted_at IS NULL AND p.is_active=1
                 ORDER BY p.id LIMIT 1",
                postgres_compat::params![&record.project_name, &record.group_id.unwrap_or(-1), &record.lab_name],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()?;
        match linked {
            Some((project_id, group_id)) => (project_id, Some(group_id)),
            None => {
                let project_id: i64 = conn.query_row(
                    "SELECT id FROM projects
                     WHERE name=?1 AND deleted_at IS NULL AND is_active=1
                     ORDER BY id LIMIT 1",
                    [&record.project_name],
                    |row| row.get(0),
                )?;
                (project_id, record.group_id.or(record.execution_group_id))
            }
        }
    };
    let workload = crate::models::record::RecordCreate {
        project_id: project_and_group.0,
        method_id,
        // 工作量归属到实际点击“录入工作量”的登录账号，而非取样执行人。
        user_name: ctx.user.username.clone(),
        sender_user_id: Some(ctx.user.id),
        quantity: body.quantity,
        recorded_at: chrono::Utc::now()
            .with_timezone(&chrono::FixedOffset::east_opt(8 * 60 * 60).expect("valid UTC+8"))
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string(),
        group_id: project_and_group.1,
        multiplier: Some(body.multiplier),
        high_item: None,
        division_id: record.division_id,
        extra_fields: Some(serde_json::json!({
            "source_business_no": record.business_no,
            "source_notes": body.notes.unwrap_or_default(),
            "source_instrument_id": source_instrument_id,
            "source_custom_method": custom_method_name.is_some(),
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
    let ctx = authz_service::authenticate(&pool, &headers)?;
    let existing = sample_info_repo::get_by_id(&pool, id)?;
    ensure_record_access(&pool, &ctx, &existing)?;
    let user_name = ensure_status_permission(&pool, &headers, "已检测")?;
    Ok(Json(ApiResponse::ok(sample_info_repo::update_status(
        &pool,
        id,
        "已检测",
        &user_name,
    )?)))
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

fn sample_info_ownership_column(q: &SampleInfoQuery, prefix: &str) -> Result<String> {
    let column = match q.ownership_basis.as_deref().unwrap_or("submitted") {
        "submitted" => "submitted_division_id",
        "execution" => "execution_division_id",
        "project" => "project_division_id",
        _ => {
            return Err(AppError::Validation(
                "样品信息统计口径只能是 submitted、execution 或 project".into(),
            ))
        }
    };
    Ok(format!("{prefix}{column}"))
}

fn stats_where(q: &SampleInfoQuery, table_alias: Option<&str>) -> Result<(String, Vec<String>)> {
    let prefix = table_alias
        .map(|value| format!("{value}."))
        .unwrap_or_default();
    let mut clauses: Vec<String> = vec![format!("{}deleted_at IS NULL", prefix)];
    let mut params: Vec<String> = vec![];
    if !q.include_pending_ownership.unwrap_or(false) {
        clauses.push(format!(
            "COALESCE({}ownership_status,'confirmed')<>'pending_confirmation'",
            prefix
        ));
    }
    if let Some(division_id) = q.division_id {
        let i = params.len() + 1;
        clauses.push(format!(
            "{}=?{}",
            sample_info_ownership_column(q, &prefix)?,
            i
        ));
        params.push(division_id.to_string());
    }
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
    if !q.scope_filters.is_empty() {
        let mut scope_clauses = Vec::new();
        for scope in &q.scope_filters {
            let mut parts = Vec::new();
            if let Some(user_id) = scope.created_by_user_id {
                let i = params.len() + 1;
                parts.push(format!("{}created_by_user_id=?{}", prefix, i));
                params.push(user_id.to_string());
            }
            if !scope.division_ids.is_empty() {
                let mut placeholders = Vec::new();
                for division_id in &scope.division_ids {
                    let i = params.len() + 1;
                    placeholders.push(format!("?{}", i));
                    params.push(division_id.to_string());
                }
                parts.push(format!(
                    "{}division_id IN ({})",
                    prefix,
                    placeholders.join(",")
                ));
            }
            if !scope.type_keys.is_empty() {
                let mut placeholders = Vec::new();
                for type_key in &scope.type_keys {
                    let i = params.len() + 1;
                    placeholders.push(format!("?{}", i));
                    params.push(type_key.clone());
                }
                parts.push(format!(
                    "{}type_key IN ({})",
                    prefix,
                    placeholders.join(",")
                ));
            }
            if !parts.is_empty() {
                scope_clauses.push(format!("({})", parts.join(" AND ")));
            }
        }
        if !scope_clauses.is_empty() {
            clauses.push(format!("({})", scope_clauses.join(" OR ")));
        }
    }
    Ok((clauses.join(" AND "), params))
}

async fn stats(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(mut q): Query<SampleInfoQuery>,
) -> Result<Json<ApiResponse<SampleInfoStats>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "stats:portal:sample-info")?;
    if !ctx.is_system_admin() && apply_role_data_scopes(&pool, &ctx, &mut q)? {
        // Explicit role scopes intentionally narrow the data set even for a role that also
        // has a broad statistics entry permission.
    } else if ctx.is_system_admin() || ctx.has_permission("stats:sample-info:view-all") {
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
    let (wc, params) = stats_where(&q, None)?;
    let (wc_sir, _) = stats_where(&q, Some("sir"))?;
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
    use super::effective_submitter_name;

    #[test]
    fn entered_submitter_is_preserved_and_blank_falls_back_to_login() {
        assert_eq!(
            effective_submitter_name("代送样人", "login_user"),
            "代送样人"
        );
        assert_eq!(effective_submitter_name("  ", "login_user"), "login_user");
    }
}
