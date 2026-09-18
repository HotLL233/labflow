use serde::{Deserialize, Serialize};

/// A select option can reveal one additional free-text input when selected.
/// Rules are stored as JSON in `rd_record_columns.option_detail_rules`.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct RdOptionDetailRule {
    pub trigger_value: String,
    pub label: String,
    #[serde(default)]
    pub placeholder: String,
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct RdRecordColumn {
    pub id: i64,
    pub name: String,
    pub label: String,
    pub data_type: String,
    pub width: i64,
    pub sort_order: i64,
    pub is_predefined: bool,
    pub is_required: bool,
    pub is_active: bool,
    pub show_in_list: bool,
    pub show_in_form: bool,
    pub show_in_export: bool,
    pub options: String,
    pub option_detail_rules: String,
    pub default_value: String,
    pub placeholder: String,
    pub applicable_types: String,
    pub entry_row: i64,
    pub created_at: String,
    pub updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RdRecordColumnUpdate {
    pub label: Option<String>,
    pub data_type: Option<String>,
    pub width: Option<i64>,
    pub sort_order: Option<i64>,
    pub is_required: Option<bool>,
    pub is_active: Option<bool>,
    pub show_in_list: Option<bool>,
    pub show_in_form: Option<bool>,
    pub show_in_export: Option<bool>,
    pub options: Option<String>,
    pub option_detail_rules: Option<String>,
    pub default_value: Option<String>,
    pub placeholder: Option<String>,
    pub applicable_types: Option<String>,
    pub entry_row: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct RdRecordColumnCreate {
    pub name: String,
    pub label: String,
    #[serde(default = "default_data_type")]
    pub data_type: String,
    #[serde(default)]
    pub width: Option<i64>,
    #[serde(default)]
    pub is_required: bool,
    #[serde(default = "default_true")]
    pub show_in_list: bool,
    #[serde(default = "default_true")]
    pub show_in_form: bool,
    #[serde(default = "default_true")]
    pub show_in_export: bool,
    #[serde(default)]
    pub options: String,
    #[serde(default)]
    pub option_detail_rules: String,
    #[serde(default)]
    pub default_value: String,
    #[serde(default)]
    pub placeholder: String,
    #[serde(default)]
    pub applicable_types: String,
    #[serde(default = "default_entry_row")]
    pub entry_row: i64,
}

fn default_data_type() -> String {
    "text".to_string()
}
fn default_true() -> bool {
    true
}
fn default_entry_row() -> i64 {
    2
}
