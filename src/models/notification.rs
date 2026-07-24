use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct NotificationChannel {
    pub id: i64,
    pub name: String,
    pub channel_type: String,
    pub webhook_url_masked: String,
    pub has_secret: bool,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct NotificationChannelInput {
    pub name: String,
    pub webhook_url: String,
    pub secret: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct NotificationRule {
    pub id: i64,
    pub name: String,
    pub event_type: String,
    pub group_id: Option<i64>,
    pub group_name: Option<String>,
    pub project_name: String,
    pub detection_type: String,
    pub channel_id: Option<i64>,
    pub channel_name: Option<String>,
    pub recipient_user_ids: Vec<i64>,
    pub is_default: bool,
    pub is_active: bool,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct NotificationRuleInput {
    pub name: String,
    pub group_id: Option<i64>,
    pub project_name: Option<String>,
    pub detection_type: Option<String>,
    pub channel_id: Option<i64>,
    #[serde(default)]
    pub recipient_user_ids: Vec<i64>,
    pub is_default: Option<bool>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct NotificationDelivery {
    pub id: i64,
    pub event_id: i64,
    pub channel_name: String,
    pub status: String,
    pub attempts: i64,
    pub response_summary: String,
    pub last_error: String,
    pub sent_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct NotificationSummary {
    pub pending_count: i64,
    pub failed_count: i64,
    pub sent_today_count: i64,
    pub skipped_count: i64,
}

#[derive(Debug, Serialize)]
pub struct InAppNotification {
    pub id: i64,
    pub title: String,
    pub body: String,
    pub target_url: String,
    pub is_read: bool,
    pub created_at: String,
}
