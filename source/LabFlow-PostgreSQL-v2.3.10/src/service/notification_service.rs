use crate::db::DbPool;
use crate::error::Result;
use crate::models::notification::{NotificationTemplate, NotificationTemplateField};
use crate::models::rd_record::RdRecordResponse;
use crate::models::sample_info::SampleInfoResponse;
use crate::repo::{division_repo, notification_repo, sample_info_attachment_repo};
use base64::{engine::general_purpose::STANDARD, Engine};
use hmac::{Hmac, Mac};
use regex::Regex;
use serde_json::json;
use sha2::Sha256;
use std::collections::HashSet;

pub(crate) const EVENT_SAMPLE_SUBMITTED: &str = "sample_submitted";
pub(crate) const EVENT_RD_REJECTED: &str = "rd_record_rejected";
pub(crate) const EVENT_RD_RESUBMITTED: &str = "rd_record_resubmitted";
pub(crate) const EVENT_PERSONNEL_CHANGE: &str = "personnel_change_feedback";
pub(crate) const EVENT_PERSONNEL_CHANGE_REJECTED: &str = "personnel_change_feedback_rejected";
pub const TEMPLATE_RD: &str = "rd_work_record";
pub const TEMPLATE_RD_REJECTED: &str = "rd_work_record_rejected";
pub const TEMPLATE_RD_RESUBMITTED: &str = "rd_work_record_resubmitted";
pub const TEMPLATE_SAMPLE_INFO: &str = "sample_info";
pub const TEMPLATE_PERSONNEL_CHANGE: &str = "personnel_change_feedback";
pub const TEMPLATE_PERSONNEL_CHANGE_REJECTED: &str = "personnel_change_feedback_rejected";
const MAX_ATTEMPTS: i64 = 3;

pub fn form_setting_keys_for_template(key: &str) -> &'static [&'static str] {
    if key == TEMPLATE_PERSONNEL_CHANGE || key == TEMPLATE_PERSONNEL_CHANGE_REJECTED {
        &[]
    } else if key == TEMPLATE_SAMPLE_INFO || key.starts_with("sample_info:") {
        &["form_sample_info_entry"]
    } else {
        &["form_sample_entry"]
    }
}

fn template_fields(keys: &[(&str, &str)]) -> Vec<NotificationTemplateField> {
    keys.iter()
        .enumerate()
        .map(|(index, (key, label))| NotificationTemplateField {
            key: (*key).into(),
            label: (*label).into(),
            visible: true,
            bold: false,
            sort_order: index as i64 + 1,
        })
        .collect()
}

pub fn default_template(key: &str) -> NotificationTemplate {
    if key == TEMPLATE_PERSONNEL_CHANGE || key == TEMPLATE_PERSONNEL_CHANGE_REJECTED {
        let rejected = key == TEMPLATE_PERSONNEL_CHANGE_REJECTED;
        return NotificationTemplate {
            key: key.into(),
            name: if rejected {
                "人员通知反馈驳回"
            } else {
                "人员变动反馈通知"
            }
            .into(),
            title: if rejected {
                "人员通知反馈驳回：{{通知编号}} · {{实验室}}".into()
            } else {
                "人员变动反馈：{{变动类型}} · {{实验室}}".into()
            },
            // 人员反馈字段唯一来源是 personnel-change-data/fields.json。
            // 这里不再放置硬编码字段，避免配置文件异常时旧字段重新出现。
            fields: Vec::new(),
            available_fields: Vec::new(),
            footer: if rejected {
                "请根据驳回原因修改后重新发起人员变动通知。"
            } else {
                "请管理员或分析检测组长按通知内容手动调整实际人员数据。"
            }
            .into(),
        };
    }
    let sample = key == TEMPLATE_SAMPLE_INFO || key.starts_with("sample_info:");
    let rejected = key == TEMPLATE_RD_REJECTED;
    let resubmitted = key == TEMPLATE_RD_RESUBMITTED;
    let mut fields = template_fields(&[
        ("batch_no", "批号"),
        ("business_no", "送样编号"),
        ("submitted_at", "送样时间"),
        ("submitted_by", "送样人"),
        ("division_name", "部门"),
        ("lab_name", "实验室"),
        ("project_name", "项目"),
        ("detection_type", "检测类型"),
        ("method_name", "方法"),
    ]);
    if !sample {
        fields.push(NotificationTemplateField {
            key: "instrument_code".into(),
            label: "仪器".into(),
            visible: true,
            bold: false,
            sort_order: fields.len() as i64 + 1,
        });
    }
    for (key, label) in if rejected {
        vec![
            ("quantity", "数量"),
            ("returned_at", "驳回时间"),
            ("returned_by", "驳回人"),
            ("return_reason", "驳回原因"),
            ("status", "当前状态"),
        ]
    } else if sample {
        vec![
            ("quantity", "数量"),
            ("main_components", "主要成分"),
            ("notes", "备注"),
        ]
    } else {
        vec![("quantity", "数量"), ("notes", "备注")]
    } {
        fields.push(NotificationTemplateField {
            key: key.into(),
            label: label.into(),
            visible: true,
            bold: false,
            sort_order: fields.len() as i64 + 1,
        });
    }
    if sample {
        fields.push(NotificationTemplateField {
            key: "attachment_count".into(),
            label: "附件数".into(),
            visible: true,
            bold: false,
            sort_order: fields.len() as i64 + 1,
        });
    }
    if !rejected {
        fields.push(NotificationTemplateField {
            key: "status".into(),
            label: "状态".into(),
            visible: true,
            bold: false,
            sort_order: fields.len() as i64 + 1,
        });
    }
    NotificationTemplate {
        key: key.into(),
        name: if rejected {
            "研发送样驳回通知"
        } else if resubmitted {
            "研发送样退回后重新提交通知"
        } else if sample {
            "样品信息登记通知"
        } else {
            "研发送样通知"
        }
        .into(),
        title: if rejected {
            "【{{检测类型}}】研发送样记录被驳回"
        } else if resubmitted {
            "【{{检测类型}}】退回研发送样记录已重新提交"
        } else if sample {
            "新样品待取样"
        } else {
            "【{{检测类型}}】新研发送样记录"
        }
        .into(),
        fields,
        available_fields: Vec::new(),
        footer: if rejected {
            "请根据完整驳回原因处理后重新提交。".into()
        } else if resubmitted {
            "退回记录已修改并重新提交，请分析检测人员进入系统处理。".into()
        } else {
            "请分析检测人员进入系统处理。".into()
        },
    }
}

pub fn sample_type_template_key(type_key: &str) -> String {
    if type_key.trim().is_empty() {
        TEMPLATE_SAMPLE_INFO.to_string()
    } else {
        format!("sample_info:{}", type_key.trim())
    }
}

pub fn load_template(pool: &crate::db::DbPool, key: &str) -> Result<NotificationTemplate> {
    let mut default = default_template(key);
    let sample_type_key = key.strip_prefix("sample_info:");
    if let Some(type_key) = sample_type_key {
        if let Ok(types) = crate::repo::sample_info_type_repo::list_all(pool) {
            if let Some(item) = types.into_iter().find(|item| item.type_key == type_key) {
                default.name = format!("样品登记 · {}", item.label);
            }
        }
    }
    let configured = notification_repo::get_template(pool, key).or_else(|_| {
        notification_repo::get_template(
            pool,
            if sample_type_key.is_some() {
                TEMPLATE_SAMPLE_INFO
            } else {
                key
            },
        )
    });
    let Ok((title, footer, config)) = configured else {
        let candidates = notification_repo::template_available_fields(
            pool,
            sample_type_key,
            form_setting_keys_for_template(key),
            key,
        )?;
        default.fields = candidates
            .into_iter()
            .filter_map(|item| {
                let key = item.get("key")?.as_str()?.to_string();
                let sort_order = item
                    .get("sort_order")
                    .and_then(|value| value.as_i64())
                    .unwrap_or(i64::MAX);
                Some(NotificationTemplateField {
                    label: item
                        .get("label")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&key)
                        .to_string(),
                    key,
                    visible: true,
                    bold: false,
                    sort_order,
                })
            })
            .filter(|field| !field.key.trim().is_empty())
            .collect();
        default.fields.sort_by_key(|field| field.sort_order);
        return Ok(default);
    };
    let mut template = default;
    template.title = title;
    template.footer = footer;
    let candidates = notification_repo::template_available_fields(
        pool,
        sample_type_key,
        form_setting_keys_for_template(key),
        key,
    )?
    .into_iter()
    .filter_map(|item| {
        Some((
            item.get("key")?.as_str()?.to_string(),
            item.get("label")?.as_str()?.to_string(),
            item.get("sort_order")
                .and_then(|value| value.as_i64())
                .unwrap_or(i64::MAX),
        ))
    })
    .collect::<Vec<_>>();
    let allowed: HashSet<String> = candidates
        .iter()
        .map(|(field_key, _, _)| field_key.clone())
        .collect();
    let mut enabled = Vec::new();
    let mut available = Vec::new();
    let mut configured_keys = HashSet::new();
    if let Some(items) = config.as_array() {
        for (index, item) in items.iter().enumerate() {
            let Some(field_key) = item.get("key").and_then(|value| value.as_str()) else {
                continue;
            };
            if !allowed.contains(field_key) {
                continue;
            }
            configured_keys.insert(field_key.to_string());
            let source_sort_order =
                if key == TEMPLATE_PERSONNEL_CHANGE || key == TEMPLATE_PERSONNEL_CHANGE_REJECTED {
                    candidates
                        .iter()
                        .find(|(candidate_key, _, _)| candidate_key == field_key)
                        .map(|(_, _, sort_order)| *sort_order)
                } else {
                    None
                };
            let field = NotificationTemplateField {
                key: field_key.to_string(),
                label: item
                    .get("label")
                    .and_then(|value| value.as_str())
                    .unwrap_or(field_key)
                    .to_string(),
                visible: item
                    .get("visible")
                    .or_else(|| item.get("enabled"))
                    .and_then(|value| value.as_bool())
                    .unwrap_or(true),
                bold: item
                    .get("bold")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false),
                sort_order: source_sort_order.unwrap_or_else(|| {
                    item.get("sort_order")
                        .and_then(|value| value.as_i64())
                        .unwrap_or(index as i64 + 1)
                }),
            };
            if field.visible {
                enabled.push(field);
            } else {
                available.push(field);
            }
        }
    }
    if enabled.is_empty() && available.is_empty() {
        enabled = if candidates.is_empty() {
            template.fields
        } else {
            candidates
                .iter()
                .map(|(field_key, label, sort_order)| NotificationTemplateField {
                    key: field_key.clone(),
                    label: label.clone(),
                    visible: true,
                    bold: false,
                    sort_order: *sort_order,
                })
                .collect()
        };
    }
    for (field_key, label, source_sort_order) in candidates {
        if configured_keys.contains(&field_key)
            || enabled.iter().any(|field| field.key == field_key)
        {
            continue;
        }
        let auto_notify =
            key == TEMPLATE_PERSONNEL_CHANGE || key == TEMPLATE_PERSONNEL_CHANGE_REJECTED;
        let field = NotificationTemplateField {
            key: field_key,
            label,
            visible: auto_notify,
            bold: false,
            sort_order: source_sort_order,
        };
        if auto_notify {
            enabled.push(field);
        } else {
            available.push(field);
        }
    }
    enabled.sort_by_key(|field| field.sort_order);
    available.sort_by_key(|field| field.sort_order);
    template.fields = enabled;
    template.available_fields = available;
    template.key = key.into();
    Ok(template)
}

fn dingtalk_url(webhook: &str, secret: &str) -> String {
    if secret.trim().is_empty() {
        return webhook.to_string();
    }
    let timestamp = chrono::Utc::now().timestamp_millis().to_string();
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(format!("{}\n{}", timestamp, secret).as_bytes());
    let sign =
        url_escape::encode_component(&STANDARD.encode(mac.finalize().into_bytes())).into_owned();
    let joiner = if webhook.contains('?') { "&" } else { "?" };
    format!("{webhook}{joiner}timestamp={timestamp}&sign={sign}")
}

fn dingtalk_markdown(title: &str, text: &str) -> serde_json::Value {
    json!({ "msgtype":"markdown", "markdown": { "title": title, "text": text } })
}

fn send_dingtalk(
    webhook: &str,
    secret: &str,
    title: &str,
    text: &str,
) -> std::result::Result<String, String> {
    let response = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|e| e.to_string())?
        .post(dingtalk_url(webhook, secret))
        .json(&dingtalk_markdown(title, text))
        .send()
        .map_err(|e| e.to_string())?;
    let status = response.status();
    let body = response.text().unwrap_or_default();
    if !status.is_success() {
        return Err(format!(
            "HTTP {}: {}",
            status.as_u16(),
            body.chars().take(300).collect::<String>()
        ));
    }
    let success = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v.get("errcode").and_then(|x| x.as_i64()))
        .unwrap_or(-1)
        == 0;
    if success {
        Ok(body.chars().take(300).collect())
    } else {
        Err(body.chars().take(300).collect())
    }
}

fn sample_payload(record: &SampleInfoResponse) -> serde_json::Value {
    json!({
        "record_id": record.id,
        "business_no": record.business_no,
        "batch_no": record.batch_no,
        "lab_name": record.lab_name,
        "project_name": record.project_name,
        "submitted_by": record.user_name,
        "submitted_at": record.submitted_at,
        "detection_type": record.detection_type,
        "type_key": record.type_key,
        "method_name": "",
        "quantity": record.quantity.to_string(),
        "status": record.status,
        "main_components": record.main_components,
        "notes": record.notes,
        "detection_date": record.detection_date,
        "instrument_code": "",
        "instrument_type": "",
        "extra_fields": record.extra_fields.clone().unwrap_or_else(|| json!({})),
        "division_name": record.division_name.clone().unwrap_or_default(),
        "submitted_division_id": record.submitted_division_id,
        "execution_division_id": record.execution_division_id,
        "group_id": record.group_id,
    })
}

pub fn enqueue_sample_submitted(pool: &DbPool, record: &SampleInfoResponse) -> Result<()> {
    notification_repo::enqueue_event(pool, "sample_info", record.id, &sample_payload(record))
}

fn rd_payload(pool: &DbPool, record: &RdRecordResponse) -> serde_json::Value {
    let detection_type = record
        .method_type
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&record.instrument_type);
    let division_name = record
        .division_id
        .and_then(|id| division_repo::get_by_id(pool, id).ok())
        .map(|division| division.name)
        .unwrap_or_default();
    json!({
        "record_id": record.id,
        "business_no": record.business_no,
        "batch_no": record.batch_no.clone().unwrap_or_default(),
        "lab_name": record.group_name,
        "project_name": record.project_name,
        "submitted_by": record.user_name,
        "submitted_at": record.recorded_at,
        "detection_type": detection_type,
        "instrument_type": record.instrument_type,
        "instrument_code": record.instrument_code,
        "method_name": record.method_name.clone().unwrap_or_default(),
        "quantity": record.quantity.to_string(),
        "status": record.status,
        "division_name": division_name,
        "division_id": record.division_id,
        "group_id": record.group_id,
        "target_url": format!("/sample-records?record_id={}", record.id),
        "return_reason": record.return_reason,
        "returned_by": record.returned_by,
        "returned_at": record.returned_at,
        "extra_fields": record.extra_fields.clone().unwrap_or_else(|| json!({})),
    })
}

pub fn enqueue_rd_submitted(pool: &DbPool, record: &RdRecordResponse) -> Result<()> {
    notification_repo::enqueue_event(pool, "rd_work_record", record.id, &rd_payload(pool, record))
}

pub fn enqueue_rd_rejected(pool: &DbPool, record: &RdRecordResponse) -> Result<()> {
    notification_repo::enqueue_event_with_type(
        pool,
        EVENT_RD_REJECTED,
        "rd_work_record_rejected",
        record.id,
        &rd_payload(pool, record),
    )
}

pub fn enqueue_rd_resubmitted(pool: &DbPool, record: &RdRecordResponse) -> Result<()> {
    notification_repo::enqueue_event_with_type(
        pool,
        EVENT_RD_RESUBMITTED,
        "rd_work_record_resubmitted",
        record.id,
        &rd_payload(pool, record),
    )
}

pub fn enqueue_personnel_change(
    pool: &DbPool,
    notice_no: &str,
    payload: &serde_json::Value,
) -> Result<()> {
    let id = notice_no
        .bytes()
        .fold(0_i64, |hash, byte| {
            hash.wrapping_mul(31).wrapping_add(byte as i64)
        })
        .abs();
    notification_repo::enqueue_event_with_type(
        pool,
        EVENT_PERSONNEL_CHANGE,
        "personnel_change_feedback",
        id,
        payload,
    )
}

pub fn enqueue_personnel_change_rejected(
    pool: &DbPool,
    notice_no: &str,
    payload: &serde_json::Value,
) -> Result<()> {
    let id = notice_no
        .bytes()
        .fold(0_i64, |hash, byte| {
            hash.wrapping_mul(31).wrapping_add(byte as i64)
        })
        .abs();
    notification_repo::enqueue_event_with_type(
        pool,
        EVENT_PERSONNEL_CHANGE_REJECTED,
        "personnel_change_feedback_rejected",
        id,
        payload,
    )
}

fn field(payload: &serde_json::Value, key: &str) -> String {
    payload
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

fn configured_field_value(payload: &serde_json::Value, key: &str) -> String {
    if let Some(value) = payload.get(key) {
        return match value {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Null => String::new(),
            other => other.to_string(),
        };
    }
    payload
        .get("extra_fields")
        .and_then(|fields| fields.get(key))
        .map(|value| match value {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Null => String::new(),
            other => other.to_string(),
        })
        .unwrap_or_default()
}

fn title_placeholder_value(payload: &serde_json::Value, key: &str) -> String {
    let field_key = match key.trim() {
        "检测类型" => "detection_type",
        "仪器类型" => "instrument_type",
        "仪器编号" => "instrument_code",
        "项目" => "project_name",
        "实验室" => "lab_name",
        "送样人" => "submitted_by",
        "送样时间" => "submitted_at",
        "部门" => "division_name",
        "批号" => "batch_no",
        "送样编号" => "business_no",
        "方法" => "method_name",
        "数量" => "quantity",
        "驳回时间" => "returned_at",
        "驳回人" => "returned_by",
        "驳回原因" => "return_reason",
        "当前状态" => "status",
        "通知编号" => "notice_no",
        "提交时间" => "created_at",
        "提交人" => "submitted_by",
        "变动类型" => "change_type",
        "实验室负责人" => "lab_leader",
        "人员" => "person_name",
        "生效日期" => "effective_at",
        "高新项目" => "is_high_tech",
        "高新项目名称" => "high_tech_name",
        "变动状态" => "status",
        "审批人" => "reviewed_by",
        "审批时间" => "reviewed_at",
        other => other,
    };
    configured_field_value(payload, field_key)
}

fn render_template_text(template: &str, payload: &serde_json::Value) -> String {
    let token = Regex::new(r"\{\{\s*([^{}]+?)\s*\}\}").expect("title token pattern");
    let rendered = token.replace_all(template, |captures: &regex::Captures<'_>| {
        title_placeholder_value(payload, &captures[1])
    });
    rendered
        .replace("【】", "")
        .replace("[]", "")
        .replace("()", "")
        .trim()
        .to_string()
}

fn render_template_title(template: &str, payload: &serde_json::Value) -> String {
    render_template_text(template, payload)
}

fn should_dispatch_channel(dispatched_channels: &mut HashSet<i64>, channel_id: i64) -> bool {
    dispatched_channels.insert(channel_id)
}

fn notification_text(payload: &serde_json::Value) -> (String, String, String) {
    let batch = field(payload, "batch_no");
    let business = field(payload, "business_no");
    let lab = field(payload, "lab_name");
    let project = field(payload, "project_name");
    let sender = field(payload, "submitted_by");
    let submitted = field(payload, "submitted_at");
    let division = field(payload, "division_name");
    let detection_type = field(payload, "detection_type");
    let method = field(payload, "method_name");
    let quantity = field(payload, "quantity");
    let title = "新送样待取样".to_string();
    if payload.get("notice_no").is_some() {
        let rejected = field(payload, "status") == "已驳回";
        let title = if rejected {
            "人员通知反馈驳回"
        } else {
            "人员变动反馈通知"
        };
        let body = format!(
            "### 【{}】\n\n- 通知编号：{}\n- 变动类型：{}\n- 实验室：{}\n- 人员：{}\n- 驳回原因：{}",
            title,
            field(payload, "notice_no"),
            field(payload, "change_type"),
            lab,
            field(payload, "person_name"),
            field(payload, "review_reason"),
        );
        return (
            title.into(),
            body,
            format!("{} · {}", title, field(payload, "notice_no")),
        );
    }
    let body = format!(
        "### 【新送样待取样】\n\n- 批号：{}\n- 送样编号：{}\n- 送样时间：{}\n- 送样人：{}\n- 部门：{}\n- 实验室：{}\n- 项目：{}\n- 检测类型：{}\n- 方法：{}\n- 数量：{}\n- 状态：待取样\n\n请分析检测人员进入系统处理。",
        batch,
        business,
        submitted,
        sender,
        if division.is_empty() { "未分配" } else { &division },
        lab,
        project,
        if detection_type.is_empty() { "未填写" } else { &detection_type },
        if method.is_empty() { "未填写" } else { &method },
        if quantity.is_empty() { "未填写" } else { &quantity },
    );
    let body = payload
        .get("attachment_count")
        .and_then(|value| value.as_u64())
        .map(|count| format!("{body}\n- 附件：{count} 个"))
        .unwrap_or(body);
    let in_app = format!("{} · {} · {}", batch, lab, project);
    (title, body, in_app)
}

fn notification_text_with_template(
    pool: &DbPool,
    payload: &serde_json::Value,
    template_key: &str,
) -> (String, String, String) {
    let (title, body, in_app) = notification_text(payload);
    if let Ok(template) = load_template(pool, template_key) {
        let mut fields = template.fields;
        fields.sort_by_key(|field| field.sort_order);
        let resolved_title = render_template_title(&template.title, payload);
        let display_title = if resolved_title.is_empty() {
            &title
        } else {
            &resolved_title
        };
        let markdown_title = if display_title.contains('【') || display_title.contains('】') {
            display_title.to_string()
        } else {
            format!("【{display_title}】")
        };
        let mut lines = vec![format!("### {markdown_title}")];
        for item in fields {
            if !item.visible {
                continue;
            }
            let value = configured_field_value(payload, &item.key);
            if value.trim().is_empty() {
                continue;
            }
            if item.key == "return_reason" {
                lines.push(if item.bold {
                    format!("- **{}：**\n\n{}", item.label, value)
                } else {
                    format!("- {}：\n\n{}", item.label, value)
                });
            } else {
                let text = format!("{}：{}", item.label, value);
                lines.push(if item.bold {
                    format!("- **{}**", text)
                } else {
                    format!("- {}", text)
                });
            }
        }
        if !template.footer.trim().is_empty() {
            let footer = render_template_text(&template.footer, payload);
            if !footer.trim().is_empty() {
                lines.push(format!("\n{}", footer));
            }
        }
        return (
            if resolved_title.is_empty() {
                title
            } else {
                resolved_title
            },
            lines.join("\n"),
            in_app,
        );
    }
    (title, body, in_app)
}

pub fn process_pending(pool: &DbPool, limit: i64) -> Result<usize> {
    let events = notification_repo::claim_pending_events(pool, limit)?;
    let mut handled = 0;
    for (event_id, event_type, resource_type, resource_id, mut payload) in events {
        if resource_type == "sample_info" {
            let attachment_count =
                sample_info_attachment_repo::list_by_record(pool, resource_id)?.len();
            payload["attachment_count"] = json!(attachment_count);
        }
        let group_id = payload.get("group_id").and_then(|v| v.as_i64());
        let project = field(&payload, "project_name");
        let detection_type = field(&payload, "detection_type");
        let instrument_type = field(&payload, "instrument_type");
        let sample_info_type_key = field(&payload, "type_key");
        let rules = notification_repo::matching_rules(
            pool,
            &event_type,
            &resource_type,
            group_id,
            &project,
            &detection_type,
            &instrument_type,
            &sample_info_type_key,
        )?;
        if rules.is_empty() {
            // The record was saved successfully, but no notification destination exists yet.
            notification_repo::set_event_status(pool, event_id, "skipped", false)?;
            handled += 1;
            continue;
        }
        let template_key = if event_type == EVENT_PERSONNEL_CHANGE_REJECTED {
            TEMPLATE_PERSONNEL_CHANGE_REJECTED.to_string()
        } else if event_type == EVENT_PERSONNEL_CHANGE {
            TEMPLATE_PERSONNEL_CHANGE.to_string()
        } else if event_type == EVENT_RD_REJECTED {
            TEMPLATE_RD_REJECTED.to_string()
        } else if event_type == EVENT_RD_RESUBMITTED {
            TEMPLATE_RD_RESUBMITTED.to_string()
        } else if resource_type == "rd_work_record" {
            TEMPLATE_RD.to_string()
        } else {
            sample_type_template_key(&sample_info_type_key)
        };
        let (title, markdown, in_app_body) =
            notification_text_with_template(pool, &payload, &template_key);
        let target = payload
            .get("target_url")
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| {
                format!(
                    "/sample-info/entry?record_id={}",
                    payload
                        .get("record_id")
                        .and_then(|value| value.as_i64())
                        .unwrap_or(0)
                )
            });
        let mut retry_needed = false;
        let mut dispatched_channels = HashSet::new();
        for rule in rules {
            if event_type != EVENT_RD_REJECTED {
                let notification_division_id = payload
                    .get("execution_division_id")
                    .or_else(|| payload.get("division_id"))
                    .and_then(|value| value.as_i64());
                let recipients = if event_type == EVENT_PERSONNEL_CHANGE_REJECTED {
                    if rule.recipient_user_ids.is_empty() {
                        payload
                            .get("submitted_by_user_id")
                            .and_then(|value| value.as_i64())
                            .map(|user_id| notification_repo::active_user_ids_for(pool, &[user_id]))
                            .transpose()?
                            .unwrap_or_default()
                    } else {
                        notification_repo::active_user_ids_for(pool, &rule.recipient_user_ids)?
                    }
                } else if rule.recipient_user_ids.is_empty() {
                    notification_repo::analysis_user_ids_for_work_division(
                        pool,
                        notification_division_id,
                        None,
                    )?
                } else {
                    notification_repo::analysis_user_ids_for_work_division(
                        pool,
                        notification_division_id,
                        Some(&rule.recipient_user_ids),
                    )?
                };
                notification_repo::create_in_app(
                    pool,
                    event_id,
                    &recipients,
                    &title,
                    &in_app_body,
                    &target,
                )?;
            }
            if let Some(channel_id) = rule.channel_id {
                if !should_dispatch_channel(&mut dispatched_channels, channel_id) {
                    continue;
                }
                let attempts = notification_repo::delivery_attempts(pool, event_id, channel_id)?;
                if attempts >= MAX_ATTEMPTS {
                    continue;
                }
                match notification_repo::get_channel_secret(pool, channel_id).and_then(
                    |(_, webhook, secret)| {
                        send_dingtalk(&webhook, &secret, &title, &markdown)
                            .map_err(crate::error::AppError::Internal)
                    },
                ) {
                    Ok(response) => notification_repo::record_delivery(
                        pool,
                        event_id,
                        channel_id,
                        "sent",
                        attempts + 1,
                        &response,
                        "",
                        false,
                    )?,
                    Err(error) => {
                        let next_attempt = attempts + 1;
                        let retry = next_attempt < MAX_ATTEMPTS;
                        notification_repo::record_delivery(
                            pool,
                            event_id,
                            channel_id,
                            if retry { "retry" } else { "failed" },
                            next_attempt,
                            "",
                            &error.to_string(),
                            retry,
                        )?;
                        retry_needed |= retry;
                    }
                }
            }
        }
        notification_repo::set_event_status(
            pool,
            event_id,
            if retry_needed { "retry" } else { "sent" },
            retry_needed,
        )?;
        handled += 1;
    }
    Ok(handled)
}

pub fn test_channel(pool: &DbPool, channel_id: i64) -> Result<()> {
    let (name, webhook, secret) = notification_repo::get_channel_secret(pool, channel_id)?;
    let body = "### 【样品管理系统】通知渠道测试\n\n该消息用于验证钉钉机器人配置，不产生业务记录。";
    send_dingtalk(&webhook, &secret, "通知渠道测试", body).map_err(|e| {
        crate::error::AppError::Validation(format!("渠道「{}」发送失败: {}", name, e))
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        default_template, dingtalk_url, notification_text, notification_text_with_template,
        render_template_title, should_dispatch_channel, TEMPLATE_PERSONNEL_CHANGE_REJECTED,
        TEMPLATE_RD_REJECTED, TEMPLATE_SAMPLE_INFO,
    };
    use crate::repo::notification_repo;
    use serde_json::json;
    use std::collections::HashSet;

    #[test]
    fn signing_adds_timestamp_and_signature_without_changing_base_url() {
        assert_eq!(
            dingtalk_url("https://example.test/robot", ""),
            "https://example.test/robot"
        );
        let signed = dingtalk_url("https://example.test/robot?access_token=x", "SECtest");
        assert!(signed.contains("access_token=x&timestamp="));
        assert!(signed.contains("&sign="));
    }

    #[test]
    fn sample_notification_includes_attachment_count_only_when_supplied() {
        let payload = json!({
            "batch_no": "B-001",
            "business_no": "SI-001",
            "lab_name": "实验室A",
            "project_name": "项目A",
            "submitted_by": "测试人员",
            "submitted_at": "2026-07-23 10:00:00",
            "division_name": "部门A",
            "detection_type": "液相",
            "method_name": "",
            "quantity": "1",
            "attachment_count": 2
        });
        let (_, markdown, _) = notification_text(&payload);
        assert!(markdown.contains("附件：2 个"));

        let (_, markdown_without_attachment, _) = notification_text(&json!({}));
        assert!(!markdown_without_attachment.contains("附件："));
    }

    #[test]
    fn template_title_replaces_detection_type_and_standard_aliases() {
        let payload = json!({
            "detection_type": "液相",
            "project_name": "项目A",
            "batch_no": "B-001"
        });
        assert_eq!(
            render_template_title("【{{检测类型}}】{{项目}}送样（{{batch_no}}）", &payload),
            "【液相】项目A送样（B-001）"
        );
        assert_eq!(
            render_template_title("【{{检测类型}}】新送样", &json!({})),
            "新送样"
        );
    }

    #[test]
    fn rejection_template_keeps_the_complete_multiline_reason() {
        let pool = crate::db::init_pool("postgres-test");
        crate::db::test_migrations::run(&pool.get().unwrap()).unwrap();
        let reason = "批号与送样单不一致\n请补充第二页检测条件，不要删除原附件。";
        notification_repo::save_template(
            &pool,
            TEMPLATE_RD_REJECTED,
            "驳回通知",
            "请按原因处理",
            &json!([{"key":"return_reason","label":"完整驳回原因","visible":true,"bold":true,"sort_order":1}]),
        )
        .unwrap();
        let (_, markdown, _) = notification_text_with_template(
            &pool,
            &json!({"return_reason": reason}),
            TEMPLATE_RD_REJECTED,
        );
        assert!(markdown.contains(reason));
        assert!(markdown.contains("完整驳回原因"));
    }

    #[test]
    fn default_rejection_template_exposes_rejection_fields() {
        let template = default_template(TEMPLATE_RD_REJECTED);
        assert_eq!(template.name, "研发送样驳回通知");
        assert!(template
            .fields
            .iter()
            .any(|field| field.key == "return_reason"));
        assert!(template
            .fields
            .iter()
            .any(|field| field.key == "returned_at"));
        assert!(template
            .fields
            .iter()
            .any(|field| field.key == "returned_by"));
    }

    #[test]
    fn personnel_rejection_template_exposes_reason_and_notice_number() {
        let template = default_template(TEMPLATE_PERSONNEL_CHANGE_REJECTED);
        assert_eq!(template.name, "人员通知反馈驳回");
        assert!(template.title.contains("通知编号"));
        // Personnel-feedback fields come from the runtime configuration file;
        // defaults must not resurrect the removed hard-coded field list.
        assert!(template.fields.is_empty());
        assert!(template.available_fields.is_empty());
    }

    #[test]
    fn configured_markdown_resolves_title_and_footer_tokens() {
        let pool = crate::db::init_pool("postgres-test");
        crate::db::test_migrations::run(&pool.get().unwrap()).unwrap();
        notification_repo::save_template(
            &pool,
            TEMPLATE_SAMPLE_INFO,
            "【{{检测类型}}】{{项目}}送样通知",
            "请{{部门}}及时处理",
            &json!([]),
        )
        .unwrap();
        let payload =
            json!({"detection_type":"液相","project_name":"项目A","division_name":"研究院"});
        let (title, markdown, _) =
            notification_text_with_template(&pool, &payload, TEMPLATE_SAMPLE_INFO);
        assert_eq!(title, "【液相】项目A送样通知");
        assert!(markdown.contains("### 【液相】项目A送样通知"));
        assert!(markdown.ends_with("请研究院及时处理"));
        assert!(!markdown.contains("{{"));
    }

    #[test]
    fn one_event_only_dispatches_each_channel_once() {
        let mut dispatched = HashSet::new();
        assert!(should_dispatch_channel(&mut dispatched, 7));
        assert!(!should_dispatch_channel(&mut dispatched, 7));
        assert!(should_dispatch_channel(&mut dispatched, 8));
    }

    #[test]
    fn configured_title_notes_and_footer_are_used_in_sent_markdown() {
        let pool = crate::db::init_pool("postgres-test");
        crate::db::test_migrations::run(&pool.get().unwrap()).unwrap();
        let visible_record_columns: i64 = pool
            .get()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM rd_record_columns WHERE name IN ('batch_no','instrument_code') AND show_in_list=1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(visible_record_columns, 2);
        notification_repo::save_template(
            &pool,
            TEMPLATE_SAMPLE_INFO,
            "自定义通知标题",
            "自定义底部提示",
            &json!([
                {"key":"notes","label":"通知备注","visible":true,"bold":true,"sort_order":1},
                {"key":"quantity","label":"数量","visible":true,"bold":false,"sort_order":2}
            ]),
        )
        .unwrap();

        let payload = json!({"notes":"请优先处理","quantity":"3"});
        let (title, markdown, _) =
            notification_text_with_template(&pool, &payload, TEMPLATE_SAMPLE_INFO);
        assert_eq!(title, "自定义通知标题");
        assert!(markdown.contains("### 【自定义通知标题】"));
        assert!(markdown.contains("**通知备注：请优先处理**"));
        assert!(markdown.contains("数量：3"));
        assert!(markdown.ends_with("自定义底部提示"));
    }

    #[test]
    fn sample_type_templates_only_offer_fields_enabled_for_that_type() {
        let pool = crate::db::init_pool("postgres-test");
        crate::db::test_migrations::run(&pool.get().unwrap()).unwrap();
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO sample_info_columns(field_key,label,data_type,is_predefined,is_required,is_active,width,sort_order,show_in_list,show_in_export,show_in_form) VALUES('icp_only_field','ICP专用字段','text',0,0,1,100,99,1,1,1)",
            [],
        )
        .unwrap();
        let column_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO sample_info_column_visibility(type_key,column_id,is_visible,is_required,show_in_form,show_in_list,show_in_export,sort_order) VALUES('icp',?1,1,0,1,1,1,99)",
            [column_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO sample_info_column_visibility(type_key,column_id,is_visible,is_required,show_in_form,show_in_list,show_in_export,sort_order) VALUES('thermal',?1,0,0,0,0,0,99)",
            [column_id],
        )
        .unwrap();

        let icp = super::load_template(&pool, "sample_info:icp").unwrap();
        let thermal = super::load_template(&pool, "sample_info:thermal").unwrap();
        assert!(icp
            .available_fields
            .iter()
            .any(|field| field.key == "icp_only_field"));
        assert!(!thermal
            .fields
            .iter()
            .any(|field| field.key == "icp_only_field"));

        notification_repo::save_template(
            &pool,
            "sample_info:icp",
            "ICP独立通知",
            "ICP底部提示",
            &json!([{"key":"icp_only_field","label":"专用值","visible":true,"bold":true,"sort_order":1}]),
        )
        .unwrap();
        let payload = json!({"extra_fields":{"icp_only_field":"100 mg/L"}});
        let (title, markdown, _) =
            notification_text_with_template(&pool, &payload, "sample_info:icp");
        assert_eq!(title, "ICP独立通知");
        assert!(markdown.contains("**专用值：100 mg/L**"));
        assert!(markdown.ends_with("ICP底部提示"));
    }
}
