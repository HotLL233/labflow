use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::notification::{NotificationTemplate, NotificationTemplateField};
use crate::models::rd_record::RdRecordResponse;
use crate::models::sample_info::SampleInfoResponse;
use crate::repo::{division_repo, notification_repo, sample_info_attachment_repo};
use base64::{engine::general_purpose::STANDARD, Engine};
use hmac::{Hmac, Mac};
use regex::Regex;
use serde_json::json;
use sha2::Sha256;
use std::collections::{BTreeMap, BTreeSet, HashSet};

pub(crate) const EVENT_SAMPLE_SUBMITTED: &str = "sample_submitted";
pub(crate) const EVENT_RD_REJECTED: &str = "rd_record_rejected";
pub(crate) const EVENT_RD_RESUBMITTED: &str = "rd_record_resubmitted";
pub(crate) const EVENT_PERSONNEL_CHANGE: &str = "personnel_change_feedback";
pub(crate) const EVENT_PERSONNEL_CHANGE_REJECTED: &str = "personnel_change_feedback_rejected";
pub(crate) const EVENT_PERSONNEL_CHANGE_APPROVED: &str = "personnel_change_feedback_approved";
pub const TEMPLATE_RD: &str = "rd_work_record";
pub const TEMPLATE_RD_REJECTED: &str = "rd_work_record_rejected";
pub const TEMPLATE_RD_RESUBMITTED: &str = "rd_work_record_resubmitted";
pub const TEMPLATE_SAMPLE_INFO: &str = "sample_info";
pub const TEMPLATE_PERSONNEL_CHANGE: &str = "personnel_change_feedback";
pub const TEMPLATE_PERSONNEL_CHANGE_REJECTED: &str = "personnel_change_feedback_rejected";
pub const TEMPLATE_PERSONNEL_CHANGE_APPROVED: &str = "personnel_change_feedback_approved";
const MAX_ATTEMPTS: i64 = 3;

pub fn form_setting_keys_for_template(key: &str) -> &'static [&'static str] {
    if is_personnel_event(key) {
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
            source_label: String::new(),
            follow_source_label: Some(true),
            visible: true,
            bold: false,
            sort_order: index as i64 + 1,
        })
        .collect()
}

pub fn default_template(key: &str) -> NotificationTemplate {
    if is_personnel_event(key) {
        let rejected = key == TEMPLATE_PERSONNEL_CHANGE_REJECTED;
        let approved = key == TEMPLATE_PERSONNEL_CHANGE_APPROVED;
        return NotificationTemplate {
            key: key.into(),
            name: if rejected {
                "人员通知反馈驳回"
            } else if approved {
                "人员变动审批通过"
            } else {
                "人员变动反馈通知"
            }
            .into(),
            title: if rejected {
                "人员通知反馈驳回：{{通知编号}} · {{实验室}}".into()
            } else if approved {
                "人员变动审批通过：{{通知编号}} · {{实验室}}".into()
            } else {
                "人员变动反馈：{{变动类型}} · {{实验室}}".into()
            },
            // Business fields follow fields.json; review metadata is supplied as event fields.
            fields: Vec::new(),
            available_fields: Vec::new(),
            footer: if rejected {
                "请根据驳回原因修改后重新发起人员变动通知。"
            } else if approved {
                "人员变动已审批通过，请查看通知详情。"
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
            source_label: String::new(),
            follow_source_label: Some(true),
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
            source_label: String::new(),
            follow_source_label: Some(true),
            visible: true,
            bold: false,
            sort_order: fields.len() as i64 + 1,
        });
    }
    if sample {
        fields.push(NotificationTemplateField {
            key: "attachment_count".into(),
            label: "附件数".into(),
            source_label: String::new(),
            follow_source_label: Some(true),
            visible: true,
            bold: false,
            sort_order: fields.len() as i64 + 1,
        });
    }
    if !rejected {
        fields.push(NotificationTemplateField {
            key: "status".into(),
            label: "状态".into(),
            source_label: String::new(),
            follow_source_label: Some(true),
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

fn is_personnel_event(key: &str) -> bool {
    matches!(
        key,
        EVENT_PERSONNEL_CHANGE | EVENT_PERSONNEL_CHANGE_REJECTED | EVENT_PERSONNEL_CHANGE_APPROVED
    )
}

pub fn sample_type_template_key(type_key: &str) -> String {
    if type_key.trim().is_empty() {
        TEMPLATE_SAMPLE_INFO.to_string()
    } else {
        format!("sample_info:{}", type_key.trim())
    }
}

pub fn load_template(pool: &DbPool, key: &str) -> Result<NotificationTemplate> {
    let mut template = default_template(key);
    let sample_type_key = key.strip_prefix("sample_info:");
    if let Some(type_key) = sample_type_key {
        if let Some(item) = crate::repo::sample_info_type_repo::list_all(pool)?
            .into_iter()
            .find(|item| item.type_key == type_key)
        {
            template.name = format!("样品登记 · {}", item.label);
        }
    }
    let configured = match notification_repo::get_template(pool, key) {
        Err(AppError::NotFound(_)) if sample_type_key.is_some() => {
            notification_repo::get_template(pool, TEMPLATE_SAMPLE_INFO)
        }
        result => result,
    };
    let config = match configured {
        Ok((title, footer, config)) => {
            template.title = title;
            template.footer = footer;
            Some(config)
        }
        Err(AppError::NotFound(_)) => None,
        Err(error) => return Err(error),
    };
    let candidates = notification_repo::template_available_fields(
        pool,
        sample_type_key,
        form_setting_keys_for_template(key),
        key,
    )?;
    let legacy_defaults = default_template(key);
    let mut fields = Vec::new();
    let mut available = Vec::new();
    let mut configured_keys = HashSet::new();
    if let Some(items) = config.as_ref().and_then(|value| value.as_array()) {
        for (index, item) in items.iter().enumerate() {
            let Some(stored_key) = item.get("key").and_then(|value| value.as_str()) else {
                continue;
            };
            let field_key = notification_repo::canonical_template_field_key(key, stored_key);
            let Some(candidate) = candidates.iter().find(|value| value["key"] == field_key) else {
                continue;
            };
            if !configured_keys.insert(field_key.to_string()) {
                continue;
            }
            let source_label = candidate["label"].as_str().unwrap_or(field_key).to_string();
            let stored_label = item
                .get("label")
                .and_then(|value| value.as_str())
                .unwrap_or(&source_label);
            // Old default labels follow the source; distinct administrator labels remain independent.
            let follow_source_label = item
                .get("follow_source_label")
                .and_then(|value| value.as_bool())
                .unwrap_or_else(|| {
                    stored_label == source_label
                        || legacy_defaults
                            .fields
                            .iter()
                            .any(|field| field.key == field_key && field.label == stored_label)
                });
            let field = NotificationTemplateField {
                key: field_key.into(),
                label: if follow_source_label {
                    source_label.clone()
                } else {
                    stored_label.into()
                },
                source_label,
                follow_source_label: Some(follow_source_label),
                visible: item
                    .get("visible")
                    .or_else(|| item.get("enabled"))
                    .and_then(|value| value.as_bool())
                    .unwrap_or(true),
                bold: item
                    .get("bold")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false),
                sort_order: item
                    .get("sort_order")
                    .and_then(|value| value.as_i64())
                    .unwrap_or(index as i64 + 1),
            };
            if field.visible {
                fields.push(field);
            } else {
                available.push(field);
            }
        }
    }
    let mut next_order = fields
        .iter()
        .chain(available.iter())
        .map(|field| field.sort_order)
        .max()
        .unwrap_or(0);
    for candidate in candidates {
        let Some(field_key) = candidate["key"].as_str() else {
            continue;
        };
        if configured_keys.contains(field_key) {
            continue;
        }
        next_order += 1;
        let label = candidate["label"].as_str().unwrap_or(field_key).to_string();
        let visible = config.is_none() || is_personnel_event(key);
        let field = NotificationTemplateField {
            key: field_key.into(),
            label: label.clone(),
            source_label: label,
            follow_source_label: Some(true),
            visible,
            bold: false,
            sort_order: next_order,
        };
        if visible {
            fields.push(field);
        } else {
            available.push(field);
        }
    }
    fields.sort_by_key(|field| field.sort_order);
    available.sort_by_key(|field| field.sort_order);
    template.fields = fields;
    template.available_fields = available;
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

fn dingtalk_markdown(title: &str, text: &str, mobiles: &[String]) -> serde_json::Value {
    let mut message = json!({ "msgtype":"markdown", "markdown": { "title": title, "text": text } });
    if !mobiles.is_empty() {
        let mobiles = mobiles.iter().collect::<BTreeSet<_>>();
        let mentions = mobiles
            .iter()
            .map(|mobile| format!("@{}", mobile))
            .collect::<Vec<_>>()
            .join(" ");
        message["markdown"]["text"] = json!(format!("{}\n\n提醒：{}", text, mentions));
        message["at"] = json!({ "atMobiles": mobiles, "isAtAll": false });
    }
    message
}

fn send_dingtalk(
    webhook: &str,
    secret: &str,
    title: &str,
    text: &str,
    mobiles: &[String],
) -> std::result::Result<String, String> {
    let response = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|e| e.to_string())?
        .post(dingtalk_url(webhook, secret))
        .json(&dingtalk_markdown(title, text, mobiles))
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
        "quantity": record.quantity.to_string(),
        "status": record.status,
        "main_components": record.main_components,
        "notes": record.notes,
        "detection_date": record.detection_date,
        "sampled_by": record.sampled_by,
        "sampled_at": record.sampled_at,
        "detected_by": record.detected_by,
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
        "notes": record.notes.clone().unwrap_or_default(),
        "sampling_person": record.sampler,
        "sampling_time": record.sampled_at,
        "detected_by": record.detected_by,
        "detected_at": record.detected_at,
        "high_item": record.high_item,
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
    let conn = pool.get()?;
    enqueue_personnel_change_on_conn(&conn, notice_no, payload)
}

pub fn enqueue_personnel_change_on_conn(
    conn: &postgres_compat::Connection,
    notice_no: &str,
    payload: &serde_json::Value,
) -> Result<()> {
    notification_repo::enqueue_personnel_event_on_conn(
        conn,
        EVENT_PERSONNEL_CHANGE,
        notice_no,
        payload,
    )
}

pub fn enqueue_personnel_change_rejected(
    pool: &DbPool,
    notice_no: &str,
    payload: &serde_json::Value,
) -> Result<()> {
    let conn = pool.get()?;
    enqueue_personnel_change_rejected_on_conn(&conn, notice_no, payload)
}

pub fn enqueue_personnel_change_rejected_on_conn(
    conn: &postgres_compat::Connection,
    notice_no: &str,
    payload: &serde_json::Value,
) -> Result<()> {
    notification_repo::enqueue_personnel_event_on_conn(
        conn,
        EVENT_PERSONNEL_CHANGE_REJECTED,
        notice_no,
        payload,
    )
}

pub fn enqueue_personnel_change_approved(
    pool: &DbPool,
    notice_no: &str,
    payload: &serde_json::Value,
) -> Result<()> {
    let conn = pool.get()?;
    enqueue_personnel_change_approved_on_conn(&conn, notice_no, payload)
}

pub fn enqueue_personnel_change_approved_on_conn(
    conn: &postgres_compat::Connection,
    notice_no: &str,
    payload: &serde_json::Value,
) -> Result<()> {
    notification_repo::enqueue_personnel_event_on_conn(
        conn,
        EVENT_PERSONNEL_CHANGE_APPROVED,
        notice_no,
        payload,
    )
}

pub fn personnel_delivery_status(pool: &DbPool, notice_no: &str) -> Result<String> {
    notification_repo::personnel_delivery_status(pool, notice_no)
}

fn field(payload: &serde_json::Value, key: &str) -> String {
    payload
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

fn configured_field_value(payload: &serde_json::Value, key: &str) -> String {
    let extra = payload.get("extra_fields");
    let value = payload
        .get(key)
        .or_else(|| extra.and_then(|fields| fields.get(key)));
    let mut text = value.map(display_notification_value).unwrap_or_default();
    if !text.trim().is_empty() {
        if let Some(detail) = extra.and_then(|fields| fields.get(format!("{key}__detail"))) {
            let detail = display_notification_value(detail);
            if !detail.trim().is_empty() {
                text = format!("{text}（{detail}）");
            }
        }
    }
    text
}

fn display_notification_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Null => String::new(),
        serde_json::Value::Bool(value) => if *value { "是" } else { "否" }.into(),
        serde_json::Value::Array(values) => values
            .iter()
            .map(display_notification_value)
            .filter(|text| !text.trim().is_empty())
            .collect::<Vec<_>>()
            .join("、"),
        other => other.to_string(),
    }
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
        "注意事项" | "备注" => "notes",
        "送样编号" => "business_no",
        "方法" => "method_name",
        "数量" => "quantity",
        "驳回时间" => "returned_at",
        "驳回人" => "returned_by",
        "驳回原因" if payload.get("notice_no").is_some() => "review_reason",
        "驳回原因" => "return_reason",
        "审批意见" => "review_reason",
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
        "变动状态" | "审批状态" => "status",
        "修订次数" => "revision",
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
        } else if matches!(field(payload, "status").as_str(), "已同步" | "已通过") {
            "人员变动审批通过"
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

pub fn render_template_message(
    template: &NotificationTemplate,
    payload: &serde_json::Value,
    fallback_title: &str,
) -> (String, String) {
    let resolved_title = render_template_title(&template.title, payload);
    let title = if resolved_title.is_empty() {
        fallback_title.to_string()
    } else {
        resolved_title
    };
    let markdown_title = if title.contains('【') || title.contains('】') {
        title.clone()
    } else {
        format!("【{title}】")
    };
    let mut lines = vec![format!("### {markdown_title}")];
    let mut fields: Vec<_> = template
        .fields
        .iter()
        .filter(|field| field.visible)
        .collect();
    fields.sort_by_key(|field| field.sort_order);
    for item in fields {
        let value = configured_field_value(payload, &item.key);
        if value.trim().is_empty() {
            continue;
        }
        if item.key == "return_reason" || value.contains('\n') {
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
    let footer = render_template_text(&template.footer, payload);
    if !footer.trim().is_empty() {
        lines.push(format!("\n{footer}"));
    }
    (title, lines.join("\n"))
}

fn notification_text_with_template(
    pool: &DbPool,
    payload: &serde_json::Value,
    template_key: &str,
) -> Result<(String, String, String)> {
    let (title, _, in_app) = notification_text(payload);
    let mut template = load_template(pool, template_key)?;
    let event_type = if is_personnel_event(template_key) {
        payload.get("change_type").and_then(|value| value.as_str())
    } else if template_key.starts_with("sample_info") {
        template_key.strip_prefix("sample_info:")
    } else {
        payload
            .get("detection_type")
            .and_then(|value| value.as_str())
    };
    let mut candidates = notification_repo::template_available_fields(
        pool,
        event_type,
        form_setting_keys_for_template(template_key),
        template_key,
    )?;
    // Existing configurations use display labels; new ones may use the stable action key.
    if is_personnel_event(template_key) {
        if let Some(action) = payload["action_key"]
            .as_str()
            .filter(|action| Some(*action) != event_type)
        {
            for candidate in
                notification_repo::template_available_fields(pool, Some(action), &[], template_key)?
            {
                if !candidates
                    .iter()
                    .any(|field| field["key"] == candidate["key"])
                {
                    candidates.push(candidate);
                }
            }
        }
    }
    let allowed: HashSet<_> = candidates
        .iter()
        .filter_map(|item| item["key"].as_str())
        .collect();
    template
        .fields
        .retain(|field| allowed.contains(field.key.as_str()));
    let (title, markdown) = render_template_message(&template, payload, &title);
    Ok((title, markdown, in_app))
}

pub fn process_pending(pool: &DbPool, limit: i64) -> Result<usize> {
    process_pending_with_sender(pool, limit, &mut send_dingtalk)
}

fn process_pending_with_sender(
    pool: &DbPool,
    limit: i64,
    sender: &mut impl FnMut(&str, &str, &str, &str, &[String]) -> std::result::Result<String, String>,
) -> Result<usize> {
    let mut handled = 0;
    // Claim only work that is about to start. A slow batch must not expire unseen leases.
    for _ in 0..limit.clamp(0, 200) {
        let Some((event_id, event_type, resource_type, resource_id, mut payload, token)) =
            notification_repo::claim_pending_events(pool, 1)?.pop()
        else {
            break;
        };
        handled += 1;
        let result = (|| -> Result<()> {
            if resource_type == "sample_info" {
                payload["attachment_count"] =
                    json!(sample_info_attachment_repo::list_by_record(pool, resource_id)?.len());
            }
            let rules = notification_repo::matching_rules(
                pool,
                &event_type,
                &resource_type,
                payload["group_id"].as_i64(),
                &field(&payload, "project_name"),
                &field(&payload, "detection_type"),
                &field(&payload, "instrument_type"),
                &field(&payload, "type_key"),
            )?;
            let personnel = is_personnel_event(&event_type);
            let review_result = matches!(
                event_type.as_str(),
                EVENT_PERSONNEL_CHANGE_REJECTED | EVENT_PERSONNEL_CHANGE_APPROVED
            );
            if rules.is_empty() && !review_result {
                notification_repo::set_event_status(pool, event_id, &token, "skipped", false)?;
                return Ok(());
            }
            let template_key = match event_type.as_str() {
                EVENT_PERSONNEL_CHANGE_REJECTED => TEMPLATE_PERSONNEL_CHANGE_REJECTED.to_string(),
                EVENT_PERSONNEL_CHANGE_APPROVED => TEMPLATE_PERSONNEL_CHANGE_APPROVED.to_string(),
                EVENT_PERSONNEL_CHANGE => TEMPLATE_PERSONNEL_CHANGE.to_string(),
                EVENT_RD_REJECTED => TEMPLATE_RD_REJECTED.to_string(),
                EVENT_RD_RESUBMITTED => TEMPLATE_RD_RESUBMITTED.to_string(),
                _ if resource_type == "rd_work_record" => TEMPLATE_RD.to_string(),
                _ => sample_type_template_key(&field(&payload, "type_key")),
            };
            let (title, markdown, in_app_body) =
                notification_text_with_template(pool, &payload, &template_key)?;
            let target = notification_repo::notification_target_url(
                &resource_type,
                resource_id,
                &payload,
                payload["target_url"].as_str().unwrap_or_default(),
            );
            let actor_id = payload["actor_user_id"]
                .as_i64()
                .or_else(|| payload["submitted_by_user_id"].as_i64());
            let mut recipients = std::collections::BTreeSet::new();
            // Review outcomes always reach the original stable account, even without a rule.
            // A revoked/disabled account is still filtered through the actual detail boundary.
            if review_result {
                if let Some(id) = actor_id {
                    recipients.extend(notification_repo::personnel_recipient_ids(
                        pool,
                        actor_id,
                        Some(&[id]),
                    )?);
                }
            }
            let mandatory_recipients = recipients.clone();
            let mut channel_mentions: BTreeMap<i64, BTreeSet<i64>> = BTreeMap::new();
            for rule in &rules {
                if let Some(channel_id) = rule.channel_id {
                    channel_mentions
                        .entry(channel_id)
                        .or_default()
                        .extend(rule.dingtalk_mention_user_ids.iter().copied());
                }
                if event_type == EVENT_RD_REJECTED {
                    continue;
                }
                let candidates = if rule.recipient_user_ids.is_empty() {
                    None
                } else {
                    Some(rule.recipient_user_ids.as_slice())
                };
                let ids = if personnel {
                    if review_result && candidates.is_none() {
                        Vec::new()
                    } else {
                        notification_repo::personnel_recipient_ids(pool, actor_id, candidates)?
                    }
                } else {
                    notification_repo::analysis_user_ids_for_work_division(
                        pool,
                        payload
                            .get("execution_division_id")
                            .or_else(|| payload.get("division_id"))
                            .and_then(|value| value.as_i64()),
                        candidates,
                    )?
                };
                if rule.dingtalk_follow_recipients {
                    if let Some(channel_id) = rule.channel_id {
                        let targets = channel_mentions.entry(channel_id).or_default();
                        targets.extend(ids.iter().copied());
                        targets.extend(mandatory_recipients.iter().copied());
                    }
                }
                recipients.extend(ids);
            }
            notification_repo::renew_event_lease(pool, event_id, &token)?;
            notification_repo::create_in_app_for_claim(
                pool,
                event_id,
                &token,
                &recipients.into_iter().collect::<Vec<_>>(),
                &title,
                &in_app_body,
                &target,
            )?;
            let conn = pool.get()?;
            let mut succeeded: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM in_app_notifications WHERE event_id=?1)",
                [event_id],
                |row| row.get(0),
            )?;
            drop(conn);
            let mut failed = false;
            let mut retry = false;
            let mut dispatched_channels = HashSet::new();
            for rule in &rules {
                let Some(channel_id) = rule.channel_id else {
                    continue;
                };
                if !should_dispatch_channel(&mut dispatched_channels, channel_id) {
                    continue;
                }
                let (status, attempts) =
                    notification_repo::delivery_state(pool, event_id, channel_id)?;
                if status == "sent" {
                    succeeded = true;
                    continue;
                }
                if attempts >= MAX_ATTEMPTS {
                    failed = true;
                    continue;
                }
                notification_repo::renew_event_lease(pool, event_id, &token)?;
                let (mobiles, missing) = crate::repo::dingtalk_mention_repo::resolve(
                    pool,
                    channel_mentions
                        .get(&channel_id)
                        .unwrap_or(&BTreeSet::new()),
                )?;
                let response = notification_repo::get_channel_secret(pool, channel_id).and_then(
                    |(_, webhook, secret)| {
                        sender(&webhook, &secret, &title, &markdown, &mobiles)
                            .map_err(AppError::Internal)
                    },
                );
                notification_repo::renew_event_lease(pool, event_id, &token)?;
                match response {
                    Ok(mut response) => {
                        for mobile in &mobiles {
                            response = response.replace(mobile, "[手机号已隐藏]");
                        }
                        if !missing.is_empty() {
                            response.push_str(&format!("；以下账号未@：{}", missing.join("、")));
                        }
                        notification_repo::record_delivery(
                            pool,
                            event_id,
                            &token,
                            channel_id,
                            "sent",
                            attempts + 1,
                            &response,
                            "",
                            false,
                        )?;
                        succeeded = true;
                    }
                    Err(error) => {
                        let mut error_message = error.to_string();
                        for mobile in &mobiles {
                            error_message = error_message.replace(mobile, "[手机号已隐藏]");
                        }
                        let again = attempts + 1 < MAX_ATTEMPTS;
                        notification_repo::record_delivery(
                            pool,
                            event_id,
                            &token,
                            channel_id,
                            if again { "retry" } else { "failed" },
                            attempts + 1,
                            "",
                            &error_message,
                            again,
                        )?;
                        failed = true;
                        retry |= again;
                    }
                }
            }
            let status = if retry {
                "retry"
            } else if failed && succeeded {
                "partial"
            } else if failed {
                "failed"
            } else if personnel && !succeeded && dispatched_channels.is_empty() {
                "skipped"
            } else {
                "sent"
            };
            notification_repo::set_event_status(pool, event_id, &token, status, retry)
        })();
        if let Err(error) = result {
            tracing::warn!(event_id,%error,"通知处理未完成，等待恢复或重试");
            // Token matching prevents a worker with an expired lease from overwriting its successor.
            if let Err(status_error) =
                notification_repo::set_event_status(pool, event_id, &token, "retry", true)
            {
                tracing::warn!(event_id,%status_error,"通知租约已由其他任务接管");
            }
        }
    }
    Ok(handled)
}

pub fn test_channel(pool: &DbPool, channel_id: i64, mention_user_ids: &[i64]) -> Result<()> {
    if mention_user_ids.len() > 200 {
        return Err(AppError::Validation("测试最多选择200个账号".into()));
    }
    let (mobiles, missing) = crate::repo::dingtalk_mention_repo::resolve(
        pool,
        &mention_user_ids.iter().copied().collect(),
    )?;
    if !missing.is_empty() {
        return Err(AppError::Validation(format!(
            "请先维护钉钉绑定：{}",
            missing.join("、")
        )));
    }
    let (name, webhook, secret) = notification_repo::get_channel_secret(pool, channel_id)?;
    let body = "### 【样品管理系统】通知渠道测试\n\n该消息用于验证钉钉机器人配置，不产生业务记录。";
    send_dingtalk(&webhook, &secret, "通知渠道测试", body, &mobiles).map_err(|mut e| {
        for mobile in &mobiles {
            e = e.replace(mobile, "[手机号已隐藏]");
        }
        crate::error::AppError::Validation(format!("渠道「{}」发送失败: {}", name, e))
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn dingtalk_sender_posts_mentions_to_an_isolated_local_robot() {
        use std::io::{BufRead, Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/robot", listener.local_addr().unwrap());
        let receiver = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
            let mut length = 0;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse::<usize>().unwrap();
                }
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
            let response = r#"{"errcode":0,"errmsg":"ok"}"#;
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response).unwrap();
            payload
        });
        super::send_dingtalk(&url, "", "通知标题", "业务正文", &["13800138000".into()]).unwrap();
        let payload = receiver.join().unwrap();
        assert_eq!(
            payload["at"]["atMobiles"],
            serde_json::json!(["13800138000"])
        );
        assert_eq!(payload["at"]["isAtAll"], false);
        assert!(payload["markdown"]["text"]
            .as_str()
            .unwrap()
            .starts_with("业务正文"));
        assert!(payload["markdown"]["text"]
            .as_str()
            .unwrap()
            .contains("@13800138000"));
    }

    #[test]
    fn dingtalk_markdown_keeps_legacy_payload_and_deduplicates_mentions() {
        let old = super::dingtalk_markdown("原始标题", "原始正文", &[]);
        assert_eq!(
            old,
            serde_json::json!({"msgtype":"markdown","markdown":{"title":"原始标题","text":"原始正文"}})
        );
        let message = super::dingtalk_markdown(
            "原始标题",
            "原始正文",
            &[
                "13900139000".into(),
                "13800138000".into(),
                "13800138000".into(),
            ],
        );
        assert_eq!(message["at"]["isAtAll"], false);
        assert_eq!(
            message["at"]["atMobiles"],
            serde_json::json!(["13800138000", "13900139000"])
        );
        assert_eq!(
            message["markdown"]["text"],
            "原始正文\n\n提醒：@13800138000 @13900139000"
        );
    }

    #[test]
    fn dingtalk_queue_unions_same_group_and_keeps_other_groups_and_retries_separate() {
        let pool = crate::db::init_pool("dingtalk-queue-tests");
        let conn = pool.get().unwrap();
        crate::db::test_migrations::run(&conn).unwrap();
        let actor: i64 = conn
            .query_row("SELECT id FROM users WHERE username='admin'", [], |row| {
                row.get(0)
            })
            .unwrap();
        let role: i64 = conn
            .query_row(
                "SELECT id FROM roles WHERE name='系统管理员'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let mut people = Vec::new();
        for (name, mobile) in [
            ("定向甲", "13800138000"),
            ("定向乙", "13800138000"),
            ("跟随丙", "13900139000"),
            ("无权限丁", "13700137000"),
        ] {
            conn.execute(
                "INSERT INTO users(username,password,is_admin,is_active) VALUES(?1,'unused',0,1)",
                [name],
            )
            .unwrap();
            let id = conn.last_insert_rowid();
            if name != "无权限丁" {
                conn.execute(
                    "INSERT INTO user_roles(user_id,role_id) VALUES(?1,?2)",
                    postgres_compat::params![id, role],
                )
                .unwrap();
            }
            crate::repo::dingtalk_mention_repo::save_contact(&pool, id, mobile, actor, "admin")
                .unwrap();
            people.push(id);
        }
        let channels = [
            "https://example.test/group-a",
            "https://example.test/group-b",
        ]
        .map(|url| {
            crate::repo::notification_repo::create_channel(
                &pool,
                &crate::models::notification::NotificationChannelInput {
                    name: url.into(),
                    webhook_url: Some(url.into()),
                    secret: None,
                    is_active: Some(true),
                },
            )
            .unwrap()
        });
        for (channel, selected, follow, recipients) in [
            (channels[0], vec![people[0]], false, vec![people[0]]),
            (channels[0], vec![people[1]], false, vec![people[1]]),
            (channels[1], Vec::new(), true, vec![people[2], people[3]]),
        ] {
            let input = serde_json::from_value(serde_json::json!({"name":"定向提醒规则", "targets":[{"target_kind":"rd_work_record","sample_info_type_key":""}], "channel_id":channel, "recipient_user_ids":recipients, "dingtalk_mention_user_ids":selected, "dingtalk_follow_recipients":follow})).unwrap();
            crate::repo::notification_repo::create_rule(&pool, &input).unwrap();
        }
        crate::repo::notification_repo::enqueue_event(
            &pool,
            "rd_work_record",
            780001,
            &serde_json::json!({"notes":"保留模板正文"}),
        )
        .unwrap();
        let mut calls = Vec::new();
        super::process_pending_with_sender(&pool, 1, &mut |url, _, _, text, mobiles| {
            assert!(text.contains("保留模板正文"));
            calls.push((url.to_string(), mobiles.to_vec()));
            if url.ends_with("group-b") {
                Err("mock retry 13900139000".into())
            } else {
                Ok("ok 13800138000".into())
            }
        })
        .unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(
            calls
                .iter()
                .find(|(url, _)| url.ends_with("group-a"))
                .unwrap()
                .1,
            vec!["13800138000"]
        );
        assert_eq!(
            calls
                .iter()
                .find(|(url, _)| url.ends_with("group-b"))
                .unwrap()
                .1,
            vec!["13900139000"]
        );
        let leaked: i64 = conn.query_row("SELECT COUNT(*) FROM notification_deliveries WHERE response_summary LIKE '%13800138000%' OR last_error LIKE '%13900139000%'", [], |row| row.get(0)).unwrap();
        assert_eq!(leaked, 0);
        let event: i64 = conn
            .query_row(
                "SELECT id FROM notification_events WHERE resource_id=780001",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let unexpected_inbox: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM in_app_notifications WHERE event_id=?1 AND user_id=?2",
                postgres_compat::params![event, people[3]],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(unexpected_inbox, 0);
        crate::repo::dingtalk_mention_repo::save_contact(&pool, people[2], "", actor, "admin")
            .unwrap();
        conn.execute(
            "UPDATE notification_events SET next_attempt_at=NULL WHERE id=?1",
            [event],
        )
        .unwrap();
        calls.clear();
        super::process_pending_with_sender(&pool, 1, &mut |url, _, _, _, mobiles| {
            calls.push((url.to_string(), mobiles.to_vec()));
            Ok("ok".into())
        })
        .unwrap();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].0.ends_with("group-b"));
        assert!(calls[0].1.is_empty());
        let summary: String = conn.query_row("SELECT response_summary FROM notification_deliveries WHERE event_id=?1 AND channel_id=?2", postgres_compat::params![event,channels[1]], |row| row.get(0)).unwrap();
        assert!(summary.contains("未@"));
        assert!(summary.contains("跟随丙"));
        assert!(!summary.contains("13900139000"));
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM in_app_notifications WHERE event_id=?1",
                [event],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            3
        );
    }

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
        )
        .unwrap();
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
    fn personnel_review_tokens_resolve_system_values_without_changing_rd_reason() {
        let payload = json!({"notice_no":"N-61","review_reason":"缺少生效日期","return_reason":"不应显示","status":"已驳回","revision":2,"reviewed_by":"组长","reviewed_at":"2026-10-15 10:00:00"});
        assert_eq!(
            render_template_title(
                "{{通知编号}}：{{驳回原因}} / {{审批意见}} / {{审批状态}} / {{修订次数}}",
                &payload
            ),
            "N-61：缺少生效日期 / 缺少生效日期 / 已驳回 / 2"
        );
        assert_eq!(
            render_template_title(
                "{{驳回原因}}",
                &json!({"return_reason":"研发退回原因","review_reason":"人员审批原因"})
            ),
            "研发退回原因"
        );
        let template = default_template(super::TEMPLATE_PERSONNEL_CHANGE_APPROVED);
        assert!(template.name.contains("通过"));
        assert!(
            super::form_setting_keys_for_template(super::TEMPLATE_PERSONNEL_CHANGE_APPROVED)
                .is_empty()
        );
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
            notification_text_with_template(&pool, &payload, TEMPLATE_SAMPLE_INFO).unwrap();
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
            notification_text_with_template(&pool, &payload, TEMPLATE_SAMPLE_INFO).unwrap();
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
            notification_text_with_template(&pool, &payload, "sample_info:icp").unwrap();
        assert_eq!(title, "ICP独立通知");
        assert!(markdown.contains("**专用值：100 mg/L**"));
        assert!(markdown.ends_with("ICP底部提示"));
    }
    #[test]
    fn notes_uses_source_name_and_keeps_explicit_custom_name() {
        let pool = crate::db::init_pool("notification-label-test");
        let conn = pool.get().unwrap();
        crate::db::test_migrations::run(&conn).unwrap();
        conn.execute(
            "UPDATE rd_record_columns SET label='注意事项' WHERE name='notes'",
            [],
        )
        .unwrap();
        notification_repo::save_template(
            &pool,
            super::TEMPLATE_RD,
            "通知",
            "",
            &json!([
                {"key":"notes","label":"备注","visible":true,"bold":true,"sort_order":4}
            ]),
        )
        .unwrap();
        let loaded = super::load_template(&pool, super::TEMPLATE_RD).unwrap();
        let notes = loaded
            .fields
            .iter()
            .find(|field| field.key == "notes")
            .unwrap();
        assert_eq!(notes.label, "注意事项");
        assert_eq!(notes.follow_source_label, Some(true));
        notification_repo::save_template(&pool, super::TEMPLATE_RD, "通知", "", &json!([
            {"key":"notes","label":"特殊说明","follow_source_label":false,"visible":true,"bold":true,"sort_order":4}
        ])).unwrap();
        conn.execute(
            "UPDATE rd_record_columns SET label='样品说明' WHERE name='notes'",
            [],
        )
        .unwrap();
        let loaded = super::load_template(&pool, super::TEMPLATE_RD).unwrap();
        let notes = loaded
            .fields
            .iter()
            .find(|field| field.key == "notes")
            .unwrap();
        assert_eq!(notes.label, "特殊说明");
        assert_eq!(notes.source_label, "样品说明");
        assert!(notes.bold);
        assert_eq!(notes.sort_order, 4);
    }

    #[test]
    fn new_fields_are_available_and_invalidated_config_does_not_enable_everything() {
        let pool = crate::db::init_pool("notification-new-field-test");
        let conn = pool.get().unwrap();
        crate::db::test_migrations::run(&conn).unwrap();
        conn.execute(
            "DELETE FROM notification_templates WHERE event_key='sample_info'",
            [],
        )
        .unwrap();
        notification_repo::save_template(
            &pool,
            TEMPLATE_SAMPLE_INFO,
            "通知",
            "",
            &json!([
                {"key":"notes","label":"备注","visible":true,"bold":false,"sort_order":1}
            ]),
        )
        .unwrap();
        conn.execute("INSERT INTO sample_info_columns(field_key,label,data_type,is_predefined,is_active) VALUES('notify_new_note','新增注意事项','text',0,1)", []).unwrap();
        let loaded = super::load_template(&pool, TEMPLATE_SAMPLE_INFO).unwrap();
        assert_eq!(loaded.fields.len(), 1);
        assert!(loaded
            .available_fields
            .iter()
            .any(|field| field.key == "notify_new_note" && !field.visible));
        conn.execute(
            "UPDATE sample_info_columns SET is_active=0 WHERE field_key='notes'",
            [],
        )
        .unwrap();
        let paused = super::load_template(&pool, TEMPLATE_SAMPLE_INFO).unwrap();
        assert!(paused.fields.is_empty());
        assert!(!paused.available_fields.is_empty());
        let (_, markdown, _) = notification_text_with_template(
            &pool,
            &json!({"notes":"停用说明","extra_fields":{"notify_new_note":"尚未启用"}}),
            TEMPLATE_SAMPLE_INFO,
        )
        .unwrap();
        assert!(!markdown.contains("停用说明"));
        assert!(!markdown.contains("尚未启用"));
        let (_, _, stored) = notification_repo::get_template(&pool, TEMPLATE_SAMPLE_INFO).unwrap();
        assert!(stored
            .as_array()
            .unwrap()
            .iter()
            .any(|field| field["key"] == "notes"));
    }

    #[test]
    fn rd_notes_and_custom_details_survive_record_save_and_notification_render() {
        let pool = crate::db::init_pool("notification-rd-payload-test");
        let conn = pool.get().unwrap();
        crate::db::test_migrations::run(&conn).unwrap();
        conn.execute(
            "INSERT INTO project_groups(name) VALUES('notification-lab')",
            [],
        )
        .unwrap();
        let group = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO projects(name,group_id) VALUES('notification-project',?1)",
            [group],
        )
        .unwrap();
        let project = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO project_lab_links(project_id,group_id) VALUES(?1,?2)",
            postgres_compat::params![project, group],
        )
        .unwrap();
        let admin: i64 = conn
            .query_row("SELECT id FROM users WHERE is_admin=1 LIMIT 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        let input: crate::models::record::RecordCreate = serde_json::from_value(json!({"project_id":project,"user_name":"测试送样人","quantity":2,
            "recorded_at":"2026-10-14 10:00:00","group_id":group,"extra_fields":{"notify_note":"其他","notify_note__detail":"低温保存"}})).unwrap();
        let record = crate::repo::rd_record_repo::create(
            &pool,
            &input,
            None,
            Some("避光保存\n尽快检测".into()),
            admin,
            "测试账号",
        )
        .unwrap();
        conn.execute("INSERT INTO rd_record_columns(name,label,data_type,is_predefined,is_active,show_in_form) VALUES('notify_note','新增注意事项','select_other',0,1,1)", []).unwrap();
        notification_repo::save_template(&pool, super::TEMPLATE_RD, "通知", "", &json!([
            {"key":"notes","label":"注意事项","visible":true,"bold":true,"sort_order":1},
            {"key":"notify_note","label":"新增注意事项","visible":true,"bold":false,"sort_order":2}
        ])).unwrap();
        let payload = super::rd_payload(&pool, &record);
        assert_eq!(payload["notes"], "避光保存\n尽快检测");
        let (_, markdown, _) =
            notification_text_with_template(&pool, &payload, super::TEMPLATE_RD).unwrap();
        assert!(markdown.contains("注意事项"));
        assert!(markdown.contains("避光保存\n尽快检测"));
        assert!(markdown.contains("其他（低温保存）"));
    }

    #[test]
    fn personnel_template_order_is_independent_of_source_order() {
        let pool = crate::db::init_pool("notification-personnel-order-test");
        crate::db::test_migrations::run(&pool.get().unwrap()).unwrap();
        notification_repo::save_template(
            &pool,
            super::TEMPLATE_PERSONNEL_CHANGE,
            "人员通知",
            "",
            &json!([
                {"key":"method_names","label":"检测方法","visible":true,"bold":true,"sort_order":1},
                {"key":"project_codes","label":"项目","visible":true,"bold":false,"sort_order":2}
            ]),
        )
        .unwrap();
        let template = super::load_template(&pool, super::TEMPLATE_PERSONNEL_CHANGE).unwrap();
        assert_eq!(template.fields[0].key, "method_names");
        assert_eq!(template.fields[1].key, "project_codes");
        let (_, markdown) = super::render_template_message(
            &template,
            &json!({"project_codes":["项目A","项目B"],"method_names":["方法A","方法B"]}),
            "人员通知",
        );
        assert!(markdown.contains("方法A、方法B"));
        assert!(markdown.find("方法A").unwrap() < markdown.find("项目A").unwrap());
    }

    #[test]
    fn damaged_template_retries_one_event_without_blocking_later_events() {
        let pool = crate::db::init_pool("notification-template-retry-test");
        let conn = pool.get().unwrap();
        crate::db::test_migrations::run(&conn).unwrap();
        let rule: crate::models::notification::NotificationRuleInput =
            serde_json::from_value(json!({
                "name":"本地队列回归",
                "is_default":true,
                "channel_id":null,
                "recipient_user_ids":[]
            }))
            .unwrap();
        notification_repo::create_rule(&pool, &rule).unwrap();
        let fields =
            json!([{"key":"notes","label":"注意事项","visible":true,"bold":false,"sort_order":1}]);
        notification_repo::save_template(&pool, super::TEMPLATE_RD, "通知", "", &fields).unwrap();
        notification_repo::save_template(&pool, TEMPLATE_SAMPLE_INFO, "通知", "", &fields).unwrap();
        conn.execute(
            "UPDATE notification_templates SET field_config_json='invalid-json' WHERE event_key=?1",
            [super::TEMPLATE_RD],
        )
        .unwrap();
        notification_repo::enqueue_event(
            &pool,
            "rd_work_record",
            800001,
            &json!({"record_id":800001,"notes":"损坏模板事件"}),
        )
        .unwrap();
        notification_repo::enqueue_event(
            &pool,
            "sample_info",
            800002,
            &json!({"record_id":800002,"notes":"后续正常事件"}),
        )
        .unwrap();

        assert_eq!(super::process_pending(&pool, 20).unwrap(), 2);
        let (bad_status, retry_at): (String, Option<String>) = conn
            .query_row(
                "SELECT status,next_attempt_at FROM notification_events WHERE resource_type='rd_work_record' AND resource_id=800001",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(bad_status, "retry");
        assert!(retry_at.is_some_and(|value| !value.is_empty()));
        let (good_status, good_retry_at): (String, Option<String>) = conn
            .query_row(
                "SELECT status,next_attempt_at FROM notification_events WHERE resource_type='sample_info' AND resource_id=800002",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(good_status, "sent");
        assert!(good_retry_at.is_none());
        let bad_notifications: i64 = conn.query_row(
            "SELECT COUNT(*) FROM in_app_notifications n JOIN notification_events e ON e.id=n.event_id WHERE e.resource_type='rd_work_record' AND e.resource_id=800001",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(bad_notifications, 0);
        let deliveries: i64 = conn
            .query_row("SELECT COUNT(*) FROM notification_deliveries", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(deliveries, 0);
    }
}
