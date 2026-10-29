use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::personnel_feedback::{
    PersonnelFeedback, PersonnelFeedbackCreate, PersonnelFeedbackUpdate,
};
use crate::models::ApiResponse;
use crate::repo::personnel_feedback_repo;
use crate::service::authz_service;
use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json, Router,
};
use serde::Deserialize;

fn can_view_global(ctx: &authz_service::AuthContext) -> bool {
    ctx.is_system_admin() || ctx.is_analysis_leader()
}
fn require_view(pool: &DbPool, headers: &HeaderMap) -> Result<authz_service::AuthContext> {
    let ctx = authz_service::authenticate(pool, headers)?;
    authz_service::require_permission(&ctx, "feedback:personnel:view")?;
    Ok(ctx)
}
fn require_editor(pool: &DbPool, headers: &HeaderMap) -> Result<authz_service::AuthContext> {
    let ctx = require_view(pool, headers)?;
    if !ctx.is_system_admin() && !ctx.has_permission("feedback:personnel:edit") {
        return Err(AppError::Forbidden("缺少人员变动反馈编辑权限".into()));
    }
    Ok(ctx)
}
fn assert_lab(ctx: &authz_service::AuthContext, lab_id: i64) -> Result<()> {
    if ctx.is_system_admin() || ctx.user.group_id == Some(lab_id) {
        Ok(())
    } else {
        Err(AppError::Forbidden("\u{53ea}\u{80fd}\u{64cd}\u{4f5c}\u{672c}\u{5b9e}\u{9a8c}\u{5ba4}\u{7684}\u{4eba}\u{5458}\u{53d8}\u{52a8}\u{53cd}\u{9988}".into()))
    }
}

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route(
            "/api/personnel-feedback",
            axum::routing::get(list).post(create),
        )
        .route(
            "/api/personnel-feedback/:id",
            axum::routing::put(update).delete(delete),
        )
        .route(
            "/api/personnel-feedback/:id/withdraw",
            axum::routing::post(withdraw),
        )
        .with_state(pool)
}
async fn list(
    State(pool): State<DbPool>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<PersonnelFeedback>>>> {
    let ctx = require_view(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(personnel_feedback_repo::list(
        &pool,
        ctx.user.group_id,
        can_view_global(&ctx),
    )?)))
}
async fn create(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(body): Json<PersonnelFeedbackCreate>,
) -> Result<Json<ApiResponse<PersonnelFeedback>>> {
    let ctx = require_editor(&pool, &headers)?;
    assert_lab(&ctx, body.lab_id)?;
    Ok(Json(ApiResponse::ok(personnel_feedback_repo::create(
        &pool,
        &body,
        ctx.user.id,
        &ctx.user.username,
    )?)))
}
async fn update(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<PersonnelFeedbackUpdate>,
) -> Result<Json<ApiResponse<PersonnelFeedback>>> {
    let ctx = require_editor(&pool, &headers)?;
    let item = personnel_feedback_repo::list(&pool, ctx.user.group_id, false)?
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| {
            AppError::Forbidden("\u{65e0}\u{6743}\u{7f16}\u{8f91}\u{8be5}\u{53cd}\u{9988}".into())
        })?;
    assert_lab(&ctx, item.lab_id)?;
    Ok(Json(ApiResponse::ok(personnel_feedback_repo::update(
        &pool,
        id,
        &body,
        ctx.user.id,
        &ctx.user.username,
    )?)))
}
async fn withdraw(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<PersonnelFeedback>>> {
    let ctx = require_editor(&pool, &headers)?;
    let item = personnel_feedback_repo::list(&pool, ctx.user.group_id, false)?
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| {
            AppError::Forbidden("\u{65e0}\u{6743}\u{64a4}\u{56de}\u{8be5}\u{53cd}\u{9988}".into())
        })?;
    assert_lab(&ctx, item.lab_id)?;
    Ok(Json(ApiResponse::ok(personnel_feedback_repo::withdraw(
        &pool,
        id,
        ctx.user.id,
        &ctx.user.username,
    )?)))
}
#[derive(Deserialize)]
struct DeleteQuery {
    reason: Option<String>,
}
async fn delete(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(query): Query<DeleteQuery>,
) -> Result<Json<ApiResponse<()>>> {
    let ctx = require_editor(&pool, &headers)?;
    let item = personnel_feedback_repo::list(&pool, ctx.user.group_id, false)?
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| {
            AppError::Forbidden("\u{65e0}\u{6743}\u{5220}\u{9664}\u{8be5}\u{53cd}\u{9988}".into())
        })?;
    assert_lab(&ctx, item.lab_id)?;
    personnel_feedback_repo::delete(
        &pool,
        id,
        ctx.user.id,
        &ctx.user.username,
        query
            .reason
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("用户删除"),
    )?;
    Ok(Json(ApiResponse::ok_msg("已移入回收站")))
}
