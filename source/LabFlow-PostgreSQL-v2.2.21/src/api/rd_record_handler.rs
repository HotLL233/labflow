use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::rd_record::{RdRecordCreate, RdRecordResponse};
use crate::models::rd_record_column::RdOptionDetailRule;
use crate::models::record::{RecordCreate, RecordUpdate};
use crate::models::trash::DeleteReasonRequest;
use crate::models::ApiResponse;
use crate::models::PaginatedResponse;
use crate::repo::rd_record_repo;
use crate::service::authz_service::{self, RecordScope};
use crate::service::notification_service;
use crate::service::rd_record_service;
use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::get,
    Json, Router,
};
use chrono::{FixedOffset, Utc};
use serde::Deserialize;

fn can_access_entry_group(ctx: &authz_service::AuthContext, group_id: i64) -> bool {
    ctx.is_system_admin()
        || ctx.has_permission("records:rd:portal-all-labs")
        || ctx.user.group_id == Some(group_id)
}

fn can_edit_related_record(ctx: &authz_service::AuthContext, record: &RdRecordResponse) -> bool {
    ctx.is_system_admin()
        || (record.created_by_user_id == Some(ctx.user.id)
            && ctx.has_permission("records:rd:edit-created"))
        || (record.subject_user_id == Some(ctx.user.id)
            && ctx.has_permission("records:rd:edit-subject"))
}

fn ensure_rd_record_department_access(
    pool: &DbPool,
    ctx: &authz_service::AuthContext,
    record: &RdRecordResponse,
) -> Result<()> {
    let allowed = match record.group_id {
        Some(group_id) => authz_service::rd_group_allowed(pool, ctx, group_id)?,
        None => authz_service::rd_division_allowed(pool, ctx, record.division_id)?,
    };
    if allowed {
        Ok(())
    } else {
        Err(AppError::Forbidden("当前角色无权访问该记录所属部门".into()))
    }
}

fn can_confirm_related_return(ctx: &authz_service::AuthContext, record: &RdRecordResponse) -> bool {
    ctx.has_permission("records:rd:return-delegate")
        || (ctx.has_permission("records:rd:return-confirm")
            && (record.created_by_user_id == Some(ctx.user.id)
                || record.subject_user_id == Some(ctx.user.id)))
}

fn can_edit_delegated_return_draft(
    ctx: &authz_service::AuthContext,
    record: &RdRecordResponse,
) -> bool {
    record.status == "退回待修改" && ctx.has_permission("records:rd:return-delegate")
}

#[derive(Deserialize)]
struct ReturnRecordRequest {
    reason: String,
}

#[derive(Deserialize)]
struct WithdrawSampleRequest {
    reason: String,
    subject_user_id: Option<i64>,
}

#[derive(Deserialize)]
struct SampleRequest {
    subject_user_id: Option<i64>,
}

fn beijing_now() -> String {
    Utc::now()
        .with_timezone(&FixedOffset::east_opt(8 * 60 * 60).expect("UTC+8 is valid"))
        .format("%Y-%m-%dT%H:%M:%S")
        .to_string()
}

fn configured_types(source: &str) -> Vec<String> {
    let value = source.trim();
    if value.is_empty() {
        return Vec::new();
    }
    if let Ok(values) = serde_json::from_str::<Vec<String>>(value) {
        return values
            .into_iter()
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect();
    }
    value
        .split(|ch| ch == ',' || ch == '，' || ch == '\n')
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

fn validate_configured_fields(pool: &DbPool, body: &RdRecordCreate) -> Result<()> {
    let extra = body
        .extra_fields
        .as_ref()
        .and_then(|value| value.as_object());
    for column in crate::repo::rd_record_column_repo::list_active_in_form(pool)? {
        let types = configured_types(&column.applicable_types);
        if !types.is_empty() && !types.iter().any(|item| item == body.detection_type.trim()) {
            continue;
        }
        let value = match column.name.as_str() {
            "batch_no" => body.batch_no.clone().unwrap_or_default(),
            "notes" => body.notes.clone().unwrap_or_default(),
            key => extra
                .and_then(|values| values.get(key))
                .map(|value| match value {
                    serde_json::Value::String(text) => text.clone(),
                    serde_json::Value::Null => String::new(),
                    other => other.to_string(),
                })
                .unwrap_or_default(),
        };
        if column.is_required
            && ![
                "user_name",
                "project_name",
                "detection_type",
                "method_name",
                "quantity",
            ]
            .contains(&column.name.as_str())
            && value.trim().is_empty()
        {
            return Err(AppError::Validation(format!("请填写{}", column.label)));
        }
        let rules: Vec<RdOptionDetailRule> =
            serde_json::from_str(&column.option_detail_rules).unwrap_or_default();
        let (selected, legacy_detail) = value
            .split_once('：')
            .filter(|(selected, _)| {
                rules
                    .iter()
                    .any(|rule| rule.trigger_value == selected.trim())
            })
            .map(|(selected, detail)| (selected.trim().to_string(), detail.trim().to_string()))
            .unwrap_or_else(|| (value.trim().to_string(), String::new()));
        if let Some(rule) = rules.iter().find(|rule| rule.trigger_value == selected) {
            let detail_key = format!("{}__detail", column.name);
            let detail = extra
                .and_then(|values| values.get(&detail_key))
                .and_then(|value| value.as_str())
                .unwrap_or(&legacy_detail);
            if rule.required && detail.trim().is_empty() {
                return Err(AppError::Validation(format!("请填写{}", rule.label)));
            }
        }
    }
    Ok(())
}

/// Keep option-triggered required supplemental fields valid during record edits.
fn validate_updated_option_details(
    pool: &DbPool,
    existing: &RdRecordResponse,
    body: &crate::models::record::RecordUpdate,
) -> Result<()> {
    let method_types = existing
        .method_type
        .as_deref()
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let extra_value = body
        .extra_fields
        .clone()
        .or_else(|| existing.extra_fields.clone())
        .unwrap_or_else(|| serde_json::json!({}));
    let extra = extra_value.as_object();
    let notes = body
        .notes
        .as_deref()
        .or(existing.notes.as_deref())
        .unwrap_or_default();
    for column in crate::repo::rd_record_column_repo::list_active_in_form(pool)? {
        let configured = configured_types(&column.applicable_types);
        if !configured.is_empty()
            && !method_types
                .iter()
                .any(|current| configured.iter().any(|value| value == current))
        {
            continue;
        }
        let rules: Vec<RdOptionDetailRule> =
            serde_json::from_str(&column.option_detail_rules).unwrap_or_default();
        if rules.is_empty() {
            continue;
        }
        let value = if column.name == "notes" {
            notes.to_string()
        } else {
            extra
                .and_then(|values| values.get(&column.name))
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string()
        };
        let (selected, legacy_detail) = value
            .split_once('：')
            .filter(|(selected, _)| {
                rules
                    .iter()
                    .any(|rule| rule.trigger_value == selected.trim())
            })
            .map(|(selected, detail)| (selected.trim().to_string(), detail.trim().to_string()))
            .unwrap_or_else(|| (value.trim().to_string(), String::new()));
        if let Some(rule) = rules.iter().find(|rule| rule.trigger_value == selected) {
            let key = format!("{}__detail", column.name);
            let detail = extra
                .and_then(|values| values.get(&key))
                .and_then(|value| value.as_str())
                .unwrap_or(&legacy_detail);
            if rule.required && detail.trim().is_empty() {
                return Err(AppError::Validation(format!("请填写{}", rule.label)));
            }
        }
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct RecordQuery {
    pub project_id: Option<i64>,
    pub group_id: Option<i64>,
    pub user_name: Option<String>,
    pub division_id: Option<i64>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub include_deleted: Option<bool>,
    pub sort_by: Option<String>,
    pub sort_dir: Option<String>,
}

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/rd-records", get(list).post(create))
        .route(
            "/api/rd-records/:id",
            axum::routing::put(update).delete(soft_delete),
        )
        .route(
            "/api/rd-records/:id/return",
            axum::routing::post(return_record),
        )
        .route(
            "/api/rd-records/:id/confirm-return",
            axum::routing::post(confirm_return),
        )
        .route("/api/rd-records/:id/sample", axum::routing::put(sample))
        .route(
            "/api/rd-records/:id/withdraw-sample",
            axum::routing::post(withdraw_sample),
        )
        .route("/api/rd-records/restore/:id", axum::routing::post(restore))
        .route(
            "/api/rd-records/by-user/:user_name",
            axum::routing::delete(delete_by_user),
        )
        .with_state(pool)
}

async fn list(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<RecordQuery>,
) -> Result<Json<ApiResponse<PaginatedResponse<RdRecordResponse>>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    if !ctx.has_permission("entry:sample")
        && !ctx.has_permission("records:rd:view-all")
        && !ctx.has_permission("records:rd:view-lab")
    {
        return Err(AppError::Forbidden("无研发送样记录查看权限".into()));
    }
    let page = q.page.unwrap_or(1);
    let page_size = q.page_size.unwrap_or(50).min(500);
    let scope = if q.include_deleted.unwrap_or(false) && !ctx.has_permission("manage:trash") {
        RecordScope::Own
    } else {
        ctx.rd_scope()?
    };
    let scoped_group = match scope {
        RecordScope::Lab(group_id) => Some(group_id),
        _ => q.group_id,
    };
    let scoped_user = match scope {
        RecordScope::Own => None,
        _ => q.user_name.as_deref(),
    };
    let scoped_user_id = if matches!(scope, RecordScope::Own) {
        Some(ctx.user.id)
    } else {
        None
    };
    let allowed_division_ids = authz_service::rd_allowed_division_ids(&pool, &ctx)?;
    let (items, total) = rd_record_repo::list(
        &pool,
        q.project_id,
        scoped_group,
        scoped_user_id,
        scoped_user,
        q.division_id,
        allowed_division_ids.as_deref(),
        q.start.as_deref(),
        q.end.as_deref(),
        page,
        page_size,
        q.include_deleted.unwrap_or(false),
        q.sort_by.as_deref(),
        q.sort_dir.as_deref(),
    )?;
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
    Json(body): Json<RdRecordCreate>,
) -> Result<Json<ApiResponse<RdRecordResponse>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "entry:sample")?;
    let method_id = body
        .method_id
        .ok_or_else(|| AppError::Validation("请选择检测方法".into()))?;
    let detection_type = body.detection_type.trim();
    if detection_type.is_empty() {
        return Err(AppError::Validation("请选择检测类型".into()));
    }
    validate_configured_fields(&pool, &body)?;
    let group_id = body
        .group_id
        .ok_or_else(|| AppError::Validation("请选择实验室".into()))?;
    if !can_access_entry_group(&ctx, group_id) {
        return Err(AppError::Forbidden("无权在该实验室录入送样记录".into()));
    }
    if !authz_service::rd_group_allowed(&pool, &ctx, group_id)? {
        return Err(AppError::Forbidden(
            "当前角色无权访问该实验室所属部门".into(),
        ));
    }
    if !authz_service::rd_project_party_allowed(&pool, &ctx, body.project_id)? {
        return Err(AppError::Forbidden(
            "当前部门不是该项目的归属部门或协作部门，无法提交研发送样".into(),
        ));
    }
    let execution_division_id =
        authz_service::require_project_execution_group(&pool, body.project_id, group_id)?;
    if let Some(requested_division_id) = body.division_id {
        if requested_division_id != execution_division_id {
            return Err(AppError::Validation(
                "所选部门与实验室所属执行部门不一致".into(),
            ));
        }
    }
    let requested_sender_id = body.sender_user_id.unwrap_or(ctx.user.id);
    if requested_sender_id != ctx.user.id && !ctx.has_permission("records:rd:select-sender") {
        return Err(AppError::Forbidden("缺少代送样人员选择权限".into()));
    }
    let sender = crate::repo::user_repo::find_by_id(&pool, requested_sender_id)?
        .ok_or_else(|| AppError::Validation("所选送样人不存在".into()))?;
    if !sender.is_active
        || sender.group_id != Some(group_id)
        || !sender
            .permissions
            .iter()
            .any(|p| p == "*" || p == "entry:sample")
    {
        return Err(AppError::Validation(
            "所选送样人不属于当前实验室，或未开通研发送样权限".into(),
        ));
    }
    {
        let conn = pool.get()?;
        crate::repo::rd_record_repo::validate_submission_selection(
            &conn,
            body.project_id,
            method_id,
            detection_type,
        )?;
    }
    crate::repo::method_repo::ensure_method_visible_for_group(&pool, method_id, group_id, "rd")?;
    let record = RecordCreate {
        project_id: body.project_id,
        method_id: Some(method_id),
        // The visible sender may differ from the login account. Creation and
        // authorization identity remain in the immutable created_by fields.
        user_name: sender.username,
        sender_user_id: Some(sender.id),
        quantity: body.quantity,
        // 送样时间以服务器提交时刻为准，客户端传入值仅为兼容旧接口保留。
        recorded_at: beijing_now(),
        group_id: Some(group_id),
        multiplier: None,
        // The selected laboratory is the authority for execution ownership.
        division_id: Some(execution_division_id),
        high_item: None,
        extra_fields: body.extra_fields,
    };
    // Service layer: validates quantity > 0 and project existence
    let result = rd_record_service::create_record(
        &pool,
        &record,
        body.batch_no,
        body.notes,
        ctx.user.id,
        &ctx.user.username,
    )?;
    // Queue only after the business record is committed; notification failures never block sending.
    notification_service::enqueue_rd_submitted(&pool, &result)?;
    let notification_pool = pool.clone();
    tokio::task::spawn_blocking(move || {
        if let Err(error) = notification_service::process_pending(&notification_pool, 20) {
            tracing::warn!("rd sample notification processing failed: {error}");
        }
    });
    Ok(Json(ApiResponse::ok(result)))
}

async fn update(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<RecordUpdate>,
) -> Result<Json<ApiResponse<RdRecordResponse>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    let existing = rd_record_repo::get_by_id(&pool, id)?;
    ensure_rd_record_department_access(&pool, &ctx, &existing)?;
    if !can_edit_related_record(&ctx, &existing)
        && !can_edit_delegated_return_draft(&ctx, &existing)
    {
        return Err(AppError::Forbidden(
            "只能修改本人相关记录，或代处理已确认退回的待修改记录".into(),
        ));
    }
    // v0.4.34: 已取样记录不可修改
    if rd_record_repo::is_sampled(&pool, id)? {
        return Err(crate::error::AppError::Forbidden(
            "该记录已取样，不可修改".to_string(),
        ));
    }
    let final_project_id = body.project_id.unwrap_or(existing.project_id);
    let final_group_id = body.group_id.or(existing.group_id);
    if let Some(group_id) = final_group_id {
        if !can_access_entry_group(&ctx, group_id) {
            return Err(AppError::Forbidden(
                "无权将研发送样记录调整到该实验室".into(),
            ));
        }
        if !authz_service::rd_group_allowed(&pool, &ctx, group_id)? {
            return Err(AppError::Forbidden(
                "当前角色无权访问目标实验室所属部门".into(),
            ));
        }
        if !authz_service::rd_project_party_allowed(&pool, &ctx, final_project_id)? {
            return Err(AppError::Forbidden(
                "当前部门不是目标项目的归属部门或协作部门".into(),
            ));
        }
        let execution_division_id =
            authz_service::require_project_execution_group(&pool, final_project_id, group_id)?;
        if let Some(requested_division_id) = body.division_id {
            if requested_division_id != execution_division_id {
                return Err(AppError::Validation(
                    "所选部门与实验室所属执行部门不一致".into(),
                ));
            }
        }
    }
    validate_updated_option_details(&pool, &existing, &body)?;
    let result = rd_record_service::update_record(&pool, id, &body, &ctx.user.username)?;
    if existing.status == "退回待修改" && result.status == "待取样" && result.quantity != 0
    {
        if let Err(error) = notification_service::enqueue_rd_resubmitted(&pool, &result) {
            tracing::warn!(record_id = id, %error, "退回后重新提交通知入队失败");
        } else if let Err(error) = notification_service::process_pending(&pool, 20) {
            tracing::warn!(record_id = id, %error, "退回后重新提交通知处理失败");
        }
    }
    Ok(Json(ApiResponse::ok(result)))
}

async fn soft_delete(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(body): Query<DeleteReasonRequest>,
) -> Result<Json<ApiResponse<()>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "entry:sample")?;
    let existing = rd_record_repo::get_by_id(&pool, id)?;
    ensure_rd_record_department_access(&pool, &ctx, &existing)?;
    if !ctx.is_system_admin() && existing.created_by_user_id != Some(ctx.user.id) {
        return Err(AppError::Forbidden("只能删除本人提交的研发送样记录".into()));
    }
    let reason = body
        .reason
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("用户删除");
    rd_record_service::delete_record(&pool, id, &ctx.user.username, reason)?;
    Ok(Json(ApiResponse::ok_msg("删除成功")))
}

async fn restore(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<RdRecordResponse>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    let existing = rd_record_repo::get_by_id(&pool, id)?;
    ensure_rd_record_department_access(&pool, &ctx, &existing)?;
    let can_restore = ctx.is_system_admin()
        || can_edit_related_record(&ctx, &existing)
        || (ctx.has_permission("manage:trash")
            && ctx.has_permission("records:rd:view-lab")
            && existing.group_id == ctx.user.group_id);
    if !can_restore {
        return Err(AppError::Forbidden("只能恢复本人提交的研发送样记录".into()));
    }
    Ok(Json(ApiResponse::ok(rd_record_repo::restore(
        &pool,
        id,
        &ctx.user.username,
    )?)))
}

#[derive(Deserialize)]
pub struct DeleteByUserQuery {
    pub start: Option<String>,
    pub end: Option<String>,
    pub reason: Option<String>,
}

async fn delete_by_user(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(user_name): Path<String>,
    Query(q): Query<DeleteByUserQuery>,
) -> Result<Json<ApiResponse<serde_json::Value>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "entry:sample")?;
    if !ctx.is_system_admin() && user_name != ctx.user.username {
        return Err(AppError::Forbidden("只能批量删除本人提交的记录".into()));
    }
    if let Some(ids) = authz_service::rd_allowed_division_ids(&pool, &ctx)? {
        let conn = pool.get()?;
        let values = ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
        let mut sql = format!(
            "SELECT COUNT(*) FROM rd_work_records wr
             WHERE wr.deleted_at IS NULL AND wr.user_name=?1
               AND NOT (COALESCE(wr.execution_division_id,wr.division_id,(SELECT division_id FROM project_groups WHERE id=wr.group_id)) IN ({values}))"
        );
        let mut params: Vec<Box<dyn postgres_compat::types::ToSql>> =
            vec![Box::new(user_name.clone())];
        if let Some(start) = &q.start {
            sql.push_str(" AND wr.recorded_at>=?2");
            params.push(Box::new(start.clone()));
        }
        if let Some(end) = &q.end {
            let index = params.len() + 1;
            sql.push_str(&format!(" AND wr.recorded_at<=?{index}"));
            params.push(Box::new(format!("{end}T23:59:59")));
        }
        let outside_scope: i64 = conn.query_row(
            &sql,
            postgres_compat::params_from_iter(params.iter().map(|value| value.as_ref())),
            |row| row.get(0),
        )?;
        if outside_scope > 0 {
            return Err(AppError::Forbidden("批量删除范围包含未授权部门记录".into()));
        }
    }
    let count = rd_record_repo::delete_by_user(
        &pool,
        &user_name,
        q.start.as_deref(),
        q.end.as_deref(),
        &ctx.user.username,
        q.reason
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("批量删除"),
    )?;
    Ok(Json(ApiResponse::ok(
        serde_json::json!({"deleted_count": count}),
    )))
}

async fn sample(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<SampleRequest>,
) -> Result<Json<ApiResponse<RdRecordResponse>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "sample:collect")?;
    let existing = rd_record_repo::get_by_id(&pool, id)?;
    ensure_rd_record_department_access(&pool, &ctx, &existing)?;
    let sampler = if ctx.is_analysis_public_account() {
        let subject_user_id = body
            .subject_user_id
            .ok_or_else(|| AppError::Validation("请选择实际分析人员".into()))?;
        if !crate::repo::user_repo::analysis_public_account_candidate_allowed(
            &pool,
            ctx.user.id,
            subject_user_id,
        )? {
            return Err(AppError::Forbidden(
                "所选账号不属于当前公共账号可用人员范围".into(),
            ));
        }
        let subject = crate::repo::user_repo::find_by_id(&pool, subject_user_id)?
            .ok_or_else(|| AppError::Validation("所选分析人员不存在".into()))?;
        if !subject.is_active {
            return Err(AppError::Validation("所选分析人员已停用".into()));
        }
        subject.username
    } else {
        if body.subject_user_id.is_some() {
            return Err(AppError::Forbidden(
                "非分析公共账号不能指定实际分析人员".into(),
            ));
        }
        ctx.user.username.clone()
    };
    let result = rd_record_service::sample(&pool, id, &sampler, &ctx.user.username)?;
    Ok(Json(ApiResponse::ok(result)))
}

async fn withdraw_sample(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<WithdrawSampleRequest>,
) -> Result<Json<ApiResponse<RdRecordResponse>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "sample:withdraw")?;
    let existing = rd_record_repo::get_by_id(&pool, id)?;
    ensure_rd_record_department_access(&pool, &ctx, &existing)?;
    let is_original_sampler = if ctx.is_analysis_public_account() {
        let subject_user_id = body
            .subject_user_id
            .ok_or_else(|| AppError::Validation("请选择实际分析人员".into()))?;
        if !crate::repo::user_repo::analysis_public_account_candidate_allowed(
            &pool,
            ctx.user.id,
            subject_user_id,
        )? {
            return Err(AppError::Forbidden(
                "所选账号不属于当前公共账号可用人员范围".into(),
            ));
        }
        let subject = crate::repo::user_repo::find_by_id(&pool, subject_user_id)?
            .ok_or_else(|| AppError::Validation("所选分析人员不存在".into()))?;
        existing.sampler.as_deref() == Some(subject.username.as_str())
    } else {
        if body.subject_user_id.is_some() {
            return Err(AppError::Forbidden(
                "非分析公共账号不能指定实际分析人员".into(),
            ));
        }
        existing.sampler.as_deref() == Some(ctx.user.username.as_str())
    };
    if !ctx.is_system_admin() && !ctx.is_analysis_leader() && !is_original_sampler {
        return Err(AppError::Forbidden(
            "仅原取样人、分析检测组长或系统管理员可以撤回取样".into(),
        ));
    }
    let result = rd_record_service::withdraw_sample(&pool, id, &body.reason, &ctx.user.username)?;
    Ok(Json(ApiResponse::ok(result)))
}

async fn return_record(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<ReturnRecordRequest>,
) -> Result<Json<ApiResponse<RdRecordResponse>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "sample:return")?;
    let existing = rd_record_repo::get_by_id(&pool, id)?;
    ensure_rd_record_department_access(&pool, &ctx, &existing)?;
    let record = rd_record_repo::return_record(&pool, id, &body.reason, &ctx.user.username)?;
    if let Err(error) = notification_service::enqueue_rd_rejected(&pool, &record) {
        tracing::warn!(
            record_id = record.id,
            "failed to queue rd rejection notification: {error}"
        );
    } else {
        let notification_pool = pool.clone();
        tokio::task::spawn_blocking(move || {
            if let Err(error) = notification_service::process_pending(&notification_pool, 20) {
                tracing::warn!("rd rejection notification processing failed: {error}");
            }
        });
    }
    Ok(Json(ApiResponse::ok(record)))
}

async fn confirm_return(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<RdRecordResponse>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    let existing = rd_record_repo::get_by_id(&pool, id)?;
    ensure_rd_record_department_access(&pool, &ctx, &existing)?;
    if !can_confirm_related_return(&ctx, &existing) {
        return Err(AppError::Forbidden(
            "仅提交账号、实际送样人或具有代确认修改权限的人员可以确认退回".into(),
        ));
    }
    let record = rd_record_repo::confirm_return(&pool, id, &ctx.user.username)?;
    Ok(Json(ApiResponse::ok(record)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(user_id: i64, permissions: &[&str]) -> authz_service::AuthContext {
        authz_service::AuthContext {
            user: crate::models::user::User {
                id: user_id,
                username: format!("user-{user_id}"),
                password: String::new(),
                division_id: None,
                division_name: None,
                primary_division_id: None,
                primary_division_name: None,
                division_ids: vec![],
                division_names: vec![],
                business_division_ids: vec![],
                business_division_names: vec![],
                group_id: None,
                group_name: None,
                group_ids: vec![],
                group_names: vec![],
                is_admin: false,
                is_active: true,
                role_id: None,
                role_name: None,
                role_ids: vec![],
                role_names: vec![],
                role_keys: vec![],
                role_template_names: vec![],
                is_public_account: false,
                is_rd_public_account: false,
                is_analysis_public_account: false,
                affiliation_groups: vec![],
                permissions: permissions
                    .iter()
                    .map(|value| (*value).to_string())
                    .collect(),
                created_at: String::new(),
                updated_at: None,
            },
            role_names: vec![],
        }
    }

    fn record(status: &str, creator_id: i64, sender_id: i64) -> RdRecordResponse {
        RdRecordResponse {
            id: 1,
            business_no: "RD-TEST-001".into(),
            project_id: 1,
            method_id: None,
            project_name: String::new(),
            group_name: String::new(),
            user_name: String::new(),
            quantity: 1,
            recorded_at: String::new(),
            last_activity_at: String::new(),
            batch_no: None,
            notes: None,
            created_at: String::new(),
            deleted_at: None,
            method_name: None,
            method_type: None,
            instrument_code: String::new(),
            instrument_type: String::new(),
            status: status.into(),
            return_reason: String::new(),
            returned_by: String::new(),
            returned_at: None,
            return_confirmed_at: None,
            return_confirmed_by: String::new(),
            voided_at: None,
            voided_by: String::new(),
            void_reason: String::new(),
            sampler: None,
            sampled_at: None,
            detected_by: None,
            detected_at: None,
            division_id: None,
            group_id: None,
            high_item: None,
            coefficient_snapshot: 1.0,
            subject_user_id: Some(sender_id),
            created_by_user_id: Some(creator_id),
            extra_fields: None,
            sequence_no: 1,
            project_division_id: None,
            execution_division_id: None,
            execution_group_id: None,
        }
    }

    #[test]
    fn delegated_return_permission_only_handles_return_confirmation_and_draft_editing() {
        let delegate = context(99, &["records:rd:return-delegate"]);
        let returned = record("已退回", 1, 2);
        let draft = record("退回待修改", 1, 2);
        let ordinary = record("待取样", 1, 2);

        assert!(can_confirm_related_return(&delegate, &returned));
        assert!(can_edit_delegated_return_draft(&delegate, &draft));
        assert!(!can_edit_delegated_return_draft(&delegate, &ordinary));
        assert!(!can_edit_related_record(&delegate, &draft));
    }

    #[test]
    fn ordinary_return_confirmation_permission_stays_limited_to_related_users() {
        let unrelated = context(99, &["records:rd:return-confirm"]);
        let sender = context(2, &["records:rd:return-confirm"]);
        let returned = record("已退回", 1, 2);

        assert!(!can_confirm_related_return(&unrelated, &returned));
        assert!(can_confirm_related_return(&sender, &returned));
    }
}
