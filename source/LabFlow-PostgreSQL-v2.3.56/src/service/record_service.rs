use crate::db::DbPool;
use crate::error::AppError;
use crate::models::record::{RecordCreate, RecordResponse, RecordUpdate};
use crate::repo;
use chrono::{Datelike, FixedOffset, NaiveDateTime, Timelike, Utc};

/// 取样工作量使用北京时间；旧客户端未传时间时沿用当前时间。
pub(crate) fn sample_workload_recorded_at(value: Option<&str>) -> Result<String, AppError> {
    let Some(value) = value else {
        return Ok(Utc::now()
            .with_timezone(&FixedOffset::east_opt(8 * 60 * 60).expect("valid UTC+8"))
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string());
    };
    let mut normalized = value.trim().to_string();
    if normalized.len() == 16 {
        normalized.push_str(":00");
    }
    let date = NaiveDateTime::parse_from_str(&normalized, "%Y-%m-%dT%H:%M:%S")
        .ok()
        .filter(|date| {
            (1..=9999).contains(&date.year())
                && date.nanosecond() == 0
                && date.format("%Y-%m-%dT%H:%M:%S").to_string() == normalized
        })
        .ok_or_else(|| {
            AppError::Validation(
                "工作量日期时间无效，请使用 YYYY-MM-DDTHH:mm 或 YYYY-MM-DDTHH:mm:ss".into(),
            )
        })?;
    Ok(date.format("%Y-%m-%dT%H:%M:%S").to_string())
}

/// Validate and create a work record
pub fn create_record(
    pool: &DbPool,
    input: &RecordCreate,
    operator: &str,
) -> Result<RecordResponse, AppError> {
    if input.quantity <= 0 {
        return Err(AppError::Validation("数量必须大于0".into()));
    }
    if input.multiplier.is_some_and(|value| value < 0.0) {
        return Err(AppError::Validation("单价倍率不能小于0".into()));
    }
    // Verify project exists (returns error if not found)
    repo::project_repo::get_by_id(pool, input.project_id)?;
    repo::record_repo::create(pool, input, operator)
}

/// 样品工作量允许未绑定仪器及明确自定义方法，标准项目/实验室/方法关系仍在事务内校验。
pub fn create_sample_workload_record(
    pool: &DbPool,
    input: &RecordCreate,
    operator: &str,
    custom: Option<(&str, Option<&str>)>,
) -> Result<RecordResponse, AppError> {
    if input.quantity <= 0 {
        return Err(AppError::Validation("数量必须大于0".into()));
    }
    if input.multiplier.is_some_and(|value| value < 0.0) {
        return Err(AppError::Validation("单价倍率不能小于0".into()));
    }
    repo::record_repo::create_sample_workload(pool, input, operator, custom)
}

/// Update a work record (with change detection and deleted check)
pub fn update_record(
    pool: &DbPool,
    id: i64,
    input: &RecordUpdate,
    user_name: &str,
) -> Result<RecordResponse, AppError> {
    let old = repo::record_repo::get_by_id(pool, id)?;

    if old.deleted_at.is_some() {
        return Err(AppError::Validation("记录已被删除，无法编辑".into()));
    }
    if input.multiplier.is_some_and(|value| value < 0.0) {
        return Err(AppError::Validation("单价倍率不能小于0".into()));
    }

    let un_changed = input
        .user_name
        .as_ref()
        .map_or(true, |u| u == &old.user_name);
    let qty_changed = input.quantity.map_or(true, |q| q == old.quantity);
    let dt_changed = input
        .recorded_at
        .as_ref()
        .map_or(true, |d| d == &old.recorded_at);
    let mul_changed = input
        .multiplier
        .map_or(true, |m| (m - old.multiplier).abs() < f64::EPSILON);
    let hi_changed = input
        .high_item
        .as_ref()
        .map_or(true, |h| Some(h.as_str()) == old.high_item.as_deref());
    if un_changed && qty_changed && dt_changed && mul_changed && hi_changed {
        return Err(AppError::Validation("没有需要更新的字段".into()));
    }

    repo::record_repo::update(pool, id, input, user_name)
}

/// Soft-delete a single record
pub fn delete_record(
    pool: &DbPool,
    id: i64,
    user_name: &str,
    reason: &str,
) -> Result<(), AppError> {
    repo::record_repo::soft_delete(pool, id, user_name, reason)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_workload_recorded_at_normalizes_native_inputs_and_defaults_to_beijing_now() {
        assert_eq!(
            sample_workload_recorded_at(Some("2028-02-29T09:05")).unwrap(),
            "2028-02-29T09:05:00"
        );
        assert_eq!(
            sample_workload_recorded_at(Some("2026-10-12T09:05:37")).unwrap(),
            "2026-10-12T09:05:37"
        );
        let offset = FixedOffset::east_opt(8 * 60 * 60).unwrap();
        let before = Utc::now()
            .with_timezone(&offset)
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string();
        let actual = sample_workload_recorded_at(None).unwrap();
        let after = Utc::now()
            .with_timezone(&offset)
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string();
        assert!(before <= actual && actual <= after);
    }

    #[test]
    fn sample_workload_recorded_at_rejects_invalid_and_non_native_values() {
        for value in [
            "",
            " ",
            "0000-01-01T00:00",
            "2026-02-29T00:00",
            "2028-02-30T00:00",
            "2026-04-31T00:00",
            "2026-10-12T24:00",
            "2026-10-12T12:60",
            "2026-10-12T23:59:60",
            "2026-10-12",
            "2026-10-12T09:05:00Z",
            "2026-10-12T09:05:00+08:00",
            "2026-1-12T09:05:00",
        ] {
            assert!(
                matches!(
                    sample_workload_recorded_at(Some(value)),
                    Err(AppError::Validation(_))
                ),
                "accepted {value}"
            );
        }
    }
}
