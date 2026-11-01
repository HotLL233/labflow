use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::notification::{DingTalkContact, NotificationRuleInput};
use std::collections::BTreeSet;

pub fn list_contacts(pool: &DbPool) -> Result<Vec<DingTalkContact>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare("SELECT u.id,u.username,u.is_active,COALESCE(c.mobile,'') FROM users u LEFT JOIN notification_dingtalk_contacts c ON c.user_id=u.id WHERE u.deleted_at IS NULL ORDER BY u.id")?;
    let contacts = stmt
        .query_map([], |row| {
            Ok(DingTalkContact {
                user_id: row.get(0)?,
                username: row.get(1)?,
                is_active: row.get::<_, i64>(2)? != 0,
                mobile: row.get(3)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(contacts)
}

fn normalize_mobile(value: &str) -> Result<String> {
    let value = value.trim();
    let digits = value.strip_prefix('+').unwrap_or(value);
    if !value.is_empty()
        && (!(7..=15).contains(&digits.len()) || !digits.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return Err(AppError::Validation(
            "请输入钉钉绑定手机号：7至15位数字，可带开头的+，留空解除绑定".into(),
        ));
    }
    Ok(value.to_string())
}

pub fn save_contact(
    pool: &DbPool,
    user_id: i64,
    value: &str,
    actor_id: i64,
    actor_name: &str,
) -> Result<()> {
    let mobile = normalize_mobile(value)?;
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let active = tx
        .query_row(
            "SELECT is_active FROM users WHERE id=?1 AND deleted_at IS NULL FOR UPDATE",
            [user_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| match error {
            postgres_compat::Error::QueryReturnedNoRows => AppError::NotFound("账号不存在".into()),
            other => other.into(),
        })?;
    if !mobile.is_empty() && active == 0 {
        return Err(AppError::Validation("停用账号不能绑定钉钉手机号".into()));
    }
    if mobile.is_empty() {
        tx.execute(
            "DELETE FROM notification_dingtalk_contacts WHERE user_id=?1",
            [user_id],
        )?;
    } else {
        tx.execute("INSERT INTO notification_dingtalk_contacts(user_id,mobile) VALUES(?1,?2) ON CONFLICT(user_id) DO UPDATE SET mobile=EXCLUDED.mobile,updated_at=to_char(CURRENT_TIMESTAMP,'YYYY-MM-DD HH24:MI:SS')", postgres_compat::params![user_id, mobile])?;
    }
    crate::repo::audit_repo::log_actor_on_conn(
        &tx,
        "update",
        "notification_dingtalk_contacts",
        Some(user_id),
        actor_id,
        actor_name,
        if mobile.is_empty() {
            "解除钉钉手机号绑定"
        } else {
            "更新钉钉手机号绑定"
        },
        "notifications",
    )?;
    tx.commit()?;
    Ok(())
}

pub fn validate_rule(
    conn: &postgres_compat::Connection,
    input: &NotificationRuleInput,
) -> Result<()> {
    if input.dingtalk_mention_user_ids.len() > 200 {
        return Err(AppError::Validation(
            "一条规则最多选择200个钉钉提醒账号".into(),
        ));
    }
    if input.dingtalk_follow_recipients && !input.dingtalk_mention_user_ids.is_empty() {
        return Err(AppError::Validation(
            "指定钉钉人员与跟随程序内接收人只能选择一种".into(),
        ));
    }
    if (input.dingtalk_follow_recipients || !input.dingtalk_mention_user_ids.is_empty())
        && input.channel_id.is_none()
    {
        return Err(AppError::Validation(
            "配置钉钉@人员前请选择群通知渠道".into(),
        ));
    }
    if input.dingtalk_follow_recipients
        && input
            .targets
            .iter()
            .any(|target| target.target_kind == "rd_work_record_rejected")
    {
        return Err(AppError::Validation(
            "研发送样驳回不发送站内通知，请指定钉钉@人员".into(),
        ));
    }
    for user_id in input
        .dingtalk_mention_user_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
    {
        let available: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM users u JOIN notification_dingtalk_contacts c ON c.user_id=u.id WHERE u.id=?1 AND u.is_active=1 AND u.deleted_at IS NULL AND c.mobile<>'')", [user_id], |row| row.get(0))?;
        if !available {
            return Err(AppError::Validation(format!(
                "钉钉提醒账号{}未绑定手机号、已停用或不存在，请先维护绑定",
                user_id
            )));
        }
    }
    Ok(())
}

pub fn resolve(pool: &DbPool, ids: &BTreeSet<i64>) -> Result<(Vec<String>, Vec<String>)> {
    if ids.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let contacts = list_contacts(pool)?;
    let mut mobiles = BTreeSet::new();
    let mut missing = Vec::new();
    for user_id in ids {
        match contacts.iter().find(|contact| contact.user_id == *user_id) {
            Some(contact) if contact.is_active && !contact.mobile.is_empty() => {
                mobiles.insert(contact.mobile.clone());
            }
            Some(contact) => missing.push(format!("{}（未绑定或已停用）", contact.username)),
            None => missing.push(format!("账号{}（不存在）", user_id)),
        }
    }
    Ok((mobiles.into_iter().collect(), missing))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dingtalk_bindings_rules_and_migration_preserve_legacy_behavior() {
        let pool = crate::db::init_pool("dingtalk-bindings-tests");
        let conn = pool.get().unwrap();
        crate::db::test_migrations::run(&conn).unwrap();
        let actor: i64 = conn
            .query_row("SELECT id FROM users WHERE username='admin'", [], |row| {
                row.get(0)
            })
            .unwrap();
        conn.execute("INSERT INTO users(username,password,is_admin,is_active) VALUES('dingtalk-person','unused',0,1)", []).unwrap();
        let user_id = conn.last_insert_rowid();
        let mut input: NotificationRuleInput =
            serde_json::from_value(serde_json::json!({"name":"旧版规则","recipient_user_ids":[],"targets":[{"target_kind":"rd_work_record","sample_info_type_key":""}]}))
                .unwrap();
        let legacy = crate::repo::notification_repo::create_rule(&pool, &input).unwrap();
        let legacy_rule = crate::repo::notification_repo::list_rules(&pool)
            .unwrap()
            .into_iter()
            .find(|rule| rule.id == legacy)
            .unwrap();
        assert!(legacy_rule.dingtalk_mention_user_ids.is_empty());
        assert!(!legacy_rule.dingtalk_follow_recipients);
        assert!(save_contact(&pool, user_id, "@all", actor, "admin").is_err());
        save_contact(&pool, user_id, "13800138000", actor, "admin").unwrap();
        let (phones, missing) = resolve(&pool, &[user_id, user_id].into_iter().collect()).unwrap();
        assert_eq!(phones, vec!["13800138000"]);
        assert!(missing.is_empty());
        let audit: String = conn.query_row("SELECT detail FROM audit_log WHERE table_name='notification_dingtalk_contacts' ORDER BY id DESC LIMIT 1", [], |row| row.get(0)).unwrap();
        assert!(!audit.contains("13800138000"));
        input.dingtalk_mention_user_ids = vec![user_id];
        assert!(crate::repo::notification_repo::create_rule(&pool, &input).is_err());
        input.channel_id = Some(
            crate::repo::notification_repo::create_channel(
                &pool,
                &crate::models::notification::NotificationChannelInput {
                    name: "测试群".into(),
                    webhook_url: Some("https://example.test/not-a-real-robot".into()),
                    secret: None,
                    is_active: Some(true),
                },
            )
            .unwrap(),
        );
        let configured = crate::repo::notification_repo::create_rule(&pool, &input).unwrap();
        assert_eq!(
            crate::repo::notification_repo::list_rules(&pool)
                .unwrap()
                .into_iter()
                .find(|rule| rule.id == configured)
                .unwrap()
                .dingtalk_mention_user_ids,
            vec![user_id]
        );
        input.dingtalk_follow_recipients = true;
        assert!(crate::repo::notification_repo::update_rule(&pool, configured, &input).is_err());
        input.dingtalk_mention_user_ids.clear();
        crate::repo::notification_repo::update_rule(&pool, configured, &input).unwrap();
        assert!(
            crate::repo::notification_repo::list_rules(&pool)
                .unwrap()
                .into_iter()
                .find(|rule| rule.id == configured)
                .unwrap()
                .dingtalk_follow_recipients
        );
        conn.execute(
            "DELETE FROM schema_migrations WHERE version='2.3.78-dingtalk-mentions'",
            [],
        )
        .unwrap();
        crate::db::test_migrations::run(&conn).unwrap();
        assert_eq!(
            resolve(&pool, &[user_id].into_iter().collect()).unwrap().0,
            vec!["13800138000"]
        );
        conn.execute("UPDATE users SET is_active=0 WHERE id=?1", [user_id])
            .unwrap();
        let (phones, missing) = resolve(&pool, &[user_id].into_iter().collect()).unwrap();
        assert!(phones.is_empty());
        assert_eq!(missing.len(), 1);
        assert!(save_contact(&pool, user_id, "13900139000", actor, "admin").is_err());
        save_contact(&pool, user_id, "", actor, "admin").unwrap();
        conn.execute(
            "UPDATE users SET is_active=1,deleted_at='2026-10-29' WHERE id=?1",
            [user_id],
        )
        .unwrap();
        assert!(save_contact(&pool, user_id, "13800138000", actor, "admin").is_err());
        assert!(!list_contacts(&pool)
            .unwrap()
            .iter()
            .any(|contact| contact.user_id == user_id));
    }

    #[test]
    fn mobile_input_rejects_invalid_mentions() {
        for value in ["13800138000", "+8613800138000", "", " 13800138000 "] {
            assert!(normalize_mobile(value).is_ok());
        }
        for value in [
            "@all",
            "13800138000,@all",
            "１３８００１３８０００",
            "123456",
            "+",
            "1234567890123456",
        ] {
            assert!(normalize_mobile(value).is_err());
        }
    }
}
