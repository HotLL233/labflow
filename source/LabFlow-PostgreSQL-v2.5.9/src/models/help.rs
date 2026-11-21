use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct HelpDocument {
    pub id: i64,
    pub title: String,
    pub filename: String,
    pub file_path: String,
    pub file_type: String,
    pub file_size: i64,
    pub is_visible: bool,
    pub sort_order: i64,
    pub page_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct HelpDocUpdateRequest {
    pub title: Option<String>,
    pub is_visible: Option<bool>,
    pub sort_order: Option<i64>,
}
