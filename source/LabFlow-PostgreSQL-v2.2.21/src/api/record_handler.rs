use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::record::{RecordCreate, RecordResponse, RecordUpdate};
use crate::models::trash::DeleteReasonRequest;
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

#[derive(Deserialize)]
pub struct RecordQuery {
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

fn can_mutate_record(ctx: &authz_service::AuthContext, record: &RecordResponse) -> bool {
    match ctx.workload_scope() {
        RecordScope::Global | RecordScope::AnalysisAll => true,
        // A public account owns the submission operation, while the selected
        // detector remains the business owner shown in the record.
        RecordScope::PublicCreated => record.created_by_user_id == Some(ctx.user.id),
        _ => record.subject_user_id == Some(ctx.user.id),
    }
}

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/records", get(list).post(create))
        .route("/api/records/users", get(users))
        .route(
            "/api/records/:id",
            axum::routing::put(update).delete(soft_delete),
        )
        .route("/api/records/restore/:id", axum::routing::post(restore))
        .route(
            "/api/records/by-user/:user_name",
            axum::routing::delete(delete_by_user),
        )
        .with_state(pool)
}

async fn list(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<RecordQuery>,
) -> Result<Json<ApiResponse<PaginatedResponse<RecordResponse>>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "entry:workload")?;
    if ctx.is_analysis_public_account() {
        authz_service::require_permission(&ctx, "records:work:portal-scoped")?;
    }
    let page = q.page.unwrap_or(1);
    let page_size = q.page_size.unwrap_or(50).min(500);
    let scope = ctx.workload_scope();
    let allowed_division_ids = authz_service::work_allowed_division_ids(&pool, &ctx)?;
    let can_view_scoped_stats = ctx.can_view_workload_scope();
    let scoped_user = match scope {
        RecordScope::Global | RecordScope::AnalysisAll => q.user_name.as_deref(),
        _ if can_view_scoped_stats => q.user_name.as_deref(),
        _ => None,
    };
    let scoped_user_id = match scope {
        RecordScope::Global | RecordScope::AnalysisAll => None,
        _ if can_view_scoped_stats => None,
        RecordScope::PublicCreated => q.subject_user_id,
        _ => Some(ctx.user.id),
    };
    let created_by_user_id = matches!(scope, RecordScope::PublicCreated).then_some(ctx.user.id);
    let (items, total) = record_repo::list(
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
    Json(body): Json<RecordCreate>,
) -> Result<Json<ApiResponse<RecordResponse>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "entry:workload")?;
    if ctx.is_analysis_public_account() {
        authz_service::require_permission(&ctx, "records:work:portal-scoped")?;
    }
    let (user_name, subject_user_id) = if ctx.is_analysis_public_account() {
        authz_service::require_permission(&ctx, "records:work:select-detector")?;
        let detector_id = body
            .sender_user_id
            .ok_or_else(|| AppError::Validation("请选择实际检测人员".into()))?;
        let detector = crate::repo::user_repo::find_by_id(&pool, detector_id)?
            .ok_or_else(|| AppError::Validation("所选检测人员不存在".into()))?;
        (detector.username, Some(detector.id))
    } else {
        (
            if matches!(
                ctx.workload_scope(),
                RecordScope::Global | RecordScope::AnalysisAll
            ) {
                body.user_name
            } else {
                ctx.user.username.clone()
            },
            None,
        )
    };
    let record = RecordCreate {
        project_id: body.project_id,
        method_id: body.method_id,
        user_name,
        sender_user_id: subject_user_id,
        quantity: body.quantity,
        recorded_at: body.recorded_at,
        group_id: body.group_id,
        multiplier: body.multiplier,
        division_id: body.division_id,
        high_item: body.high_item,
        extra_fields: None,
    };
    if let Some(group_id) = record.group_id {
        if ctx.is_analysis_public_account() {
            let Some(subject_user_id) = record.sender_user_id else {
                return Err(AppError::Validation("请选择实际检测人员".into()));
            };
            if !crate::repo::user_repo::analysis_public_account_candidate_allowed(
                &pool,
                ctx.user.id,
                subject_user_id,
            )? {
                return Err(AppError::Forbidden(
                    "所选账号不属于当前公共账号可用人员范围".into(),
                ));
            }
        }
        if !authz_service::work_group_allowed(&pool, &ctx, group_id)? {
            return Err(AppError::Forbidden(
                "当前角色无权在该部门实验室录入分析检测记录".into(),
            ));
        }
        if let Some(method_id) = record.method_id {
            if !authz_service::work_method_allowed_for_group(&pool, &ctx, method_id, group_id)? {
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

async fn update(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<RecordUpdate>,
) -> Result<Json<ApiResponse<RecordResponse>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "entry:workload")?;
    let existing = record_repo::get_by_id(&pool, id)?;
    if !authz_service::work_record_allowed(&pool, &ctx, id)? {
        return Err(AppError::Forbidden(
            "当前角色无权修改该部门的分析检测记录".into(),
        ));
    }
    if !can_mutate_record(&ctx, &existing) {
        return Err(AppError::Forbidden("只能修改本人的分析检测记录".into()));
    }
    let conn = pool.get()?;
    let (current_group_id, current_division_id): (Option<i64>, Option<i64>) = conn.query_row(
        "SELECT group_id, division_id FROM work_records WHERE id=?1",
        [id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let final_group_id = body.group_id.or(current_group_id);
    let final_project_id = body.project_id.unwrap_or(existing.project_id);
    let final_method_id = body.method_id.or(existing.method_id);
    if let Some(group_id) = final_group_id {
        if !authz_service::work_group_allowed(&pool, &ctx, group_id)? {
            return Err(AppError::Forbidden(
                "当前角色无权将记录调整到该部门实验室".into(),
            ));
        }
        if let Some(method_id) = final_method_id {
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
            || !authz_service::work_method_allowed_for_division(
                &pool,
                &ctx,
                method_id,
                final_division_id,
            )?
        {
            return Err(AppError::Forbidden(
                "当前角色无权调整为该部门或检测方法".into(),
            ));
        }
    }
    // Service layer: validates not-deleted, change detection
    Ok(Json(ApiResponse::ok(record_service::update_record(
        &pool,
        id,
        &body,
        &ctx.user.username,
    )?)))
}

async fn soft_delete(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(body): Query<DeleteReasonRequest>,
) -> Result<Json<ApiResponse<()>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "entry:workload")?;
    let existing = record_repo::get_by_id(&pool, id)?;
    if !authz_service::work_record_allowed(&pool, &ctx, id)? {
        return Err(AppError::Forbidden(
            "当前角色无权删除该部门的分析检测记录".into(),
        ));
    }
    if !can_mutate_record(&ctx, &existing) {
        return Err(AppError::Forbidden("只能删除本人的分析检测记录".into()));
    }
    // Service layer: validates record exists and not already deleted
    let reason = body
        .reason
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("用户删除");
    record_service::delete_record(&pool, id, &ctx.user.username, reason)?;
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
    if !can_mutate_record(&ctx, &existing) {
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
    if let Some(ref s) = q.start {
        let idx = params.len() + 1;
        sql.push_str(&format!(" AND recorded_at>=?{}", idx));
        params.push(Box::new(s.to_string()));
    }
    if let Some(ref e) = q.end {
        let idx = params.len() + 1;
        sql.push_str(&format!(" AND recorded_at<=?{}", idx));
        params.push(Box::new(format!("{}T23:59:59", e)));
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
