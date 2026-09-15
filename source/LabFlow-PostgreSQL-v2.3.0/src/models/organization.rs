use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Clone)]
pub struct DepartmentRole {
    pub id: i64,
    pub role_id: i64,
    pub role_name: String,
    pub template_name: Option<String>,
    pub division_id: i64,
    pub division_name: String,
    pub business_domain: String,
    pub is_department_admin: bool,
    pub is_active: bool,
    pub user_ids: Vec<i64>,
    pub user_names: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct DepartmentRoleCreate {
    pub role_id: i64,
    pub division_id: i64,
    #[serde(default = "default_business_domain")]
    pub business_domain: String,
    #[serde(default)]
    pub is_department_admin: bool,
}

#[derive(Debug, Deserialize)]
pub struct DepartmentRoleUpdate {
    pub business_domain: Option<String>,
    pub is_department_admin: Option<bool>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct DepartmentRoleUsersUpdate {
    #[serde(default)]
    pub user_ids: Vec<i64>,
}

#[derive(Debug, Serialize, Clone)]
pub struct OwnershipPendingRecord {
    pub record_type: String,
    pub record_id: i64,
    pub business_no: String,
    pub status: String,
    pub project_division_id: Option<i64>,
    pub execution_division_id: Option<i64>,
    pub execution_group_id: Option<i64>,
    pub submitted_division_id: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct OwnershipConfirmation {
    pub project_division_id: Option<i64>,
    pub execution_division_id: Option<i64>,
    pub execution_group_id: Option<i64>,
    pub submitted_division_id: Option<i64>,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct PermissionAuditRow {
    pub department_role_id: i64,
    pub division_name: String,
    pub business_domain: String,
    pub role_name: String,
    pub template_name: String,
    pub is_active: bool,
    pub is_department_admin: bool,
    pub assigned_users: Vec<String>,
    pub permissions: Vec<String>,
    pub division_scope_names: Vec<String>,
    pub work_division_scope_names: Vec<String>,
    pub sample_info_type_scope_names: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct PublicAccountAuditRow {
    pub username: String,
    pub division_names: Vec<String>,
    pub candidate_users: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct OwnershipMigrationSummary {
    pub record_type: String,
    pub total_records: i64,
    pub confirmed_records: i64,
    pub pending_records: i64,
    pub manually_confirmed_records: i64,
}

#[derive(Debug, Serialize, Clone)]
pub struct LegacyRoleGap {
    pub user_id: i64,
    pub username: String,
    pub is_active: bool,
    pub role_id: i64,
    pub role_name: String,
    pub template_name: String,
    pub primary_division_name: String,
    pub business_division_names: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct GovernanceOverview {
    pub permission_audit: Vec<PermissionAuditRow>,
    pub public_accounts: Vec<PublicAccountAuditRow>,
    pub ownership_migration: Vec<OwnershipMigrationSummary>,
    pub legacy_role_gaps: Vec<LegacyRoleGap>,
}

fn default_business_domain() -> String {
    "all".to_string()
}
