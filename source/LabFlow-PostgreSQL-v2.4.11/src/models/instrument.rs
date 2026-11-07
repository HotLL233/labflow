use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct InstrumentResponse {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub instrument_type: String,
    pub is_active: bool,
    pub notes: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct InstrumentCreate {
    pub code: String,
    pub name: Option<String>,
    pub instrument_type: String,
    pub is_active: Option<bool>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct InstrumentUpdate {
    pub code: Option<String>,
    pub name: Option<String>,
    pub instrument_type: Option<String>,
    pub is_active: Option<bool>,
    pub notes: Option<String>,
}
