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
    let (title, footer, config): (String, String, String) = conn.query_row(
        "SELECT title,footer,field_config_json FROM notification_templates WHERE event_key=?1 AND is_active=1",
        [event_key],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).map_err(|e| match e { postgres_compat::Error::QueryReturnedNoRows => AppError::NotFound("通知模板不存在".into()), other => other.into() })?;
    let config: serde_json::Value = serde_json::from_str(&config)
        .map_err(|error| AppError::Internal(format!("通知模板字段配置解析失败: {error}")))?;
    if !config.is_array() {
        return Err(AppError::Internal("通知模板字段配置必须为数组".into()));
    }
    Ok((title, footer, config))
}

pub(crate) fn canonical_template_field_key<'a>(template_key: &str, key: &'a str) -> &'a str {
    if template_key == "personnel_change_feedback"
        || template_key == "personnel_change_feedback_rejected"
    {
        return key;
    }
    match key {
        "user_name" => "submitted_by",
        "division_id" => "division_name",
        other => other,
    }
}

fn notification_value_field(key: &str, data_type: &str) -> bool {
    !key.trim().is_empty()
        && !key.starts_with("action_")
        && !matches!(
            key,
            "seq_no" | "attachment_files" | "attachments" | "extra_fields"
        )
        && !matches!(data_type, "action" | "heading" | "divider" | "attachment")
}

fn append_template_field(fields: &mut Vec<serde_json::Value>, key: &str, label: &str) {
    let key = canonical_template_field_key("", key);
    if !fields
        .iter()
        .any(|field| field.get("key").and_then(|value| value.as_str()) == Some(key))
    {
        fields.push(json!({"key":key,"label":label,"sort_order":fields.len() as i64 + 1}));
    }
}

fn configured_notification_type_applies(source: &str, type_key: Option<&str>) -> bool {
    let Some(type_key) = type_key else {
        return true;
    };
    let values = serde_json::from_str::<Vec<String>>(source).unwrap_or_else(|_| {
        source
            .split([',', '，', '\n'])
            .map(str::to_string)
            .collect()
    });
    let values: Vec<_> = values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect();
    values.is_empty()
        || type_key
            .split([',', '，', '\n'])
            .map(str::trim)
            .any(|current| values.contains(&current))
}

/// Business candidates use the current record-column source, not obsolete form settings.
/// A concrete type narrows runtime candidates; editors pass None to see all configured fields.
pub fn template_available_fields(
    pool: &DbPool,
    type_key: Option<&str>,
    _form_setting_keys: &[&str],
    template_key: &str,
) -> Result<Vec<serde_json::Value>> {
    let sample = template_key == "sample_info" || template_key.starts_with("sample_info:");
    let personnel = template_key == "personnel_change_feedback"
        || template_key == "personnel_change_feedback_rejected";
    let rejected = template_key == "rd_work_record_rejected";
    let mut fields = Vec::new();
    if !personnel {
        let conn = pool.get()?;
        let definition_count: i64 = conn.query_row(
            if sample {
                "SELECT COUNT(*) FROM sample_info_columns"
            } else {
                "SELECT COUNT(*) FROM rd_record_columns"
            },
            [],
            |row| row.get(0),
        )?;
        if definition_count == 0 {
            // Empty legacy column stores retain the original notification fields. Disabled
            // or type-hidden definitions are deliberate configuration and never trigger this.
            for (key, label) in [
                ("batch_no", "批号"),
                ("submitted_at", "送样时间"),
                ("submitted_by", "送样人"),
                ("division_name", "部门"),
                ("lab_name", "实验室"),
                ("project_name", "项目"),
                ("detection_type", "检测类型"),
                ("quantity", "数量"),
                ("notes", "注意事项"),
                ("status", "状态"),
            ] {
                append_template_field(&mut fields, key, label);
            }
            if sample {
                append_template_field(&mut fields, "main_components", "主要成分");
            } else {
                append_template_field(&mut fields, "method_name", "方法");
                append_template_field(&mut fields, "instrument_code", "仪器");
            }
        }
        if sample {
            let mut stmt = conn.prepare(
                "SELECT c.field_key,c.label,c.data_type FROM sample_info_columns c \
             LEFT JOIN sample_info_column_visibility v ON v.column_id=c.id AND v.type_key=?1 \
             WHERE c.is_active=1 AND c.deleted_at IS NULL \
               AND (?1='' OR COALESCE(v.is_visible,0)=1) \
             ORDER BY COALESCE(v.sort_order,c.sort_order),c.id",
            )?;
            for row in stmt.query_map([type_key.unwrap_or("")], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })? {
                let (key, label, data_type) = row?;
                if notification_value_field(&key, &data_type) {
                    append_template_field(&mut fields, &key, &label);
                }
            }
            append_template_field(&mut fields, "attachment_count", "附件数量");
        } else {
            let mut stmt = conn.prepare(
                "SELECT name,label,data_type,applicable_types FROM rd_record_columns WHERE is_active=1 ORDER BY sort_order,id",
            )?;
            for row in stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })? {
                let (key, label, data_type, applicable_types) = row?;
                if notification_value_field(&key, &data_type)
                    && configured_notification_type_applies(&applicable_types, type_key)
                {
                    append_template_field(&mut fields, &key, &label);
                }
            }
        }
        if rejected {
            for (key, label) in [
                ("returned_at", "驳回时间"),
                ("returned_by", "驳回人"),
                ("return_reason", "驳回原因"),
            ] {
                append_template_field(&mut fields, key, label);
            }
        }
        // Event values without a configured column remain available. A real column with
        // the same key, when present, keeps its source label and position.
        append_template_field(&mut fields, "business_no", "送样编号");
    }

    let feedback_dir = std::env::var("PERSONNEL_CHANGE_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            crate::config::AppConfig::load()
                .data_dir()
                .join("personnel-change-data")
        });
    if personnel {
        let text = fs::read_to_string(feedback_dir.join("fields.json"))
            .map_err(|error| AppError::Internal(format!("人员变动字段配置读取失败: {error}")))?;
        let items = serde_json::from_str::<Vec<serde_json::Value>>(&text)
            .map_err(|error| AppError::Internal(format!("人员变动字段配置解析失败: {error}")))?;
        for item in items
            .into_iter()
            .filter(|item| {
                item.get("enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true)
                    && item.get("notify").and_then(|v| v.as_bool()).unwrap_or(true)
            })
            .filter(|item| {
                let applicable = item.get("change_types").and_then(|value| value.as_array());
                type_key.is_none()
                    || applicable.is_none_or(|values| {
                        values.is_empty()
                            || values.iter().any(|value| {
                                value.as_str().is_some_and(|value| {
                                    Some(value.trim()) == type_key.map(str::trim)
                                })
                            })
                    })
            })
        {
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
    if !field_config.is_array() {
        return Err(AppError::Validation("通知字段配置必须为数组".into()));
    }
    let conn = pool.get()?;
    let mut stored_config = field_config.clone();
    if let Some(new_items) = field_config.as_array() {
        match get_template(pool, event_key) {
            Ok((_, _, old_config)) => {
                if let Some(old_items) = old_config.as_array() {
                    let submitted_keys: std::collections::HashSet<&str> = new_items
                        .iter()
                        .filter_map(|item| item.get("key").and_then(|value| value.as_str()))
                        .map(|key| canonical_template_field_key(event_key, key))
                        .collect();
                    let mut merged = new_items.clone();
                    merged.extend(
                        old_items
                            .iter()
                            .filter(|item| {
                                item.get("key")
                                    .and_then(|value| value.as_str())
                                    .map(|key| {
                                        !submitted_keys
                                            .contains(canonical_template_field_key(event_key, key))
                                    })
                                    .unwrap_or(false)
                            })
                            .cloned(),
                    );
                    stored_config = serde_json::Value::Array(merged);
                }
            }
            Err(AppError::NotFound(_)) => {}
            Err(error) => return Err(error),
        }
    }
    if let Some(items) = stored_config.as_array_mut() {
        let mut keys = std::collections::HashSet::new();
        items.retain_mut(|item| {
            let Some(key) = item.get("key").and_then(|value| value.as_str()) else {
                return false;
            };
            let canonical_key = canonical_template_field_key(event_key, key).to_string();
            if !keys.insert(canonical_key.clone()) {
                return false;
            }
            item["key"] = json!(canonical_key);
            true
        });
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

pub(crate) fn notification_target_url(
    resource_type: &str,
    resource_id: i64,
    payload: &serde_json::Value,
    stored_url: &str,
) -> String {
    match resource_type {
        "rd_work_record" | "rd_work_record_rejected" | "rd_work_record_resubmitted"
            if resource_id > 0 =>
        {
            format!("/sample-records?record_id={resource_id}")
        }
        "sample_info" if resource_id > 0 => {
            let mut target = format!("/sample-info/entry?record_id={resource_id}");
            if let Some(type_key) = payload["type_key"]
                .as_str()
                .filter(|value| !value.is_empty())
            {
                target.push_str("&type=");
                target.push_str(&url_escape::encode_component(type_key));
            }
            target
        }
        "personnel_change_feedback" | "personnel_change_feedback_rejected" => {
            if let Some(notice_no) = payload["notice_no"]
                .as_str()
                .filter(|value| !value.is_empty())
            {
                format!(
                    "/personnel-change?notice_no={}",
                    url_escape::encode_component(notice_no)
                )
            } else {
                "/personnel-change".to_string()
            }
        }
        _ => {
            let target = stored_url.trim();
            if target.starts_with('/')
                && !target.starts_with("//")
                && !target.contains('\\')
                && !target.chars().any(char::is_control)
            {
                target.to_string()
            } else {
                String::new()
            }
        }
    }
}

pub fn inbox(pool: &DbPool, user_id: i64) -> Result<Vec<InAppNotification>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare("SELECT n.id,n.title,n.body,n.target_url,n.is_read,n.created_at,COALESCE(e.resource_type,''),COALESCE(e.resource_id,0),COALESCE(e.payload_json,'{}') FROM in_app_notifications n LEFT JOIN notification_events e ON e.id=n.event_id WHERE n.user_id=?1 ORDER BY n.id DESC LIMIT 100")?;
    Ok(stmt
        .query_map([user_id], |r| {
            let resource_type: String = r.get(6)?;
            let resource_id: i64 = r.get(7)?;
            let payload: String = r.get(8)?;
            let payload = serde_json::from_str(&payload).unwrap_or_else(|_| json!({}));
            let stored_url: String = r.get(3)?;
            Ok(InAppNotification {
                id: r.get(0)?,
                title: r.get(1)?,
                body: r.get(2)?,
                target_url: notification_target_url(
                    &resource_type,
                    resource_id,
                    &payload,
                    &stored_url,
                ),
                analysis_group_id: if matches!(
                    resource_type.as_str(),
                    "rd_work_record" | "rd_work_record_rejected" | "rd_work_record_resubmitted"
                ) {
                    payload["group_id"].as_i64().filter(|id| *id > 0)
                } else {
                    None
                },
                is_read: r.get::<_, i64>(4)? != 0,
                created_at: r.get(5)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?)
}

pub fn inbox_unread_count(pool: &DbPool, user_id: i64) -> Result<i64> {
    let conn = pool.get()?;
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM in_app_notifications WHERE user_id=?1 AND is_read=0",
        [user_id],
        |row| row.get(0),
    )?)
}

pub fn mark_all_inbox_read(pool: &DbPool, user_id: i64) -> Result<i64> {
    let conn = pool.get()?;
    Ok(conn.execute(
        "UPDATE in_app_notifications SET is_read=1 WHERE user_id=?1 AND is_read=0",
        [user_id],
    )? as i64)
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

    #[test]
    fn inbox_target_urls_are_precise_encoded_and_internal() {
        for resource_type in [
            "rd_work_record",
            "rd_work_record_rejected",
            "rd_work_record_resubmitted",
        ] {
            assert_eq!(
                notification_target_url(resource_type, 17, &json!({}), "/old"),
                "/sample-records?record_id=17"
            );
        }
        assert_eq!(
            notification_target_url("sample_info", 18, &json!({"type_key":"lc&gc"}), "/old"),
            "/sample-info/entry?record_id=18&type=lc%26gc"
        );
        assert_eq!(
            notification_target_url("sample_info", 18, &json!({}), "/old"),
            "/sample-info/entry?record_id=18"
        );
        for resource_type in [
            "personnel_change_feedback",
            "personnel_change_feedback_rejected",
        ] {
            assert_eq!(
                notification_target_url(
                    resource_type,
                    19,
                    &json!({"notice_no":"P/57 A"}),
                    "/personnel-change"
                ),
                "/personnel-change?notice_no=P%2F57%20A"
            );
            assert_eq!(
                notification_target_url(resource_type, 19, &json!({}), ""),
                "/personnel-change"
            );
        }
        assert_eq!(
            notification_target_url("", 0, &json!({}), "/help?id=1"),
            "/help?id=1"
        );
        for invalid in [
            "https://example.com",
            "//example.com",
            "/\\example.com",
            "/help\nmalformed",
            "javascript:alert(1)",
        ] {
            assert_eq!(notification_target_url("", 0, &json!({}), invalid), "");
        }
    }

    #[test]
    fn inbox_unread_and_read_all_cover_all_owned_rows_and_are_idempotent() {
        let pool = test_pool();
        let conn = pool.get().unwrap();
        conn.execute("INSERT INTO in_app_notifications(event_id,user_id,title,body) SELECT 20000+n,1001,'我的通知','内容' FROM generate_series(1,105) AS n", []).unwrap();
        conn.execute("INSERT INTO in_app_notifications(event_id,user_id,title,body) VALUES(30001,1002,'其他通知','内容'),(30002,1002,'其他通知','内容')", []).unwrap();
        assert_eq!(inbox(&pool, 1001).unwrap().len(), 100);
        assert_eq!(inbox_unread_count(&pool, 1001).unwrap(), 105);
        let mine: i64 = conn
            .query_row(
                "SELECT MIN(id) FROM in_app_notifications WHERE user_id=1001",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let other: i64 = conn
            .query_row(
                "SELECT MIN(id) FROM in_app_notifications WHERE user_id=1002",
                [],
                |row| row.get(0),
            )
            .unwrap();
        mark_inbox_read(&pool, mine, 1001).unwrap();
        mark_inbox_read(&pool, mine, 1001).unwrap();
        assert_eq!(inbox_unread_count(&pool, 1001).unwrap(), 104);
        assert!(matches!(
            mark_inbox_read(&pool, other, 1001),
            Err(AppError::NotFound(_))
        ));
        assert_eq!(mark_all_inbox_read(&pool, 1001).unwrap(), 104);
        assert_eq!(inbox_unread_count(&pool, 1001).unwrap(), 0);
        assert_eq!(inbox_unread_count(&pool, 1002).unwrap(), 2);
        assert_eq!(mark_all_inbox_read(&pool, 1001).unwrap(), 0);
        assert_eq!(mark_all_inbox_read(&pool, 999999).unwrap(), 0);
    }

    #[test]
    fn inbox_restores_legacy_targets_without_rewriting_saved_messages() {
        let pool = test_pool();
        let conn = pool.get().unwrap();
        for (resource_type, resource_id, payload) in [
            ("rd_work_record", 71, json!({"group_id":31})),
            ("rd_work_record_rejected", 75, json!({"group_id":31})),
            ("rd_work_record_resubmitted", 76, json!({"group_id":31})),
            ("sample_info", 72, json!({"type_key":"icp"})),
            (
                "personnel_change_feedback",
                73,
                json!({"notice_no":"PC-57-1"}),
            ),
            (
                "personnel_change_feedback_rejected",
                74,
                json!({"notice_no":"PC-57-2"}),
            ),
        ] {
            enqueue_event(&pool, resource_type, resource_id, &payload).unwrap();
            let event: i64 = conn
                .query_row(
                    "SELECT id FROM notification_events WHERE resource_type=?1 AND resource_id=?2",
                    postgres_compat::params![resource_type, resource_id],
                    |row| row.get(0),
                )
                .unwrap();
            create_in_app(
                &pool,
                event,
                &[1001, 1002],
                resource_type,
                "历史内容",
                "/old-business-page",
            )
            .unwrap();
        }
        let items = inbox(&pool, 1001).unwrap();
        assert_eq!(items.len(), 6);
        let rd = items
            .iter()
            .find(|item| item.title == "rd_work_record")
            .unwrap();
        assert_eq!(rd.target_url, "/sample-records?record_id=71");
        assert_eq!(rd.analysis_group_id, Some(31));
        for (title, target) in [
            ("rd_work_record_rejected", "/sample-records?record_id=75"),
            ("rd_work_record_resubmitted", "/sample-records?record_id=76"),
        ] {
            let item = items.iter().find(|item| item.title == title).unwrap();
            assert_eq!(item.target_url, target);
            assert_eq!(item.analysis_group_id, Some(31));
        }
        let sample = items
            .iter()
            .find(|item| item.title == "sample_info")
            .unwrap();
        assert_eq!(
            sample.target_url,
            "/sample-info/entry?record_id=72&type=icp"
        );
        assert_eq!(sample.analysis_group_id, None);
        assert!(items
            .iter()
            .any(|item| item.target_url == "/personnel-change?notice_no=PC-57-1"));
        assert!(items
            .iter()
            .any(|item| item.target_url == "/personnel-change?notice_no=PC-57-2"));
        let unchanged: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM in_app_notifications WHERE target_url='/old-business-page'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(unchanged, 12);
        conn.execute("UPDATE notification_events SET payload_json='invalid-json' WHERE resource_type='rd_work_record'", []).unwrap();
        create_in_app(
            &pool,
            999999,
            &[1001],
            "孤立通知",
            "内容",
            "https://example.com",
        )
        .unwrap();
        let restored = inbox(&pool, 1001).unwrap();
        let rd = restored
            .iter()
            .find(|item| item.title == "rd_work_record")
            .unwrap();
        assert_eq!(rd.target_url, "/sample-records?record_id=71");
        assert_eq!(rd.analysis_group_id, None);
        assert!(restored
            .iter()
            .any(|item| item.title == "孤立通知" && item.target_url.is_empty()));
    }

    #[test]
    fn template_candidates_normalize_business_aliases_but_keep_personnel_keys() {
        assert_eq!(
            canonical_template_field_key("rd_work_record", "user_name"),
            "submitted_by"
        );
        assert_eq!(
            canonical_template_field_key("sample_info:icp", "division_id"),
            "division_name"
        );
        assert_eq!(
            canonical_template_field_key("personnel_change_feedback", "user_name"),
            "user_name"
        );
        assert_eq!(
            canonical_template_field_key("personnel_change_feedback_rejected", "user_name"),
            "user_name"
        );
        assert_eq!(
            canonical_template_field_key("personnel_change_feedback", "division_id"),
            "division_id"
        );
        assert!(configured_notification_type_applies(
            "[\"ICP\",\"质谱\"]",
            Some("ICP")
        ));
        assert!(configured_notification_type_applies(
            "ICP，质谱",
            Some("质谱")
        ));
        assert!(!configured_notification_type_applies("ICP", Some("液相")));
        assert!(configured_notification_type_applies(
            "ICP",
            Some("ICP,质谱")
        ));
        assert!(configured_notification_type_applies(
            "[\"质谱\"]",
            Some("ICP， 质谱")
        ));
        assert!(!configured_notification_type_applies(
            "液相",
            Some("ICP,质谱")
        ));
        assert!(configured_notification_type_applies("ICP", None));
        for (key, data_type) in [
            ("seq_no", "number"),
            ("action_collect", "text"),
            ("files", "attachment"),
            ("section", "heading"),
        ] {
            assert!(!notification_value_field(key, data_type));
        }
    }

    #[test]
    fn rd_template_candidates_use_record_columns_and_runtime_type_scope() {
        let pool = test_pool();
        let conn = pool.get().expect("test database connection");
        let suffix = uuid::Uuid::new_v4().simple().to_string();
        let key = format!("notify_value_{suffix}");
        let stale_key = format!("notify_stale_{suffix}");
        let setting_key = format!("notify_form_{suffix}");
        conn.execute(
            "INSERT INTO rd_record_columns(name,label,data_type,is_active,show_in_form,show_in_list,applicable_types) VALUES(?1,'实际列名称','text',1,0,1,'ICP')",
            [&key],
        ).expect("record column");
        conn.execute(
            "INSERT INTO system_settings(key,value) VALUES(?1,?2)",
            postgres_compat::params![
                setting_key,
                json!({"fields":[{"key":stale_key,"label":"旧表单字段"}]}).to_string()
            ],
        )
        .expect("obsolete form setting");
        let icp = template_available_fields(&pool, Some("ICP"), &[&setting_key], "rd_work_record")
            .expect("ICP candidates");
        assert!(icp
            .iter()
            .any(|field| field["key"] == key && field["label"] == "实际列名称"));
        assert!(!icp.iter().any(|field| field["key"] == stale_key
            || field["key"] == "user_name"
            || field["key"] == "division_id"
            || field["key"] == "seq_no"));
        let note_label: String = conn
            .query_row(
                "SELECT label FROM rd_record_columns WHERE name='notes' AND is_active=1",
                [],
                |row| row.get(0),
            )
            .expect("notes source label");
        assert!(icp
            .iter()
            .any(|field| field["key"] == "notes" && field["label"] == note_label));
        let other = template_available_fields(&pool, Some("液相"), &[], "rd_work_record")
            .expect("other candidates");
        assert!(!other.iter().any(|field| field["key"] == key));
    }

    #[test]
    fn sample_template_candidates_include_predefined_and_hidden_form_fields_for_one_type() {
        let pool = test_pool();
        let conn = pool.get().expect("test database connection");
        let suffix = uuid::Uuid::new_v4().simple().to_string();
        let type_key = format!("notify_type_{suffix}");
        let field_key = format!("notify_custom_{suffix}");
        conn.execute(
            "INSERT INTO sample_info_types(type_key,label) VALUES(?1,'通知测试类型')",
            [&type_key],
        )
        .expect("sample type");
        conn.execute("INSERT INTO sample_info_columns(field_key,label,data_type,is_predefined,is_active,show_in_form) VALUES(?1,'仅列表字段','text',0,1,0)", [&field_key]).expect("custom sample column");
        let column_id = conn.last_insert_rowid();
        conn.execute("INSERT INTO sample_info_column_visibility(type_key,column_id,is_visible,show_in_form,show_in_list,show_in_export) VALUES(?1,?2,1,0,1,0)", postgres_compat::params![type_key, column_id]).expect("type visibility");
        let predefined_visibility = conn.execute("INSERT INTO sample_info_column_visibility(type_key,column_id,is_visible,show_in_form,show_in_list,show_in_export) SELECT ?1,id,1,0,1,0 FROM sample_info_columns WHERE field_key='submitted_at'", [&type_key]).expect("predefined date visibility");
        assert_eq!(predefined_visibility, 1, "seeded source column must exist");
        let fields = template_available_fields(
            &pool,
            Some(&type_key),
            &["form_sample_info_entry"],
            &format!("sample_info:{type_key}"),
        )
        .expect("sample candidates");
        assert!(fields.iter().any(|field| field["key"] == field_key));
        assert!(fields.iter().any(|field| field["key"] == "submitted_at"));
        assert!(!fields.iter().any(|field| field["key"] == "seq_no"
            || field["key"]
                .as_str()
                .is_some_and(|key| key.starts_with("action_"))));
        conn.execute("UPDATE sample_info_column_visibility SET is_visible=0 WHERE type_key=?1 AND column_id=?2", postgres_compat::params![type_key, column_id]).expect("hide from type");
        let hidden = template_available_fields(
            &pool,
            Some(&type_key),
            &[],
            &format!("sample_info:{type_key}"),
        )
        .expect("hidden candidates");
        assert!(!hidden.iter().any(|field| field["key"] == field_key));
    }

    #[test]
    fn saving_template_normalizes_aliases_and_preserves_unavailable_configuration() {
        let pool = test_pool();
        let event_key = format!("sample_info:notify_{}", uuid::Uuid::new_v4().simple());
        save_template(
            &pool,
            &event_key,
            "test",
            "",
            &json!([
                {"key":"user_name","label":"旧送样人","visible":false},
                {"key":"temporarily_disabled","label":"停用字段","visible":false}
            ]),
        )
        .expect("old template");
        save_template(
            &pool,
            &event_key,
            "test",
            "",
            &json!([
                {"key":"submitted_by","label":"新送样人","visible":true}
            ]),
        )
        .expect("new template");
        let (_, _, config) = get_template(&pool, &event_key).expect("saved template");
        let items = config.as_array().expect("array");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["key"], "submitted_by");
        assert_eq!(items[0]["label"], "新送样人");
        assert_eq!(items[1]["key"], "temporarily_disabled");
        assert_eq!(items[1]["visible"], false);
    }

    #[test]
    fn saving_template_does_not_overwrite_damaged_stored_configuration() {
        let pool = test_pool();
        let event_key = format!("sample_info:damaged_{}", uuid::Uuid::new_v4().simple());
        assert!(matches!(
            get_template(&pool, &event_key),
            Err(AppError::NotFound(_))
        ));
        let input = json!([{"key":"submitted_by","label":"送样人","visible":true}]);
        save_template(&pool, &event_key, "old", "", &input).expect("initial template");
        let conn = pool.get().expect("test database connection");
        for damaged in ["invalid-json", "{}"] {
            conn.execute(
                "UPDATE notification_templates SET field_config_json=?1 WHERE event_key=?2",
                postgres_compat::params![damaged, event_key],
            )
            .expect("damaged stored config");
            assert!(matches!(
                get_template(&pool, &event_key),
                Err(AppError::Internal(_))
            ));
            assert!(matches!(
                save_template(&pool, &event_key, "new", "", &input),
                Err(AppError::Internal(_))
            ));
            let (title, stored): (String, String) = conn
                .query_row(
                    "SELECT title,field_config_json FROM notification_templates WHERE event_key=?1",
                    [&event_key],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("unchanged stored template");
            assert_eq!(title, "old");
            assert_eq!(stored, damaged);
        }
        assert!(matches!(
            save_template(&pool, &event_key, "new", "", &json!({})),
            Err(AppError::Validation(_))
        ));
    }

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
