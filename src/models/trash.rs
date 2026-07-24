use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct TrashEntry {
    pub id: i64,
    pub entity_type: String,
    pub table_name: String,
    pub record_id: i64,
    pub category: String,
    pub module: String,
    pub display_name: String,
    pub business_no: String,
    pub snapshot: serde_json::Value,
    pub delete_reason: String,
    pub deleted_by_user_id: Option<i64>,
    pub deleted_by_username: String,
    pub owner_user_id: Option<i64>,
    pub owner_group_id: Option<i64>,
    pub deleted_at: String,
    pub dependency_summary: String,
    pub can_purge: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrashPrecheck {
    pub entity_type: String,
    pub table_name: String,
    pub record_id: i64,
    pub display_name: String,
    pub dependency_summary: String,
    pub can_purge: bool,
}

#[derive(Debug, Deserialize)]
pub struct TrashQuery {
    pub category: Option<String>,
    pub module: Option<String>,
    pub keyword: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[derive(Debug, Deserialize, Default)]
pub struct DeleteReasonRequest {
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PurgeRequest {
    pub admin_username: String,
    pub admin_password: String,
}
