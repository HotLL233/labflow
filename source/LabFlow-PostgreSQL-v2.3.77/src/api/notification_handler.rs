use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::notification::{
    NotificationChannelInput, NotificationRetryInput, NotificationRuleInput, NotificationTemplate,
    NotificationTemplateInput,
};
use crate::models::ApiResponse;
use crate::repo::{audit_repo, notification_repo};
use crate::service::{authz_service, notification_service};
use axum::{
    extract::{Path, Query, State},
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
        .route("/api/notifications/events", get(event_statuses))
        .route("/api/notifications/recipients", get(recipient_candidates))
        .route("/api/notifications/events/:id/retry", post(retry_event))
        .route("/api/notifications/process", post(process))
        .route("/api/notifications/business-templates", get(templates))
        .route(
            "/api/notifications/business-templates/:key/preview",
            post(preview_template),
        )
        .route(
            "/api/notifications/business-templates/:key",
            axum::routing::put(update_template),
        )
        .route("/api/notifications/inbox", get(inbox))
        .route(
            "/api/notifications/inbox/unread-count",
            get(inbox_unread_count),
        )
        .route(
            "/api/notifications/inbox/read-all",
            axum::routing::put(mark_all_inbox_read),
        )
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
        notification_service::load_template(
            &pool,
            notification_service::TEMPLATE_PERSONNEL_CHANGE_APPROVED,
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

fn prepare_template_input(
    pool: &DbPool,
    key: &str,
    input: &mut NotificationTemplateInput,
) -> Result<NotificationTemplate> {
    let valid_sample_type = if let Some(type_key) = key.strip_prefix("sample_info:") {
        crate::repo::sample_info_type_repo::list_all(pool)?
            .into_iter()
            .any(|item| item.type_key == type_key && item.is_active != 0)
    } else {
        false
    };
    if !matches!(
        key,
        notification_service::TEMPLATE_RD
            | notification_service::TEMPLATE_RD_REJECTED
            | notification_service::TEMPLATE_RD_RESUBMITTED
            | notification_service::TEMPLATE_SAMPLE_INFO
            | notification_service::TEMPLATE_PERSONNEL_CHANGE
            | notification_service::TEMPLATE_PERSONNEL_CHANGE_REJECTED
            | notification_service::TEMPLATE_PERSONNEL_CHANGE_APPROVED
    ) && !valid_sample_type
    {
        return Err(AppError::Validation("不支持的通知模板类型".into()));
    }
    if input.title.trim().is_empty() {
        return Err(AppError::Validation("通知标题不能为空".into()));
    }
    let current = notification_service::load_template(pool, key)?;
    let mut seen = std::collections::HashSet::new();
    for field in &mut input.fields {
        field.key = notification_repo::canonical_template_field_key(key, &field.key).to_string();
        let source = current
            .fields
            .iter()
            .chain(current.available_fields.iter())
            .find(|candidate| candidate.key == field.key)
            .ok_or_else(|| {
                AppError::Validation("通知字段已停用或不属于当前类型，请刷新字段后重试".into())
            })?;
        if !seen.insert(field.key.clone()) {
            return Err(AppError::Validation("通知模板包含重复字段".into()));
        }
        if field.sort_order < 1 {
            return Err(AppError::Validation("字段显示顺序必须大于零".into()));
        }
        field.source_label = source.source_label.clone();
        let follows = field.follow_source_label.unwrap_or_else(|| {
            field.label == source.label && source.follow_source_label.unwrap_or(false)
        });
        field.follow_source_label = Some(follows);
        if follows {
            field.label = source.source_label.clone();
        }
        if field.label.trim().is_empty() {
            return Err(AppError::Validation("字段显示名称不能为空".into()));
        }
    }
    Ok(current)
}

async fn preview_template(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(key): Path<String>,
    Json(mut input): Json<NotificationTemplateInput>,
) -> Result<Json<ApiResponse<Value>>> {
    require_view(&pool, &headers)?;
    let mut template = prepare_template_input(&pool, &key, &mut input)?;
    template.title = input.title;
    template.footer = input.footer;
    template.fields = input.fields;
    let mut payload = serde_json::json!({"detection_type":"液相", "instrument_type":"液相", "instrument_code":"仪器示例",
        "batch_no":"样品批号示例", "business_no":"送样编号示例", "submitted_by":"送样人示例", "submitted_at":"2026-10-14 09:00:00",
        "division_name":"部门示例", "lab_name":"实验室示例", "project_name":"项目示例", "method_name":"方法示例", "quantity":"3",
        "notes":"注意事项示例", "notice_no":"通知编号示例", "change_type":"变动类型示例", "person_name":"人员示例"});
    for field in &template.fields {
        payload
            .as_object_mut()
            .unwrap()
            .entry(field.key.clone())
            .or_insert_with(|| serde_json::json!(format!("{}示例", field.source_label)));
    }
    let (title, markdown) =
        notification_service::render_template_message(&template, &payload, "通知示例");
    Ok(Json(ApiResponse::ok(
        serde_json::json!({"title":title,"markdown":markdown}),
    )))
}

async fn update_template(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(key): Path<String>,
    Json(mut input): Json<NotificationTemplateInput>,
) -> Result<Json<ApiResponse<()>>> {
    let ctx = require_manage(&pool, &headers)?;
    prepare_template_input(&pool, &key, &mut input)?;
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
    let sender_pool = pool.clone();
    tokio::task::spawn_blocking(move || notification_service::test_channel(&sender_pool, id))
        .await
        .map_err(|error| AppError::Internal(error.to_string()))??;
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
    let mut input: NotificationTemplateInput = serde_json::from_value(body)
        .map_err(|_| AppError::Validation("通知模板格式无效，请检查标题和字段配置".into()))?;
    prepare_template_input(&pool, &event_key, &mut input)?;
    let fields = serde_json::to_value(&input.fields)
        .map_err(|error| AppError::Internal(error.to_string()))?;
    notification_repo::save_template(&pool, &event_key, &input.title, &input.footer, &fields)?;
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
    let count =
        tokio::task::spawn_blocking(move || notification_service::process_pending(&pool, 50))
            .await
            .map_err(|error| AppError::Internal(error.to_string()))??;
    Ok(Json(ApiResponse::ok_msg(format!(
        "已处理 {} 条待发送通知",
        count
    ))))
}

async fn event_statuses(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(query): Query<crate::models::notification::NotificationEventQuery>,
) -> Result<Json<ApiResponse<Vec<crate::models::notification::NotificationEventStatus>>>> {
    require_view(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(notification_repo::event_statuses(
        &pool, &query,
    )?)))
}

async fn recipient_candidates(
    State(pool): State<DbPool>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<crate::models::notification::NotificationRecipient>>>> {
    require_manage(&pool, &headers)?;
    let candidates = crate::repo::user_repo::list_all(&pool)?
        .into_iter()
        .filter(|user| user.is_active)
        .filter_map(|user| {
            let ctx = authz_service::AuthContext {
                role_names: user.role_names.clone(),
                user,
            };
            let analysis = ctx.has_permission("entry:workload");
            let personnel = ctx.is_system_admin()
                || (ctx.has_permission("feedback:personnel:view")
                    && (ctx.is_analysis_leader() || ctx.is_rd_leader()));
            (analysis || personnel).then(|| crate::models::notification::NotificationRecipient {
                id: ctx.user.id,
                username: ctx.user.username,
                analysis,
                personnel,
            })
        })
        .collect();
    Ok(Json(ApiResponse::ok(candidates)))
}

async fn retry_event(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<NotificationRetryInput>,
) -> Result<Json<ApiResponse<()>>> {
    let ctx = require_manage(&pool, &headers)?;
    notification_repo::retry_event(&pool, id, &input.reason, ctx.user.id, &ctx.user.username)?;
    Ok(Json(ApiResponse::ok_msg(
        "已加入补发队列；成功渠道和已生成的个人通知不会重复发送",
    )))
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

async fn inbox_unread_count(
    State(pool): State<DbPool>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<i64>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(
        notification_repo::inbox_unread_count(&pool, ctx.user.id)?,
    )))
}

async fn mark_all_inbox_read(
    State(pool): State<DbPool>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<i64>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(
        notification_repo::mark_all_inbox_read(&pool, ctx.user.id)?,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn inbox_count_and_read_all_require_auth_and_only_touch_current_user() {
        let pool = crate::db::init_pool("notification-inbox-api-test");
        let conn = pool.get().unwrap();
        crate::db::test_migrations::run(&conn).unwrap();
        let mut accounts = Vec::new();
        for name in ["inbox-person-a", "inbox-person-b"] {
            conn.execute(
                "INSERT INTO users(username,password,is_admin,is_active) VALUES(?1,'unused',0,1)",
                [name],
            )
            .unwrap();
            let id = conn.last_insert_rowid();
            let user = crate::repo::user_repo::find_by_id(&pool, id)
                .unwrap()
                .unwrap();
            let login = crate::service::auth_service::create_login_response(
                &pool,
                user,
                false,
                Some("inbox-test-device"),
                Some("test"),
            )
            .unwrap();
            let mut headers = HeaderMap::new();
            headers.insert(
                axum::http::header::AUTHORIZATION,
                format!("Bearer {}", login.token).parse().unwrap(),
            );
            accounts.push((id, headers));
        }
        conn.execute("INSERT INTO in_app_notifications(event_id,user_id,title,body) SELECT 40000+n,?1,'我的通知','内容' FROM generate_series(1,105) AS n", [accounts[0].0]).unwrap();
        conn.execute("INSERT INTO in_app_notifications(event_id,user_id,title,body) VALUES(50001,?1,'他人通知','内容')", [accounts[1].0]).unwrap();
        let other: i64 = conn
            .query_row(
                "SELECT id FROM in_app_notifications WHERE user_id=?1",
                [accounts[1].0],
                |row| row.get(0),
            )
            .unwrap();
        assert!(inbox_unread_count(State(pool.clone()), HeaderMap::new())
            .await
            .is_err());
        assert!(mark_all_inbox_read(State(pool.clone()), HeaderMap::new())
            .await
            .is_err());
        assert_eq!(
            inbox_unread_count(State(pool.clone()), accounts[0].1.clone())
                .await
                .unwrap()
                .0
                .data,
            Some(105)
        );
        assert!(matches!(
            mark_inbox_read(State(pool.clone()), accounts[0].1.clone(), Path(other)).await,
            Err(AppError::NotFound(_))
        ));
        assert_eq!(
            mark_all_inbox_read(State(pool.clone()), accounts[0].1.clone())
                .await
                .unwrap()
                .0
                .data,
            Some(105)
        );
        assert_eq!(
            mark_all_inbox_read(State(pool.clone()), accounts[0].1.clone())
                .await
                .unwrap()
                .0
                .data,
            Some(0)
        );
        assert_eq!(
            inbox_unread_count(State(pool.clone()), accounts[0].1.clone())
                .await
                .unwrap()
                .0
                .data,
            Some(0)
        );
        assert_eq!(
            inbox_unread_count(State(pool.clone()), accounts[1].1.clone())
                .await
                .unwrap()
                .0
                .data,
            Some(1)
        );
    }

    #[tokio::test]
    async fn template_preview_checks_permissions_and_does_not_write_notifications() {
        let pool = crate::db::init_pool("notification-template-api-test");
        let conn = pool.get().unwrap();
        crate::db::test_migrations::run(&conn).unwrap();
        conn.execute(
            "INSERT INTO roles(name,is_system) VALUES('notification-template-manager',0)",
            [],
        )
        .unwrap();
        let role = conn.last_insert_rowid();
        conn.execute("INSERT INTO role_permissions(role_id,permission_key) VALUES(?1,'manage:notifications')", [role]).unwrap();
        let mut headers = Vec::new();
        for (name, manager) in [("template-ordinary", false), ("template-manager", true)] {
            conn.execute(
                "INSERT INTO users(username,password,is_admin,is_active) VALUES(?1,'unused',0,1)",
                [name],
            )
            .unwrap();
            let id = conn.last_insert_rowid();
            if manager {
                conn.execute(
                    "INSERT INTO user_roles(user_id,role_id) VALUES(?1,?2)",
                    postgres_compat::params![id, role],
                )
                .unwrap();
            }
            let user = crate::repo::user_repo::find_by_id(&pool, id)
                .unwrap()
                .unwrap();
            let login = crate::service::auth_service::create_login_response(
                &pool,
                user,
                false,
                Some("template-test-device"),
                Some("test"),
            )
            .unwrap();
            let mut item = HeaderMap::new();
            item.insert(
                axum::http::header::AUTHORIZATION,
                format!("Bearer {}", login.token).parse().unwrap(),
            );
            headers.push(item);
        }
        let key = notification_service::TEMPLATE_RD.to_string();
        let input: NotificationTemplateInput = serde_json::from_value(serde_json::json!({
            "title":"预览 {{注意事项}}", "footer":"示例页脚", "fields":[
                {"key":"notes","label":"伪造名称","source_label":"伪造来源","follow_source_label":true,"visible":true,"bold":true,"sort_order":1}
            ]
        })).unwrap();
        let snapshot = || {
            conn.query_row("SELECT (SELECT COUNT(*) FROM notification_templates),(SELECT COUNT(*) FROM notification_events),(SELECT COUNT(*) FROM notification_deliveries),(SELECT COUNT(*) FROM audit_log)", [], |row| Ok((row.get::<_,i64>(0)?,row.get::<_,i64>(1)?,row.get::<_,i64>(2)?,row.get::<_,i64>(3)?))).unwrap()
        };
        let before = snapshot();
        assert!(matches!(
            preview_template(
                State(pool.clone()),
                headers[0].clone(),
                Path(key.clone()),
                Json(input.clone())
            )
            .await,
            Err(AppError::Forbidden(_))
        ));
        assert!(matches!(
            update_template(
                State(pool.clone()),
                headers[0].clone(),
                Path(key.clone()),
                Json(input.clone())
            )
            .await,
            Err(AppError::Forbidden(_))
        ));
        let preview = preview_template(
            State(pool.clone()),
            headers[1].clone(),
            Path(key.clone()),
            Json(input.clone()),
        )
        .await
        .unwrap()
        .0
        .data
        .unwrap();
        let source_label: String = conn
            .query_row(
                "SELECT label FROM rd_record_columns WHERE name='notes'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(preview["markdown"]
            .as_str()
            .unwrap()
            .contains(&source_label));
        assert!(!preview["markdown"].as_str().unwrap().contains("伪造"));
        assert_eq!(snapshot(), before);
        let mut duplicate = input.clone();
        duplicate.fields.push(duplicate.fields[0].clone());
        assert!(matches!(
            prepare_template_input(&pool, &key, &mut duplicate),
            Err(AppError::Validation(_))
        ));
        let mut invalid = input.clone();
        invalid.fields[0].key = "not_a_business_field".into();
        assert!(matches!(
            prepare_template_input(&pool, &key, &mut invalid),
            Err(AppError::Validation(_))
        ));
        let _ = update_template(
            State(pool.clone()),
            headers[1].clone(),
            Path(key.clone()),
            Json(input),
        )
        .await
        .unwrap();
        let loaded = notification_service::load_template(&pool, &key).unwrap();
        assert_eq!(loaded.fields[0].label, source_label);
        assert_eq!(loaded.fields[0].follow_source_label, Some(true));
        assert_eq!(snapshot().1, before.1);
        assert_eq!(snapshot().2, before.2);
    }

    #[tokio::test]
    async fn notification_retry_and_recipient_candidates_require_notification_management() {
        let pool = crate::db::init_pool("notification-retry-permission-test");
        let conn = pool.get().unwrap();
        crate::db::test_migrations::run(&conn).unwrap();
        conn.execute(
            "INSERT INTO roles(name,is_system) VALUES('notify-retry-manager',0)",
            [],
        )
        .unwrap();
        let role = conn.last_insert_rowid();
        conn.execute("INSERT INTO role_permissions(role_id,permission_key) VALUES(?1,'manage:notifications')",[role]).unwrap();
        let mut headers = Vec::new();
        for (name, manager) in [
            ("notify-retry-ordinary", false),
            ("notify-retry-manager", true),
        ] {
            conn.execute(
                "INSERT INTO users(username,password,is_admin,is_active) VALUES(?1,'unused',0,1)",
                [name],
            )
            .unwrap();
            let id = conn.last_insert_rowid();
            if manager {
                conn.execute(
                    "INSERT INTO user_roles(user_id,role_id) VALUES(?1,?2)",
                    [id, role],
                )
                .unwrap();
            }
            let user = crate::repo::user_repo::find_by_id(&pool, id)
                .unwrap()
                .unwrap();
            let login = crate::service::auth_service::create_login_response(
                &pool,
                user,
                false,
                Some("notify-retry-test"),
                Some("test"),
            )
            .unwrap();
            let mut item = HeaderMap::new();
            item.insert(
                axum::http::header::AUTHORIZATION,
                format!("Bearer {}", login.token).parse().unwrap(),
            );
            headers.push(item);
        }
        notification_repo::enqueue_event(&pool, "rd_work_record", 6101, &serde_json::json!({}))
            .unwrap();
        conn.execute("UPDATE notification_events SET status='failed'", [])
            .unwrap();
        let event: i64 = conn
            .query_row("SELECT id FROM notification_events LIMIT 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        let input = || {
            Json(NotificationRetryInput {
                reason: "修复后补发".into(),
            })
        };
        assert!(matches!(
            retry_event(
                State(pool.clone()),
                headers[0].clone(),
                Path(event),
                input()
            )
            .await,
            Err(AppError::Forbidden(_))
        ));
        assert!(matches!(
            recipient_candidates(State(pool.clone()), headers[0].clone()).await,
            Err(AppError::Forbidden(_))
        ));
        assert!(matches!(
            retry_event(
                State(pool.clone()),
                headers[1].clone(),
                Path(event),
                Json(NotificationRetryInput {
                    reason: String::new()
                })
            )
            .await,
            Err(AppError::Validation(_))
        ));
        // Managing notifications is sufficient; no user-management grant is needed.
        recipient_candidates(State(pool.clone()), headers[1].clone())
            .await
            .unwrap();
        retry_event(
            State(pool.clone()),
            headers[1].clone(),
            Path(event),
            input(),
        )
        .await
        .unwrap();
        assert!(matches!(
            retry_event(
                State(pool.clone()),
                headers[1].clone(),
                Path(event),
                input()
            )
            .await,
            Err(AppError::Conflict(_))
        ));
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM audit_log WHERE action='retry'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }
}
