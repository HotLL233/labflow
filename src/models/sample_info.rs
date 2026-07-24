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
}

/// 创建请求
#[derive(Debug, Deserialize)]
pub struct SampleInfoCreate {
    pub batch_no: String,
    pub user_name: String,
    pub lab_name: String,
    pub project_name: String,
    pub submitted_at: Option<String>,
    pub detection_date: Option<String>,
    pub main_components: String,
    pub detection_type: String,
    pub type_key: String,
    pub division_id: Option<i64>,
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
#[derive(Debug, Deserialize)]
pub struct SampleInfoQuery {
    pub detection_type: Option<String>,
    pub type_key: Option<String>,
    pub status: Option<String>,
    pub user_name: Option<String>,
    pub lab_name: Option<String>,
    pub project_name: Option<String>,
    pub division_id: Option<i64>,
    pub group_id: Option<i64>,
    pub created_by_user_id: Option<i64>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub extra_fields: Option<String>,
    pub include_deleted: Option<bool>,
}

/// 状态流转请求
#[derive(Debug, Deserialize)]
pub struct SampleInfoStatusUpdate {
    pub status: String,
}
