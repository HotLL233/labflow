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
    /// 用户主归属部门。division_id 是兼容旧客户端的同义字段。
    #[serde(default)]
    pub primary_division_id: Option<i64>,
    #[serde(default)]
    pub primary_division_name: Option<String>,
    /// 用户可工作的多个部门；division_id 保留为主部门兼容字段。
    #[serde(default)]
    pub division_ids: Vec<i64>,
    #[serde(default)]
    pub division_names: Vec<String>,
    /// 用户可承担业务的部门集合；与主归属部门分开表达。
    #[serde(default)]
    pub business_division_ids: Vec<i64>,
    #[serde(default)]
    pub business_division_names: Vec<String>,
    pub group_id: Option<i64>,
    pub group_name: Option<String>,
    /// 用户可工作的多个实验室；group_id 保留为主实验室兼容字段。
    #[serde(default)]
    pub group_ids: Vec<i64>,
    #[serde(default)]
    pub group_names: Vec<String>,
    pub is_admin: bool,
    pub is_active: bool,
    /// 关联角色 id（NULL 表示未分配角色，无权限点；v0.4.74 后建议改用 role_ids）
    pub role_id: Option<i64>,
    pub role_name: Option<String>,
    /// v0.4.74: 多角色 id 列表
    pub role_ids: Vec<i64>,
    pub role_names: Vec<String>,
    /// Stable internal role keys. These are never shown in the UI and prevent
    /// permission-sensitive workflows from depending on display names.
    #[serde(skip)]
    pub role_keys: Vec<String>,
    /// Names of the templates from which the assigned roles were created.
    /// Kept server-side so authorization can distinguish role identity from
    /// unrelated permissions such as statistics or user management.
    #[serde(skip)]
    pub role_template_names: Vec<String>,
    /// Legacy compatibility flag for the RD public-account workflow.
    #[serde(default)]
    pub is_public_account: bool,
    /// Whether this account is the built-in RD public account.
    #[serde(default)]
    pub is_rd_public_account: bool,
    /// Whether this account is the built-in analysis/detection public account.
    #[serde(default)]
    pub is_analysis_public_account: bool,
    /// Fixed user-only affiliations derived from assigned roles. They are not business laboratories.
    #[serde(default)]
    pub affiliation_groups: Vec<String>,
    /// 角色对应的权限点集合（JOIN role_permissions 加载；is_admin 时由 Claims 直接置为全部权限）
    #[serde(default)]
    pub permissions: Vec<String>,
    pub created_at: String,
    pub updated_at: Option<String>,
}

/// Eligible people exposed to an analysis public account. The configured
/// departments remain a server-side boundary and are not exposed as a
/// laboratory hierarchy in the portal.
#[derive(Debug, Serialize, Clone)]
pub struct AnalysisPublicAccountScope {
    pub accounts: Vec<AnalysisPublicAccountCandidate>,
}

#[derive(Debug, Serialize, Clone)]
pub struct AnalysisPublicAccountCandidate {
    pub id: i64,
    pub username: String,
    pub role_names: Vec<String>,
}

/// 登录请求
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    /// Persist this device's authenticated session for up to 30 days.
    #[serde(default)]
    pub keep_signed_in: bool,
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
    #[serde(default)]
    pub primary_division_id: Option<i64>,
    pub group_id: Option<i64>,
    #[serde(default)]
    pub division_ids: Vec<i64>,
    #[serde(default)]
    pub business_division_ids: Vec<i64>,
    #[serde(default)]
    pub group_ids: Vec<i64>,
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
    #[serde(default)]
    pub primary_division_id: Option<Option<i64>>,
    pub group_id: Option<Option<i64>>,
    #[serde(default)]
    pub division_ids: Option<Vec<i64>>,
    #[serde(default)]
    pub business_division_ids: Option<Vec<i64>>,
    #[serde(default)]
    pub group_ids: Option<Vec<i64>>,
    pub is_admin: Option<bool>,
    pub is_active: Option<bool>,
    /// 关联角色 id：`Some(Some(id))` 设置角色，`Some(None)` 清空角色，`None` 不改动
    pub role_id: Option<Option<i64>>,
    /// v0.4.74: 多角色 id（Some=替换全部，None=不改动）
    pub role_ids: Option<Vec<i64>>,
}
