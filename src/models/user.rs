use serde::{Deserialize, Serialize};

/// 用户 Model
#[derive(Debug, Serialize, Clone)]
pub struct User {
    pub id: i64,
    pub username: String,
    #[serde(skip_serializing)]
    pub password: String,
    pub division_id: Option<i64>,
    pub division_name: Option<String>,
    pub group_id: Option<i64>,
    pub group_name: Option<String>,
    pub is_admin: bool,
    pub is_active: bool,
    /// 关联角色 id（NULL 表示未分配角色，无权限点；v0.4.74 后建议改用 role_ids）
    pub role_id: Option<i64>,
    pub role_name: Option<String>,
    /// v0.4.74: 多角色 id 列表
    pub role_ids: Vec<i64>,
    pub role_names: Vec<String>,
    /// Fixed user-only affiliations derived from assigned roles. They are not business laboratories.
    #[serde(default)]
    pub affiliation_groups: Vec<String>,
    /// 角色对应的权限点集合（JOIN role_permissions 加载；is_admin 时由 Claims 直接置为全部权限）
    #[serde(default)]
    pub permissions: Vec<String>,
    pub created_at: String,
    pub updated_at: Option<String>,
}

/// 登录请求
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub device_id: Option<String>,
    #[serde(default)]
    pub device_name: Option<String>,
}

/// 登录响应
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: User,
}

/// 注册请求
#[derive(Debug, Deserialize)]
pub struct UserCreate {
    pub username: String,
    pub password: String,
    pub division_id: Option<i64>,
    pub group_id: Option<i64>,
    /// 关联角色 id（可选，默认 NULL → 无权限点）
    pub role_id: Option<i64>,
    /// v0.4.74: 多角色 id
    #[serde(default)]
    pub role_ids: Vec<i64>,
}

/// 用户更新请求
#[derive(Debug, Deserialize)]
pub struct UserUpdate {
    pub username: Option<String>,
    pub password: Option<String>,
    pub division_id: Option<Option<i64>>,
    pub group_id: Option<Option<i64>>,
    pub is_admin: Option<bool>,
    pub is_active: Option<bool>,
    /// 关联角色 id：`Some(Some(id))` 设置角色，`Some(None)` 清空角色，`None` 不改动
    pub role_id: Option<Option<i64>>,
    /// v0.4.74: 多角色 id（Some=替换全部，None=不改动）
    pub role_ids: Option<Vec<i64>>,
}
