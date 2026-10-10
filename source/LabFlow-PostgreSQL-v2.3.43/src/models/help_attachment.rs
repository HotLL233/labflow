use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Clone)]
pub struct HelpAttachment {
    pub id: i64,
    pub title: String,
    pub filename: String,
    pub file_path: String,
    pub file_type: String,
    pub file_size: i64,
    pub is_visible: bool,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct HelpAttachmentUpdate {
    pub title: Option<String>,
    pub is_visible: Option<bool>,
    pub sort_order: Option<i64>,
}
