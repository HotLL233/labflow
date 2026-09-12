use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct RecordEvent {
    pub id: i64,
    pub module: String,
    pub table_name: String,
    pub record_id: i64,
    pub business_no: String,
    pub event_type: String,
    pub from_status: Option<String>,
    pub to_status: Option<String>,
    pub operator: String,
    pub operated_at: String,
    pub reason: String,
    pub before_data: Option<serde_json::Value>,
    pub after_data: Option<serde_json::Value>,
}
