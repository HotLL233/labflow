use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Clone)]
pub struct SampleInfoColumn {
    pub id: i64,
    pub field_key: String,
    pub label: String,
    pub data_type: String,
    pub is_predefined: bool,
    pub is_required: bool,
    pub is_active: bool,
    pub width: i64,
    pub sort_order: i64,
    pub options: Option<String>,
    pub show_in_list: bool,
    pub show_in_export: bool,
    pub show_in_form: bool,
    pub type_key: Option<String>,
    pub visible_types: Vec<String>,
    pub required_types: Vec<String>,
    pub created_at: String,
    pub updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ColumnCreate {
    pub field_key: String,
    pub label: String,
    pub data_type: String,
    pub type_key: Option<String>,
    pub width: Option<i64>,
    pub sort_order: Option<i64>,
    pub options: Option<String>,
    pub is_required: Option<bool>,
    pub show_in_list: Option<bool>,
    pub show_in_export: Option<bool>,
    pub show_in_form: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ColumnUpdate {
    pub label: Option<String>,
    pub data_type: Option<String>,
    pub is_active: Option<bool>,
    pub is_required: Option<bool>,
    pub width: Option<i64>,
    pub options: Option<String>,
    pub show_in_list: Option<bool>,
    pub show_in_export: Option<bool>,
    pub show_in_form: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ColumnReorder {
    pub ids: Vec<ReorderItem>,
}

#[derive(Debug, Deserialize)]
pub struct ReorderItem {
    pub id: i64,
    pub sort_order: i64,
}

#[derive(Debug, Deserialize)]
pub struct ColumnTypesUpdate {
    #[serde(default)]
    pub type_rules: Vec<ColumnTypeRule>,
}

#[derive(Debug, Deserialize)]
pub struct ColumnTypeRule {
    pub type_key: String,
    pub is_visible: bool,
    #[serde(default)]
    pub is_required: bool,
    #[serde(default = "default_true")]
    pub show_in_form: bool,
    #[serde(default = "default_true")]
    pub show_in_list: bool,
    #[serde(default = "default_true")]
    pub show_in_export: bool,
    #[serde(default)]
    pub sort_order: i64,
}

fn default_true() -> bool {
    true
}
