use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct RecordResponse {
    pub id: i64,
    pub business_no: String,
    pub project_id: i64,
    pub method_id: Option<i64>,
    pub project_name: String,
    pub group_name: String,
    pub user_name: String,
    pub quantity: i32,
    pub recorded_at: String,
    pub created_at: String,
    pub deleted_at: Option<String>,
    pub method_name: Option<String>,
    pub method_type: Option<String>,
    pub instrument_code: String,
    pub instrument_type: String,
    pub multiplier: f64,
    pub high_item: Option<String>,
    pub coefficient_snapshot: f64,
    pub subject_user_id: Option<i64>,
    pub created_by_user_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct RecordCreate {
    pub project_id: i64,
    pub method_id: Option<i64>,
    pub user_name: String,
    pub quantity: i32,
    pub recorded_at: String,
    pub group_id: Option<i64>,
    pub multiplier: Option<f64>,
    #[serde(default)]
    pub high_item: Option<String>,
    #[serde(default)]
    pub division_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct RecordUpdate {
    pub user_name: Option<String>,
    pub quantity: Option<i32>,
    pub recorded_at: Option<String>,
    pub multiplier: Option<f64>,
    // v0.4.34: 行内编辑扩展字段
    pub project_id: Option<i64>,
    pub method_id: Option<i64>,
    pub group_id: Option<i64>,
    pub division_id: Option<i64>,
    pub batch_no: Option<String>,
    pub notes: Option<String>,
    // v0.4.67: 高项自定义编辑
    pub high_item: Option<String>,
}
