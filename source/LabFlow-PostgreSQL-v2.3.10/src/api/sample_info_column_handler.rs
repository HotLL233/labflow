use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::sample_info_column::*;
use crate::models::sample_info_column_visibility::VisibilityUpdateRequest;
use crate::models::trash::DeleteReasonRequest;
use crate::models::ApiResponse;
use crate::repo::{sample_info_column_repo, sample_info_column_visibility_repo};
use crate::service::authz_service;
use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json, Router,
};
use serde::Deserialize;
use serde::Serialize;

#[derive(Deserialize, Default)]
pub struct ColumnQuery {
    pub type_key: Option<String>,
}

#[derive(Serialize)]
pub struct ColumnWithVisibility {
    pub id: i64,
    pub field_key: String,
    pub label: String,
    pub data_type: String,
    pub is_predefined: bool,
    pub is_required: bool,
    pub is_active: bool,
    pub width: i64,
    pub sort_order: i64,
    pub options: Option<String>,
    pub show_in_list: bool,
    pub show_in_export: bool,
    pub show_in_form: bool,
    pub type_key: Option<String>,
    pub created_at: String,
    pub updated_at: Option<String>,
    pub is_visible_in_type: bool,
}

impl From<(SampleInfoColumn, bool)> for ColumnWithVisibility {
    fn from((col, visible): (SampleInfoColumn, bool)) -> Self {
        ColumnWithVisibility {
            id: col.id,
            field_key: col.field_key,
            label: col.label,
            data_type: col.data_type,
            is_predefined: col.is_predefined,
            is_required: col.is_required,
            is_active: col.is_active,
            width: col.width,
            sort_order: col.sort_order,
            options: col.options,
            show_in_list: col.show_in_list,
            show_in_export: col.show_in_export,
            show_in_form: col.show_in_form,
            type_key: col.type_key,
            created_at: col.created_at,
            updated_at: col.updated_at,
            is_visible_in_type: visible,
        }
    }
}

/// 从 HeaderMap 中提取 JWT claims 并校验管理员权限
fn require_admin(pool: &DbPool, headers: &HeaderMap) -> Result<authz_service::AuthContext> {
    let ctx = authz_service::authenticate(pool, headers)?;
    authz_service::require_permission(&ctx, "manage:sampleinfo")?;
    Ok(ctx)
}

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route(
            "/api/sample-info/columns",
            axum::routing::get(list).post(create),
        )
        .route(
            "/api/sample-info/columns/active",
            axum::routing::get(list_active),
        )
        .route(
            "/api/sample-info/columns/manage",
            axum::routing::get(list_manage),
        )
        .route(
            "/api/sample-info/columns/visibility",
            axum::routing::put(update_visibility),
        )
        .route("/api/sample-info/columns/sort", axum::routing::put(reorder))
        .route(
            "/api/sample-info/columns/:id/types",
            axum::routing::put(update_column_types),
        )
        .route(
            "/api/sample-info/columns/:id",
            axum::routing::put(update).delete(delete),
        )
        .with_state(pool)
}

async fn list(
    State(pool): State<DbPool>,
    Query(q): Query<ColumnQuery>,
) -> Result<Json<ApiResponse<Vec<SampleInfoColumn>>>> {
    let items = if let Some(ref tk) = q.type_key {
        if tk.is_empty() {
            sample_info_column_repo::list_all(&pool)?
        } else {
            sample_info_column_repo::list_active_by_type(&pool, tk)?
        }
    } else {
        sample_info_column_repo::list_all(&pool)?
    };
    Ok(Json(ApiResponse::ok(items)))
}

async fn list_active(
    State(pool): State<DbPool>,
    Query(q): Query<ColumnQuery>,
) -> Result<Json<ApiResponse<Vec<SampleInfoColumn>>>> {
    let items = if let Some(ref tk) = q.type_key {
        if tk.is_empty() {
            sample_info_column_repo::list_active_common_for_list(&pool)?
        } else {
            sample_info_column_repo::list_active_by_type(&pool, tk)?
        }
    } else {
        sample_info_column_repo::list_active_common_for_list(&pool)?
    };
    Ok(Json(ApiResponse::ok(items)))
}

/// GET /api/sample-info/columns/manage?type_key=xxx — 管理页专用（列 + 可见性）
async fn list_manage(
    State(pool): State<DbPool>,
    Query(q): Query<ColumnQuery>,
) -> Result<Json<ApiResponse<Vec<ColumnWithVisibility>>>> {
    let type_key = q.type_key.unwrap_or_default();
    if type_key.is_empty() {
        return Err(AppError::Validation("type_key 不能为空".into()));
    }
    let items = sample_info_column_repo::list_all_with_visibility(&pool, &type_key)?;
    let result: Vec<ColumnWithVisibility> =
        items.into_iter().map(ColumnWithVisibility::from).collect();
    Ok(Json(ApiResponse::ok(result)))
}

/// PUT /api/sample-info/columns/visibility — 批量更新预置列可见性
async fn update_visibility(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(body): Json<VisibilityUpdateRequest>,
) -> Result<Json<ApiResponse<()>>> {
    let ctx = require_admin(&pool, &headers)?;
    sample_info_column_visibility_repo::batch_update(&pool, &body, &ctx.user.username)?;
    Ok(Json(ApiResponse::ok_msg("可见性更新成功")))
}

/// PUT /api/sample-info/columns/:id/types — 更新单列的可见类型
async fn update_column_types(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<ColumnTypesUpdate>,
) -> Result<Json<ApiResponse<SampleInfoColumn>>> {
    let ctx = require_admin(&pool, &headers)?;
    let item =
        sample_info_column_repo::set_type_rules(&pool, id, &body.type_rules, &ctx.user.username)?;
    Ok(Json(ApiResponse::ok(item)))
}

async fn create(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(body): Json<ColumnCreate>,
) -> Result<Json<ApiResponse<SampleInfoColumn>>> {
    let ctx = require_admin(&pool, &headers)?;
    let item = sample_info_column_repo::create(&pool, &body, &ctx.user.username)?;
    Ok(Json(ApiResponse::ok(item)))
}

async fn update(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<ColumnUpdate>,
) -> Result<Json<ApiResponse<SampleInfoColumn>>> {
    let ctx = require_admin(&pool, &headers)?;
    let item = sample_info_column_repo::update(&pool, id, &body, &ctx.user.username)?;
    Ok(Json(ApiResponse::ok(item)))
}

async fn delete(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(body): Query<DeleteReasonRequest>,
) -> Result<Json<ApiResponse<()>>> {
    let ctx = require_admin(&pool, &headers)?;
    let reason = body
        .reason
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("用户删除");
    sample_info_column_repo::soft_delete(&pool, id, &ctx.user.username, reason)?;
    Ok(Json(ApiResponse::ok_msg("删除成功")))
}

async fn reorder(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(body): Json<ColumnReorder>,
) -> Result<Json<ApiResponse<Vec<SampleInfoColumn>>>> {
    let ctx = require_admin(&pool, &headers)?;
    let items = sample_info_column_repo::reorder(&pool, &body, &ctx.user.username)?;
    Ok(Json(ApiResponse::ok(items)))
}
