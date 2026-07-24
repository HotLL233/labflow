use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct AuditLogResponse {
    pub id: i64,
    pub user_id: Option<i64>,
    pub action: String,
    pub table_name: String,
    pub record_id: Option<i64>,
    pub user_name: String,
    pub detail: String,
    pub module: String,
    pub business_no: String,
    pub before_data: Option<serde_json::Value>,
    pub after_data: Option<serde_json::Value>,
    pub source: String,
    pub created_at: String,
}
