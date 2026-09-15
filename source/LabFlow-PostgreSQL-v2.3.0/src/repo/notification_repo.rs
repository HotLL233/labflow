use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::notification::{
    InAppNotification, NotificationChannel, NotificationChannelInput, NotificationDelivery,
    NotificationRule, NotificationRuleInput, NotificationRuleTarget, NotificationSummary,
};
use serde_json::json;
use std::fs;
use std::path::PathBuf;

pub fn get_template(pool: &DbPool, event_key: &str) -> Result<(String, String, serde_json::Value)> {
    let conn = pool.get()?;
    conn.query_row(
        "SELECT title,footer,field_config_json FROM notification_templates WHERE event_key=?1 AND is_active=1",
        [event_key],
        |row| {
            let config: String = row.get(2)?;
            Ok((row.get(0)?, row.get(1)?, serde_json::from_str(&config).unwrap_or_else(|_| json!([]))))
        },
    ).map_err(|e| match e { postgres_compat::Error::QueryReturnedNoRows => AppError::NotFound("通知模板不存在".into()), other => other.into() })
}

/// Core fields are always available. Configured form and sample-information fields are appended
/// so administrators can include custom fields without changing notification code.
pub fn template_available_fields(
    pool: &DbPool,
    type_key: Option<&str>,
    form_setting_keys: &[&str],
    template_key: &str,
) -> Result<Vec<serde_json::Value>> {
    let sample = template_key == "sample_info" || template_key.starts_with("sample_info:");
    let personnel = template_key == "personnel_change_feedback"
        || template_key == "personnel_change_feedback_rejected";
    let rejected = template_key == "rd_work_record_rejected";
    let mut fields = if personnel {
        // 人员反馈通知字段只来自 personnel-change-data/fields.json；不再维护第二套硬编码字段清单。
        Vec::new()
    } else {
        let mut base = vec![
            json!({"key":"batch_no","label":"批号"}),
            json!({"key":"business_no","label":"送样编号"}),
            json!({"key":"submitted_at","label":"送样时间"}),
            json!({"key":"submitted_by","label":"送样人"}),
            json!({"key":"division_name","label":"部门"}),
            json!({"key":"lab_name","label":"实验室"}),
            json!({"key":"project_name","label":"项目"}),
            json!({"key":"detection_type","label":"检测类型"}),
            json!({"key":"method_name","label":"方法"}),
        ];
        if !sample {
            base.push(json!({"key":"instrument_code","label":"仪器"}));
        }
        base.push(json!({"key":"quantity","label":"数量"}));
        if sample {
            base.extend([
                json!({"key":"main_components","label":"主要成分"}),
                json!({"key":"notes","label":"备注"}),
                json!({"key":"attachment_count","label":"附件数量"}),
            ]);
        } else {
            base.push(json!({"key":"notes","label":"备注"}));
        }
        base.push(json!({"key":"status","label":"状态"}));
        if rejected {
            base.extend([
                json!({"key":"returned_at","label":"驳回时间"}),
                json!({"key":"returned_by","label":"驳回人"}),
                json!({"key":"return_reason","label":"驳回原因"}),
            ]);
        }
        base
    };
    let conn = pool.get()?;
    if sample {
        let mut stmt = conn.prepare(
            "SELECT c.field_key,c.label FROM sample_info_columns c \
             LEFT JOIN sample_info_column_visibility v ON v.column_id=c.id AND v.type_key=?1 \
             WHERE c.is_active=1 AND c.is_predefined=0 AND c.deleted_at IS NULL \
               AND (?1='' OR (COALESCE(v.is_visible,0)=1 AND (COALESCE(v.show_in_form,0)=1 OR COALESCE(v.show_in_list,0)=1 OR COALESCE(v.show_in_export,0)=1))) \
             ORDER BY COALESCE(v.sort_order,c.sort_order),c.id",
        )?;
        for row in stmt.query_map([type_key.unwrap_or("")], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })? {
            let (key, label) = row?;
            if !fields.iter().any(|field| {
                field.get("key").and_then(|value| value.as_str()) == Some(key.as_str())
            }) {
                fields.push(json!({"key":key,"label":label,"sort_order":fields.len() as i64 + 1}));
            }
        }
    }

    for setting_key in form_setting_keys {
        let raw: String = match conn.query_row(
            "SELECT value FROM system_settings WHERE key=?1",
            [*setting_key],
            |row| row.get(0),
        ) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let Ok(config) = serde_json::from_str::<serde_json::Value>(&raw) else {
            continue;
        };
        let Some(items) = config.get("fields").and_then(|value| value.as_array()) else {
            continue;
        };
        for item in items {
            let Some(key) = item.get("key").and_then(|value| value.as_str()) else {
                continue;
            };
            if key.trim().is_empty()
                || item.get("visible").and_then(|value| value.as_bool()) == Some(false)
                || item.get("show_in_form").and_then(|value| value.as_bool()) == Some(false)
            {
                continue;
            }
            let label = item
                .get("label")
                .and_then(|value| value.as_str())
                .unwrap_or(key);
            if !fields
                .iter()
                .any(|field| field.get("key").and_then(|value| value.as_str()) == Some(key))
            {
                fields.push(
                    json!({"key": key, "label": label, "sort_order": fields.len() as i64 + 1}),
                );
            }
        }
    }

    if !sample && !personnel {
        if let Ok(mut stmt) = conn.prepare(
        "SELECT name,label FROM rd_record_columns WHERE is_active=1 AND show_in_form=1 ORDER BY sort_order,id",
        ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }) {
            for row in rows.flatten() {
                let (key, label) = row;
                if !fields
                    .iter()
                    .any(|field| field.get("key").and_then(|value| value.as_str()) == Some(key.as_str()))
                {
                    fields.push(json!({"key": key, "label": label, "sort_order": fields.len() as i64 + 1}));
                }
            }
        }
        }
    }

    let feedback_dir = std::env::var("PERSONNEL_CHANGE_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            crate::config::AppConfig::load()
                .data_dir()
                .join("personnel-change-data")
        });
    if personnel {
        if let Ok(text) = fs::read_to_string(feedback_dir.join("fields.json")) {
            if let Ok(items) = serde_json::from_str::<Vec<serde_json::Value>>(&text) {
                for item in items.into_iter().filter(|item| {
                    item.get("enabled")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true)
                        && item.get("notify").and_then(|v| v.as_bool()).unwrap_or(true)
                }) {
                    let Some(key) = item.get("key").and_then(|v| v.as_str()) else {
                        continue;
                    };
                    let label = item.get("label").and_then(|v| v.as_str()).unwrap_or(key);
                    let sort_order = item
                        .get("sort_order")
                        .and_then(|value| value.as_i64())
                        .unwrap_or(i64::MAX);
                    if !fields
                        .iter()
                        .any(|field| field.get("key").and_then(|v| v.as_str()) == Some(key))
                    {
                        fields.push(json!({"key": key, "label": label, "sort_order": sort_order}));
                    }
                }
            }
        }
    }
    if personnel {
        fields.sort_by_key(|field| {
            field
                .get("sort_order")
                .and_then(|value| value.as_i64())
                .unwrap_or(i64::MAX)
        });
    }
    Ok(fields)
}

pub fn save_template(
    pool: &DbPool,
    event_key: &str,
    title: &str,
    footer: &str,
    field_config: &serde_json::Value,
) -> Result<()> {
    let conn = pool.get()?;
    let mut stored_config = field_config.clone();
    if let Some(new_items) = field_config.as_array() {
        if let Ok((_, _, old_config)) = get_template(pool, event_key) {
            if let Some(old_items) = old_config.as_array() {
                let submitted_keys: std::collections::HashSet<&str> = new_items
                    .iter()
                    .filter_map(|item| item.get("key").and_then(|value| value.as_str()))
                    .collect();
                let mut merged = new_items.clone();
                merged.extend(
                    old_items
                        .iter()
                        .filter(|item| {
                            item.get("key")
                                .and_then(|value| value.as_str())
                                .map(|key| !submitted_keys.contains(key))
                                .unwrap_or(false)
                        })
                        .cloned(),
                );
                stored_config = serde_json::Value::Array(merged);
            }
        }
    }
    conn.execute(
        "INSERT INTO notification_templates(event_key,title,footer,field_config_json,is_active) VALUES(?1,?2,?3,?4,1)
         ON CONFLICT(event_key) DO UPDATE SET title=excluded.title,footer=excluded.footer,field_config_json=excluded.field_config_json,is_active=1,updated_at=to_char(CURRENT_TIMESTAMP,'YYYY-MM-DD HH24:MI:SS')",
        postgres_compat::params![event_key,title.trim(),footer,stored_config.to_string()],
    )?;
    Ok(())
}

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
    let webhook_url = input.webhook_url.as_deref().unwrap_or_default();
    conn.execute(&format!("INSERT INTO notification_channels(name,channel_type,webhook_url,secret,is_active,created_at,updated_at) VALUES(?1,'dingtalk_robot',?2,?3,?4,{0},{0})", now_sql()), postgres_compat::params![input.name.trim(), webhook_url.trim(), input.secret.clone().unwrap_or_default().trim(), if input.is_active.unwrap_or(true) {1} else {0}])?;
    Ok(conn.last_insert_rowid())
}

pub fn update_channel(pool: &DbPool, id: i64, input: &NotificationChannelInput) -> Result<()> {
    let conn = pool.get()?;
    let webhook_url = input.webhook_url.as_deref().unwrap_or_default();
    let secret = input.secret.clone().unwrap_or_default();
    let changed = conn.execute(&format!("UPDATE notification_channels SET name=?1,webhook_url=CASE WHEN ?2='' THEN webhook_url ELSE ?2 END,secret=CASE WHEN ?3='' THEN secret ELSE ?3 END,is_active=?4,updated_at={} WHERE id=?5", now_sql()), postgres_compat::params![input.name.trim(), webhook_url.trim(), secret.trim(), if input.is_active.unwrap_or(true) {1} else {0}, id])?;
    if changed == 0 {
        return Err(AppError::NotFound("通知渠道不存在".into()));
    }
    Ok(())
}

fn rule_from_row(r: &postgres_compat::Row<'_>) -> postgres_compat::Result<NotificationRule> {
    let recipients: String = r.get(10)?;
    Ok(NotificationRule {
        id: r.get(0)?,
        name: r.get(1)?,
        event_type: r.get(2)?,
        group_id: r.get(3)?,
        group_name: r.get(4)?,
        group_ids: vec![],
        group_names: vec![],
        project_name: r.get(5)?,
        detection_type: r.get(6)?,
        sample_info_type_key: r.get(7)?,
        targets: vec![],
        channel_id: r.get(8)?,
        channel_name: r.get(9)?,
        recipient_user_ids: serde_json::from_str(&recipients).unwrap_or_default(),
        is_default: r.get::<_, i64>(11)? != 0,
        is_active: r.get::<_, i64>(12)? != 0,
        created_at: r.get(13)?,
    })
}

fn normalize_targets(input: &NotificationRuleInput) -> Result<Vec<NotificationRuleTarget>> {
    let mut targets = input
        .targets
        .iter()
        .filter_map(|target| {
            let kind = target.target_kind.trim();
            if kind == "rd_work_record"
                || kind == "rd_work_record_rejected"
                || kind == "rd_work_record_resubmitted"
            {
                Some(NotificationRuleTarget {
                    target_kind: kind.into(),
                    sample_info_type_key: String::new(),
                })
            } else if kind == "sample_info" {
                Some(NotificationRuleTarget {
                    target_kind: kind.into(),
                    sample_info_type_key: target.sample_info_type_key.trim().to_string(),
                })
            } else if kind == "personnel_change_feedback" {
                Some(NotificationRuleTarget {
                    target_kind: kind.into(),
                    sample_info_type_key: String::new(),
                })
            } else if kind == "personnel_change_feedback_rejected" {
                Some(NotificationRuleTarget {
                    target_kind: kind.into(),
                    sample_info_type_key: String::new(),
                })
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    if targets.is_empty() && !input.is_default.unwrap_or(false) {
        return Err(AppError::Validation(
            "请至少选择一个通知来源或样品类型".into(),
        ));
    }
    if targets.is_empty() {
        targets = vec![
            NotificationRuleTarget {
                target_kind: "rd_work_record".into(),
                sample_info_type_key: String::new(),
            },
            NotificationRuleTarget {
                target_kind: "sample_info".into(),
                sample_info_type_key: String::new(),
            },
        ];
    }
    targets.sort_by(|a, b| {
        (a.target_kind.as_str(), a.sample_info_type_key.as_str())
            .cmp(&(b.target_kind.as_str(), b.sample_info_type_key.as_str()))
    });
    targets.dedup_by(|a, b| {
        a.target_kind == b.target_kind && a.sample_info_type_key == b.sample_info_type_key
    });
    Ok(targets)
}

fn rule_event_type(targets: &[NotificationRuleTarget]) -> Result<&'static str> {
    if targets.iter().any(|target| {
        target.target_kind == "personnel_change_feedback"
            || target.target_kind == "personnel_change_feedback_rejected"
    }) {
        if targets.len() != 1 {
            return Err(AppError::Validation(
                "人员变动反馈通知规则不能与其他通知来源混合配置".into(),
            ));
        }
        return Ok(
            if targets[0].target_kind == "personnel_change_feedback_rejected" {
                crate::service::notification_service::EVENT_PERSONNEL_CHANGE_REJECTED
            } else {
                crate::service::notification_service::EVENT_PERSONNEL_CHANGE
            },
        );
    }
    let has_rejected = targets
        .iter()
        .any(|target| target.target_kind == "rd_work_record_rejected");
    let has_resubmitted = targets
        .iter()
        .any(|target| target.target_kind == "rd_work_record_resubmitted");
    if has_rejected && has_resubmitted {
        return Err(AppError::Validation(
            "研发送样驳回和退回后重新提交必须分别配置通知规则".into(),
        ));
    }
    if has_rejected {
        if targets.len() != 1 {
            return Err(AppError::Validation(
                "研发送样驳回通知规则不能与其他通知来源混合配置".into(),
            ));
        }
        return Ok(crate::service::notification_service::EVENT_RD_REJECTED);
    }
    if has_resubmitted {
        if targets.len() != 1 {
            return Err(AppError::Validation(
                "研发送样退回后重新提交通知规则不能与其他通知来源混合配置".into(),
            ));
        }
        return Ok(crate::service::notification_service::EVENT_RD_RESUBMITTED);
    }
    Ok(crate::service::notification_service::EVENT_SAMPLE_SUBMITTED)
}

fn validate_rule_delivery(input: &NotificationRuleInput, event_type: &str) -> Result<()> {
    if event_type == crate::service::notification_service::EVENT_RD_REJECTED {
        if input.channel_id.is_none() {
            return Err(AppError::Validation(
                "研发送样驳回通知必须选择钉钉群通知渠道".into(),
            ));
        }
        if !input.recipient_user_ids.is_empty() {
            return Err(AppError::Validation(
                "研发送样驳回通知仅支持钉钉群通知，不支持个人通知".into(),
            ));
        }
    }
    Ok(())
}

fn hydrate_rule_targets(
    conn: &postgres_compat::Connection,
    rule: &mut NotificationRule,
) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT target_kind,sample_info_type_key FROM notification_rule_targets WHERE rule_id=?1 ORDER BY target_kind,sample_info_type_key",
    )?;
    rule.targets = stmt
        .query_map([rule.id], |row| {
            Ok(NotificationRuleTarget {
                target_kind: row.get(0)?,
                sample_info_type_key: row.get(1)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(())
}

fn hydrate_rule_groups(
    conn: &postgres_compat::Connection,
    rule: &mut NotificationRule,
) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT rg.group_id,g.name FROM notification_rule_groups rg \
         JOIN project_groups g ON g.id=rg.group_id \
         WHERE rg.rule_id=?1 ORDER BY g.sort_order,g.id",
    )?;
    let rows = stmt.query_map([rule.id], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (id, name) = row?;
        rule.group_ids.push(id);
        rule.group_names.push(name);
    }
    // Existing rules are backfilled by migration. This fallback keeps a rule readable if a
    // partially upgraded database is inspected before the migration has run.
    if rule.group_ids.is_empty() {
        if let Some(id) = rule.group_id {
            rule.group_ids.push(id);
        }
        if let Some(name) = rule.group_name.clone() {
            rule.group_names.push(name);
        }
    }
    Ok(())
}

fn normalize_group_ids(input: &NotificationRuleInput) -> Vec<i64> {
    let mut ids = input.group_ids.clone();
    if ids.is_empty() {
        if let Some(group_id) = input.group_id {
            ids.push(group_id);
        }
    }
    ids.sort_unstable();
    ids.dedup();
    ids
}

fn validate_group_ids(conn: &postgres_compat::Connection, ids: &[i64]) -> Result<()> {
    for group_id in ids {
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM project_groups WHERE id=?1 AND deleted_at IS NULL)",
            [group_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(AppError::Validation(format!(
                "所属实验室不存在或已删除: {}",
                group_id
            )));
        }
    }
    Ok(())
}

pub fn list_rules(pool: &DbPool) -> Result<Vec<NotificationRule>> {
    let conn = pool.get()?;
    let sql = "SELECT r.id,r.name,r.event_type,r.group_id,g.name,r.project_name,r.detection_type,r.sample_info_type_key,r.channel_id,c.name,r.recipient_user_ids,r.is_default,r.is_active,r.created_at FROM notification_rules r LEFT JOIN project_groups g ON g.id=r.group_id LEFT JOIN notification_channels c ON c.id=r.channel_id ORDER BY r.is_default ASC,r.id DESC";
    let mut stmt = conn.prepare(sql)?;
    let mut rules = stmt
        .query_map([], rule_from_row)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for rule in &mut rules {
        hydrate_rule_groups(&conn, rule)?;
        hydrate_rule_targets(&conn, rule)?;
    }
    Ok(rules)
}

pub fn create_rule(pool: &DbPool, input: &NotificationRuleInput) -> Result<i64> {
    let mut conn = pool.get()?;
    let group_ids = normalize_group_ids(input);
    let targets = normalize_targets(input)?;
    let event_type = rule_event_type(&targets)?;
    validate_rule_delivery(input, event_type)?;
    validate_group_ids(&conn, &group_ids)?;
    let recipient_json = json!(input.recipient_user_ids).to_string();
    let tx = conn.transaction()?;
    tx.execute(&format!("INSERT INTO notification_rules(name,event_type,group_id,project_name,detection_type,sample_info_type_key,channel_id,recipient_user_ids,is_default,is_active,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,{})", now_sql()), postgres_compat::params![input.name.trim(), event_type, group_ids.first().copied(), input.project_name.clone().unwrap_or_default().trim(), input.detection_type.clone().unwrap_or_default().trim(), input.sample_info_type_key.clone().unwrap_or_default().trim(), input.channel_id, recipient_json, if input.is_default.unwrap_or(false) {1} else {0}, if input.is_active.unwrap_or(true) {1} else {0}])?;
    let id = tx.last_insert_rowid();
    for group_id in group_ids {
        tx.execute(
            "INSERT INTO notification_rule_groups(rule_id,group_id) VALUES(?1,?2)",
            postgres_compat::params![id, group_id],
        )?;
    }
    for target in targets {
        tx.execute(
            "INSERT INTO notification_rule_targets(rule_id,target_kind,sample_info_type_key) VALUES(?1,?2,?3)",
            postgres_compat::params![id, target.target_kind, target.sample_info_type_key],
        )?;
    }
    tx.commit()?;
    Ok(id)
}

pub fn update_rule(pool: &DbPool, id: i64, input: &NotificationRuleInput) -> Result<()> {
    let mut conn = pool.get()?;
    let group_ids = normalize_group_ids(input);
    let targets = normalize_targets(input)?;
    let event_type = rule_event_type(&targets)?;
    validate_rule_delivery(input, event_type)?;
    validate_group_ids(&conn, &group_ids)?;
    let tx = conn.transaction()?;
    let changed = tx.execute("UPDATE notification_rules SET name=?1,event_type=?2,group_id=?3,project_name=?4,detection_type=?5,sample_info_type_key=?6,channel_id=?7,recipient_user_ids=?8,is_default=?9,is_active=?10 WHERE id=?11", postgres_compat::params![input.name.trim(), event_type, group_ids.first().copied(), input.project_name.clone().unwrap_or_default().trim(), input.detection_type.clone().unwrap_or_default().trim(), input.sample_info_type_key.clone().unwrap_or_default().trim(), input.channel_id, json!(input.recipient_user_ids).to_string(), if input.is_default.unwrap_or(false) {1} else {0}, if input.is_active.unwrap_or(true) {1} else {0}, id])?;
    if changed == 0 {
        return Err(AppError::NotFound("通知规则不存在".into()));
    }
    tx.execute(
        "DELETE FROM notification_rule_groups WHERE rule_id=?1",
        [id],
    )?;
    for group_id in group_ids {
        tx.execute(
            "INSERT INTO notification_rule_groups(rule_id,group_id) VALUES(?1,?2)",
            postgres_compat::params![id, group_id],
        )?;
    }
    tx.execute(
        "DELETE FROM notification_rule_targets WHERE rule_id=?1",
        [id],
    )?;
    for target in targets {
        tx.execute(
            "INSERT INTO notification_rule_targets(rule_id,target_kind,sample_info_type_key) VALUES(?1,?2,?3)",
            postgres_compat::params![id, target.target_kind, target.sample_info_type_key],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub fn enqueue_event(
    pool: &DbPool,
    resource_type: &str,
    record_id: i64,
    payload: &serde_json::Value,
) -> Result<()> {
    enqueue_event_with_type(
        pool,
        crate::service::notification_service::EVENT_SAMPLE_SUBMITTED,
        resource_type,
        record_id,
        payload,
    )
}

pub fn enqueue_event_with_type(
    pool: &DbPool,
    event_type: &str,
    resource_type: &str,
    record_id: i64,
    payload: &serde_json::Value,
) -> Result<()> {
    let conn = pool.get()?;
    let key = format!("{}:{}:{}", event_type, resource_type, record_id);
    conn.execute(&format!("INSERT OR IGNORE INTO notification_events(event_type,resource_type,resource_id,dedupe_key,payload_json,status,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,'pending',{0},{0})", now_sql()), postgres_compat::params![event_type, resource_type, record_id,key,payload.to_string()])?;
    Ok(())
}

/// Atomically claims pending notification events for one worker.
///
/// A record submission triggers immediate processing while the background worker also
/// polls periodically. Reading pending rows without claiming them allowed both workers
/// to send the same DingTalk message. The conditional update makes ownership exclusive;
/// a worker that crashes is recovered after five minutes.
pub fn claim_pending_events(
    pool: &DbPool,
    limit: i64,
) -> Result<Vec<(i64, String, String, i64, serde_json::Value)>> {
    let conn = pool.get()?;
    conn.execute(
        &format!(
            "UPDATE notification_events
             SET status='retry',next_attempt_at=NULL,updated_at={}
             WHERE status='processing'
               AND updated_at < to_char(CURRENT_TIMESTAMP - INTERVAL '5 minutes', 'YYYY-MM-DD HH24:MI:SS')",
            now_sql()
        ),
        [],
    )?;
    let mut stmt = conn.prepare("SELECT id,event_type,resource_type,resource_id,payload_json FROM notification_events WHERE status IN ('pending','retry') AND (next_attempt_at IS NULL OR next_attempt_at<=to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS')) ORDER BY id LIMIT ?1")?;
    let rows = stmt.query_map([limit], |r| {
        let s: String = r.get(4)?;
        Ok((
            r.get(0)?,
            r.get(1)?,
            r.get(2)?,
            r.get(3)?,
            serde_json::from_str(&s).unwrap_or_else(|_| json!({})),
        ))
    })?;
    let candidates = rows.collect::<std::result::Result<Vec<_>, _>>()?;
    let mut claimed = Vec::new();
    for event @ (event_id, _, _, _, _) in candidates {
        let updated = conn.execute(
            &format!(
                "UPDATE notification_events
                 SET status='processing',next_attempt_at=NULL,updated_at={}
                 WHERE id=?1
                   AND status IN ('pending','retry')
                   AND (next_attempt_at IS NULL OR next_attempt_at<=to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS'))",
                now_sql()
            ),
            postgres_compat::params![event_id],
        )?;
        if updated == 1 {
            claimed.push(event);
        }
    }
    Ok(claimed)
}

pub fn matching_rules(
    pool: &DbPool,
    event_type: &str,
    resource_type: &str,
    group_id: Option<i64>,
    project_name: &str,
    detection_type: &str,
    instrument_type: &str,
    sample_info_type_key: &str,
) -> Result<Vec<NotificationRule>> {
    let conn = pool.get()?;
    let sql = "SELECT r.id,r.name,r.event_type,r.group_id,g.name,r.project_name,r.detection_type,r.sample_info_type_key,r.channel_id,c.name,r.recipient_user_ids,r.is_default,r.is_active,r.created_at FROM notification_rules r LEFT JOIN project_groups g ON g.id=r.group_id LEFT JOIN notification_channels c ON c.id=r.channel_id WHERE r.event_type=?1 AND r.is_active=1 AND (r.is_default=1 OR (((NOT EXISTS(SELECT 1 FROM notification_rule_groups rg WHERE rg.rule_id=r.id)) OR EXISTS(SELECT 1 FROM notification_rule_groups rg WHERE rg.rule_id=r.id AND rg.group_id=?2)) AND (r.project_name='' OR r.project_name=?3) AND EXISTS(SELECT 1 FROM notification_rule_targets rt WHERE rt.rule_id=r.id AND rt.target_kind=?4 AND (rt.sample_info_type_key='' OR rt.sample_info_type_key=?7)) AND (r.detection_type='' OR r.detection_type=?5 OR r.detection_type=?6))) ORDER BY r.is_default ASC,CASE WHEN EXISTS(SELECT 1 FROM notification_rule_targets rt WHERE rt.rule_id=r.id AND rt.sample_info_type_key<>'') AND (r.group_id IS NOT NULL OR EXISTS(SELECT 1 FROM notification_rule_groups rg WHERE rg.rule_id=r.id)) AND r.project_name<>'' THEN 1 WHEN EXISTS(SELECT 1 FROM notification_rule_targets rt WHERE rt.rule_id=r.id AND rt.sample_info_type_key<>'') AND (r.group_id IS NOT NULL OR EXISTS(SELECT 1 FROM notification_rule_groups rg WHERE rg.rule_id=r.id)) THEN 2 WHEN EXISTS(SELECT 1 FROM notification_rule_targets rt WHERE rt.rule_id=r.id AND rt.sample_info_type_key<>'') THEN 3 ELSE 4 END,r.id";
    let mut stmt = conn.prepare(sql)?;
    let mut rules = stmt
        .query_map(
            postgres_compat::params![
                event_type,
                group_id,
                project_name,
                resource_type,
                detection_type,
                instrument_type,
                sample_info_type_key
            ],
            rule_from_row,
        )?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for rule in &mut rules {
        hydrate_rule_groups(&conn, rule)?;
        hydrate_rule_targets(&conn, rule)?;
    }
    if let Some(best_priority) = rules.iter().map(notification_rule_priority).min() {
        rules.retain(|rule| notification_rule_priority(rule) == best_priority);
    }
    Ok(rules)
}

fn notification_rule_priority(rule: &NotificationRule) -> u8 {
    if rule.is_default {
        return 8;
    }
    let has_type = !rule.sample_info_type_key.trim().is_empty();
    let has_group = !rule.group_ids.is_empty() || rule.group_id.is_some();
    let has_project = !rule.project_name.trim().is_empty();
    match (has_type, has_group, has_project) {
        (true, true, true) => 1,
        (true, true, false) => 2,
        (true, false, _) => 3,
        (false, true, true) => 4,
        (false, true, false) => 5,
        (false, false, true) => 6,
        (false, false, false) => 7,
    }
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
    let mut stmt = conn.prepare("SELECT DISTINCT u.id FROM users u JOIN user_roles ur ON ur.user_id=u.id JOIN roles r ON r.id=ur.role_id JOIN role_permissions rp ON rp.role_id=r.id WHERE u.is_active=1 AND u.deleted_at IS NULL AND r.deleted_at IS NULL AND rp.permission_key IN ('entry:workload','*')")?;
    Ok(stmt
        .query_map([], |r| r.get(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?)
}

pub fn analysis_user_ids_for(pool: &DbPool, candidates: &[i64]) -> Result<Vec<i64>> {
    let conn = pool.get()?;
    let mut valid = Vec::new();
    for user_id in candidates {
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM users u JOIN user_roles ur ON ur.user_id=u.id JOIN roles r ON r.id=ur.role_id JOIN role_permissions rp ON rp.role_id=r.id WHERE u.id=?1 AND u.is_active=1 AND u.deleted_at IS NULL AND r.deleted_at IS NULL AND rp.permission_key IN ('entry:workload','*'))",
            [user_id], |r| r.get(0),
        )?;
        if exists {
            valid.push(*user_id);
        }
    }
    Ok(valid)
}

pub fn active_user_ids_for(pool: &DbPool, candidates: &[i64]) -> Result<Vec<i64>> {
    let conn = pool.get()?;
    let mut valid = Vec::new();
    for user_id in candidates {
        let active: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM users WHERE id=?1 AND is_active=1 AND deleted_at IS NULL)",
            [user_id],
            |row| row.get(0),
        )?;
        if active {
            valid.push(*user_id);
        }
    }
    Ok(valid)
}

/// Returns analysis recipients that are authorized for one execution department.
/// The notification rule may choose people, but it cannot bypass the role data range.
pub fn analysis_user_ids_for_work_division(
    pool: &DbPool,
    division_id: Option<i64>,
    candidates: Option<&[i64]>,
) -> Result<Vec<i64>> {
    let conn = pool.get()?;
    let user_ids: Vec<i64> = if let Some(candidates) = candidates {
        candidates.to_vec()
    } else {
        let mut stmt = conn
            .prepare("SELECT id FROM users WHERE is_active=1 AND deleted_at IS NULL ORDER BY id")?;
        stmt.query_map([], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?
    };
    let mut valid = Vec::new();
    for user_id in user_ids {
        let allowed: bool = conn.query_row(
            "SELECT EXISTS(
               SELECT 1 FROM users u
                WHERE u.id=?1 AND u.is_active=1 AND u.deleted_at IS NULL
                  AND EXISTS(
                    SELECT 1 FROM user_roles ur JOIN roles r ON r.id=ur.role_id
                    JOIN role_permissions rp ON rp.role_id=r.id
                    WHERE ur.user_id=u.id AND r.deleted_at IS NULL
                      AND rp.permission_key IN ('entry:workload','*')
                  )
                  AND NOT EXISTS(
                    SELECT 1 FROM user_roles pur JOIN roles pr ON pr.id=pur.role_id
                    WHERE pur.user_id=u.id AND pr.deleted_at IS NULL
                      AND pr.system_key IN ('public_account','rd_public_account','analysis_public_account')
                  )
                  AND (
                    EXISTS(
                      SELECT 1 FROM user_roles vur JOIN roles vr ON vr.id=vur.role_id
                      JOIN role_permissions vrp ON vrp.role_id=vr.id
                      WHERE vur.user_id=u.id AND vr.deleted_at IS NULL
                        AND vrp.permission_key IN ('records:work:view-all','stats:workload:view-all','*')
                    )
                    OR (
                      CAST(?2 AS BIGINT) IS NOT NULL AND EXISTS(
                        SELECT 1 FROM user_roles sur JOIN roles sr ON sr.id=sur.role_id
                        JOIN role_permissions srp ON srp.role_id=sr.id
                        JOIN role_work_division_scopes rwds ON rwds.role_id=sr.id
                        WHERE sur.user_id=u.id AND sr.deleted_at IS NULL
                          AND srp.permission_key IN ('records:work:view-scope','sample-info:collect','sample-info:complete')
                          AND rwds.division_id=CAST(?2 AS BIGINT)
                      )
                    )
                  )
             )",
            postgres_compat::params![user_id, division_id],
            |row| row.get(0),
        )?;
        if allowed {
            valid.push(user_id);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{self, DbPool};
    use crate::models::notification::NotificationRuleTargetInput;
    use serde_json::json;

    fn rule(
        type_key: &str,
        group_id: Option<i64>,
        project: &str,
        is_default: bool,
    ) -> NotificationRule {
        NotificationRule {
            id: 1,
            name: "test".into(),
            event_type: "sample_submitted".into(),
            group_id,
            group_name: None,
            group_ids: group_id.into_iter().collect(),
            group_names: vec![],
            project_name: project.into(),
            detection_type: String::new(),
            sample_info_type_key: type_key.into(),
            targets: vec![],
            channel_id: None,
            channel_name: None,
            recipient_user_ids: vec![],
            is_default,
            is_active: true,
            created_at: String::new(),
        }
    }

    #[test]
    fn sample_notification_rules_rank_specific_scope_before_fallback() {
        assert_eq!(
            notification_rule_priority(&rule("icp", Some(1), "P1", false)),
            1
        );
        assert_eq!(
            notification_rule_priority(&rule("icp", Some(1), "", false)),
            2
        );
        assert_eq!(notification_rule_priority(&rule("icp", None, "", false)), 3);
        assert_eq!(notification_rule_priority(&rule("", None, "", true)), 8);
    }

    fn test_pool() -> DbPool {
        let pool = db::init_pool("");
        let conn = pool.get().expect("test database connection");
        crate::db::postgres_migrations::run(&conn, "admin123").expect("test migrations");
        pool
    }

    #[test]
    fn notification_event_can_only_be_claimed_once() {
        let pool = test_pool();
        enqueue_event(&pool, "rd_work_record", 101, &json!({"record_id": 101}))
            .expect("enqueue event");

        let first = claim_pending_events(&pool, 20).expect("first worker claim");
        let second = claim_pending_events(&pool, 20).expect("second worker claim");

        assert_eq!(first.len(), 1);
        assert!(second.is_empty());
    }

    #[test]
    fn notification_rule_accepts_multiple_laboratories() {
        let pool = test_pool();
        let conn = pool.get().expect("test database connection");
        let suffix = uuid::Uuid::new_v4().simple().to_string();
        conn.execute(
            "INSERT INTO project_groups(name,sort_order) VALUES(?1,0)",
            [format!("通知范围实验室甲-{suffix}")],
        )
        .expect("first laboratory");
        let first = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO project_groups(name,sort_order) VALUES(?1,0)",
            [format!("通知范围实验室乙-{suffix}")],
        )
        .expect("second laboratory");
        let second = conn.last_insert_rowid();
        let id = create_rule(
            &pool,
            &NotificationRuleInput {
                name: format!("多实验室通知-{suffix}"),
                group_id: None,
                group_ids: vec![first, second, first],
                project_name: None,
                detection_type: None,
                sample_info_type_key: None,
                targets: vec![NotificationRuleTargetInput {
                    target_kind: "rd_work_record".into(),
                    sample_info_type_key: String::new(),
                }],
                channel_id: None,
                recipient_user_ids: vec![],
                is_default: Some(false),
                is_active: Some(true),
            },
        )
        .expect("create multi laboratory rule");
        let rule = list_rules(&pool)
            .expect("list rules")
            .into_iter()
            .find(|item| item.id == id)
            .expect("created rule");
        assert_eq!(rule.group_ids, vec![first, second]);
    }

    #[test]
    fn notification_rule_accepts_resubmission_as_a_separate_event() {
        let pool = test_pool();
        let suffix = uuid::Uuid::new_v4().simple().to_string();
        let id = create_rule(
            &pool,
            &NotificationRuleInput {
                name: format!("退回重提通知-{suffix}"),
                group_id: None,
                group_ids: vec![],
                project_name: None,
                detection_type: None,
                sample_info_type_key: None,
                targets: vec![NotificationRuleTargetInput {
                    target_kind: "rd_work_record_resubmitted".into(),
                    sample_info_type_key: String::new(),
                }],
                channel_id: None,
                recipient_user_ids: vec![],
                is_default: Some(false),
                is_active: Some(true),
            },
        )
        .expect("create resubmission rule");
        let rule = list_rules(&pool)
            .expect("list rules")
            .into_iter()
            .find(|item| item.id == id)
            .expect("created resubmission rule");
        assert_eq!(
            rule.event_type,
            crate::service::notification_service::EVENT_RD_RESUBMITTED
        );
        assert_eq!(rule.targets[0].target_kind, "rd_work_record_resubmitted");
    }

    #[test]
    fn updating_channel_without_credentials_keeps_saved_connection() {
        let pool = test_pool();
        let suffix = uuid::Uuid::new_v4().simple().to_string();
        let id = create_channel(
            &pool,
            &NotificationChannelInput {
                name: format!("渠道-{suffix}"),
                webhook_url: Some("https://example.test/robot".into()),
                secret: Some("secret-value".into()),
                is_active: Some(true),
            },
        )
        .expect("create channel");
        update_channel(
            &pool,
            id,
            &NotificationChannelInput {
                name: format!("渠道更新-{suffix}"),
                webhook_url: None,
                secret: None,
                is_active: Some(false),
            },
        )
        .expect("update channel");
        let conn = pool.get().expect("connection");
        let saved: (String, String, String, i64) = conn
            .query_row(
                "SELECT name,webhook_url,secret,is_active FROM notification_channels WHERE id=?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("saved channel");
        assert_eq!(saved.0, format!("渠道更新-{suffix}"));
        assert_eq!(saved.1, "https://example.test/robot");
        assert_eq!(saved.2, "secret-value");
        assert_eq!(saved.3, 0);
    }

    #[test]
    fn notification_rule_matches_each_configured_source_or_type() {
        let pool = test_pool();
        let suffix = uuid::Uuid::new_v4().simple().to_string();
        let id = create_rule(
            &pool,
            &NotificationRuleInput {
                name: format!("多来源通知-{suffix}"),
                group_id: None,
                group_ids: vec![],
                project_name: None,
                detection_type: None,
                sample_info_type_key: None,
                targets: vec![
                    NotificationRuleTargetInput {
                        target_kind: "rd_work_record".into(),
                        sample_info_type_key: String::new(),
                    },
                    NotificationRuleTargetInput {
                        target_kind: "sample_info".into(),
                        sample_info_type_key: "ICP".into(),
                    },
                ],
                channel_id: None,
                recipient_user_ids: vec![],
                is_default: Some(false),
                is_active: Some(true),
            },
        )
        .expect("create multi source rule");

        let has_rule = |resource_type: &str, type_key: &str| {
            matching_rules(
                &pool,
                crate::service::notification_service::EVENT_SAMPLE_SUBMITTED,
                resource_type,
                None,
                "",
                "",
                "",
                type_key,
            )
            .expect("match rule")
            .iter()
            .any(|rule| rule.id == id)
        };
        assert!(has_rule("rd_work_record", ""));
        assert!(has_rule("sample_info", "ICP"));
        assert!(!has_rule("sample_info", "热稳定性"));
    }

    #[test]
    fn notification_rule_accepts_personnel_feedback_rejection_as_separate_event() {
        let pool = test_pool();
        let suffix = uuid::Uuid::new_v4().simple().to_string();
        let id = create_rule(
            &pool,
            &NotificationRuleInput {
                name: format!("人员反馈驳回通知-{suffix}"),
                group_id: None,
                group_ids: vec![],
                project_name: None,
                detection_type: None,
                sample_info_type_key: None,
                targets: vec![NotificationRuleTargetInput {
                    target_kind: "personnel_change_feedback_rejected".into(),
                    sample_info_type_key: String::new(),
                }],
                channel_id: None,
                recipient_user_ids: vec![],
                is_default: Some(false),
                is_active: Some(true),
            },
        )
        .expect("create personnel rejection rule");
        let rule = list_rules(&pool)
            .expect("list rules")
            .into_iter()
            .find(|item| item.id == id)
            .expect("created personnel rejection rule");
        assert_eq!(
            rule.event_type,
            crate::service::notification_service::EVENT_PERSONNEL_CHANGE_REJECTED
        );
        assert_eq!(
            rule.targets[0].target_kind,
            "personnel_change_feedback_rejected"
        );
    }
}
