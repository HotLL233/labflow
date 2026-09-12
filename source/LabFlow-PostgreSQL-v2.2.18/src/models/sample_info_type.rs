use serde::{Deserialize, Serialize};

/// 数据库行映射（检测类型）
#[derive(Debug, Serialize, Clone)]
pub struct SampleInfoType {
    pub id: i64,
    pub type_key: String,
    pub label: String,
    pub description: String,
    pub color: String,
    pub sort_order: i64,
    pub is_active: i64,
    pub created_at: String,
}

/// 创建请求
#[derive(Debug, Deserialize)]
pub struct SampleInfoTypeCreate {
    pub type_key: String,
    pub label: String,
    pub description: Option<String>,
    pub color: Option<String>,
    pub sort_order: Option<i64>,
}

/// 更新请求（全量可选）
#[derive(Debug, Deserialize)]
pub struct SampleInfoTypeUpdate {
    pub type_key: Option<String>,
    pub label: Option<String>,
    pub description: Option<String>,
    pub color: Option<String>,
    pub sort_order: Option<i64>,
    pub is_active: Option<i64>,
}
