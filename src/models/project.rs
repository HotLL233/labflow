use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct ProjectResponse {
    pub id: i64,
    pub name: String,
    pub full_name: String,
    pub notes: String,
    pub sort_order: i64,
    pub is_active: bool,
    pub lab_ids: Vec<i64>,
    pub lab_names: Vec<String>,
    pub method_ids: Vec<i64>,
    pub method_names: Vec<String>,
    pub created_at: String,
    // v0.4.64: 高项（纯文本，自定义填写）
    pub high_item: Option<String>,
    pub project_status: String,
    pub archived_at: Option<String>,
    pub archived_by: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ProjectCreate {
    pub name: String,
    pub full_name: Option<String>,
    pub notes: Option<String>,
    pub sort_order: Option<i64>,
    pub is_active: Option<bool>,
    pub lab_ids: Option<Vec<i64>>,
    pub method_ids: Option<Vec<i64>>,
    pub high_item: Option<String>,
    pub project_status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ProjectUpdate {
    pub name: Option<String>,
    pub full_name: Option<String>,
    pub notes: Option<String>,
    pub sort_order: Option<i64>,
    pub is_active: Option<bool>,
    pub lab_ids: Option<Vec<i64>>,
    pub method_ids: Option<Vec<i64>>,
    pub high_item: Option<String>,
    pub project_status: Option<String>,
}

// ── 方法类型 (method_types 表) ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodType {
    pub id: i64,
    pub name: String,
    pub sort_order: i64,
}

#[derive(Debug, Deserialize)]
pub struct MethodTypeCreate {
    pub name: String,
    pub sort_order: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct MethodTypeUpdate {
    pub name: Option<String>,
    pub sort_order: Option<i64>,
}

// ── 导入相关 ──

#[derive(Debug, Clone, Deserialize)]
pub struct MethodImportItem {
    pub group_name: String,
    pub project_name: String,
    pub method_name: String,
    pub method_type: String,
    pub coefficient: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportSummary {
    pub total_methods: usize,
    pub total_projects: usize,
    pub total_groups: usize,
    pub by_type: Vec<TypeCount>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TypeCount {
    pub method_type: String,
    pub count: usize,
}
