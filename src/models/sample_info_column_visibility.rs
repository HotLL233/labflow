use serde::{Deserialize, Serialize};

/// 列可见性桥接 Model
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SampleInfoColumnVisibility {
    pub id: i64,
    pub type_key: String,
    pub column_id: i64,
    pub is_visible: bool,
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
}
