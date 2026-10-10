use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct RdRecordResponse {
    pub id: i64,
    pub business_no: String,
    pub project_id: i64,
    pub method_id: Option<i64>,
    pub project_name: String,
    pub group_name: String,
    pub user_name: String,
    pub quantity: i32,
    pub recorded_at: String,
    pub last_activity_at: String,
    pub batch_no: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub deleted_at: Option<String>,
    pub method_name: Option<String>,
    pub method_type: Option<String>,
    pub instrument_code: String,
    pub instrument_type: String,
    pub status: String,
    pub return_reason: String,
    pub returned_by: String,
    pub returned_at: Option<String>,
    pub return_confirmed_at: Option<String>,
    pub return_confirmed_by: String,
    pub voided_at: Option<String>,
    pub voided_by: String,
    pub void_reason: String,
    pub sampler: Option<String>,
    pub sampled_at: Option<String>,
    pub detected_by: Option<String>,
    pub detected_at: Option<String>,
    pub division_id: Option<i64>,
    pub group_id: Option<i64>,
    pub high_item: Option<String>,
    pub coefficient_snapshot: f64,
    pub subject_user_id: Option<i64>,
    pub created_by_user_id: Option<i64>,
    pub extra_fields: Option<serde_json::Value>,
    pub sequence_no: i64,
    pub project_division_id: Option<i64>,
    pub execution_division_id: Option<i64>,
    pub execution_group_id: Option<i64>,
    pub workload_recorded: bool,
    pub recorded_quantity: i64,
    pub workload_recorders: Vec<WorkloadRecorder>,
    pub workload_unknown_recorder_entries: i64,
    pub workload_unknown_recorder_quantity: i64,
}

#[derive(Debug, Serialize, Clone)]
pub struct WorkloadRecorder {
    pub user_id: i64,
    pub username: String,
    pub quantity: i64,
    pub entry_count: i64,
}

#[derive(Debug, Serialize)]
pub struct RdFilterOption {
    pub value: String,
    pub label: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct RdFilterOptions {
    pub field: String,
    pub options: Vec<RdFilterOption>,
    pub total: i64,
    pub truncated: bool,
}

#[derive(Debug, Serialize)]
pub struct RdWorkloadEntry {
    pub id: i64,
    pub business_no: String,
    pub recorder_user_id: Option<i64>,
    pub recorder_username: String,
    pub subject_user_id: Option<i64>,
    pub user_name: String,
    pub quantity: i64,
    pub multiplier: f64,
    pub created_at: String,
    pub recorded_at: String,
}

#[derive(Debug, Serialize)]
pub struct RdWorkloadEntries {
    pub items: Vec<RdWorkloadEntry>,
    pub total: i64,
    pub visible_quantity: i64,
    pub recorded_quantity: i64,
    pub has_hidden_entries: bool,
}

#[derive(Debug, Deserialize)]
pub struct RdSampleUpdate {
    pub sampler: String,
}

/// RdRecordCreate — mirrors RecordCreate with batch_no/notes
#[derive(Debug, Deserialize)]
pub struct RdRecordCreate {
    pub project_id: i64,
    pub method_id: Option<i64>,
    #[serde(default)]
    pub detection_type: String,
    pub user_name: String,
    #[serde(default)]
    pub sender_user_id: Option<i64>,
    pub quantity: i32,
    pub recorded_at: String,
    pub group_id: Option<i64>,
    pub division_id: Option<i64>,
    pub batch_no: Option<String>,
    pub notes: Option<String>,
    #[serde(default)]
    pub extra_fields: Option<serde_json::Value>,
}
