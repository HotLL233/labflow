use serde::{Deserialize, Serialize};

/// 事业部主数据（divisions 表）
#[derive(Debug, Serialize)]
pub struct Division {
    pub id: i64,
    pub name: String,
    pub sort_order: i64,
    pub color: String,
    pub is_active: bool,
    pub created_at: String,
}

/// 事业部列表响应（聚合下属实验室数量 lab_count）
#[derive(Debug, Serialize)]
pub struct DivisionResponse {
    pub id: i64,
    pub name: String,
    pub sort_order: i64,
    pub lab_count: i64,
    pub color: String,
    pub is_active: bool,
}

#[derive(Debug, Deserialize)]
pub struct DivisionCreate {
    pub name: String,
    pub sort_order: Option<i64>,
    pub color: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DivisionUpdate {
    pub name: Option<String>,
    pub sort_order: Option<i64>,
    pub color: Option<String>,
}
