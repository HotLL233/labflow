use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::notification::{
    NotificationChannelInput, NotificationRuleInput, NotificationTemplate,
    NotificationTemplateInput,
};
use crate::models::ApiResponse;
use crate::repo::{audit_repo, notification_repo};
use crate::service::{authz_service, notification_service};
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use serde_json::Value;

fn can_view(ctx: &authz_service::AuthContext) -> bool {
    ctx.has_permission("manage:notifications")
}
fn require_view(pool: &DbPool, headers: &HeaderMap) -> Result<authz_service::AuthContext> {
    let ctx = authz_service::authenticate(pool, headers)?;
    if can_view(&ctx) {
        Ok(ctx)
    } else {
        Err(AppError::Forbidden("缺少通知中心配置权限".into()))
    }
}
fn require_manage(pool: &DbPool, headers: &HeaderMap) -> Result<authz_service::AuthContext> {
    let ctx = authz_service::authenticate(pool, headers)?;
    if ctx.has_permission("manage:notifications") {
        Ok(ctx)
    } else {
        Err(AppError::Forbidden("缺少通知中心配置权限".into()))
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
            "/api/notifications/content-template/:event_key",
            get(get_template).put(save_template),
        )
        .route(
            "/api/notifications/rules/:id",
            axum::routing::put(update_rule),
        )
        .route("/api/notifications/deliveries", get(deliveries))
        .route("/api/notifications/process", post(process))
        .route("/api/notifications/business-templates", get(templates))
        .route(
            "/api/notifications/business-templates/:key",
            axum::routing::put(update_template),
        )
        .route("/api/notifications/inbox", get(inbox))
        .route(
            "/api/notifications/inbox/:id/read",
            axum::routing::put(mark_inbox_read),
        )
        .with_state(pool)
}

async fn templates(
    State(pool): State<DbPool>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<NotificationTemplate>>>> {
    require_view(&pool, &headers)?;
    let mut items = vec![
        notification_service::load_template(&pool, notification_service::TEMPLATE_RD)?,
        notification_service::load_template(&pool, notification_service::TEMPLATE_RD_REJECTED)?,
        notification_service::load_template(&pool, notification_service::TEMPLATE_RD_RESUBMITTED)?,
        notification_service::load_template(&pool, notification_service::TEMPLATE_SAMPLE_INFO)?,
        notification_service::load_template(
            &pool,
            notification_service::TEMPLATE_PERSONNEL_CHANGE,
        )?,
        notification_service::load_template(
            &pool,
            notification_service::TEMPLATE_PERSONNEL_CHANGE_REJECTED,
        )?,
    ];
    for sample_type in crate::repo::sample_info_type_repo::list_all(&pool)? {
        items.push(notification_service::load_template(
            &pool,
            &notification_service::sample_type_template_key(&sample_type.type_key),
        )?);
    }
    Ok(Json(ApiResponse::ok(items)))
}

async fn update_template(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(key): Path<String>,
    Json(input): Json<NotificationTemplateInput>,
) -> Result<Json<ApiResponse<()>>> {
    let ctx = require_manage(&pool, &headers)?;
    let valid_sample_type = key
        .strip_prefix("sample_info:")
        .map(|type_key| {
            crate::repo::sample_info_type_repo::list_all(&pool)
                .map(|items| {
                    items
                        .into_iter()
                        .any(|item| item.type_key == type_key && item.is_active != 0)
                })
                .unwrap_or(false)
        })
        .unwrap_or(false);
    if !matches!(
        key.as_str(),
        notification_service::TEMPLATE_RD
            | notification_service::TEMPLATE_RD_REJECTED
            | notification_service::TEMPLATE_RD_RESUBMITTED
            | notification_service::TEMPLATE_SAMPLE_INFO
            | notification_service::TEMPLATE_PERSONNEL_CHANGE
            | notification_service::TEMPLATE_PERSONNEL_CHANGE_REJECTED
    ) && !valid_sample_type
    {
        return Err(AppError::Validation("不支持的通知模板类型".into()));
    }
    if input.title.trim().is_empty() {
        return Err(AppError::Validation("通知标题不能为空".into()));
    }
    let allowed: std::collections::HashSet<String> = {
        let current = notification_service::load_template(&pool, &key)?;
        current
            .fields
            .into_iter()
            .chain(current.available_fields)
            .map(|field| field.key)
            .collect()
    };
    if input
        .fields
        .iter()
        .any(|field| !allowed.contains(&field.key))
    {
        return Err(AppError::Validation(
            "通知模板包含不属于当前通知类型的字段".into(),
        ));
    }
    let value = serde_json::to_value(&input.fields)
        .map_err(|error| AppError::Internal(error.to_string()))?;
    notification_repo::save_template(&pool, &key, &input.title, &input.footer, &value)?;
    audit_repo::log_actor(
        &pool,
        "update",
        "notification_templates",
        Some(0),
        ctx.user.id,
        &ctx.user.username,
        &format!("更新通知模板: {key}"),
        "notifications",
    )?;
    Ok(Json(ApiResponse::ok(())))
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
    let webhook_url = input.webhook_url.as_deref().unwrap_or("");
    if input.name.trim().is_empty() || !webhook_url.trim().starts_with("https://") {
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
    if input.name.trim().is_empty()
        || input
            .webhook_url
            .as_deref()
            .map(|value| !value.trim().starts_with("https://"))
            .unwrap_or(false)
    {
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
async fn get_template(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(event_key): Path<String>,
) -> Result<Json<ApiResponse<Value>>> {
    require_view(&pool, &headers)?;
    let template = notification_service::load_template(&pool, &event_key)?;
    Ok(Json(ApiResponse::ok(
        serde_json::json!({"event_key":event_key,"title":template.title,"footer":template.footer,"fields":template.fields,"available_fields":template.available_fields}),
    )))
}

async fn save_template(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(event_key): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<ApiResponse<()>>> {
    let ctx = require_manage(&pool, &headers)?;
    let title = body
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let footer = body
        .get("footer")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let fields = body
        .get("fields")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    if title.trim().is_empty() || !fields.is_array() {
        return Err(AppError::Validation("通知标题和字段配置不能为空".into()));
    }
    let allowed: std::collections::HashSet<String> = {
        let current = notification_service::load_template(&pool, &event_key)?;
        current
            .fields
            .into_iter()
            .chain(current.available_fields)
            .map(|field| field.key)
            .collect()
    };
    if fields.as_array().unwrap().iter().any(|field| {
        field
            .get("key")
            .and_then(|value| value.as_str())
            .map(|key| !allowed.contains(key))
            .unwrap_or(true)
    }) {
        return Err(AppError::Validation(
            "通知模板包含不属于当前通知类型的字段".into(),
        ));
    }
    notification_repo::save_template(&pool, &event_key, title, footer, &fields)?;
    audit_repo::log_actor(
        &pool,
        "update",
        "notification_templates",
        None,
        ctx.user.id,
        &ctx.user.username,
        "更新通知内容模板",
        "notifications",
    )?;
    Ok(Json(ApiResponse::ok_msg("通知模板已保存")))
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
