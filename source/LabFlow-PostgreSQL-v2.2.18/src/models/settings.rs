use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SystemSetting {
    pub key: String,
    pub value: String, // JSON string
    pub updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SettingUpdate {
    pub value: serde_json::Value,
}
