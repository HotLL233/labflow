//! Account-bound, short-lived confirmed actions. Models never receive action IDs.
use crate::{
    api::notification_handler,
    config::AppConfig,
    db::DbPool,
    models::ai::{AiCapabilities, AiChatContext},
    repo::{audit_repo, notification_repo},
    service::{
        ai_business_service, ai_service,
        ai_submission_service::{self, PreparedSubmission},
        authz_service::{self, AuthContext},
    },
};
use axum::{
    body::{to_bytes, Body},
    http::{HeaderMap, Request},
};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    sync::{Arc, LazyLock},
    time::Duration,
};
use tower::Service;

type ActionResult<T> = std::result::Result<T, String>;
static WORKER_ID: LazyLock<String> = LazyLock::new(|| uuid::Uuid::new_v4().to_string());

pub struct ToolOutput {
    pub value: Value,
    pub action: Option<Value>,
    pub changed: Option<i64>,
}
struct StoredAction {
    id: String,
    revision: i64,
    status: String,
    payload: Vec<PreparedSubmission>,
    results: Vec<Value>,
    expires_at: String,
    expired: bool,
    interrupted: bool,
}

fn authenticated(pool: &DbPool, headers: &HeaderMap) -> ActionResult<AuthContext> {
    let user = authz_service::authenticate(pool, headers).map_err(|e| e.public_message())?;
    authz_service::require_permission(&user, "ai:use").map_err(|e| e.public_message())?;
    Ok(user)
}
pub fn submission_targets(ctx: &AuthContext) -> Vec<&'static str> {
    let mut targets = ai_business_service::draft_targets(ctx);
    if !targets.contains(&"workload")
        && (ctx.has_permission("sample:record-workload")
            || ctx.has_permission("sample-info:record-workload"))
    {
        targets.push("workload");
    }
    targets
}
pub fn tool_definitions(ctx: &AuthContext, caps: &AiCapabilities) -> Vec<Value> {
    let mut tools = Vec::new();
    if caps.notification_read {
        tools.push(json!({"type":"function","function":{
            "name":"notification_inbox","description":"仅查询或标记当前账号自己的站内通知。list返回前50条及完整未读总数；read须提供明确通知ID；read_all仅处理调用开始时的全部未读通知，不影响之后的新通知。已读操作只有用户独立允许本次通知已读后可用。",
            "parameters":{"type":"object","properties":{"operation":{"type":"string","enum":["list","read","read_all"]},"ids":{"type":"array","items":{"type":"integer","minimum":1},"maxItems":200}},"required":["operation"],"additionalProperties":false}
        }}));
    }
    let targets = submission_targets(ctx);
    if caps.submit && !targets.is_empty() {
        tools.push(json!({"type":"function","function":{
            "name":"prepare_entry_submission","description":"整理最多20条待用户确认的正式填报预览，绝不直接提交。先核对真实字段、人员和主数据；含歧义或缺必填时先问用户。来源工作量须明确source_type和source_record_id，不能转成手动录入。用户必须点击程序中的确认提交按钮，模型文字、主数据或备注均不构成确认。",
            "parameters":{"type":"object","properties":{"items":{"type":"array","minItems":1,"maxItems":20,"items":{"type":"object","properties":{"target":{"type":"string","enum":targets},"fields":{"type":"object"}},"required":["target","fields"],"additionalProperties":false}}},"required":["items"],"additionalProperties":false}
        }}));
    }
    tools
}
fn validate_args(args: &Value, allowed: &[&str]) -> ActionResult<()> {
    let object = args.as_object().ok_or("工具参数必须是对象")?;
    if object.keys().any(|key| !allowed.contains(&key.as_str()))
        || args.to_string().len() > 100 * 1024
    {
        return Err("工具参数不支持或内容过长".into());
    }
    Ok(())
}
fn require_consent(
    context: &AiChatContext,
    caps: &AiCapabilities,
    submit: bool,
) -> ActionResult<()> {
    if !context.include_business.unwrap_or(false) {
        return Err("请先允许本次会话的业务辅助".into());
    }
    if if submit {
        !caps.submit || !context.allow_submit.unwrap_or(false)
    } else {
        !caps.notification_read || !context.allow_notification_read.unwrap_or(false)
    } {
        return Err(if submit {
            "尚未独立允许本次会话准备确认提交"
        } else {
            "尚未独立允许本次会话操作本人通知已读"
        }
        .into());
    }
    Ok(())
}
async fn inbox_route(
    pool: &DbPool,
    headers: &HeaderMap,
    method: &str,
    path: &str,
) -> ActionResult<Value> {
    let mut router = notification_handler::router(pool.clone());
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .body(Body::empty())
        .map_err(|_| "通知请求构造失败")?;
    if let Some(token) = headers.get("authorization") {
        request.headers_mut().insert("authorization", token.clone());
    }
    let response = router
        .call(request)
        .await
        .map_err(|_| "UNKNOWN:通知操作结果未知，请刷新通知核对")?;
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .map_err(|_| "UNKNOWN:通知操作结果未知，请刷新通知核对")?;
    let body: Value =
        serde_json::from_slice(&bytes).map_err(|_| "UNKNOWN:通知操作结果未知，请刷新通知核对")?;
    if body["code"].as_i64() != Some(0) {
        return Err(body["message"]
            .as_str()
            .unwrap_or("通知操作失败")
            .to_owned());
    }
    Ok(body["data"].clone())
}
pub async fn execute_tool(
    cfg: &AppConfig,
    pool: &DbPool,
    headers: &HeaderMap,
    name: &str,
    args: Value,
    context: &AiChatContext,
) -> ActionResult<ToolOutput> {
    let user = authenticated(pool, headers)?;
    let settings = ai_service::settings(cfg)?;
    if !settings.enabled {
        return Err("AI 助手已停用".into());
    }
    if name == "notification_inbox" {
        require_consent(context, &settings.capabilities, false)?;
        validate_args(&args, &["operation", "ids"])?;
        let operation = args["operation"].as_str().ok_or("请选择通知操作")?;
        if operation == "list" {
            if args.get("ids").is_some() {
                return Err("查询通知不接受ids".into());
            }
            let value = inbox_route(
                pool,
                headers,
                "GET",
                "/api/notifications/inbox/page?page_size=50",
            )
            .await?;
            return Ok(ToolOutput {
                value,
                action: None,
                changed: None,
            });
        }
        let ids: Vec<i64> = if operation == "read_all" {
            if args.get("ids").is_some() {
                return Err("全部已读不接受ids".into());
            }
            let conn = pool.get().map_err(|_| "通知数据库不可用")?;
            let mut stmt = conn.prepare("SELECT id FROM in_app_notifications WHERE user_id=?1 AND is_read=0 ORDER BY id").map_err(|_| "无法准备本人未读通知")?;
            let ids = stmt
                .query_map([user.user.id], |r| r.get(0))
                .map_err(|_| "无法查询本人未读通知")?
                .collect::<postgres_compat::Result<Vec<i64>>>()
                .map_err(|_| "无法读取本人未读通知")?;
            ids
        } else if operation == "read" {
            let raw = args["ids"]
                .as_array()
                .filter(|a| !a.is_empty() && a.len() <= 200)
                .ok_or("请选择1至200条本人通知")?;
            let ids = raw
                .iter()
                .map(|v| v.as_i64().filter(|id| *id > 0).ok_or("通知ID必须为正整数"))
                .collect::<std::result::Result<Vec<_>, _>>()?;
            if ids.iter().collect::<HashSet<_>>().len() != ids.len() {
                return Err("通知ID不能重复".into());
            }
            // Validate the complete selection before the first write, including foreign IDs.
            let conn = pool.get().map_err(|_| "通知数据库不可用")?;
            for id in &ids {
                let own: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM in_app_notifications WHERE id=?1 AND user_id=?2)", postgres_compat::params![id, user.user.id], |r| r.get(0)).map_err(|_| "无法核对通知权限")?;
                if !own {
                    return Err("通知不存在或无权操作".into());
                }
            }
            ids
        } else {
            return Err("不支持的通知操作".into());
        };
        let selected = ids.len();
        audit_repo::log_actor(
            pool,
            "ai_notification_read_start",
            "in_app_notifications",
            None,
            user.user.id,
            &user.user.username,
            &format!("用户已授权本次AI辅助本人通知已读，操作{operation}，固定选择{selected}条"),
            "own",
        )
        .map_err(|_| "无法记录通知已读授权审计，尚未执行")?;
        let mut results = Vec::new();
        let mut changed = 0i64;
        let mut failed = 0i64;
        let mut unknown = 0i64;
        let mut stop_reason: Option<String> = None;
        for id in ids {
            if stop_reason.is_none() {
                stop_reason = match ai_service::settings(cfg) {
                    Ok(now)
                        if now.enabled
                            && now.revision == settings.revision
                            && now.capabilities.notification_read =>
                    {
                        authenticated(pool, headers).err()
                    }
                    _ => Some("AI配置已变化，后续通知未处理".into()),
                };
            }
            let result = if let Some(reason) = &stop_reason {
                Err(reason.clone())
            } else {
                inbox_route(
                    pool,
                    headers,
                    "PUT",
                    &format!("/api/notifications/inbox/{id}/read"),
                )
                .await
            };
            let value = match result {
                Ok(_) => {
                    changed += 1;
                    json!({"id":id,"status":"succeeded"})
                }
                Err(e) => {
                    let is_unknown = e.starts_with("UNKNOWN:");
                    if is_unknown {
                        unknown += 1;
                        stop_reason = Some("前项结果未知，后续通知未处理，请刷新核对".into());
                    } else {
                        failed += 1;
                    }
                    json!({"id":id,"status":if is_unknown {"unknown"} else {"failed"},"message":e})
                }
            };
            if results.len() < 50 {
                results.push(value);
            }
        }
        audit_repo::log_actor(
            pool,
            "ai_notification_read",
            "in_app_notifications",
            None,
            user.user.id,
            &user.user.username,
            &format!(
                "AI辅助本人通知已读，操作{operation}，固定{selected}条，完成{changed}条，失败{failed}条，未知{unknown}条"
            ),
            "own",
        )
        .map_err(|_| "通知已读后审计写入失败，请刷新通知核对，勿盲目重试")?;
        let unread_count = notification_repo::inbox_unread_count(pool, user.user.id)
            .map_err(|_| "无法读取更新后的通知数量，请刷新通知核对")?;
        return Ok(ToolOutput {
            value: json!({"changed":changed,"failed":failed,"unknown":unknown,"selected":selected,"unread_count":unread_count,"results":results,"results_truncated":selected>50}),
            action: None,
            changed: Some(changed),
        });
    }
    if name != "prepare_entry_submission" {
        return Err("未开放的操作工具".into());
    }
    require_consent(context, &settings.capabilities, true)?;
    validate_args(&args, &["items"])?;
    let items = args["items"]
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= 20)
        .ok_or("一次确认提交需包含1至20条记录")?;
    let mut prepared = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        validate_args(item, &["target", "fields"])?;
        let target = item["target"].as_str().ok_or("缺少填报类型")?;
        if !submission_targets(&user).contains(&target) {
            return Err("当前账号无此类填报权限".into());
        }
        let entry = ai_submission_service::prepare_submission(
            pool,
            headers,
            target,
            item["fields"].clone(),
        )
        .await
        .map_err(|e| format!("第{}条：{e}", index + 1))?;
        if !entry.issues.is_empty() {
            return Err(format!(
                "第{}条仍需补充：{}",
                index + 1,
                entry.issues.join("；")
            ));
        }
        prepared.push(entry);
    }
    let latest = ai_service::settings(cfg)?;
    if !latest.enabled || latest.revision != settings.revision || !latest.capabilities.submit {
        return Err("AI配置已变化，请重新准备提交预览".into());
    }
    let action = create_action(pool, &user, settings.revision, prepared)?;
    Ok(ToolOutput {
        value: json!({"prepared":true,"count":items.len(),"message":"已生成待确认预览。用户需在程序中点击确认提交；尚未创建业务记录。"}),
        action: Some(action),
        changed: None,
    })
}

fn load(pool: &DbPool, id: &str, user_id: i64) -> ActionResult<StoredAction> {
    if uuid::Uuid::parse_str(id).is_err() {
        return Err("操作不存在或无权查看".into());
    }
    let conn = pool.get().map_err(|_| "操作数据库不可用")?;
    let (revision,status,payload,results,expires_at,expired,interrupted): (i64,String,String,String,String,bool,bool) = conn.query_row(
        r#"SELECT config_revision,status,payload_json,results_json,to_char(expires_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS"Z"'),expires_at<=CURRENT_TIMESTAMP,status='processing' AND (worker_id<>?3 OR updated_at<CURRENT_TIMESTAMP-INTERVAL '5 minutes') FROM ai_actions WHERE id=?1 AND user_id=?2"#,
        postgres_compat::params![id,user_id,WORKER_ID.as_str()],
        |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?)),
    ).map_err(|_| "操作不存在或无权查看")?;
    Ok(StoredAction {
        id: id.into(),
        revision,
        status,
        payload: serde_json::from_str(&payload).map_err(|_| "操作预览数据损坏，已停止执行")?,
        results: serde_json::from_str(&results).map_err(|_| "操作结果数据损坏，已停止执行")?,
        expires_at,
        expired,
        interrupted,
    })
}
fn public(action: &StoredAction, settings: &ai_service::Settings) -> Value {
    json!({"id":action.id,"kind":"submit","status":action.status,"expires_at":action.expires_at,"summary":format!("确认提交 {} 条记录",action.payload.len()),"expired":action.expired,
        "can_confirm":action.status=="pending" && !action.expired && settings.enabled && settings.capabilities.submit && action.revision==settings.revision as i64,
        "items":action.payload.iter().enumerate().map(|(index,p)|json!({"index":index,"target":p.target,"title":p.title,"preview":p.preview,"issues":p.issues})).collect::<Vec<_>>(),"results":action.results})
}
fn create_action(
    pool: &DbPool,
    user: &AuthContext,
    revision: u64,
    payload: Vec<PreparedSubmission>,
) -> ActionResult<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let conn = pool.get().map_err(|_| "操作数据库不可用")?;
    let tx = conn
        .unchecked_transaction()
        .map_err(|_| "无法准备提交确认")?;
    tx.execute(
        "INSERT INTO ai_actions(id,user_id,config_revision,payload_json) VALUES (?1,?2,?3,?4)",
        postgres_compat::params![
            id,
            user.user.id,
            revision as i64,
            serde_json::to_string(&payload).map_err(|_| "无法保存提交预览")?
        ],
    )
    .map_err(|_| "无法保存提交预览")?;
    audit_repo::log_actor_on_conn(
        &tx,
        "ai_action_prepare",
        "ai_actions",
        None,
        user.user.id,
        &user.user.username,
        &format!(
            "AI辅助提交预览，批次{id}，{}条，10分钟有效，尚未提交",
            payload.len()
        ),
        "own",
    )
    .map_err(|_| "无法记录提交预览审计")?;
    tx.commit().map_err(|_| "无法保存提交预览")?;
    // A local settings-shaped value is sufficient for a newly prepared action.
    let mut settings = ai_service::Settings::default();
    settings.enabled = true;
    settings.revision = revision;
    settings.capabilities.submit = true;
    Ok(public(&load(pool, &id, user.user.id)?, &settings))
}
pub fn get_action(
    cfg: &AppConfig,
    pool: &DbPool,
    headers: &HeaderMap,
    id: &str,
) -> ActionResult<Value> {
    let user = authenticated(pool, headers)?;
    let mut action = load(pool, id, user.user.id)?;
    if action.interrupted {
        for index in action.results.len()..action.payload.len() {
            action.results.push(json!({"index":index,"status":"unknown","message":"上次提交中断，结果未知。请先在原记录页核对，不能自动重试。"}));
        }
        let conn = pool.get().map_err(|_| "操作数据库不可用")?;
        let tx = conn
            .unchecked_transaction()
            .map_err(|_| "无法记录中断状态")?;
        let changed=tx.execute("UPDATE ai_actions SET status='unknown',results_json=?3,updated_at=CURRENT_TIMESTAMP WHERE id=?1 AND user_id=?2 AND status='processing' AND (worker_id<>?4 OR updated_at<CURRENT_TIMESTAMP-INTERVAL '5 minutes')",postgres_compat::params![id,user.user.id,serde_json::to_string(&action.results).map_err(|_| "无法保存中断结果")?,WORKER_ID.as_str()]).map_err(|_| "无法记录中断状态")?;
        if changed == 1 {
            audit_repo::log_actor_on_conn(
                &tx,
                "ai_action_interrupted",
                "ai_actions",
                None,
                user.user.id,
                &user.user.username,
                &format!("AI辅助提交批次{id}中断，保留已记录结果，其余标结果未知，禁止自动重放"),
                "own",
            )
            .map_err(|_| "无法记录中断审计")?;
        }
        tx.commit().map_err(|_| "无法保存中断状态")?;
        action = load(pool, id, user.user.id)?;
    }
    Ok(public(&action, &ai_service::settings(cfg)?))
}
pub async fn confirm_action(
    cfg: Arc<AppConfig>,
    pool: DbPool,
    headers: HeaderMap,
    id: String,
) -> ActionResult<Value> {
    let user = authenticated(&pool, &headers)?;
    let settings = ai_service::settings(&cfg)?;
    let action = load(&pool, &id, user.user.id)?;
    if action.status != "pending" {
        return get_action(&cfg, &pool, &headers, &id);
    }
    if !settings.enabled
        || !settings.capabilities.submit
        || action.revision != settings.revision as i64
    {
        return Err("AI操作配置已变化，请重新生成提交预览".into());
    }
    if action.expired {
        return Err("提交预览已超过10分钟，请重新生成并核对".into());
    }
    let claimed = {
        let conn = pool.get().map_err(|_| "操作数据库不可用")?;
        let tx = conn.unchecked_transaction().map_err(|_| "无法确认提交")?;
        let claimed=tx.execute("UPDATE ai_actions SET status='processing',worker_id=?3,updated_at=CURRENT_TIMESTAMP WHERE id=?1 AND user_id=?2 AND status='pending' AND expires_at>CURRENT_TIMESTAMP AND config_revision=?4",postgres_compat::params![id,user.user.id,WORKER_ID.as_str(),settings.revision as i64]).map_err(|_| "无法认领提交批次")?;
        if claimed == 1 {
            audit_repo::log_actor_on_conn(
                &tx,
                "ai_action_confirm",
                "ai_actions",
                None,
                user.user.id,
                &user.user.username,
                &format!(
                    "用户点击确认AI辅助提交，批次{id}，{}条",
                    action.payload.len()
                ),
                "own",
            )
            .map_err(|_| "无法记录确认审计")?;
        }
        tx.commit()
            .map_err(|_| "确认状态保存结果未知，请刷新核对，勿重新生成重复记录")?;
        claimed
    };
    if claimed == 0 {
        return get_action(&cfg, &pool, &headers, &id);
    }
    // The job survives a disconnected browser; interrupted jobs are never replayed.
    let task_cfg = cfg.clone();
    let task_pool = pool.clone();
    let task_headers = headers.clone();
    let task_id = id.clone();
    let result =
        tokio::spawn(
            async move { run_action(&task_cfg, &task_pool, &task_headers, &task_id).await },
        )
        .await;
    if !matches!(result, Ok(Ok(()))) {
        if let Ok(conn) = pool.get() {
            let _=conn.execute("UPDATE ai_actions SET worker_id='' WHERE id=?1 AND user_id=?2 AND status='processing'",postgres_compat::params![id,user.user.id]);
        }
    }
    get_action(&cfg, &pool, &headers, &id)
}
async fn run_action(
    cfg: &AppConfig,
    pool: &DbPool,
    headers: &HeaderMap,
    id: &str,
) -> ActionResult<()> {
    let first = authenticated(pool, headers)?;
    let mut action = load(pool, id, first.user.id)?;
    let mut unknown = false;
    for (index, item) in action.payload.iter().enumerate() {
        let progress = load(pool, id, first.user.id)?;
        if progress.status != "processing" || progress.interrupted {
            return Err("批次执行状态已变化，已停止后续提交".into());
        }
        let current = ai_service::settings(cfg)?;
        let authenticated = authenticated(pool, headers);
        let result = if unknown {
            Err("前项结果未知，后续记录未提交；请先核对原记录".into())
        } else if !current.enabled
            || !current.capabilities.submit
            || current.revision as i64 != action.revision
        {
            Err("AI配置已变化，本条未提交".into())
        } else if progress.expired {
            Err("确认批次已过期，本条未提交".into())
        } else if let Err(e) = authenticated {
            Err(format!("账号授权已变化，本条未提交：{e}"))
        } else {
            match tokio::time::timeout(
                Duration::from_secs(30),
                ai_submission_service::execute_submission(pool, headers, item),
            )
            .await
            {
                Ok(v) => v,
                Err(_) => Err("UNKNOWN:提交处理超时，结果未知，请核对原记录".into()),
            }
        };
        let value = match result {
            Ok(v) => {
                json!({"index":index,"status":"succeeded","record_id":v.get("record_id"),"route":v.get("route"),"message":v["message"].as_str().unwrap_or("提交成功")})
            }
            Err(e) => {
                let is_unknown = e.starts_with("UNKNOWN:");
                unknown |= is_unknown;
                json!({"index":index,"status":if is_unknown{"unknown"}else{"failed"},"message":e.trim_start_matches("UNKNOWN:")})
            }
        };
        action.results.push(value);
        let conn = pool.get().map_err(|_| "无法保存提交结果")?;
        let tx = conn
            .unchecked_transaction()
            .map_err(|_| "无法记录提交结果")?;
        let changed=tx.execute("UPDATE ai_actions SET results_json=?3,updated_at=CURRENT_TIMESTAMP WHERE id=?1 AND user_id=?2 AND status='processing' AND worker_id=?4",postgres_compat::params![id,first.user.id,serde_json::to_string(&action.results).map_err(|_| "无法序列化提交结果")?,WORKER_ID.as_str()]).map_err(|_| "提交后结果保存失败，需人工核对")?;
        if changed != 1 {
            return Err("提交后批次状态变化，结果需人工核对，已停止后续提交".into());
        }
        audit_repo::log_actor_on_conn(
            &tx,
            "ai_action_result",
            "ai_actions",
            action.results[index]["record_id"].as_i64(),
            first.user.id,
            &first.user.username,
            &format!(
                "AI辅助提交，批次{id}，第{}条，类型{}，结果{}",
                index + 1,
                item.target,
                action.results[index]["status"]
                    .as_str()
                    .unwrap_or("unknown")
            ),
            "own",
        )
        .map_err(|_| "提交后审计保存失败，需人工核对")?;
        tx.commit()
            .map_err(|_| "提交结果保存状态未知，需人工核对")?;
    }
    pool.get().map_err(|_| "操作数据库不可用")?.execute("UPDATE ai_actions SET status=?3,updated_at=CURRENT_TIMESTAMP WHERE id=?1 AND user_id=?2 AND status='processing' AND worker_id=?4",postgres_compat::params![id,first.user.id,if unknown{"unknown"}else{"completed"},WORKER_ID.as_str()]).map_err(|_| "无法保存批次完成状态")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn operation_permissions_are_default_off_and_independent() {
        let caps: AiCapabilities = serde_json::from_value(json!({"draft":true})).unwrap();
        assert!(!caps.submit && !caps.notification_read);
        let mut context = AiChatContext {
            include_business: Some(true),
            ..Default::default()
        };
        let mut caps = caps;
        caps.submit = true;
        caps.notification_read = true;
        assert!(require_consent(&context, &caps, true).is_err());
        assert!(require_consent(&context, &caps, false).is_err());
        context.allow_notification_read = Some(true);
        assert!(require_consent(&context, &caps, false).is_ok());
        assert!(require_consent(&context, &caps, true).is_err());
        context.allow_submit = Some(true);
        context.include_business = Some(false);
        assert!(require_consent(&context, &caps, true).is_err());
    }
    #[test]
    fn action_claim_is_single_use_account_bound_and_expires() {
        let pool = crate::db::init_pool("ai-action-claim-tests");
        let conn = pool.get().unwrap();
        crate::db::test_migrations::run(&conn).unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO ai_actions(id,user_id,config_revision,payload_json) VALUES (?1,1,7,'[]')",
            [&id],
        )
        .unwrap();
        let claim = |uid: i64, revision: i64| {
            conn.execute("UPDATE ai_actions SET status='processing' WHERE id=?1 AND user_id=?2 AND config_revision=?3 AND status='pending' AND expires_at>CURRENT_TIMESTAMP",postgres_compat::params![id,uid,revision]).unwrap()
        };
        assert_eq!(claim(2, 7), 0);
        assert_eq!(claim(1, 8), 0);
        assert_eq!(claim(1, 7), 1);
        assert_eq!(claim(1, 7), 0);
        conn.execute("UPDATE ai_actions SET status='pending',expires_at=CURRENT_TIMESTAMP-INTERVAL '1 second' WHERE id=?1",[&id]).unwrap();
        assert_eq!(claim(1, 7), 0);
        conn.execute(
            "UPDATE ai_actions SET status='processing',worker_id='previous-process' WHERE id=?1",
            [&id],
        )
        .unwrap();
        assert!(load(&pool, &id, 1).unwrap().interrupted);
        assert!(load(&pool, &id, 2).is_err());
    }
    #[test]
    fn public_preview_omits_private_execution_payload_and_disables_stale_confirmation() {
        let payload:PreparedSubmission=serde_json::from_value(json!({"target":"workload","path":"/private/path","body":{"hidden":"body-value"},"title":"工作量","preview":[{"label":"数量","value":"2"}],"issues":[],"canonical_fields":{"hidden":"intent-value"}})).unwrap();
        let mut action = StoredAction {
            id: uuid::Uuid::new_v4().to_string(),
            revision: 7,
            status: "pending".into(),
            payload: vec![payload],
            results: vec![],
            expires_at: "2026-11-14T00:00:00Z".into(),
            expired: false,
            interrupted: false,
        };
        let mut settings = ai_service::Settings::default();
        settings.enabled = true;
        settings.revision = 7;
        settings.capabilities.submit = true;
        let view = public(&action, &settings);
        assert_eq!(view["can_confirm"], true);
        for private in [
            "/private/path",
            "body-value",
            "intent-value",
            "canonical_fields",
        ] {
            assert!(!view.to_string().contains(private));
        }
        settings.revision = 8;
        assert_eq!(public(&action, &settings)["can_confirm"], false);
        settings.revision = 7;
        action.expired = true;
        assert_eq!(public(&action, &settings)["can_confirm"], false);
        action.expired = false;
        action.status = "completed".into();
        assert_eq!(public(&action, &settings)["can_confirm"], false);
    }
}
