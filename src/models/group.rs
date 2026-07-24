use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct GroupResponse {
    pub id: i64,
    pub name: String,
    pub sort_order: i64,
    pub created_at: String,
    pub project_count: i64,
    pub project_names: Option<String>,
    pub rd_record_count: Option<i64>,
    pub show_in_work: bool,
    pub show_in_rd: bool,
    pub division_id: Option<i64>,
    pub division_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GroupCreate {
    pub name: String,
    pub sort_order: Option<i64>,
    pub show_in_work: Option<bool>,
    pub show_in_rd: Option<bool>,
    pub division_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct GroupUpdate {
    pub name: Option<String>,
    pub sort_order: Option<i64>,
    pub show_in_work: Option<bool>,
    pub show_in_rd: Option<bool>,
    // Option<Option<i64>>: None=不修改；Some(None)=置空(未分配)；Some(Some(v))=设为指定事业部
    pub division_id: Option<Option<i64>>,
}
