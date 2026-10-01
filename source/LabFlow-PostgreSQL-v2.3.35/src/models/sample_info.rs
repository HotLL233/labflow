use serde::{Deserialize, Serialize};

/// 数据库行映射
#[derive(Debug, Serialize)]
pub struct SampleInfoRecord {
    pub id: i64,
    pub business_no: String,
    pub status: String,
    pub seq_no: i64,
    pub batch_no: String,
    pub user_name: String,
    pub lab_name: String,
    pub project_name: String,
    pub submitted_at: String,
    pub detection_date: String,
    pub sampled_by: String,
    pub sampled_at: Option<String>,
    pub detected_by: String,
    pub main_components: String,
    pub detection_type: String,
    pub type_key: String,
    pub division_id: Option<i64>,
    pub division_name: Option<String>,
    pub quantity: i64,
    pub extra_fields: Option<String>,
    pub notes: String,
    pub created_at: String,
    pub updated_at: Option<String>,
    pub deleted_at: Option<String>,
    pub group_id: Option<i64>,
    pub created_by_user_id: Option<i64>,
    pub return_reason: String,
    pub returned_by: String,
    pub returned_at: Option<String>,
    pub return_confirmed_by: String,
    pub return_confirmed_at: Option<String>,
    pub source_record_id: Option<i64>,
    pub project_division_id: Option<i64>,
    pub project_division_name_snapshot: String,
    pub execution_division_id: Option<i64>,
    pub execution_division_name_snapshot: String,
    pub execution_group_id: Option<i64>,
    pub execution_group_name_snapshot: String,
    pub submitted_division_id: Option<i64>,
    pub submitted_division_name_snapshot: String,
    pub business_user_id: Option<i64>,
    pub business_username_snapshot: String,
    pub ownership_status: String,
    pub workload_recorded: bool,
}

/// 创建请求
#[derive(Debug, Deserialize)]
pub struct SampleInfoCreate {
    #[serde(default)]
    pub batch_no: String,
    #[serde(default)]
    pub user_name: String,
    #[serde(default)]
    pub lab_name: String,
    #[serde(default)]
    pub project_name: String,
    pub submitted_at: Option<String>,
    pub detection_date: Option<String>,
    #[serde(default)]
    pub main_components: String,
    pub detection_type: String,
    pub type_key: String,
    pub division_id: Option<i64>,
    #[serde(default = "default_quantity")]
    pub quantity: i64,
    pub notes: Option<String>,
    pub extra_fields: Option<serde_json::Value>,
}

/// 查询响应（含所有字段，去掉 deleted_at）
#[derive(Debug, Serialize)]
pub struct SampleInfoResponse {
    pub id: i64,
    pub business_no: String,
    pub status: String,
    pub seq_no: i64,
    pub batch_no: String,
    pub user_name: String,
    pub lab_name: String,
    pub project_name: String,
    pub submitted_at: String,
    pub detection_date: String,
    pub sampled_by: String,
    pub sampled_at: Option<String>,
    pub detected_by: String,
    pub main_components: String,
    pub detection_type: String,
    pub type_key: String,
    pub division_id: Option<i64>,
    pub quantity: i64,
    pub division_name: Option<String>,
    pub extra_fields: Option<serde_json::Value>,
    pub notes: String,
    pub created_at: String,
    pub updated_at: Option<String>,
    pub deleted_at: Option<String>,
    pub group_id: Option<i64>,
    pub created_by_user_id: Option<i64>,
    pub return_reason: String,
    pub returned_by: String,
    pub returned_at: Option<String>,
    pub return_confirmed_by: String,
    pub return_confirmed_at: Option<String>,
    pub source_record_id: Option<i64>,
    pub project_division_id: Option<i64>,
    pub project_division_name_snapshot: String,
    pub execution_division_id: Option<i64>,
    pub execution_division_name_snapshot: String,
    pub execution_group_id: Option<i64>,
    pub execution_group_name_snapshot: String,
    pub submitted_division_id: Option<i64>,
    pub submitted_division_name_snapshot: String,
    pub business_user_id: Option<i64>,
    pub business_username_snapshot: String,
    pub ownership_status: String,
    pub workload_recorded: bool,
}

impl From<SampleInfoRecord> for SampleInfoResponse {
    fn from(r: SampleInfoRecord) -> Self {
        SampleInfoResponse {
            id: r.id,
            business_no: r.business_no,
            status: r.status,
            seq_no: r.seq_no,
            batch_no: r.batch_no,
            user_name: r.user_name,
            lab_name: r.lab_name,
            project_name: r.project_name,
            submitted_at: r.submitted_at,
            detection_date: r.detection_date,
            sampled_by: r.sampled_by,
            sampled_at: r.sampled_at,
            detected_by: r.detected_by,
            main_components: r.main_components,
            detection_type: r.detection_type,
            type_key: r.type_key,
            division_id: r.division_id,
            quantity: r.quantity,
            division_name: r.division_name,
            extra_fields: r.extra_fields.and_then(|s| serde_json::from_str(&s).ok()),
            notes: r.notes,
            created_at: r.created_at,
            updated_at: r.updated_at,
            deleted_at: r.deleted_at,
            group_id: r.group_id,
            created_by_user_id: r.created_by_user_id,
            return_reason: r.return_reason,
            returned_by: r.returned_by,
            returned_at: r.returned_at,
            return_confirmed_by: r.return_confirmed_by,
            return_confirmed_at: r.return_confirmed_at,
            source_record_id: r.source_record_id,
            project_division_id: r.project_division_id,
            project_division_name_snapshot: r.project_division_name_snapshot,
            execution_division_id: r.execution_division_id,
            execution_division_name_snapshot: r.execution_division_name_snapshot,
            execution_group_id: r.execution_group_id,
            execution_group_name_snapshot: r.execution_group_name_snapshot,
            submitted_division_id: r.submitted_division_id,
            submitted_division_name_snapshot: r.submitted_division_name_snapshot,
            business_user_id: r.business_user_id,
            business_username_snapshot: r.business_username_snapshot,
            ownership_status: r.ownership_status,
            workload_recorded: r.workload_recorded,
        }
    }
}

/// 更新请求
#[derive(Debug, Deserialize)]
pub struct SampleInfoUpdate {
    pub status: Option<String>,
    pub batch_no: Option<String>,
    pub user_name: Option<String>,
    pub lab_name: Option<String>,
    pub project_name: Option<String>,
    pub submitted_at: Option<String>,
    pub detection_date: Option<String>,
    pub main_components: Option<String>,
    pub division_id: Option<i64>,
    pub quantity: Option<i64>,
    pub notes: Option<String>,
    pub extra_fields: Option<serde_json::Value>,
}

/// 查询参数
#[derive(Debug, Default, Deserialize)]
pub struct SampleInfoQuery {
    pub detection_type: Option<String>,
    pub type_key: Option<String>,
    pub status: Option<String>,
    pub user_name: Option<String>,
    pub lab_name: Option<String>,
    pub project_name: Option<String>,
    pub division_id: Option<i64>,
    /// submitted (default), execution or project. This is only a reporting
    /// dimension; server-side role scope remains the authorization boundary.
    pub ownership_basis: Option<String>,
    pub include_pending_ownership: Option<bool>,
    pub group_id: Option<i64>,
    pub created_by_user_id: Option<i64>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub extra_fields: Option<String>,
    pub include_deleted: Option<bool>,
    /// 当前页面的临时排序条件；不写入数据库，也不影响其他用户。
    #[serde(default)]
    pub sort_by: Option<String>,
    #[serde(default)]
    pub sort_dir: Option<String>,
    /// 服务端根据当前登录角色写入，绝不接受浏览器传入的数据范围。
    #[serde(default, skip_deserializing)]
    pub scope_filters: Vec<SampleInfoScopeFilter>,
}

#[derive(Debug, Clone, Default)]
pub struct SampleInfoScopeFilter {
    pub division_ids: Vec<i64>,
    pub type_keys: Vec<String>,
    pub created_by_user_id: Option<i64>,
    pub business_user_id: Option<i64>,
}

fn default_quantity() -> i64 {
    1
}

/// 状态流转请求
#[derive(Debug, Deserialize)]
pub struct SampleInfoStatusUpdate {
    pub status: String,
}

#[cfg(test)]
mod tests {
    use super::SampleInfoCreate;

    #[test]
    fn hidden_configurable_fields_may_be_omitted() {
        let body: SampleInfoCreate = serde_json::from_value(serde_json::json!({
            "detection_type": "ICP",
            "type_key": "icp"
        }))
        .expect("hidden preset fields should use defaults");
        assert_eq!(body.main_components, "");
        assert_eq!(body.batch_no, "");
        assert_eq!(body.lab_name, "");
        assert_eq!(body.project_name, "");
        assert_eq!(body.quantity, 1);
    }
}
