use crate::db::DbPool;
use crate::error::Result;
use crate::models::rd_record::RdRecordResponse;
use crate::models::sample_info::SampleInfoResponse;
use crate::repo::{division_repo, notification_repo, sample_info_attachment_repo};
use base64::{engine::general_purpose::STANDARD, Engine};
use hmac::{Hmac, Mac};
use serde_json::json;
use sha2::Sha256;

pub(crate) const EVENT_SAMPLE_SUBMITTED: &str = "sample_submitted";
const MAX_ATTEMPTS: i64 = 3;

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
        "method_name": "",
        "quantity": record.quantity.to_string(),
        "division_name": record.division_name.clone().unwrap_or_default(),
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
        "method_name": record.method_name.clone().unwrap_or_default(),
        "quantity": record.quantity.to_string(),
        "division_name": division_name,
        "group_id": record.group_id,
        "target_url": format!("/sample-records?record_id={}", record.id),
    })
}

pub fn enqueue_rd_submitted(pool: &DbPool, record: &RdRecordResponse) -> Result<()> {
    notification_repo::enqueue_event(pool, "rd_work_record", record.id, &rd_payload(pool, record))
}

fn field(payload: &serde_json::Value, key: &str) -> String {
    payload
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
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

pub fn process_pending(pool: &DbPool, limit: i64) -> Result<usize> {
    let events = notification_repo::pending_events(pool, limit)?;
    let mut handled = 0;
    for (event_id, resource_type, resource_id, mut payload) in events {
        if resource_type == "sample_info" {
            let attachment_count =
                sample_info_attachment_repo::list_by_record(pool, resource_id)?.len();
            payload["attachment_count"] = json!(attachment_count);
        }
        let group_id = payload.get("group_id").and_then(|v| v.as_i64());
        let project = field(&payload, "project_name");
        let detection_type = field(&payload, "detection_type");
        let instrument_type = field(&payload, "instrument_type");
        let rules = notification_repo::matching_rules(
            pool,
            group_id,
            &project,
            &detection_type,
            &instrument_type,
        )?;
        if rules.is_empty() {
            // The record was saved successfully, but no notification destination exists yet.
            notification_repo::set_event_status(pool, event_id, "skipped", false)?;
            handled += 1;
            continue;
        }
        let (title, markdown, in_app_body) = notification_text(&payload);
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
        for rule in rules {
            let recipients = if rule.recipient_user_ids.is_empty() {
                notification_repo::analysis_user_ids(pool)?
            } else {
                notification_repo::analysis_user_ids_for(pool, &rule.recipient_user_ids)?
            };
            notification_repo::create_in_app(
                pool,
                event_id,
                &recipients,
                &title,
                &in_app_body,
                &target,
            )?;
            if let Some(channel_id) = rule.channel_id {
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
    use super::{dingtalk_url, notification_text};
    use serde_json::json;

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
}
