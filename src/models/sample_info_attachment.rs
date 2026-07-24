use serde::{Deserialize, Serialize};

/// 附件 Model
#[derive(Debug, Serialize, Clone)]
pub struct SampleInfoAttachment {
    pub id: i64,
    pub record_id: i64,
    pub file_name: String,
    pub stored_name: String,
    pub file_size: i64,
    pub file_type: String,
    pub created_at: String,
}
