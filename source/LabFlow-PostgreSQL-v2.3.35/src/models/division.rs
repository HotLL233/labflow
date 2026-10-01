use serde::{Deserialize, Serialize};

/// 事业部主数据（divisions 表）
#[derive(Debug, Serialize)]
pub struct Division {
    pub id: i64,
    pub name: String,
    pub sort_order: i64,
    pub color: String,
    pub is_active: bool,
    pub show_in_work: bool,
    pub show_in_rd: bool,
    pub show_in_sample_info: bool,
    pub code: String,
    pub manager_user_id: Option<i64>,
    pub manager_username: String,
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
    pub show_in_work: bool,
    pub show_in_rd: bool,
    pub show_in_sample_info: bool,
    pub code: String,
    pub manager_user_id: Option<i64>,
    pub manager_username: String,
}

#[derive(Debug, Deserialize)]
pub struct DivisionCreate {
    pub name: String,
    pub code: String,
    pub manager_user_id: Option<i64>,
    pub sort_order: Option<i64>,
    pub color: Option<String>,
    pub show_in_work: Option<bool>,
    pub show_in_rd: Option<bool>,
    pub show_in_sample_info: Option<bool>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct DivisionUpdate {
    pub name: Option<String>,
    pub code: Option<String>,
    /// Omitted means unchanged; explicit null clears the department manager.
    pub manager_user_id: Option<Option<i64>>,
    pub sort_order: Option<i64>,
    pub color: Option<String>,
    pub show_in_work: Option<bool>,
    pub show_in_rd: Option<bool>,
    pub show_in_sample_info: Option<bool>,
    pub is_active: Option<bool>,
}
