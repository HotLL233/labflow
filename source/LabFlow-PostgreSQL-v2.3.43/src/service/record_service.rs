use crate::db::DbPool;
use crate::error::AppError;
use crate::models::record::{RecordCreate, RecordResponse, RecordUpdate};
use crate::repo;

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

/// v2.3.13: 样品信息取样工作量录入。
/// 使用放宽的项目/方法绑定校验，让未配置项目-方法关联或方法未绑定仪器的历史数据也能补录工作量。
/// v2.3.14: 方法库检索不到候选时，可通过 `custom` 传入自定义的方法名与仪器名
/// （`(方法名, 可选仪器名)`），此时 `input.method_id` 为 `None`。
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
