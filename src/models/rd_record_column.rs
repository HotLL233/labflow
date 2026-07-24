use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Clone)]
pub struct RdRecordColumn {
    pub id: i64,
    pub name: String,
    pub label: String,
    pub data_type: String,
    pub width: i64,
    pub sort_order: i64,
    pub is_predefined: bool,
    pub show_in_list: bool,
    pub show_in_form: bool,
    pub created_at: String,
    pub updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RdRecordColumnUpdate {
    pub width: Option<i64>,
    pub show_in_list: Option<bool>,
    pub show_in_form: Option<bool>,
}
