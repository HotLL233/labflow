use serde::{Deserialize, Serialize};

/// 列可见性桥接 Model
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SampleInfoColumnVisibility {
    pub id: i64,
    pub type_key: String,
    pub column_id: i64,
    pub is_visible: bool,
    pub is_required: bool,
    pub show_in_form: bool,
    pub show_in_list: bool,
    pub show_in_export: bool,
    pub sort_order: i64,
}

/// 批量更新请求
#[derive(Debug, Deserialize)]
pub struct VisibilityUpdateRequest {
    pub type_key: String,
    pub items: Vec<VisibilityItem>,
}

#[derive(Debug, Deserialize)]
pub struct VisibilityItem {
    pub column_id: i64,
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
