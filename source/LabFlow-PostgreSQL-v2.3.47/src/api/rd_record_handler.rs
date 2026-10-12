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

fn ensure_rd_execution_access(
    pool: &DbPool,
    ctx: &authz_service::AuthContext,
    record: &RdRecordResponse,
) -> Result<()> {
    let allowed = if record.execution_division_id.is_some() {
        authz_service::work_division_allowed(pool, ctx, record.execution_division_id)?
    } else if let Some(group_id) = record.group_id {
        authz_service::work_group_allowed(pool, ctx, group_id)?
    } else {
        authz_service::work_division_allowed(pool, ctx, record.division_id)?
    };
    if allowed {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "当前角色无权处理该检测部门的取样记录".into(),
        ))
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

#[derive(serde::Serialize)]
struct SampleWorkloadPreview {
    source_record_id: i64,
    operator_username: String,
    department: String,
    laboratory: String,
    project: String,
    detection_type: String,
    method: String,
    instrument: String,
    quantity: i32,
    recorded_quantity: i32,
    remaining_quantity: i32,
    coefficient: f64,
    multiplier: f64,
    recorded: bool,
}

#[derive(Deserialize)]
struct SampleWorkloadConfirm {
    quantity: i32,
    multiplier: f64,
    notes: Option<String>,
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
    pub analysis_lab_id: Option<i64>,
    pub user_name: Option<String>,
    pub division_id: Option<i64>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub include_deleted: Option<bool>,
    pub sort_by: Option<String>,
    pub sort_dir: Option<String>,
    pub column_filters: Option<String>,
    pub operation_states: Option<String>,
    pub recorder_ids: Option<String>,
    pub field: Option<String>,
    pub search: Option<String>,
    pub limit: Option<i64>,
}

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/rd-records", get(list).post(create))
        .route("/api/rd-records/filter-options", get(filter_options))
        .route(
            "/api/rd-records/:id/workload-entries",
            get(workload_entries),
        )
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
            "/api/rd-records/:id/sample-workload",
            get(sample_workload_preview).post(sample_workload_confirm),
        )
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
    Ok(Json(ApiResponse::ok(list_authorized(&pool, &ctx, q)?)))
}

fn list_authorized(
    pool: &DbPool,
    ctx: &authz_service::AuthContext,
    q: RecordQuery,
) -> Result<PaginatedResponse<RdRecordResponse>> {
    let query = authorized_query(pool, ctx, &q)?;
    let page = q.page.unwrap_or(1).max(1);
    let page_size = q.page_size.unwrap_or(50).clamp(1, 500);
    let (items, total) = rd_record_repo::list_query(
        pool,
        &query,
        page,
        page_size,
        q.sort_by.as_deref(),
        q.sort_dir.as_deref(),
    )?;
    Ok(PaginatedResponse {
        items,
        total,
        page,
        page_size,
    })
}

fn authorized_query(
    pool: &DbPool,
    ctx: &authz_service::AuthContext,
    q: &RecordQuery,
) -> Result<rd_record_repo::RdListQuery> {
    let analysis_scope = q.analysis_lab_id.is_some();
    let allowed_division_ids = if let Some(group_id) = q.analysis_lab_id {
        authz_service::require_permission(ctx, "records:rd:view-work-lab")?;
        if group_id <= 0 {
            return Err(AppError::Validation("分析实验室编号无效".into()));
        }
        if q.include_deleted.unwrap_or(false) {
            return Err(AppError::Forbidden(
                "本实验室送样记录不包含回收站记录".into(),
            ));
        }
        let allowed = authz_service::work_allowed_division_ids(pool, ctx)?;
        let visible = crate::repo::group_repo::list_for_portal(pool, Some("work"))?
            .into_iter()
            .any(|group| {
                group.id == group_id
                    && allowed.as_ref().map_or(true, |ids| {
                        group.division_id.is_some_and(|id| ids.contains(&id))
                    })
            });
        if !visible {
            return Err(AppError::Forbidden("无权访问该分析实验室".into()));
        }
        allowed
    } else {
        authz_service::rd_allowed_division_ids(pool, ctx)?
    };
    if !analysis_scope
        && !ctx.has_permission("entry:sample")
        && !ctx.has_permission("records:rd:view-all")
        && !ctx.has_permission("records:rd:view-lab")
    {
        return Err(AppError::Forbidden("无研发送样记录查看权限".into()));
    }
    let scope = if analysis_scope {
        RecordScope::Lab(q.analysis_lab_id.unwrap())
    } else if q.include_deleted.unwrap_or(false) && !ctx.has_permission("manage:trash") {
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
    let column_filters = match q.column_filters.as_deref() {
        Some(raw) if raw.len() > 65536 => {
            return Err(AppError::Validation("列筛选条件过长".into()))
        }
        Some(raw) => serde_json::from_str(raw).map_err(|_| {
            AppError::Validation("列筛选必须是字段与字符串数组组成的JSON对象".into())
        })?,
        None => std::collections::BTreeMap::new(),
    };
    let operation_states = split_filter_tokens(q.operation_states.as_deref())?;
    let recorder_ids = split_filter_tokens(q.recorder_ids.as_deref())?
        .into_iter()
        .map(|value| {
            if value == "unknown" {
                Ok(None)
            } else {
                value
                    .parse::<i64>()
                    .ok()
                    .filter(|id| *id > 0)
                    .map(Some)
                    .ok_or_else(|| AppError::Validation("录入账号筛选编号无效".into()))
            }
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(rd_record_repo::RdListQuery {
        project_id: q.project_id,
        group_id: scoped_group,
        analysis_scope,
        related_user_id: scoped_user_id,
        user_name: scoped_user.map(str::to_string),
        division_id: q.division_id,
        allowed_division_ids,
        start: q.start.clone(),
        end: q.end.clone(),
        include_deleted: q.include_deleted.unwrap_or(false),
        column_filters,
        operation_states,
        recorder_ids,
        record_id: None,
    })
}

fn split_filter_tokens(raw: Option<&str>) -> Result<Vec<String>> {
    let Some(raw) = raw else {
        return Ok(Vec::new());
    };
    if raw.len() > 4096 {
        return Err(AppError::Validation("筛选条件过长".into()));
    }
    let mut values = Vec::new();
    for value in raw
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if !values.iter().any(|existing| existing == value) {
            values.push(value.to_string());
        }
    }
    if values.len() > 100 {
        return Err(AppError::Validation("筛选条件过多".into()));
    }
    Ok(values)
}

async fn filter_options(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<RecordQuery>,
) -> Result<Json<ApiResponse<crate::models::rd_record::RdFilterOptions>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    let query = authorized_query(&pool, &ctx, &q)?;
    let field = q
        .field
        .as_deref()
        .filter(|field| !field.is_empty())
        .ok_or_else(|| AppError::Validation("请选择候选字段".into()))?;
    Ok(Json(ApiResponse::ok(rd_record_repo::filter_options(
        &pool,
        &query,
        field,
        q.search.as_deref(),
        q.limit.unwrap_or(200),
    )?)))
}

async fn workload_entries(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(q): Query<RecordQuery>,
) -> Result<Json<ApiResponse<crate::models::rd_record::RdWorkloadEntries>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(workload_entries_authorized(
        &pool, &ctx, id, &q,
    )?)))
}

fn workload_entries_authorized(
    pool: &DbPool,
    ctx: &authz_service::AuthContext,
    id: i64,
    q: &RecordQuery,
) -> Result<crate::models::rd_record::RdWorkloadEntries> {
    // The minimal recorder summary follows RD visibility; batch fields retain the existing workload range.
    let mut query = authorized_query(pool, ctx, q)?;
    query.record_id = Some(id);
    let (_, total) = rd_record_repo::list_query(pool, &query, 1, 1, None, None)?;
    if total == 0 {
        return Err(AppError::Forbidden("无权查看该研发送样记录".into()));
    }
    authz_service::require_permission(ctx, "entry:workload")?;
    if ctx.is_analysis_public_account() {
        authz_service::require_permission(ctx, "records:work:portal-scoped")?;
    }
    let (subject, creator) = match ctx.workload_scope() {
        RecordScope::Global | RecordScope::AnalysisAll => (None, None),
        RecordScope::PublicCreated => (None, Some(ctx.user.id)),
        _ => (Some(ctx.user.id), None),
    };
    let allowed = authz_service::work_allowed_division_ids(pool, ctx)?;
    rd_record_repo::workload_entries(pool, id, subject, creator, allowed.as_deref())
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
        source_type: None,
        source_record_id: None,
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
    if crate::repo::record_repo::exists_source(&pool, "rd_sample", id)? {
        return Err(AppError::Validation(
            "该取样已录入工作量，请先按工作量记录流程处理".into(),
        ));
    }
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
    ensure_rd_execution_access(&pool, &ctx, &existing)?;
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

async fn sample_workload_preview(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<SampleWorkloadPreview>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "sample:record-workload")?;
    let record = rd_record_repo::get_by_id(&pool, id)?;
    ensure_rd_execution_access(&pool, &ctx, &record)?;
    if record.status != "已取样" || record.sampled_at.is_none() {
        return Err(AppError::Validation("请先完成取样后再录入工作量".into()));
    }
    let method_id = record
        .method_id
        .ok_or_else(|| AppError::Validation("该记录未关联检测方法".into()))?;
    let method = crate::repo::method_repo::get_by_id(&pool, method_id)?;
    let laboratory = record.group_name.clone();
    let department = record
        .division_id
        .and_then(|division_id| crate::repo::division_repo::get_by_id(&pool, division_id).ok())
        .map(|division| division.name)
        .unwrap_or_default();
    Ok(Json(ApiResponse::ok(SampleWorkloadPreview {
        source_record_id: id,
        operator_username: ctx.user.username.clone(),
        department,
        laboratory,
        project: record.project_name,
        detection_type: method.type_names.join("、"),
        method: record.method_name.unwrap_or_default(),
        instrument: method.instrument_name,
        quantity: record.quantity as i32,
        recorded_quantity: crate::repo::record_repo::source_quantity(&pool, "rd_sample", id)?,
        remaining_quantity: (record.quantity as i32
            - crate::repo::record_repo::source_quantity(&pool, "rd_sample", id)?)
        .max(0),
        coefficient: method.coefficient,
        multiplier: method.multiplier,
        recorded: crate::repo::record_repo::source_fully_recorded(
            &pool,
            "rd_sample",
            id,
            record.quantity as i32,
        )?,
    })))
}

async fn sample_workload_confirm(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<SampleWorkloadConfirm>,
) -> Result<Json<ApiResponse<crate::models::record::RecordResponse>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "sample:record-workload")?;
    let record = rd_record_repo::get_by_id(&pool, id)?;
    ensure_rd_execution_access(&pool, &ctx, &record)?;
    if record.status != "已取样" || record.sampled_at.is_none() {
        return Err(AppError::Validation("请先完成取样后再录入工作量".into()));
    }
    if body.quantity <= 0 {
        return Err(AppError::Validation("本次录入数量必须大于 0".into()));
    }
    let recorded_quantity = crate::repo::record_repo::source_quantity(&pool, "rd_sample", id)?;
    let remaining_quantity = (record.quantity as i32 - recorded_quantity).max(0);
    if body.quantity > remaining_quantity {
        return Err(AppError::Validation(format!(
            "本次录入数量不能超过剩余数量 {}",
            remaining_quantity
        )));
    }
    // 工作量归属到实际点击“录入工作量”的登录账号，而不是原取样人。
    let workload = crate::models::record::RecordCreate {
        project_id: record.project_id,
        method_id: record.method_id,
        user_name: ctx.user.username.clone(),
        sender_user_id: Some(ctx.user.id),
        quantity: body.quantity,
        recorded_at: beijing_now(),
        group_id: record.group_id,
        multiplier: Some(body.multiplier),
        high_item: record.high_item,
        division_id: record.division_id,
        extra_fields: Some(
            serde_json::json!({"source_business_no": record.business_no, "source_notes": body.notes.unwrap_or_default()}),
        ),
        source_type: Some("rd_sample".into()),
        source_record_id: Some(id),
    };
    Ok(Json(ApiResponse::ok(
        crate::service::record_service::create_record(&pool, &workload, &ctx.user.username)?,
    )))
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
    ensure_rd_execution_access(&pool, &ctx, &existing)?;
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
    ensure_rd_execution_access(&pool, &ctx, &existing)?;
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

    #[test]
    fn rd_recorder_summary_does_not_grant_workload_detail_access() {
        let pool = crate::db::init_pool("postgres-test");
        crate::db::test_migrations::run(&pool.get().unwrap()).unwrap();
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO divisions(name) VALUES('rd46-detail-scope')",
            [],
        )
        .unwrap();
        let division = conn.last_insert_rowid();
        conn.execute("INSERT INTO roles(name) VALUES('rd46-detail-role')", [])
            .unwrap();
        let role = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO role_work_division_scopes(role_id,division_id) VALUES(?1,?2)",
            postgres_compat::params![role, division],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO project_groups(name,division_id) VALUES('rd46-detail-lab',?1)",
            [division],
        )
        .unwrap();
        let group = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO projects(name,group_id) VALUES('rd46-detail-project',?1)",
            [group],
        )
        .unwrap();
        let project = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO users(username,password) VALUES('rd46-detail-own','test')",
            [],
        )
        .unwrap();
        let own = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO users(username,password) VALUES('rd46-detail-other','test')",
            [],
        )
        .unwrap();
        let other = conn.last_insert_rowid();
        conn.execute("INSERT INTO rd_work_records(project_id,user_name,quantity,recorded_at,group_id,execution_division_id,status,sampler,sampled_at) VALUES(?1,'unrelated sender',4,'2097-01-01T10:00:00',?2,?3,'已取样','unrelated collector','2097-01-01T12:00:00')",postgres_compat::params![project,group,division]).unwrap();
        let source = conn.last_insert_rowid();
        for (id, quantity) in [(own, 3), (other, 1)] {
            conn.execute("INSERT INTO work_records(project_id,user_name,quantity,recorded_at,group_id,detection_division_id,subject_user_id,created_by_user_id,created_by_username_snapshot,source_type,source_record_id,business_no) VALUES(?1,'owner',?2,'2097-01-01T13:00:00',?3,?4,?5,?5,'录入账号','rd_sample',?6,?7)",postgres_compat::params![project,quantity,group,division,id,source,format!("WK46-{}",uuid::Uuid::new_v4())]).unwrap();
        }
        drop(conn);
        let q = serde_json::from_value::<RecordQuery>(serde_json::json!({"analysis_lab_id":group}))
            .unwrap();
        let mut reader = context(own, &["records:rd:view-work-lab"]);
        reader.user.role_ids = vec![role];
        let result = list_authorized(
            &pool,
            &reader,
            serde_json::from_value(
                serde_json::json!({"analysis_lab_id":group,"recorder_ids":other.to_string()}),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(result.total, 1);
        assert_eq!(
            result.items[0].workload_recorders.len(),
            2,
            "minimal identities follow the authorized RD row"
        );
        assert!(matches!(
            workload_entries_authorized(&pool, &reader, source, &q),
            Err(AppError::Forbidden(_))
        ));
        reader.user.permissions.push("entry:workload".into());
        let own_details = workload_entries_authorized(&pool, &reader, source, &q).unwrap();
        assert_eq!(own_details.total, 1);
        assert_eq!(own_details.visible_quantity, 3);
        assert!(own_details.has_hidden_entries);
        assert!(own_details
            .items
            .iter()
            .all(|entry| entry.subject_user_id == Some(own)));
        reader
            .user
            .permissions
            .push("records:work:view-scope".into());
        let scoped = workload_entries_authorized(&pool, &reader, source, &q).unwrap();
        assert_eq!(scoped.total, 2);
        assert!(!scoped.has_hidden_entries);
        assert!(matches!(
            workload_entries_authorized(&pool, &reader, source + 10000, &q),
            Err(AppError::Forbidden(_))
        ));
        reader.user.role_ids.clear();
        assert!(matches!(
            workload_entries_authorized(&pool, &reader, source, &q),
            Err(AppError::Forbidden(_))
        ));
        let invalid = serde_json::from_value::<RecordQuery>(
            serde_json::json!({"analysis_lab_id":group,"column_filters":"{\"user_name\":true}"}),
        )
        .unwrap();
        reader.user.role_ids = vec![role];
        assert!(matches!(
            authorized_query(&pool, &reader, &invalid),
            Err(AppError::Validation(_))
        ));
        let invalid = serde_json::from_value::<RecordQuery>(
            serde_json::json!({"analysis_lab_id":group,"recorder_ids":"-1"}),
        )
        .unwrap();
        assert!(matches!(
            authorized_query(&pool, &reader, &invalid),
            Err(AppError::Validation(_))
        ));
    }

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
            workload_recorded: false,
            recorded_quantity: 0,
            workload_recorders: Vec::new(),
            workload_unknown_recorder_entries: 0,
            workload_unknown_recorder_quantity: 0,
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
    #[test]
    fn rd_sampling_uses_detection_scope_not_empty_rd_scope() {
        let pool = crate::db::init_pool("postgres-test");
        let conn = pool.get().unwrap();
        crate::db::test_migrations::run(&conn).unwrap();
        conn.execute(
            "INSERT INTO divisions(name,is_active) VALUES('rd_execution_allowed',1)",
            [],
        )
        .unwrap();
        let division = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO roles(name,is_system) VALUES('rd_execution_role',0)",
            [],
        )
        .unwrap();
        let role = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO role_work_division_scopes(role_id,division_id) VALUES(?1,?2)",
            postgres_compat::params![role, division],
        )
        .unwrap();
        let mut ctx = context(99, &["sample:collect"]);
        ctx.user.role_ids = vec![role];
        let mut item = record("待取样", 1, 2);
        item.execution_division_id = Some(division);
        assert!(ensure_rd_execution_access(&pool, &ctx, &item).is_ok());
        item.execution_division_id = Some(division + 1000);
        assert!(ensure_rd_execution_access(&pool, &ctx, &item).is_err());
        ctx.user.is_admin = true;
        assert!(ensure_rd_execution_access(&pool, &ctx, &item).is_ok());
    }

    #[test]
    fn analysis_lab_rd_list_filters_execution_scope_dates_and_pagination() {
        let pool = crate::db::init_pool("postgres-test");
        let conn = pool.get().unwrap();
        crate::db::test_migrations::run(&conn).unwrap();
        conn.execute(
            "INSERT INTO divisions(name) VALUES('analysis_rd_allowed')",
            [],
        )
        .unwrap();
        let allowed = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO divisions(name) VALUES('analysis_rd_denied')",
            [],
        )
        .unwrap();
        let denied = conn.last_insert_rowid();
        conn.execute("INSERT INTO roles(name) VALUES('analysis_rd_reader')", [])
            .unwrap();
        let role = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO role_work_division_scopes(role_id,division_id) VALUES(?1,?2)",
            postgres_compat::params![role, allowed],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO project_groups(name,division_id) VALUES('analysis_rd_lab',?1)",
            [allowed],
        )
        .unwrap();
        let lab = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO project_groups(name,division_id) VALUES('analysis_rd_other_lab',?1)",
            [allowed],
        )
        .unwrap();
        let other_lab = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO project_groups(name,division_id) VALUES('analysis_rd_denied_lab',?1)",
            [denied],
        )
        .unwrap();
        let denied_lab = conn.last_insert_rowid();
        conn.execute("INSERT INTO project_groups(name,division_id,show_in_work) VALUES('analysis_rd_hidden_lab',?1,0)", [allowed]).unwrap();
        let hidden_lab = conn.last_insert_rowid();
        conn.execute("INSERT INTO project_groups(name,division_id,deleted_at) VALUES('analysis_rd_deleted_lab',?1,'2092-10-01')", [allowed]).unwrap();
        let deleted_lab = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO projects(name,group_id) VALUES('analysis_rd_project',?1)",
            [lab],
        )
        .unwrap();
        let project = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO users(username,password) VALUES('analysis_rd_sender','test')",
            [],
        )
        .unwrap();
        let sender = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO users(username,password) VALUES('analysis_rd_creator','test')",
            [],
        )
        .unwrap();
        let creator = conn.last_insert_rowid();
        let insert = |group, execution: Option<i64>, time: &str, deleted: Option<&str>| {
            let business_no = format!("V45-RD-{}", uuid::Uuid::new_v4());
            conn.execute("INSERT INTO rd_work_records(project_id,user_name,quantity,recorded_at,group_id,execution_division_id,division_id,subject_user_id,created_by_user_id,deleted_at,business_no) VALUES(?1,'unrelated_sender',1,?2,?3,?4,?5,?6,?7,?8,?9)", postgres_compat::params![project,time,group,execution,denied,sender,creator,deleted,business_no]).unwrap();
            conn.last_insert_rowid()
        };
        insert(lab, Some(allowed), "2092-10-04T23:59:59.999999", None);
        let first = insert(lab, Some(allowed), "2092-10-05T00:00:00", None);
        let last = insert(lab, Some(allowed), "2092-10-11 23:59:59.999999", None);
        insert(lab, Some(allowed), "2092-10-12T00:00:00", None);
        insert(lab, Some(denied), "2092-10-06T12:00:00", None);
        let legacy = insert(lab, None, "2092-10-07T12:00:00", None);
        insert(
            lab,
            Some(allowed),
            "2092-10-07T12:00:00",
            Some("2092-10-08"),
        );
        let precise = insert(lab, Some(allowed), "2092-10-11T12:00:00.100", None);
        insert(other_lab, Some(allowed), "2092-10-08T12:00:00", None);
        let invalid_ids = ["", "not-a-date", "2092-02-31T10:00:00", "today"]
            .map(|time| insert(lab, Some(allowed), time, None));
        let mut ctx = context(99, &["records:rd:view-work-lab"]);
        ctx.user.role_ids = vec![role];
        ctx.user.group_id = Some(other_lab);
        let query = |value| serde_json::from_value::<RecordQuery>(value).unwrap();
        let unfiltered = list_authorized(
            &pool,
            &ctx,
            query(serde_json::json!({"analysis_lab_id":lab})),
        )
        .unwrap();
        assert_eq!(unfiltered.total, 10);
        assert!(invalid_ids
            .iter()
            .all(|id| unfiltered.items.iter().any(|item| item.id == *id)));
        let classified = list_authorized(
            &pool,
            &ctx,
            query(
                serde_json::json!({"analysis_lab_id":lab,"start":"1900-01-01","end":"2200-01-01"}),
            ),
        )
        .unwrap();
        assert_eq!(classified.total, 6);
        let range = serde_json::json!({"analysis_lab_id":lab,"group_id":other_lab,"start":"2092-10-05","end":"2092-10-11","page_size":2});
        let page_one = list_authorized(&pool, &ctx, query(range.clone())).unwrap();
        assert_eq!(page_one.total, 4);
        assert_eq!(page_one.items.len(), 2);
        let mut page_two_query = range.clone();
        page_two_query["page"] = 2.into();
        let page_two = list_authorized(&pool, &ctx, query(page_two_query)).unwrap();
        assert_eq!(page_two.total, 4);
        let mut ids = page_one
            .items
            .iter()
            .chain(&page_two.items)
            .map(|item| item.id)
            .collect::<Vec<_>>();
        ids.sort_unstable();
        let mut expected = vec![first, last, legacy, precise];
        expected.sort_unstable();
        assert_eq!(ids, expected);
        let precise_page = list_authorized(&pool, &ctx, query(serde_json::json!({"analysis_lab_id":lab,"start":"2092-10-11T12:00:00.1","end":"2092-10-11 12:00:00.100"}))).unwrap();
        assert_eq!(precise_page.total, 1);
        assert_eq!(precise_page.items[0].id, precise);
        for forbidden_lab in [denied_lab, hidden_lab, deleted_lab, lab + 10000] {
            assert!(matches!(
                list_authorized(
                    &pool,
                    &ctx,
                    query(serde_json::json!({"analysis_lab_id":forbidden_lab}))
                ),
                Err(AppError::Forbidden(_))
            ));
        }
        assert!(matches!(
            list_authorized(&pool, &ctx, query(serde_json::json!({"analysis_lab_id":0}))),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            list_authorized(
                &pool,
                &ctx,
                query(serde_json::json!({"analysis_lab_id":lab,"include_deleted":true}))
            ),
            Err(AppError::Forbidden(_))
        ));
        assert!(matches!(
            list_authorized(&pool, &ctx, query(serde_json::json!({}))),
            Err(AppError::Forbidden(_))
        ));
        ctx.user.permissions.clear();
        assert!(matches!(
            list_authorized(&pool, &ctx, query(range.clone())),
            Err(AppError::Forbidden(_))
        ));
        ctx.user.permissions = vec!["records:rd:view-work-lab".into()];
        ctx.user.role_ids.clear();
        assert!(matches!(
            list_authorized(&pool, &ctx, query(range.clone())),
            Err(AppError::Forbidden(_))
        ));
        // Public accounts see senders in their configured departments, not only
        // records belonging to the selected actual detector in the URL.
        conn.execute("INSERT INTO users(username,password,division_id) VALUES('analysis_rd_public','test',?1)", [allowed]).unwrap();
        ctx.user.id = conn.last_insert_rowid();
        ctx.user.role_keys = vec![authz_service::ROLE_KEY_ANALYSIS_PUBLIC_ACCOUNT.into()];
        let mut public_query = range;
        public_query["subject_user_id"] = 999.into();
        assert_eq!(
            list_authorized(&pool, &ctx, query(public_query))
                .unwrap()
                .total,
            4
        );
        let unrelated = record("待取样", 100, 101);
        assert!(!can_edit_related_record(&ctx, &unrelated));
        assert!(!can_confirm_related_return(&ctx, &unrelated));
    }
}
