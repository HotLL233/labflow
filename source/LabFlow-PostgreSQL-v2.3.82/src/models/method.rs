use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct MethodResponse {
    pub id: i64,
    pub method_code: String,
    pub name: String,
    pub full_name: String,
    pub coefficient: f64,
    pub amount: f64,
    pub multiplier: f64,
    pub notes: String,
    pub is_active: bool,
    pub show_in_work: bool,
    pub show_in_rd: bool,
    pub show_in_sample_info: bool,
    pub is_common: bool,
    /// Departments where a common method is available in the analysis portal.
    pub common_division_ids: Vec<i64>,
    pub type_ids: Vec<i64>,
    pub type_names: Vec<String>,
    pub instrument_id: Option<i64>,
    pub instrument_code: String,
    pub instrument_name: String,
    pub instrument_type: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct MethodCreate {
    #[serde(default)]
    pub method_code: Option<String>,
    pub name: String,
    pub full_name: Option<String>,
    pub coefficient: Option<f64>,
    pub amount: Option<f64>,
    pub multiplier: Option<f64>,
    pub notes: Option<String>,
    pub type_ids: Option<Vec<i64>>,
    pub instrument_id: i64,
    pub show_in_work: Option<bool>,
    pub show_in_rd: Option<bool>,
    pub show_in_sample_info: Option<bool>,
    pub is_common: Option<bool>,
    #[serde(default)]
    pub common_division_ids: Option<Vec<i64>>,
}

#[derive(Debug, Deserialize)]
pub struct MethodUpdate {
    pub method_code: Option<String>,
    pub name: Option<String>,
    pub full_name: Option<String>,
    pub coefficient: Option<f64>,
    pub amount: Option<f64>,
    pub multiplier: Option<f64>,
    pub notes: Option<String>,
    pub is_active: Option<bool>,
    pub type_ids: Option<Vec<i64>>,
    pub instrument_id: Option<i64>,
    pub show_in_work: Option<bool>,
    pub show_in_rd: Option<bool>,
    pub show_in_sample_info: Option<bool>,
    pub is_common: Option<bool>,
    #[serde(default)]
    pub common_division_ids: Option<Vec<i64>>,
}
