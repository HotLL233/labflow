use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::record::{RecordCreate, RecordMaintenanceTarget, RecordResponse, RecordUpdate};
use crate::models::ApiResponse;
use crate::models::PaginatedResponse;
use crate::repo::record_repo;
use crate::service::authz_service::{self, RecordScope};
use crate::service::record_service;
use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
pub struct RecordQuery {
    pub record_id: Option<i64>,
    pub project_id: Option<i64>,
    pub group_id: Option<i64>,
    pub subject_user_id: Option<i64>,
    pub user_name: Option<String>,
    pub division_id: Option<i64>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub include_deleted: Option<bool>,
    pub detection_division_ids: Option<String>,
    pub sending_division_ids: Option<String>,
    pub include_pending_ownership: Option<bool>,
}

fn scoped_record_subject(
    scope: RecordScope,
    user_id: i64,
    requested: Option<i64>,
) -> Result<Option<i64>> {
    if requested.is_some_and(|id| id <= 0) {
        return Err(AppError::Validation("人员筛选值无效".into()));
    }
    Ok(match scope {
        RecordScope::Global | RecordScope::AnalysisAll | RecordScope::PublicCreated => requested,
        _ => Some(user_id),
    })
}

fn can_mutate_record(
    ctx: &authz_service::AuthContext,
    subject_user_id: Option<i64>,
    created_by_user_id: Option<i64>,
) -> bool {
    match ctx.workload_scope() {
        RecordScope::Global | RecordScope::AnalysisAll => true,
        // A public account owns the submission operation, while the selected
        // detector remains the business owner shown in the record.
        RecordScope::PublicCreated => created_by_user_id == Some(ctx.user.id),
        _ => subject_user_id == Some(ctx.user.id),
    }
}

pub(super) fn work_record_maintenance_targets_on_conn(
    conn: &postgres_compat::Connection,
    ctx: &authz_service::AuthContext,
    allowed_divisions: Option<&[i64]>,
    ids: &[i64],
) -> Result<HashMap<i64, RecordMaintenanceTarget>> {
    if ids.is_empty() || !ctx.has_permission("entry:workload") {
        return Ok(HashMap::new());
    }
    let placeholders = (1..=ids.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(",");
    let department = authz_service::work_record_authorization_division_sql("wr");
    let mut stmt = conn.prepare(&format!(
        "SELECT wr.id,wr.subject_user_id,wr.created_by_user_id,{department},wr.deleted_at,to_jsonb(wr)::text
         FROM work_records wr WHERE wr.id IN ({placeholders})"
    ))?;
    let rows = stmt.query_map(postgres_compat::params_from_iter(ids.iter()), |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, Option<i64>>(1)?,
            row.get::<_, Option<i64>>(2)?,
            row.get::<_, Option<i64>>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, String>(5)?,
        ))
    })?;
    let mut targets = HashMap::new();
    for row in rows {
        let (id, subject, creator, division, deleted_at, row_json) = row?;
        let department_allowed = ctx.is_system_admin()
            || allowed_divisions.is_none_or(|ids| division.is_some_and(|id| ids.contains(&id)));
        let allowed =
            deleted_at.is_none() && department_allowed && can_mutate_record(ctx, subject, creator);
        targets.insert(
            id,
            RecordMaintenanceTarget {
                origin: "work_record",
                id,
                revision: record_repo::record_maintenance_revision(&row_json),
                can_edit: allowed,
                can_delete: allowed,
            },
        );
    }
    Ok(targets)
}

pub(crate) fn resolve_record_subject(
    pool: &DbPool,
    ctx: &authz_service::AuthContext,
    requested_id: Option<i64>,
    requested_name: &str,
) -> Result<crate::models::user::User> {
    let can_delegate = matches!(
        ctx.workload_scope(),
        RecordScope::Global | RecordScope::AnalysisAll
    );
    if requested_id.is_some_and(|id| id <= 0) {
        return Err(AppError::Validation("检测人员账号无效".into()));
    }
    let subject = if ctx.is_analysis_public_account() {
        authz_service::require_permission(ctx, "records:work:select-detector")?;
        let id = requested_id.ok_or_else(|| AppError::Validation("请选择实际检测人员".into()))?;
        if !crate::repo::user_repo::analysis_public_account_candidate_allowed(
            pool,
            ctx.user.id,
            id,
        )? {
            return Err(AppError::Forbidden(
                "所选账号不属于当前公共账号可用人员范围".into(),
            ));
        }
        crate::repo::user_repo::find_by_id(pool, id)?
    } else if can_delegate {
        match requested_id {
            Some(id) => crate::repo::user_repo::find_by_id(pool, id)?,
            None => crate::repo::user_repo::find_by_username(pool, requested_name.trim())?,
        }
    } else {
        if requested_id.is_some_and(|id| id != ctx.user.id)
            || (!requested_name.is_empty() && requested_name != ctx.user.username)
        {
            return Err(AppError::Forbidden("只能为本人录入或调整检测人员".into()));
        }
        Some(ctx.user.clone())
    }
    .ok_or_else(|| AppError::Validation("检测人员账号不存在，请选择有效账号".into()))?;
    if !subject.is_active
        || subject.is_public_account
        || subject.is_analysis_public_account
        || subject.is_rd_public_account
    {
        return Err(AppError::Validation("请选择启用的实际检测人员账号".into()));
    }
    Ok(subject)
}

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/records", get(list).post(create))
        .route("/api/records/users", get(users))
        .route("/api/records/method-options", get(method_options))
        .route(
            "/api/records/:id",
            axum::routing::put(update_with_revision).delete(soft_delete),
        )
        .route("/api/records/restore/:id", axum::routing::post(restore))
        .route(
            "/api/records/by-user/:user_name",
            axum::routing::delete(delete_by_user),
        )
        .with_state(pool)
}

#[derive(Deserialize)]
struct MeteringMethodQuery {
    project_id: i64,
    group_id: Option<i64>,
    division_id: Option<i64>,
}

async fn method_options(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(query): Query<MeteringMethodQuery>,
) -> Result<Json<ApiResponse<Vec<crate::models::method::MethodResponse>>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "entry:workload")?;
    if ctx.is_analysis_public_account() {
        authz_service::require_permission(&ctx, "records:work:portal-scoped")?;
    }
    let division_id = if let Some(group_id) = query.group_id {
        if !authz_service::work_group_allowed(&pool, &ctx, group_id)? {
            return Err(AppError::Forbidden("无权在此实验室录入工作量".into()));
        }
        Some(authz_service::require_project_execution_group(
            &pool,
            query.project_id,
            group_id,
        )?)
    } else {
        if !authz_service::work_division_allowed(&pool, &ctx, query.division_id)? {
            return Err(AppError::Forbidden("无权在此部门录入工作量".into()));
        }
        record_repo::validate_record_bindings(&*pool.get()?, query.project_id, None, None)?;
        query.division_id
    };
    Ok(Json(ApiResponse::ok(
        crate::repo::method_repo::metering_methods(&pool, division_id, query.group_id)?,
    )))
}

async fn list(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<RecordQuery>,
) -> Result<Json<ApiResponse<PaginatedResponse<RecordResponse>>>> {
    tokio::task::spawn_blocking(move || {
        let ctx = authz_service::authenticate(&pool, &headers)?;
        authz_service::require_permission(&ctx, "entry:workload")?;
        if ctx.is_analysis_public_account() {
            authz_service::require_permission(&ctx, "records:work:portal-scoped")?;
        }
        // 下界必须钳制，否则 page=0 会生成负 OFFSET，PostgreSQL 直接报错。
        let page = q.page.unwrap_or(1).max(1);
        let page_size = q.page_size.unwrap_or(50).clamp(1, 500);
        let scope = ctx.workload_scope();
        let allowed_division_ids = authz_service::work_allowed_division_ids(&pool, &ctx)?;
        let can_view_scoped_stats = ctx.can_view_workload_scope();
        let scoped_user = match scope {
            RecordScope::Global | RecordScope::AnalysisAll => q.user_name.as_deref(),
            _ if can_view_scoped_stats => q.user_name.as_deref(),
            _ => None,
        };
        let scoped_user_id = scoped_record_subject(scope, ctx.user.id, q.subject_user_id)?;
        let created_by_user_id = matches!(scope, RecordScope::PublicCreated).then_some(ctx.user.id);
        let (items, total) = record_repo::list_with_target(
            &pool,
            q.project_id,
            q.group_id,
            scoped_user_id,
            created_by_user_id,
            scoped_user,
            q.division_id,
            q.start.as_deref(),
            q.end.as_deref(),
            page,
            page_size,
            q.include_deleted.unwrap_or(false),
            allowed_division_ids.as_deref(),
            q.detection_division_ids.as_deref(),
            q.sending_division_ids.as_deref(),
            q.include_pending_ownership.unwrap_or(false),
            q.record_id,
        )?;
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

async fn create(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(body): Json<RecordCreate>,
) -> Result<Json<ApiResponse<RecordResponse>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "entry:workload")?;
    if ctx.is_analysis_public_account() {
        authz_service::require_permission(&ctx, "records:work:portal-scoped")?;
    }
    let subject = resolve_record_subject(&pool, &ctx, body.sender_user_id, &body.user_name)?;
    let record = RecordCreate {
        project_id: body.project_id,
        method_id: body.method_id,
        original_method_id: body.original_method_id,
        method_config_revision: body.method_config_revision,
        multiplier_source: body.multiplier_source,
        user_name: subject.username,
        sender_user_id: Some(subject.id),
        quantity: body.quantity,
        recorded_at: body.recorded_at,
        group_id: body.group_id,
        multiplier: body.multiplier,
        division_id: body.division_id,
        high_item: body.high_item,
        extra_fields: None,
        source_type: None,
        source_record_id: None,
    };
    if let Some(group_id) = record.group_id {
        if !authz_service::work_group_allowed(&pool, &ctx, group_id)? {
            return Err(AppError::Forbidden(
                "当前角色无权在该部门实验室录入分析检测记录".into(),
            ));
        }
        if let Some(method_id) = record.method_id {
            let method_allowed = if record.original_method_id.is_some() {
                let division_id = authz_service::require_project_execution_group(
                    &pool,
                    record.project_id,
                    group_id,
                )?;
                authz_service::work_method_allowed_for_division(
                    &pool,
                    &ctx,
                    method_id,
                    Some(division_id),
                )?
            } else {
                authz_service::work_method_allowed_for_group(&pool, &ctx, method_id, group_id)?
            };
            if !method_allowed {
                return Err(AppError::Forbidden(
                    "当前角色无权在该部门实验室使用此检测方法".into(),
                ));
            }
            crate::repo::method_repo::ensure_method_visible_for_group(
                &pool, method_id, group_id, "work",
            )?;
        }
    } else if ctx.is_analysis_public_account() {
        return Err(AppError::Validation(
            "分析检测公共账号录入必须选择分析组".into(),
        ));
    } else if !authz_service::work_division_allowed(&pool, &ctx, record.division_id)? {
        return Err(AppError::Forbidden(
            "当前角色无权在该部门录入分析检测记录".into(),
        ));
    } else if let Some(method_id) = record.method_id {
        if !authz_service::work_method_allowed_for_division(
            &pool,
            &ctx,
            method_id,
            record.division_id,
        )? {
            return Err(AppError::Forbidden(
                "当前角色无权在该部门使用此检测方法".into(),
            ));
        }
    }
    let record = if let Some(group_id) = record.group_id {
        let execution_division_id =
            authz_service::require_project_execution_group(&pool, record.project_id, group_id)?;
        if let Some(requested_division_id) = record.division_id {
            if requested_division_id != execution_division_id {
                return Err(AppError::Validation(
                    "所选部门与实验室所属执行部门不一致".into(),
                ));
            }
        }
        RecordCreate {
            division_id: Some(execution_division_id),
            ..record
        }
    } else {
        record
    };
    // Service layer: validates quantity > 0 and project existence
    Ok(Json(ApiResponse::ok(record_service::create_record(
        &pool,
        &record,
        &ctx.user.username,
    )?)))
}

pub(super) async fn update(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<RecordUpdate>,
) -> Result<Json<ApiResponse<RecordResponse>>> {
    update_with_revision(
        State(pool),
        headers,
        Path(id),
        Query(RecordMutationQuery {
            expected_revision: None,
        }),
        Json(body),
    )
    .await
}

async fn update_with_revision(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(query): Query<RecordMutationQuery>,
    Json(mut body): Json<RecordUpdate>,
) -> Result<Json<ApiResponse<RecordResponse>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "entry:workload")?;
    let existing = record_repo::get_by_id(&pool, id)?;
    if !authz_service::work_record_allowed(&pool, &ctx, id)? {
        return Err(AppError::Forbidden(
            "当前角色无权修改该部门的分析检测记录".into(),
        ));
    }
    if !can_mutate_record(&ctx, existing.subject_user_id, existing.created_by_user_id) {
        return Err(AppError::Forbidden("只能修改本人的分析检测记录".into()));
    }
    if let Some(revision) = query.expected_revision.as_deref() {
        validate_expected_revision(revision)?;
        return Ok(Json(ApiResponse::ok(
            record_service::update_record_metadata(&pool, id, &body, &ctx.user.username, revision)?,
        )));
    }
    if body
        .sender_user_id
        .is_some_and(|id| Some(id) != existing.subject_user_id)
        || body
            .user_name
            .as_ref()
            .is_some_and(|name| name != &existing.user_name)
    {
        let subject = resolve_record_subject(
            &pool,
            &ctx,
            body.sender_user_id,
            body.user_name.as_deref().unwrap_or(&existing.user_name),
        )?;
        body.user_name = Some(subject.username);
        body.sender_user_id = Some(subject.id);
    }
    let conn = pool.get()?;
    let (current_group_id, current_division_id, source_type, source_record_id): (
        Option<i64>,
        Option<i64>,
        Option<String>,
        Option<i64>,
    ) = conn.query_row(
        "SELECT group_id, division_id, source_type, source_record_id FROM work_records WHERE id=?1",
        [id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )?;
    let final_group_id = body.group_id.or(current_group_id);
    let final_project_id = body.project_id.unwrap_or(existing.project_id);
    let final_method_id = body.method_id.or(existing.method_id);
    let sample_metadata_only = record_repo::sample_workload_bindings_unchanged(&conn, id, &body)?;
    let unchanged_metering =
        sample_metadata_only || record_repo::metering_bindings_unchanged(&conn, id, &body)?;
    if let Some(group_id) = final_group_id {
        if !authz_service::work_group_allowed(&pool, &ctx, group_id)? {
            return Err(AppError::Forbidden(
                "当前角色无权将记录调整到该部门实验室".into(),
            ));
        }
        if let Some(method_id) = final_method_id.filter(|_| !unchanged_metering) {
            if !authz_service::work_method_allowed_for_group(&pool, &ctx, method_id, group_id)? {
                return Err(AppError::Forbidden(
                    "当前角色无权在该部门实验室使用此检测方法".into(),
                ));
            }
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
    } else if let Some(method_id) = final_method_id {
        let final_division_id = body.division_id.or(current_division_id);
        if !authz_service::work_division_allowed(&pool, &ctx, final_division_id)?
            || (!unchanged_metering
                && !authz_service::work_method_allowed_for_division(
                    &pool,
                    &ctx,
                    method_id,
                    final_division_id,
                )?)
        {
            return Err(AppError::Forbidden(
                "当前角色无权调整为该部门或检测方法".into(),
            ));
        }
    }
    if sample_metadata_only {
        // 事务内按旧绑定再次比较；并发修改来源或归属不能借元数据编辑绕过授权检查。
        body.project_id = Some(existing.project_id);
        body.method_id = existing.method_id;
        body.group_id = current_group_id;
        body.division_id = current_division_id;
        body.user_name = Some(existing.user_name.clone());
        body.sender_user_id = existing.subject_user_id;
        body.source_type = source_type;
        body.source_record_id = source_record_id;
    }
    // Service layer: validates not-deleted, change detection
    Ok(Json(ApiResponse::ok(record_service::update_record(
        &pool,
        id,
        &body,
        &ctx.user.username,
    )?)))
}

#[derive(Deserialize)]
pub(super) struct RecordMutationQuery {
    expected_revision: Option<String>,
}

fn validate_expected_revision(revision: &str) -> Result<()> {
    if revision.len() != 64 || !revision.bytes().all(|value| value.is_ascii_hexdigit()) {
        return Err(AppError::Validation("记录版本无效，请刷新后重试".into()));
    }
    Ok(())
}

#[derive(Deserialize)]
struct RecordDeleteQuery {
    reason: Option<String>,
    expected_revision: Option<String>,
}

async fn soft_delete(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(body): Query<RecordDeleteQuery>,
) -> Result<Json<ApiResponse<()>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "entry:workload")?;
    let existing = record_repo::get_by_id(&pool, id)?;
    if !authz_service::work_record_allowed(&pool, &ctx, id)? {
        return Err(AppError::Forbidden(
            "当前角色无权删除该部门的分析检测记录".into(),
        ));
    }
    if !can_mutate_record(&ctx, existing.subject_user_id, existing.created_by_user_id) {
        return Err(AppError::Forbidden("只能删除本人的分析检测记录".into()));
    }
    if let Some(revision) = body.expected_revision.as_deref() {
        validate_expected_revision(revision)?;
        let reason = body.reason.as_deref().unwrap_or("").trim();
        if reason.is_empty() || reason.chars().count() > 500 || reason.contains('\0') {
            return Err(AppError::Validation(
                "删除原因不能为空或超过500个字符".into(),
            ));
        }
    }
    // Service layer: validates record exists and not already deleted.
    let reason = body
        .reason
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("用户删除");
    record_service::delete_record(
        &pool,
        id,
        &ctx.user.username,
        reason,
        body.expected_revision.as_deref(),
    )?;
    Ok(Json(ApiResponse::ok_msg("删除成功")))
}

async fn restore(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<RecordResponse>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "entry:workload")?;
    let existing = record_repo::get_by_id(&pool, id)?;
    if !authz_service::work_record_allowed(&pool, &ctx, id)? {
        return Err(AppError::Forbidden(
            "当前角色无权恢复该部门的分析检测记录".into(),
        ));
    }
    if !can_mutate_record(&ctx, existing.subject_user_id, existing.created_by_user_id) {
        return Err(AppError::Forbidden("只能恢复本人的分析检测记录".into()));
    }
    Ok(Json(ApiResponse::ok(record_repo::restore(
        &pool,
        id,
        &ctx.user.username,
    )?)))
}

#[derive(Deserialize)]
pub struct UsersQuery {
    pub start: Option<String>,
    pub end: Option<String>,
    pub detection_division_ids: Option<String>,
    pub sending_division_ids: Option<String>,
    pub include_pending_ownership: Option<bool>,
}

async fn users(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<UsersQuery>,
) -> Result<Json<ApiResponse<Vec<String>>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    let allowed_division_ids = authz_service::work_allowed_division_ids(&pool, &ctx)?;
    let can_view_scoped_stats = ctx.can_view_workload_scope();
    if !matches!(
        ctx.workload_scope(),
        RecordScope::Global | RecordScope::AnalysisAll
    ) && !can_view_scoped_stats
    {
        return Ok(Json(ApiResponse::ok(vec![ctx.user.username])));
    }
    let conn = pool.get()?;
    let mut sql =
        String::from("SELECT DISTINCT user_name FROM work_records wr WHERE deleted_at IS NULL");
    let mut params: Vec<Box<dyn postgres_compat::types::ToSql>> = vec![];
    let (start, end) = record_repo::work_record_date_bounds(q.start.as_deref(), q.end.as_deref())?;
    if let Some(s) = start {
        let idx = params.len() + 1;
        sql.push_str(&format!(" AND labflow_rd_timestamp_or_null(recorded_at)>=CAST(CAST(?{idx} AS TEXT) AS timestamp)"));
        params.push(Box::new(s));
    }
    if let Some(e) = end {
        let idx = params.len() + 1;
        sql.push_str(&format!(" AND labflow_rd_timestamp_or_null(recorded_at)<=CAST(CAST(?{idx} AS TEXT) AS timestamp)"));
        params.push(Box::new(e));
    }
    if let Some(ids) = allowed_division_ids {
        if ids.is_empty() {
            sql.push_str(" AND 1=0");
        } else {
            let values = ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
            sql.push_str(&format!(
                " AND {} IN ({values})",
                authz_service::work_record_authorization_division_sql("wr")
            ));
        }
    }
    if !q.include_pending_ownership.unwrap_or(false) {
        sql.push_str(" AND COALESCE(wr.ownership_status,'confirmed')<>'pending_confirmation'");
    }
    if let Some(ids) =
        record_repo::parse_dimension_ids(q.detection_division_ids.as_deref(), "检测部门")?
    {
        if ids.is_empty() {
            sql.push_str(" AND 1=0");
        } else {
            sql.push_str(&format!(
                " AND wr.detection_division_id IN ({})",
                ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
            ));
        }
    }
    if let Some(ids) =
        record_repo::parse_dimension_ids(q.sending_division_ids.as_deref(), "送样部门")?
    {
        if ids.is_empty() {
            sql.push_str(" AND 1=0");
        } else {
            sql.push_str(&format!(
                " AND {} IN ({})",
                authz_service::work_record_sending_division_sql("wr"),
                ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
            ));
        }
    }
    sql.push_str(" ORDER BY user_name");

    let mut stmt = conn.prepare(&sql)?;
    let users: Vec<String> = stmt
        .query_map(
            postgres_compat::params_from_iter(params.iter().map(|p| p.as_ref())),
            |row| row.get::<_, String>(0),
        )?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(Json(ApiResponse::ok(users)))
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
    authz_service::require_permission(&ctx, "entry:workload")?;
    let allowed_division_ids = authz_service::work_allowed_division_ids(&pool, &ctx)?;
    let public_created_by_user_id =
        matches!(ctx.workload_scope(), RecordScope::PublicCreated).then_some(ctx.user.id);
    if !matches!(
        ctx.workload_scope(),
        RecordScope::Global | RecordScope::AnalysisAll | RecordScope::PublicCreated
    ) && user_name != ctx.user.username
    {
        return Err(AppError::Forbidden("只能删除本人的分析检测记录".into()));
    }
    if let Some(ids) = allowed_division_ids {
        if ids.is_empty() {
            return Err(AppError::Forbidden("当前角色未配置分析检测部门范围".into()));
        }
        let conn = pool.get()?;
        let values = ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
        let created_by_scope = public_created_by_user_id
            .map(|user_id| format!(" AND wr.created_by_user_id={user_id}"))
            .unwrap_or_default();
        let outside_scope: i64 = conn.query_row(
            &format!(
                "SELECT COUNT(*) FROM work_records wr WHERE wr.deleted_at IS NULL AND wr.user_name=?1{created_by_scope} AND NOT ({} IN ({values}))",
                authz_service::work_record_authorization_division_sql("wr")
            ),
            [&user_name],
            |row| row.get(0),
        )?;
        if outside_scope > 0 {
            return Err(AppError::Forbidden("批量删除范围包含未授权部门记录".into()));
        }
    }
    let count = record_repo::delete_by_user(
        &pool,
        &user_name,
        public_created_by_user_id,
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

#[cfg(test)]
mod filter_tests {
    #[test]
    fn historical_maintenance_targets_keep_original_department_and_stable_owner_authorization() {
        let pool = crate::db::init_pool("postgres-test");
        let conn = pool.get().unwrap();
        crate::db::test_migrations::run(&conn).unwrap();
        let department: i64 = conn
            .query_row(
                "INSERT INTO divisions(name) VALUES('delete_A') RETURNING id",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let foreign: i64 = conn
            .query_row(
                "INSERT INTO divisions(name) VALUES('delete_B') RETURNING id",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let mut users = Vec::new();
        for name in ["delete-owner", "delete-peer", "delete-public"] {
            users.push(conn.query_row(
                "INSERT INTO users(username,password,division_id) VALUES(?1,'hash',?2) RETURNING id",
                postgres_compat::params![name,department], |row| row.get::<_,i64>(0),
            ).unwrap());
        }
        let (owner, peer, public_id) = (users[0], users[1], users[2]);
        let group: i64 = conn.query_row(
            "INSERT INTO project_groups(name,division_id) VALUES('delete-old-lab',?1) RETURNING id",
            [department], |row| row.get(0),
        ).unwrap();
        let project: i64 = conn
            .query_row(
                "INSERT INTO projects(name,group_id) VALUES('delete-old-project',?1) RETURNING id",
                [group],
                |row| row.get(0),
            )
            .unwrap();
        let mut ids = Vec::new();
        for (subject, creator, division, group_id) in [
            (Some(owner), Some(owner), Some(department), None),
            (Some(peer), Some(peer), Some(department), None),
            (Some(owner), Some(owner), Some(foreign), None),
            (Some(peer), Some(public_id), Some(department), None),
            (None, None, None, None),
            (Some(owner), Some(owner), None, Some(group)),
        ] {
            ids.push(conn.query_row(
                "INSERT INTO work_records(project_id,group_id,user_name,subject_user_id,created_by_user_id,quantity,recorded_at,detection_division_id,method_name_snapshot,instrument_type_snapshot,updated_at,business_no)
                 VALUES(?1,?2,'snapshot-name',?3,?4,1,'2091-01-31T12:00:00',?5,'流动相瓶','辅助工作','',?6) RETURNING id",
                postgres_compat::params![project,group_id,subject,creator,division,format!("delete-target-{}",ids.len())], |row| row.get::<_,i64>(0),
            ).unwrap());
        }
        let user = crate::repo::user_repo::find_by_id(&pool, owner)
            .unwrap()
            .unwrap();
        let mut own = authz_service::AuthContext {
            role_names: user.role_names.clone(),
            user,
        };
        own.user.permissions = vec!["entry:workload".into()];
        let targets =
            work_record_maintenance_targets_on_conn(&conn, &own, Some(&[department]), &ids)
                .unwrap();
        assert_eq!(
            ids.iter()
                .map(|id| targets[id].can_delete)
                .collect::<Vec<_>>(),
            [true, false, false, false, false, true],
        );
        assert_eq!(
            targets[&ids[0]].revision.len(),
            64,
            "empty updated_at is still deletable"
        );
        assert!(
            !own.has_permission("entry:auxiliary"),
            "old work does not need the independent entry"
        );
        let mut read_only = own.clone();
        read_only.user.permissions = vec!["stats:workload:view-all".into()];
        assert!(work_record_maintenance_targets_on_conn(
            &conn,
            &read_only,
            Some(&[department]),
            &ids
        )
        .unwrap()
        .is_empty());
        let mut supervisor = own.clone();
        supervisor
            .user
            .permissions
            .push("records:work:view-scope".into());
        let targets =
            work_record_maintenance_targets_on_conn(&conn, &supervisor, Some(&[department]), &ids)
                .unwrap();
        assert!(targets[&ids[1]].can_delete);
        assert!(!targets[&ids[2]].can_delete);
        let mut public = own.clone();
        public.user.id = public_id;
        public.user.role_keys = vec!["analysis_public_account".into()];
        public.user.is_analysis_public_account = true;
        let targets =
            work_record_maintenance_targets_on_conn(&conn, &public, Some(&[department]), &ids)
                .unwrap();
        assert!(targets[&ids[3]].can_delete);
        assert!(!targets[&ids[0]].can_delete);
        public.user.id = peer;
        assert!(
            !work_record_maintenance_targets_on_conn(&conn, &public, Some(&[department]), &ids)
                .unwrap()[&ids[3]]
                .can_delete
        );
        let user = crate::repo::user_repo::find_by_username(&pool, "admin")
            .unwrap()
            .unwrap();
        let admin = authz_service::AuthContext {
            role_names: user.role_names.clone(),
            user,
        };
        assert!(
            work_record_maintenance_targets_on_conn(&conn, &admin, None, &ids)
                .unwrap()
                .values()
                .all(|target| target.can_delete)
        );
    }

    #[test]
    fn workload_subject_uses_stable_ids_and_rejects_unknown_or_unauthorized_people() {
        let pool = crate::db::init_pool("postgres-test");
        crate::db::test_migrations::run(&pool.get().unwrap()).unwrap();
        pool.get().unwrap().execute(
            "INSERT INTO users(username,password,is_active) VALUES('subject-worker','hash',1),('subject-other','hash',1),('subject-inactive','hash',0)",
            [],
        ).unwrap();
        let user = crate::repo::user_repo::find_by_username(&pool, "subject-worker")
            .unwrap()
            .unwrap();
        let other = crate::repo::user_repo::find_by_username(&pool, "subject-other")
            .unwrap()
            .unwrap();
        let admin = crate::repo::user_repo::find_by_username(&pool, "admin")
            .unwrap()
            .unwrap();
        let own = super::authz_service::AuthContext {
            role_names: user.role_names.clone(),
            user,
        };
        let privileged = super::authz_service::AuthContext {
            role_names: admin.role_names.clone(),
            user: admin,
        };
        assert_eq!(
            super::resolve_record_subject(&pool, &own, None, &own.user.username)
                .unwrap()
                .id,
            own.user.id
        );
        assert!(
            super::resolve_record_subject(&pool, &own, Some(other.id), &other.username).is_err()
        );
        assert!(super::resolve_record_subject(&pool, &own, None, &other.username).is_err());
        let selected =
            super::resolve_record_subject(&pool, &privileged, Some(other.id), "旧显示名").unwrap();
        assert_eq!(
            (selected.id, selected.username),
            (other.id, other.username.clone())
        );
        assert_eq!(
            super::resolve_record_subject(&pool, &privileged, None, &other.username)
                .unwrap()
                .id,
            other.id
        );
        assert!(super::resolve_record_subject(&pool, &privileged, None, "不存在的姓名").is_err());
        assert!(
            super::resolve_record_subject(&pool, &privileged, None, "subject-inactive").is_err()
        );
        assert!(
            super::resolve_record_subject(&pool, &privileged, Some(0), "subject-worker").is_err()
        );
    }

    use super::*;

    #[test]
    fn work_subject_filter_keeps_authorized_choice_and_never_broadens_own_scope() {
        for scope in [
            RecordScope::Global,
            RecordScope::AnalysisAll,
            RecordScope::PublicCreated,
        ] {
            assert_eq!(
                scoped_record_subject(scope, 11, Some(22)).unwrap(),
                Some(22)
            );
            assert_eq!(scoped_record_subject(scope, 11, None).unwrap(), None);
        }
        assert_eq!(
            scoped_record_subject(RecordScope::Own, 11, Some(22)).unwrap(),
            Some(11)
        );
        assert_eq!(
            scoped_record_subject(RecordScope::Own, 11, None).unwrap(),
            Some(11)
        );
        assert!(scoped_record_subject(RecordScope::Global, 11, Some(0)).is_err());
    }
}
