use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::notification::{
    InAppNotification, NotificationChannel, NotificationChannelInput, NotificationDelivery,
    NotificationRule, NotificationRuleInput, NotificationSummary,
};
use serde_json::json;

fn now_sql() -> &'static str {
    "to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS')"
}

fn mask_webhook(url: String) -> String {
    if url.len() <= 18 {
        return "已配置".to_string();
    }
    format!("{}...{}", &url[..12], &url[url.len() - 6..])
}

pub fn list_channels(pool: &DbPool) -> Result<Vec<NotificationChannel>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare("SELECT id,name,channel_type,webhook_url,secret,is_active,created_at,updated_at FROM notification_channels ORDER BY id DESC")?;
    let rows = stmt.query_map([], |r| {
        Ok(NotificationChannel {
            id: r.get(0)?,
            name: r.get(1)?,
            channel_type: r.get(2)?,
            webhook_url_masked: mask_webhook(r.get(3)?),
            has_secret: !r.get::<_, String>(4)?.is_empty(),
            is_active: r.get::<_, i64>(5)? != 0,
            created_at: r.get(6)?,
            updated_at: r.get(7)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

pub fn get_channel_secret(pool: &DbPool, id: i64) -> Result<(String, String, String)> {
    let conn = pool.get()?;
    conn.query_row(
        "SELECT name,webhook_url,secret FROM notification_channels WHERE id=?1 AND is_active=1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .map_err(|e| match e {
        postgres_compat::Error::QueryReturnedNoRows => {
            AppError::NotFound("通知渠道不存在或已停用".into())
        }
        _ => e.into(),
    })
}

pub fn create_channel(pool: &DbPool, input: &NotificationChannelInput) -> Result<i64> {
    let conn = pool.get()?;
    conn.execute(&format!("INSERT INTO notification_channels(name,channel_type,webhook_url,secret,is_active,created_at,updated_at) VALUES(?1,'dingtalk_robot',?2,?3,?4,{0},{0})", now_sql()), postgres_compat::params![input.name.trim(), input.webhook_url.trim(), input.secret.clone().unwrap_or_default().trim(), if input.is_active.unwrap_or(true) {1} else {0}])?;
    Ok(conn.last_insert_rowid())
}

pub fn update_channel(pool: &DbPool, id: i64, input: &NotificationChannelInput) -> Result<()> {
    let conn = pool.get()?;
    let secret = input.secret.clone().unwrap_or_default();
    let changed = conn.execute(&format!("UPDATE notification_channels SET name=?1,webhook_url=?2,secret=CASE WHEN ?3='' THEN secret ELSE ?3 END,is_active=?4,updated_at={} WHERE id=?5", now_sql()), postgres_compat::params![input.name.trim(), input.webhook_url.trim(), secret.trim(), if input.is_active.unwrap_or(true) {1} else {0}, id])?;
    if changed == 0 {
        return Err(AppError::NotFound("通知渠道不存在".into()));
    }
    Ok(())
}

fn rule_from_row(r: &postgres_compat::Row<'_>) -> postgres_compat::Result<NotificationRule> {
    let recipients: String = r.get(9)?;
    Ok(NotificationRule {
        id: r.get(0)?,
        name: r.get(1)?,
        event_type: r.get(2)?,
        group_id: r.get(3)?,
        group_name: r.get(4)?,
        project_name: r.get(5)?,
        detection_type: r.get(6)?,
        channel_id: r.get(7)?,
        channel_name: r.get(8)?,
        recipient_user_ids: serde_json::from_str(&recipients).unwrap_or_default(),
        is_default: r.get::<_, i64>(10)? != 0,
        is_active: r.get::<_, i64>(11)? != 0,
        created_at: r.get(12)?,
    })
}

pub fn list_rules(pool: &DbPool) -> Result<Vec<NotificationRule>> {
    let conn = pool.get()?;
    let sql = "SELECT r.id,r.name,r.event_type,r.group_id,g.name,r.project_name,r.detection_type,r.channel_id,c.name,r.recipient_user_ids,r.is_default,r.is_active,r.created_at FROM notification_rules r LEFT JOIN project_groups g ON g.id=r.group_id LEFT JOIN notification_channels c ON c.id=r.channel_id ORDER BY r.is_default ASC,r.id DESC";
    let mut stmt = conn.prepare(sql)?;
    Ok(stmt
        .query_map([], rule_from_row)?
        .collect::<std::result::Result<Vec<_>, _>>()?)
}

pub fn create_rule(pool: &DbPool, input: &NotificationRuleInput) -> Result<i64> {
    let conn = pool.get()?;
    let recipient_json = json!(input.recipient_user_ids).to_string();
    conn.execute(&format!("INSERT INTO notification_rules(name,event_type,group_id,project_name,detection_type,channel_id,recipient_user_ids,is_default,is_active,created_at) VALUES(?1,'sample_submitted',?2,?3,?4,?5,?6,?7,?8,{})", now_sql()), postgres_compat::params![input.name.trim(), input.group_id, input.project_name.clone().unwrap_or_default().trim(), input.detection_type.clone().unwrap_or_default().trim(), input.channel_id, recipient_json, if input.is_default.unwrap_or(false) {1} else {0}, if input.is_active.unwrap_or(true) {1} else {0}])?;
    Ok(conn.last_insert_rowid())
}

pub fn update_rule(pool: &DbPool, id: i64, input: &NotificationRuleInput) -> Result<()> {
    let conn = pool.get()?;
    let changed = conn.execute("UPDATE notification_rules SET name=?1,group_id=?2,project_name=?3,detection_type=?4,channel_id=?5,recipient_user_ids=?6,is_default=?7,is_active=?8 WHERE id=?9", postgres_compat::params![input.name.trim(), input.group_id, input.project_name.clone().unwrap_or_default().trim(), input.detection_type.clone().unwrap_or_default().trim(), input.channel_id, json!(input.recipient_user_ids).to_string(), if input.is_default.unwrap_or(false) {1} else {0}, if input.is_active.unwrap_or(true) {1} else {0}, id])?;
    if changed == 0 {
        return Err(AppError::NotFound("通知规则不存在".into()));
    }
    Ok(())
}

pub fn enqueue_event(
    pool: &DbPool,
    resource_type: &str,
    record_id: i64,
    payload: &serde_json::Value,
) -> Result<()> {
    let conn = pool.get()?;
    let key = format!(
        "{}:{}:{}",
        crate::service::notification_service::EVENT_SAMPLE_SUBMITTED,
        resource_type,
        record_id
    );
    conn.execute(&format!("INSERT OR IGNORE INTO notification_events(event_type,resource_type,resource_id,dedupe_key,payload_json,status,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,'pending',{0},{0})", now_sql()), postgres_compat::params![crate::service::notification_service::EVENT_SAMPLE_SUBMITTED, resource_type, record_id,key,payload.to_string()])?;
    Ok(())
}

pub fn pending_events(
    pool: &DbPool,
    limit: i64,
) -> Result<Vec<(i64, String, i64, serde_json::Value)>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare("SELECT id,resource_type,resource_id,payload_json FROM notification_events WHERE status IN ('pending','retry') AND (next_attempt_at IS NULL OR next_attempt_at<=to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS')) ORDER BY id LIMIT ?1")?;
    let rows = stmt.query_map([limit], |r| {
        let s: String = r.get(3)?;
        Ok((
            r.get(0)?,
            r.get(1)?,
            r.get(2)?,
            serde_json::from_str(&s).unwrap_or_else(|_| json!({})),
        ))
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

pub fn matching_rules(
    pool: &DbPool,
    group_id: Option<i64>,
    project_name: &str,
    detection_type: &str,
    instrument_type: &str,
) -> Result<Vec<NotificationRule>> {
    let conn = pool.get()?;
    let sql = "SELECT r.id,r.name,r.event_type,r.group_id,g.name,r.project_name,r.detection_type,r.channel_id,c.name,r.recipient_user_ids,r.is_default,r.is_active,r.created_at FROM notification_rules r LEFT JOIN project_groups g ON g.id=r.group_id LEFT JOIN notification_channels c ON c.id=r.channel_id WHERE r.event_type='sample_submitted' AND r.is_active=1 AND (r.is_default=1 OR ((r.group_id IS NULL OR r.group_id=?1) AND (r.project_name='' OR r.project_name=?2) AND (r.detection_type='' OR r.detection_type=?3 OR r.detection_type=?4))) ORDER BY r.is_default ASC,r.id";
    let mut stmt = conn.prepare(sql)?;
    Ok(stmt
        .query_map(
            postgres_compat::params![group_id, project_name, detection_type, instrument_type],
            rule_from_row,
        )?
        .collect::<std::result::Result<Vec<_>, _>>()?)
}

pub fn create_in_app(
    pool: &DbPool,
    event_id: i64,
    users: &[i64],
    title: &str,
    body: &str,
    target: &str,
) -> Result<()> {
    let conn = pool.get()?;
    for user_id in users {
        conn.execute(&format!("INSERT OR IGNORE INTO in_app_notifications(event_id,user_id,title,body,target_url,is_read,created_at) VALUES(?1,?2,?3,?4,?5,0,{})", now_sql()), postgres_compat::params![event_id,user_id,title,body,target])?;
    }
    Ok(())
}

pub fn analysis_user_ids(pool: &DbPool) -> Result<Vec<i64>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare("SELECT DISTINCT u.id FROM users u JOIN user_roles ur ON ur.user_id=u.id JOIN roles r ON r.id=ur.role_id WHERE u.is_active=1 AND u.deleted_at IS NULL AND r.deleted_at IS NULL AND r.name IN ('分析检测员','分析检测组长')")?;
    Ok(stmt
        .query_map([], |r| r.get(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?)
}

pub fn analysis_user_ids_for(pool: &DbPool, candidates: &[i64]) -> Result<Vec<i64>> {
    let conn = pool.get()?;
    let mut valid = Vec::new();
    for user_id in candidates {
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM users u JOIN user_roles ur ON ur.user_id=u.id JOIN roles r ON r.id=ur.role_id WHERE u.id=?1 AND u.is_active=1 AND u.deleted_at IS NULL AND r.name IN ('分析检测员','分析检测组长'))",
            [user_id], |r| r.get(0),
        )?;
        if exists {
            valid.push(*user_id);
        }
    }
    Ok(valid)
}

pub fn record_delivery(
    pool: &DbPool,
    event_id: i64,
    channel_id: i64,
    status: &str,
    attempts: i64,
    response: &str,
    error: &str,
    retry: bool,
) -> Result<()> {
    let conn = pool.get()?;
    let next = if retry {
        Some(
            (chrono::Local::now() + chrono::Duration::minutes(if attempts < 2 { 1 } else { 5 }))
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
        )
    } else {
        None
    };
    conn.execute(&format!("INSERT INTO notification_deliveries(event_id,channel_id,status,attempts,response_summary,last_error,next_attempt_at,sent_at,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,CASE WHEN ?3='sent' THEN {0} ELSE NULL END,{0},{0}) ON CONFLICT(event_id,channel_id) DO UPDATE SET status=excluded.status,attempts=excluded.attempts,response_summary=excluded.response_summary,last_error=excluded.last_error,next_attempt_at=excluded.next_attempt_at,sent_at=excluded.sent_at,updated_at={0}", now_sql()), postgres_compat::params![event_id,channel_id,status,attempts,response,error,next])?;
    Ok(())
}

pub fn delivery_attempts(pool: &DbPool, event_id: i64, channel_id: i64) -> Result<i64> {
    let conn = pool.get()?;
    Ok(conn.query_row("SELECT COALESCE(attempts,0) FROM notification_deliveries WHERE event_id=?1 AND channel_id=?2", postgres_compat::params![event_id,channel_id], |r| r.get(0)).unwrap_or(0))
}

pub fn set_event_status(pool: &DbPool, event_id: i64, status: &str, retry: bool) -> Result<()> {
    let conn = pool.get()?;
    let next = if retry {
        Some(
            (chrono::Local::now() + chrono::Duration::minutes(1))
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
        )
    } else {
        None
    };
    conn.execute(
        &format!(
            "UPDATE notification_events SET status=?1,next_attempt_at=?2,updated_at={} WHERE id=?3",
            now_sql()
        ),
        postgres_compat::params![status, next, event_id],
    )?;
    Ok(())
}

pub fn deliveries(pool: &DbPool, limit: i64) -> Result<Vec<NotificationDelivery>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare("SELECT d.id,d.event_id,COALESCE(c.name,'已删除渠道'),d.status,d.attempts,d.response_summary,d.last_error,d.sent_at,d.created_at FROM notification_deliveries d LEFT JOIN notification_channels c ON c.id=d.channel_id ORDER BY d.id DESC LIMIT ?1")?;
    let rows = stmt.query_map([limit], |r| {
        Ok(NotificationDelivery {
            id: r.get(0)?,
            event_id: r.get(1)?,
            channel_name: r.get(2)?,
            status: r.get(3)?,
            attempts: r.get(4)?,
            response_summary: r.get(5)?,
            last_error: r.get(6)?,
            sent_at: r.get(7)?,
            created_at: r.get(8)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

pub fn summary(pool: &DbPool) -> Result<NotificationSummary> {
    let conn = pool.get()?;
    let count = |condition: &str| -> Result<i64> {
        Ok(conn.query_row(
            &format!(
                "SELECT COUNT(*) FROM notification_events WHERE {}",
                condition
            ),
            [],
            |r| r.get(0),
        )?)
    };
    Ok(NotificationSummary {
        pending_count: count("status IN ('pending','retry')")?,
        failed_count: count("status='failed'")?,
        sent_today_count: count(
            "status='sent' AND LEFT(updated_at,10)=to_char(CURRENT_TIMESTAMP,'YYYY-MM-DD')",
        )?,
        skipped_count: count("status='skipped'")?,
    })
}

pub fn inbox(pool: &DbPool, user_id: i64) -> Result<Vec<InAppNotification>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare("SELECT id,title,body,target_url,is_read,created_at FROM in_app_notifications WHERE user_id=?1 ORDER BY id DESC LIMIT 100")?;
    Ok(stmt
        .query_map([user_id], |r| {
            Ok(InAppNotification {
                id: r.get(0)?,
                title: r.get(1)?,
                body: r.get(2)?,
                target_url: r.get(3)?,
                is_read: r.get::<_, i64>(4)? != 0,
                created_at: r.get(5)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?)
}

pub fn mark_inbox_read(pool: &DbPool, id: i64, user_id: i64) -> Result<()> {
    let conn = pool.get()?;
    let changed = conn.execute(
        "UPDATE in_app_notifications SET is_read=1 WHERE id=?1 AND user_id=?2",
        postgres_compat::params![id, user_id],
    )?;
    if changed == 0 {
        return Err(AppError::NotFound("通知不存在或无权操作".into()));
    }
    Ok(())
}
