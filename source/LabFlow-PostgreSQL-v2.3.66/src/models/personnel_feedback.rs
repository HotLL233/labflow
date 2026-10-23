use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Clone)]
pub struct PersonnelFeedback {
    pub id: i64,
    pub notice_no: String,
    pub lab_id: i64,
    pub lab_name: String,
    pub responsible_user_id: Option<i64>,
    pub responsible_name: String,
    pub change_type: String,
    pub person_name: String,
    pub effective_at: String,
    pub notes: String,
    pub status: String,
    pub project_ids: Vec<i64>,
    pub project_names: Vec<String>,
    pub created_by_user_id: i64,
    pub created_by_username: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct PersonnelFeedbackCreate {
    pub lab_id: i64,
    pub responsible_user_id: Option<i64>,
    pub change_type: String,
    pub person_name: String,
    pub effective_at: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub project_ids: Vec<i64>,
}

#[derive(Debug, Deserialize)]
pub struct PersonnelFeedbackUpdate {
    pub responsible_user_id: Option<Option<i64>>,
    pub change_type: Option<String>,
    pub person_name: Option<String>,
    pub effective_at: Option<String>,
    pub notes: Option<String>,
    pub project_ids: Option<Vec<i64>>,
}
