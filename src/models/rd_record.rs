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
    pub batch_no: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub deleted_at: Option<String>,
    pub method_name: Option<String>,
    pub method_type: Option<String>,
    pub instrument_code: String,
    pub instrument_type: String,
    pub status: String,
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
    pub user_name: String,
    pub quantity: i32,
    pub recorded_at: String,
    pub group_id: Option<i64>,
    pub division_id: Option<i64>,
    pub batch_no: Option<String>,
    pub notes: Option<String>,
}
