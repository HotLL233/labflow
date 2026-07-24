use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::notification::{NotificationChannelInput, NotificationRuleInput};
use crate::models::ApiResponse;
use crate::repo::{audit_repo, notification_repo};
use crate::service::{authz_service, notification_service};
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};

fn can_view(ctx: &authz_service::AuthContext) -> bool {
    ctx.is_system_admin() || ctx.is_analysis_leader()
}
fn require_view(pool: &DbPool, headers: &HeaderMap) -> Result<authz_service::AuthContext> {
    let ctx = authz_service::authenticate(pool, headers)?;
    if can_view(&ctx) {
        Ok(ctx)
    } else {
        Err(AppError::Forbidden(
            "仅系统管理员或分析检测组长可查看通知中心".into(),
        ))
    }
}
fn require_manage(pool: &DbPool, headers: &HeaderMap) -> Result<authz_service::AuthContext> {
    let ctx = authz_service::authenticate(pool, headers)?;
    if ctx.is_system_admin() {
        Ok(ctx)
    } else {
        Err(AppError::Forbidden(
            "仅系统管理员可配置通知渠道和规则".into(),
        ))
    }
}

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/notifications/summary", get(summary))
        .route(
            "/api/notifications/channels",
            get(channels).post(create_channel),
        )
        .route(
            "/api/notifications/channels/:id",
            axum::routing::put(update_channel),
        )
        .route("/api/notifications/channels/:id/test", post(test_channel))
        .route("/api/notifications/rules", get(rules).post(create_rule))
        .route(
            "/api/notifications/rules/:id",
            axum::routing::put(update_rule),
        )
        .route("/api/notifications/deliveries", get(deliveries))
        .route("/api/notifications/process", post(process))
        .route("/api/notifications/inbox", get(inbox))
        .route(
            "/api/notifications/inbox/:id/read",
            axum::routing::put(mark_inbox_read),
        )
        .with_state(pool)
}

async fn summary(
    State(pool): State<DbPool>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<crate::models::notification::NotificationSummary>>> {
    require_view(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(notification_repo::summary(&pool)?)))
}
async fn channels(
    State(pool): State<DbPool>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<crate::models::notification::NotificationChannel>>>> {
    require_view(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(notification_repo::list_channels(
        &pool,
    )?)))
}
async fn create_channel(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(input): Json<NotificationChannelInput>,
) -> Result<Json<ApiResponse<i64>>> {
    let ctx = require_manage(&pool, &headers)?;
    if input.name.trim().is_empty() || !input.webhook_url.trim().starts_with("https://") {
        return Err(AppError::Validation(
            "渠道名称不能为空，钉钉 Webhook 必须以 https:// 开头".into(),
        ));
    }
    let id = notification_repo::create_channel(&pool, &input)?;
    audit_repo::log_actor(
        &pool,
        "create",
        "notification_channels",
        Some(id),
        ctx.user.id,
        &ctx.user.username,
        &format!("新建钉钉通知渠道: {}", input.name),
        "notifications",
    )?;
    Ok(Json(ApiResponse::ok(id)))
}
async fn update_channel(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<NotificationChannelInput>,
) -> Result<Json<ApiResponse<()>>> {
    let ctx = require_manage(&pool, &headers)?;
    if input.name.trim().is_empty() || !input.webhook_url.trim().starts_with("https://") {
        return Err(AppError::Validation(
            "渠道名称不能为空，钉钉 Webhook 必须以 https:// 开头".into(),
        ));
    }
    notification_repo::update_channel(&pool, id, &input)?;
    audit_repo::log_actor(
        &pool,
        "update",
        "notification_channels",
        Some(id),
        ctx.user.id,
        &ctx.user.username,
        &format!("更新钉钉通知渠道: {}", input.name),
        "notifications",
    )?;
    Ok(Json(ApiResponse::ok(())))
}
async fn test_channel(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<String>>> {
    let ctx = require_manage(&pool, &headers)?;
    notification_service::test_channel(&pool, id)?;
    audit_repo::log_actor(
        &pool,
        "test",
        "notification_channels",
        Some(id),
        ctx.user.id,
        &ctx.user.username,
        "测试钉钉通知渠道发送",
        "notifications",
    )?;
    Ok(Json(ApiResponse::ok_msg("测试消息已发送")))
}
async fn rules(
    State(pool): State<DbPool>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<crate::models::notification::NotificationRule>>>> {
    require_view(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(notification_repo::list_rules(&pool)?)))
}
async fn create_rule(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(input): Json<NotificationRuleInput>,
) -> Result<Json<ApiResponse<i64>>> {
    let ctx = require_manage(&pool, &headers)?;
    if input.name.trim().is_empty() {
        return Err(AppError::Validation("规则名称不能为空".into()));
    }
    let id = notification_repo::create_rule(&pool, &input)?;
    audit_repo::log_actor(
        &pool,
        "create",
        "notification_rules",
        Some(id),
        ctx.user.id,
        &ctx.user.username,
        &format!("新建送样通知规则: {}", input.name),
        "notifications",
    )?;
    Ok(Json(ApiResponse::ok(id)))
}
async fn update_rule(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<NotificationRuleInput>,
) -> Result<Json<ApiResponse<()>>> {
    let ctx = require_manage(&pool, &headers)?;
    if input.name.trim().is_empty() {
        return Err(AppError::Validation("规则名称不能为空".into()));
    }
    notification_repo::update_rule(&pool, id, &input)?;
    audit_repo::log_actor(
        &pool,
        "update",
        "notification_rules",
        Some(id),
        ctx.user.id,
        &ctx.user.username,
        &format!("更新送样通知规则: {}", input.name),
        "notifications",
    )?;
    Ok(Json(ApiResponse::ok(())))
}
async fn deliveries(
    State(pool): State<DbPool>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<crate::models::notification::NotificationDelivery>>>> {
    require_view(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(notification_repo::deliveries(
        &pool, 200,
    )?)))
}
async fn process(
    State(pool): State<DbPool>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<String>>> {
    require_manage(&pool, &headers)?;
    let count = notification_service::process_pending(&pool, 50)?;
    Ok(Json(ApiResponse::ok_msg(format!(
        "已处理 {} 条待发送通知",
        count
    ))))
}
async fn inbox(
    State(pool): State<DbPool>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<crate::models::notification::InAppNotification>>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(notification_repo::inbox(
        &pool,
        ctx.user.id,
    )?)))
}

async fn mark_inbox_read(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<()>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    notification_repo::mark_inbox_read(&pool, id, ctx.user.id)?;
    Ok(Json(ApiResponse::ok(())))
}
