use serde::{Deserialize, Serialize};

fn nullable_i64<'de, D>(deserializer: D) -> Result<Option<Option<i64>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::<i64>::deserialize(deserializer)?))
}

#[derive(Debug, Serialize)]
pub struct GroupResponse {
    pub id: i64,
    pub name: String,
    pub sort_order: i64,
    pub created_at: String,
    pub project_count: i64,
    pub project_names: Option<String>,
    pub rd_record_count: Option<i64>,
    pub returned_sender_names: Option<String>,
    pub show_in_work: bool,
    pub show_in_rd: bool,
    pub show_in_sample_info: bool,
    pub division_id: Option<i64>,
    pub division_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GroupCreate {
    pub name: String,
    pub sort_order: Option<i64>,
    pub show_in_work: Option<bool>,
    pub show_in_rd: Option<bool>,
    pub show_in_sample_info: Option<bool>,
    pub division_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct GroupUpdate {
    pub name: Option<String>,
    pub sort_order: Option<i64>,
    pub show_in_work: Option<bool>,
    pub show_in_rd: Option<bool>,
    pub show_in_sample_info: Option<bool>,
    #[serde(default, deserialize_with = "nullable_i64")]
    pub division_id: Option<Option<i64>>,
}

#[cfg(test)]
mod tests {
    use super::GroupUpdate;

    #[test]
    fn group_update_keeps_null_as_explicit_division_clear() {
        assert_eq!(
            serde_json::from_str::<GroupUpdate>(r#"{}"#)
                .unwrap()
                .division_id,
            None
        );
        assert_eq!(
            serde_json::from_str::<GroupUpdate>(r#"{"division_id":null}"#)
                .unwrap()
                .division_id,
            Some(None)
        );
        assert_eq!(
            serde_json::from_str::<GroupUpdate>(r#"{"division_id":7}"#)
                .unwrap()
                .division_id,
            Some(Some(7))
        );
    }
}
