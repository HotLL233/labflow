use crate::db::DbPool;
use crate::error::Result;
use crate::models::role::{PermissionDef, RoleCreate, RolePermissionSet, RoleUpdate, PERMISSIONS};
use crate::models::trash::DeleteReasonRequest;
use crate::models::ApiResponse;
use crate::repo::role_repo;
use crate::service::authz_service;
use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json, Router,
};

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/roles", axum::routing::get(list).post(create))
        .route(
            "/api/roles/:id",
            axum::routing::put(update).delete(delete_one),
        )
        .route("/api/roles/:id/permissions", axum::routing::put(set_perms))
        .route(
            "/api/roles/permissions",
            axum::routing::get(permission_whitelist),
        )
        .with_state(pool)
}

/// GET /api/roles — 列出全部角色（含权限点）
async fn list(
    State(pool): State<DbPool>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<crate::models::role::RoleWithPermissions>>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    if !ctx.is_system_admin() && !ctx.is_analysis_leader() {
        return Err(crate::error::AppError::Forbidden("无角色查看权限".into()));
    }
    let roles = role_repo::list_with_permissions(&pool)?;
    Ok(Json(ApiResponse::ok(roles)))
}

/// POST /api/roles — 新建角色
async fn create(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(body): Json<RoleCreate>,
) -> Result<Json<ApiResponse<crate::models::role::RoleWithPermissions>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    if !ctx.is_system_admin() {
        return Err(crate::error::AppError::Forbidden(
            "仅系统管理员可新增自定义角色".into(),
        ));
    }
    let role = role_repo::create(&pool, &body, &ctx.user.username)?;
    Ok(Json(ApiResponse::ok(role)))
}

/// PUT /api/roles/:id — 更新角色基础信息
async fn update(
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    Json(body): Json<RoleUpdate>,
) -> Result<Json<ApiResponse<crate::models::role::RoleWithPermissions>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    if !ctx.is_system_admin() {
        return Err(crate::error::AppError::Forbidden(
            "仅系统管理员可编辑自定义角色".into(),
        ));
    }
    let role = role_repo::update(&pool, id, &body, &ctx.user.username)?;
    Ok(Json(ApiResponse::ok(role)))
}

/// DELETE /api/roles/:id — 删除角色（系统角色拒绝）
async fn delete_one(
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    Query(body): Query<DeleteReasonRequest>,
) -> Result<Json<ApiResponse<()>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    if !ctx.is_system_admin() {
        return Err(crate::error::AppError::Forbidden(
            "仅系统管理员可删除自定义角色".into(),
        ));
    }
    let reason = body
        .reason
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("用户删除");
    role_repo::delete(&pool, id, &ctx.user.username, reason)?;
    Ok(Json(ApiResponse::ok_msg("角色已删除")))
}

/// PUT /api/roles/:id/permissions — 设置角色权限点
async fn set_perms(
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    Json(body): Json<RolePermissionSet>,
) -> Result<Json<ApiResponse<crate::models::role::RoleWithPermissions>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    if !ctx.is_system_admin() {
        return Err(crate::error::AppError::Forbidden(
            "仅系统管理员可设置自定义角色权限".into(),
        ));
    }
    role_repo::list_all(&pool)?
        .into_iter()
        .find(|role| role.id == id)
        .ok_or_else(|| crate::error::AppError::NotFound("角色不存在".into()))?;
    let role = role_repo::set_permissions(&pool, id, &body, &ctx.user.username)?;
    Ok(Json(ApiResponse::ok(role)))
}

/// GET /api/roles/permissions — 返回权限点白名单（登录即可，无需 admin）
async fn permission_whitelist(
    State(pool): State<DbPool>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<PermissionDef>>>> {
    authz_service::authenticate(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(PERMISSIONS.to_vec())))
}
