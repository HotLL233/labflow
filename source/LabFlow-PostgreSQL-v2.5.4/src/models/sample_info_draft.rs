use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize)]
pub struct SampleInfoDraft {
    pub id: i64,
    pub user_id: i64,
    pub username_snapshot: String,
    pub type_key: String,
    pub title: String,
    pub payload: Value,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub submitted_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SampleInfoDraftInput {
    pub type_key: String,
    #[serde(default)]
    pub title: String,
    pub payload: Value,
}

#[derive(Debug, Serialize, Clone)]
pub struct SampleInfoDraftAttachment {
    pub id: i64,
    pub draft_id: i64,
    pub row_index: i64,
    pub file_name: String,
    pub stored_name: String,
    pub file_size: i64,
    pub file_type: String,
    pub created_at: String,
}
