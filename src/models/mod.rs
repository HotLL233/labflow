pub mod article;
pub mod audit;
pub mod division;
pub mod group;
pub mod help;
pub mod import;
pub mod import_record;
pub mod instrument;
pub mod method;
pub mod notification;
pub mod personnel_feedback;
pub mod project;
pub mod rd_record;
pub mod rd_record_column;
pub mod record;
pub mod role;
pub mod sample_info;
pub mod sample_info_attachment;
pub mod sample_info_column;
pub mod sample_info_column_visibility;
pub mod sample_info_type;
pub mod settings;
pub mod trace;
pub mod trash;
pub mod user;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            code: 0,
            message: "ok".into(),
            data: Some(data),
        }
    }
    pub fn ok_msg(msg: impl Into<String>) -> Self {
        Self {
            code: 0,
            message: msg.into(),
            data: None,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T: Serialize> {
    pub items: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Deserialize)]
pub struct Pagination {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}
