use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportMapping {
    pub id: i64,
    pub header_pattern: String,
    pub match_mode: String,
    pub target_table: String,
    pub default_type: String,
    pub priority: i64,
    pub is_active: bool,
}
