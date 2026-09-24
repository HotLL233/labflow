use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::rd_record_column::{
    RdOptionDetailRule, RdRecordColumn, RdRecordColumnCreate, RdRecordColumnUpdate,
};
use crate::repo::audit_repo;

const CORE_REQUIRED_FIELDS: &[&str] = &[
    "user_name",
    "project_name",
    "detection_type",
    "method_name",
    "quantity",
];
const ALLOWED_TYPES: &[&str] = &[
    "text",
    "textarea",
    "number",
    "date",
    "datetime",
    "select",
    "select_other",
];

pub fn list_all(pool: &DbPool) -> Result<Vec<RdRecordColumn>> {
    let conn = pool.get()?;
    list_all_on_conn(&conn)
}

fn list_all_on_conn(conn: &postgres_compat::Connection) -> Result<Vec<RdRecordColumn>> {
    let mut stmt = conn.prepare(
        "SELECT id,name,label,data_type,width,sort_order,is_predefined,is_required,is_active,\
         show_in_list,show_in_form,show_in_export,options,option_detail_rules,default_value,placeholder,applicable_types,entry_row,\
         width_mode,min_width,max_width,created_at,updated_at \
         FROM rd_record_columns ORDER BY sort_order ASC,id ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(RdRecordColumn {
            id: row.get(0)?,
            name: row.get(1)?,
            label: row.get(2)?,
            data_type: row.get(3)?,
            width: row.get(4)?,
            sort_order: row.get(5)?,
            is_predefined: row.get::<_, i64>(6)? != 0,
            is_required: row.get::<_, i64>(7)? != 0,
            is_active: row.get::<_, i64>(8)? != 0,
            show_in_list: row.get::<_, i64>(9)? != 0,
            show_in_form: row.get::<_, i64>(10)? != 0,
            show_in_export: row.get::<_, i64>(11)? != 0,
            options: row.get::<_, String>(12).unwrap_or_default(),
            option_detail_rules: row.get::<_, String>(13).unwrap_or_default(),
            default_value: row.get::<_, String>(14).unwrap_or_default(),
            placeholder: row.get::<_, String>(15).unwrap_or_default(),
            applicable_types: row.get::<_, String>(16).unwrap_or_default(),
            entry_row: row.get::<_, i64>(17).unwrap_or(1),
            // 老库升级后新列已由迁移补齐；此处再兜底一次，避免任何读路径炸掉。
            width_mode: row
                .get::<_, String>(18)
                .unwrap_or_else(|_| "auto".to_string()),
            min_width: row.get::<_, i64>(19).unwrap_or(0),
            max_width: row.get::<_, i64>(20).unwrap_or(0),
            created_at: row.get(21)?,
            updated_at: row.get(22)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

pub fn list_active_in_list(pool: &DbPool) -> Result<Vec<RdRecordColumn>> {
    Ok(list_all(pool)?
        .into_iter()
        .filter(|c| c.is_active && c.show_in_list)
        .collect())
}

pub fn list_active_in_form(pool: &DbPool) -> Result<Vec<RdRecordColumn>> {
    Ok(list_all(pool)?
        .into_iter()
        .filter(|c| c.is_active && c.show_in_form)
        .collect())
}

fn validate_data_type(value: &str) -> Result<()> {
    if ALLOWED_TYPES.contains(&value) {
        Ok(())
    } else {
        Err(AppError::Validation("不支持的字段输入方式".into()))
    }
}

fn validate_name(value: &str) -> Result<String> {
    let name = value.trim().to_ascii_lowercase();
    if name.is_empty()
        || name.len() > 64
        || !name
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
    {
        return Err(AppError::Validation(
            "字段标识仅支持小写字母、数字和下划线，长度不超过 64".into(),
        ));
    }
    if CORE_REQUIRED_FIELDS.contains(&name.as_str())
        || [
            "seq_no",
            "lab_name",
            "division_id",
            "submitted_at",
            "sampling_person",
            "sampling_time",
            "status",
            "instrument_code",
            "high_item",
            "batch_no",
            "notes",
        ]
        .contains(&name.as_str())
    {
        return Err(AppError::Validation(
            "该字段标识属于系统预置字段，不可用于新增字段".into(),
        ));
    }
    Ok(name)
}

fn validate_label(value: &str) -> Result<String> {
    let label = value.trim().to_string();
    if label.is_empty() || label.chars().count() > 40 {
        return Err(AppError::Validation(
            "字段名称不能为空且不超过 40 个字符".into(),
        ));
    }
    Ok(label)
}

fn normalize_width(value: i64) -> Result<i64> {
    if !(48..=500).contains(&value) {
        return Err(AppError::Validation("列宽需在 48 到 500 之间".into()));
    }
    Ok(value)
}

/// v2.3.20：列宽模式。`auto` 由前端按内容测量，`custom` 固定使用 `width`。
fn normalize_width_mode(value: &str) -> Result<String> {
    let mode = value.trim().to_ascii_lowercase();
    if mode != "auto" && mode != "custom" {
        return Err(AppError::Validation("列宽模式仅支持自动或自定义".into()));
    }
    Ok(mode)
}

/// v2.3.20：0 表示沿用前端按输入方式给出的系统默认区间。
fn normalize_custom_bound(value: i64) -> Result<i64> {
    if value == 0 {
        return Ok(0);
    }
    if !(32..=1200).contains(&value) {
        return Err(AppError::Validation(
            "自定义宽度区间需在 32 到 1200 之间，或填 0 表示沿用默认".into(),
        ));
    }
    Ok(value)
}

fn normalize_bounds(min_width: i64, max_width: i64) -> Result<()> {
    if min_width != 0 && max_width != 0 && min_width > max_width {
        return Err(AppError::Validation("列宽下限不能大于上限".into()));
    }
    Ok(())
}

fn configured_options(source: &str) -> Vec<String> {
    let raw = source.trim();
    if raw.is_empty() {
        return Vec::new();
    }
    if let Ok(values) = serde_json::from_str::<Vec<String>>(raw) {
        return values
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect();
    }
    raw.split(|ch| ch == ',' || ch == '，' || ch == ';' || ch == '；' || ch == '\n')
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn normalize_option_detail_rules(data_type: &str, options: &str, source: &str) -> Result<String> {
    let raw = source.trim();
    if raw.is_empty() {
        return if data_type == "select_other" {
            serde_json::to_string(&vec![RdOptionDetailRule {
                trigger_value: "其他".into(),
                label: "补充说明".into(),
                placeholder: "请填写补充说明".into(),
                required: false,
            }])
            .map_err(|error| AppError::Internal(error.to_string()))
        } else {
            Ok(String::new())
        };
    }
    if data_type != "select" && data_type != "select_other" {
        return Err(AppError::Validation(
            "仅下拉选择字段可配置选项补充内容".into(),
        ));
    }
    let available = configured_options(options);
    if available.is_empty() {
        return Err(AppError::Validation(
            "请先配置下拉选项，再设置选项补充内容".into(),
        ));
    }
    let mut rules: Vec<RdOptionDetailRule> = serde_json::from_str(raw)
        .map_err(|_| AppError::Validation("选项补充规则格式错误，请重新配置".into()))?;
    let mut seen = std::collections::HashSet::new();
    for rule in &mut rules {
        rule.trigger_value = rule.trigger_value.trim().to_string();
        rule.label = rule.label.trim().to_string();
        rule.placeholder = rule.placeholder.trim().to_string();
        if rule.trigger_value.is_empty()
            || !available.iter().any(|item| item == &rule.trigger_value)
        {
            return Err(AppError::Validation(
                "补充规则的触发选项必须来自当前下拉选项".into(),
            ));
        }
        if !seen.insert(rule.trigger_value.clone()) {
            return Err(AppError::Validation("同一选项只能配置一条补充规则".into()));
        }
        if rule.label.is_empty() || rule.label.chars().count() > 40 {
            return Err(AppError::Validation(
                "补充项名称不能为空且不超过 40 个字符".into(),
            ));
        }
        if rule.placeholder.chars().count() > 120 {
            return Err(AppError::Validation("补充项提示不超过 120 个字符".into()));
        }
    }
    serde_json::to_string(&rules).map_err(|error| AppError::Internal(error.to_string()))
}

pub fn create(
    pool: &DbPool,
    data: &RdRecordColumnCreate,
    operator: &str,
) -> Result<RdRecordColumn> {
    let name = validate_name(&data.name)?;
    let label = validate_label(&data.label)?;
    validate_data_type(data.data_type.trim())?;
    let width = normalize_width(data.width.unwrap_or(140))?;
    let width_mode = normalize_width_mode(&data.width_mode)?;
    let min_width = normalize_custom_bound(data.min_width)?;
    let max_width = normalize_custom_bound(data.max_width)?;
    normalize_bounds(min_width, max_width)?;
    if !(1..=2).contains(&data.entry_row) {
        return Err(AppError::Validation("表单行仅支持 1 或 2".into()));
    }
    let option_detail_rules = normalize_option_detail_rules(
        data.data_type.trim(),
        data.options.trim(),
        data.option_detail_rules.trim(),
    )?;
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let next_sort: i64 = tx.query_row(
        "SELECT COALESCE(MAX(sort_order),0)+1 FROM rd_record_columns",
        [],
        |row| row.get(0),
    )?;
    tx.execute(
        "INSERT INTO rd_record_columns(name,label,data_type,width,sort_order,is_predefined,is_required,is_active,show_in_list,show_in_form,show_in_export,options,option_detail_rules,default_value,placeholder,applicable_types,entry_row,width_mode,min_width,max_width,created_at,updated_at) \
         VALUES(?1,?2,?3,?4,?5,0,?6,1,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,datetime('now','localtime'),datetime('now','localtime'))",
        postgres_compat::params![name,label,data.data_type.trim(),width,next_sort,data.is_required as i64,data.show_in_list as i64,data.show_in_form as i64,data.show_in_export as i64,data.options.trim(),option_detail_rules,data.default_value.trim(),data.placeholder.trim(),data.applicable_types.trim(),data.entry_row,width_mode,min_width,max_width],
    )?;
    let id = tx.last_insert_rowid();
    let created = list_all_on_conn(&tx)?
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| AppError::Internal("新增字段失败".into()))?;
    audit_repo::log_structured_on_conn(
        &tx,
        "create",
        "rd_record_columns",
        Some(id),
        operator,
        &format!("新增研发送样自定义列「{}」", created.label),
        "rd",
        "",
        None,
        Some(&serde_json::to_value(&created).unwrap_or_default()),
        "management",
    )?;
    tx.commit()?;
    Ok(created)
}

pub fn update(
    pool: &DbPool,
    id: i64,
    data: &RdRecordColumnUpdate,
    operator: &str,
) -> Result<RdRecordColumn> {
    let mut conn = pool.get()?;
    let existing = list_all_on_conn(&conn)?
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| AppError::NotFound("列不存在".into()))?;
    let label = match &data.label {
        Some(value) => validate_label(value)?,
        None => existing.label.clone(),
    };
    let data_type = data
        .data_type
        .as_deref()
        .unwrap_or(&existing.data_type)
        .trim()
        .to_string();
    validate_data_type(&data_type)?;
    let width = normalize_width(data.width.unwrap_or(existing.width))?;
    // v2.3.20：未传 width_mode 时保留原值，避免部分更新把「自定义」意外重置为「自动」。
    let width_mode = match &data.width_mode {
        Some(value) => normalize_width_mode(value)?,
        None => normalize_width_mode(&existing.width_mode)?,
    };
    let min_width = normalize_custom_bound(data.min_width.unwrap_or(existing.min_width))?;
    let max_width = normalize_custom_bound(data.max_width.unwrap_or(existing.max_width))?;
    normalize_bounds(min_width, max_width)?;
    let sort_order = data.sort_order.unwrap_or(existing.sort_order).max(0);
    let is_required = data.is_required.unwrap_or(existing.is_required);
    let is_active = data.is_active.unwrap_or(existing.is_active);
    let show_in_list = data.show_in_list.unwrap_or(existing.show_in_list);
    let show_in_form = data.show_in_form.unwrap_or(existing.show_in_form);
    let show_in_export = data.show_in_export.unwrap_or(existing.show_in_export);
    let entry_row = data.entry_row.unwrap_or(existing.entry_row);
    if !(1..=2).contains(&entry_row) {
        return Err(AppError::Validation("表单行仅支持 1 或 2".into()));
    }
    if CORE_REQUIRED_FIELDS.contains(&existing.name.as_str())
        && (!is_active || !show_in_form || !is_required)
    {
        return Err(AppError::Validation(
            "送样人、项目、检测类型、方法和数量为核心提交字段，必须启用、在表单显示且为必填".into(),
        ));
    }
    let options = data
        .options
        .as_deref()
        .unwrap_or(&existing.options)
        .trim()
        .to_string();
    let option_detail_rules = normalize_option_detail_rules(
        &data_type,
        &options,
        data.option_detail_rules
            .as_deref()
            .unwrap_or(&existing.option_detail_rules),
    )?;
    let before = serde_json::to_value(&existing).unwrap_or_default();
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE rd_record_columns SET label=?1,data_type=?2,width=?3,sort_order=?4,is_required=?5,is_active=?6,show_in_list=?7,show_in_form=?8,show_in_export=?9,options=?10,option_detail_rules=?11,default_value=?12,placeholder=?13,applicable_types=?14,entry_row=?15,width_mode=?16,min_width=?17,max_width=?18,updated_at=datetime('now','localtime') WHERE id=?19",
        postgres_compat::params![label,data_type,width,sort_order,is_required as i64,is_active as i64,show_in_list as i64,show_in_form as i64,show_in_export as i64,options,option_detail_rules,data.default_value.as_deref().unwrap_or(&existing.default_value).trim(),data.placeholder.as_deref().unwrap_or(&existing.placeholder).trim(),data.applicable_types.as_deref().unwrap_or(&existing.applicable_types).trim(),entry_row,width_mode,min_width,max_width,id],
    )?;
    let updated = list_all_on_conn(&tx)?
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| AppError::Internal("更新列失败".into()))?;
    audit_repo::log_structured_on_conn(
        &tx,
        "update",
        "rd_record_columns",
        Some(id),
        operator,
        &format!("更新研发送样列配置「{}」", updated.label),
        "rd",
        "",
        Some(&before),
        Some(&serde_json::to_value(&updated).unwrap_or_default()),
        "management",
    )?;
    tx.commit()?;
    Ok(updated)
}

pub fn reorder(pool: &DbPool, ids: &[i64], operator: &str) -> Result<Vec<RdRecordColumn>> {
    if ids.is_empty() {
        return Err(AppError::Validation("请至少保留一个字段".into()));
    }
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let all = list_all_on_conn(&tx)?;
    if ids.len() != all.len()
        || ids
            .iter()
            .any(|id| !all.iter().any(|column| column.id == *id))
    {
        return Err(AppError::Validation(
            "字段排序数据不完整，请刷新后重试".into(),
        ));
    }
    for (index, id) in ids.iter().enumerate() {
        tx.execute("UPDATE rd_record_columns SET sort_order=?1,updated_at=datetime('now','localtime') WHERE id=?2", postgres_compat::params![index as i64 + 1,id])?;
    }
    let updated = list_all_on_conn(&tx)?;
    audit_repo::log_structured_on_conn(
        &tx,
        "reorder",
        "rd_record_columns",
        None,
        operator,
        "调整研发送样字段顺序",
        "rd",
        "",
        Some(&serde_json::to_value(&all).unwrap_or_default()),
        Some(&serde_json::to_value(&updated).unwrap_or_default()),
        "management",
    )?;
    tx.commit()?;
    Ok(updated)
}

pub fn delete(pool: &DbPool, id: i64, operator: &str) -> Result<()> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let existing = list_all_on_conn(&tx)?
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| AppError::NotFound("列不存在".into()))?;
    if existing.is_predefined {
        return Err(AppError::Validation(
            "系统预置字段不可删除，可停用自定义字段".into(),
        ));
    }
    tx.execute("DELETE FROM rd_record_columns WHERE id=?1", [id])?;
    audit_repo::log_structured_on_conn(
        &tx,
        "delete",
        "rd_record_columns",
        Some(id),
        operator,
        &format!(
            "删除研发送样自定义列「{}」，历史记录数据仍保留",
            existing.label
        ),
        "rd",
        "",
        Some(&serde_json::to_value(&existing).unwrap_or_default()),
        None,
        "management",
    )?;
    tx.commit()?;
    Ok(())
}
