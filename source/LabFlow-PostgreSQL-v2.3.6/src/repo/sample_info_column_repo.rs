use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::sample_info_column::*;
use crate::repo::{audit_repo, trash_repo};
use postgres_compat::params;

fn is_configured_action(field_key: &str) -> bool {
    matches!(
        field_key,
        "action_edit" | "action_record_workload" | "action_return"
    )
}

pub fn list_all(pool: &DbPool) -> Result<Vec<SampleInfoColumn>> {
    let conn = pool.get()?;
    list_all_on_conn(&conn)
}

fn list_all_on_conn(conn: &postgres_compat::Connection) -> Result<Vec<SampleInfoColumn>> {
    let mut stmt = conn.prepare(
        "SELECT c.id, c.field_key, c.label, c.data_type, c.is_predefined, c.is_required, c.is_active, \
         c.width, c.sort_order, c.options, c.show_in_list, c.show_in_export, c.show_in_form, \
         c.type_key, c.created_at, c.updated_at, \
          COALESCE((SELECT string_agg(v.type_key, ',' ORDER BY v.type_key) FROM sample_info_column_visibility v WHERE v.column_id=c.id AND v.is_visible=1), '') AS visible_types, \
          COALESCE((SELECT string_agg(v.type_key, ',' ORDER BY v.type_key) FROM sample_info_column_visibility v WHERE v.column_id=c.id AND v.is_visible=1 AND v.is_required=1), '') AS required_types \
         FROM sample_info_columns c WHERE c.deleted_at IS NULL ORDER BY c.sort_order ASC"
    )?;
    let rows = stmt.query_map([], |row| {
        let vt: String = row.get(16).unwrap_or_default();
        let rt: String = row.get(17).unwrap_or_default();
        Ok(SampleInfoColumn {
            id: row.get(0)?,
            field_key: row.get(1)?,
            label: row.get(2)?,
            data_type: row.get(3)?,
            is_predefined: row.get::<_, i64>(4)? != 0,
            is_required: row.get::<_, i64>(5)? != 0,
            is_active: row.get::<_, i64>(6)? != 0,
            width: row.get(7)?,
            sort_order: row.get(8)?,
            options: row.get(9)?,
            show_in_list: row.get::<_, i64>(10)? != 0,
            show_in_export: row.get::<_, i64>(11)? != 0,
            show_in_form: row.get::<_, i64>(12)? != 0,
            type_key: row.get(13)?,
            created_at: row.get(14)?,
            updated_at: row.get(15)?,
            visible_types: vt
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect(),
            required_types: rt
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect(),
        })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

pub fn list_active(pool: &DbPool) -> Result<Vec<SampleInfoColumn>> {
    let items = list_all(pool)?;
    Ok(items.into_iter().filter(|c| c.is_active).collect())
}

fn list_active_common_by_usage(pool: &DbPool, for_export: bool) -> Result<Vec<SampleInfoColumn>> {
    let conn = pool.get()?;
    let mut type_stmt = conn.prepare(
        "SELECT type_key FROM sample_info_types WHERE is_active=1 AND deleted_at IS NULL ORDER BY sort_order,id",
    )?;
    let type_keys = type_stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if type_keys.is_empty() {
        return list_active(pool);
    }

    let mut columns = list_active(pool)?;
    for type_key in type_keys {
        let visible = list_active_by_type(pool, &type_key)?
            .into_iter()
            .filter(|column| {
                if for_export {
                    column.show_in_export
                } else {
                    column.show_in_list
                }
            })
            .map(|column| column.field_key)
            .collect::<std::collections::HashSet<_>>();
        columns.retain(|column| visible.contains(&column.field_key));
    }
    Ok(columns)
}

/// 未选择具体检测类型时，只返回所有启用类型共同显示的列表列。
pub fn list_active_common_for_list(pool: &DbPool) -> Result<Vec<SampleInfoColumn>> {
    list_active_common_by_usage(pool, false)
}

/// 未选择具体检测类型时，只返回所有启用类型共同显示的导出列。
pub fn list_active_common_for_export(pool: &DbPool) -> Result<Vec<SampleInfoColumn>> {
    list_active_common_by_usage(pool, true)
}

/// 按检测类型加载启用的列
pub fn list_active_by_type(pool: &DbPool, type_key: &str) -> Result<Vec<SampleInfoColumn>> {
    let conn = pool.get()?;
    let sql = "
        SELECT c.id, c.field_key, c.label, c.data_type, c.is_predefined, c.is_required, c.is_active,
               c.width, COALESCE(v.sort_order,c.sort_order), c.options,
               COALESCE(v.show_in_list,c.show_in_list), COALESCE(v.show_in_export,c.show_in_export),
               COALESCE(v.show_in_form,c.show_in_form), c.type_key, c.created_at, c.updated_at,
               COALESCE(v.is_required,c.is_required)
        FROM sample_info_columns c
        LEFT JOIN sample_info_column_visibility v ON v.column_id = c.id AND v.type_key = ?1
        WHERE c.is_active = 1 AND c.deleted_at IS NULL
          AND COALESCE(v.is_visible, 0) = 1
        ORDER BY COALESCE(v.sort_order,c.sort_order) ASC,c.id ASC
    ";
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([type_key], |row| {
        Ok(SampleInfoColumn {
            id: row.get(0)?,
            field_key: row.get(1)?,
            label: row.get(2)?,
            data_type: row.get(3)?,
            is_predefined: row.get::<_, i64>(4)? != 0,
            is_required: row.get::<_, i64>(16)? != 0,
            is_active: row.get::<_, i64>(6)? != 0,
            width: row.get(7)?,
            sort_order: row.get(8)?,
            options: row.get(9)?,
            show_in_list: row.get::<_, i64>(10)? != 0,
            show_in_export: row.get::<_, i64>(11)? != 0,
            show_in_form: row.get::<_, i64>(12)? != 0,
            type_key: row.get(13)?,
            created_at: row.get(14)?,
            updated_at: row.get(15)?,
            visible_types: vec![],
            required_types: vec![],
        })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

/// 管理页专用：所有列 + visibility 信息（LEFT JOIN）
pub fn list_all_with_visibility(
    pool: &DbPool,
    type_key: &str,
) -> Result<Vec<(SampleInfoColumn, bool)>> {
    let conn = pool.get()?;
    let sql = "
        SELECT c.id, c.field_key, c.label, c.data_type, c.is_predefined, c.is_required, c.is_active,
               c.width, COALESCE(v.sort_order,c.sort_order), c.options,
               COALESCE(v.show_in_list,c.show_in_list), COALESCE(v.show_in_export,c.show_in_export),
               COALESCE(v.show_in_form,c.show_in_form), c.type_key, c.created_at, c.updated_at,
               COALESCE(v.is_visible,0), COALESCE(v.is_required,c.is_required)
        FROM sample_info_columns c
        LEFT JOIN sample_info_column_visibility v ON v.column_id = c.id AND v.type_key = ?1
        WHERE c.deleted_at IS NULL
        ORDER BY COALESCE(v.sort_order,c.sort_order) ASC,c.id ASC
    ";
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([type_key], |row| {
        let col = SampleInfoColumn {
            id: row.get(0)?,
            field_key: row.get(1)?,
            label: row.get(2)?,
            data_type: row.get(3)?,
            is_predefined: row.get::<_, i64>(4)? != 0,
            is_required: row.get::<_, i64>(17)? != 0,
            is_active: row.get::<_, i64>(6)? != 0,
            width: row.get(7)?,
            sort_order: row.get(8)?,
            options: row.get(9)?,
            show_in_list: row.get::<_, i64>(10)? != 0,
            show_in_export: row.get::<_, i64>(11)? != 0,
            show_in_form: row.get::<_, i64>(12)? != 0,
            type_key: row.get(13)?,
            created_at: row.get(14)?,
            updated_at: row.get(15)?,
            visible_types: vec![],
            required_types: vec![],
        };
        let visible: i64 = row.get(16)?;
        Ok((col, visible != 0))
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

pub fn create(pool: &DbPool, data: &ColumnCreate, operator: &str) -> Result<SampleInfoColumn> {
    let mut conn = pool.get()?;
    if data.data_type == "action" || is_configured_action(&data.field_key) {
        return Err(AppError::Validation(
            "操作按钮为系统预置字段，不支持新增".into(),
        ));
    }
    let field_key = &data.field_key;
    let label = &data.label;
    let data_type = &data.data_type;
    let width = data.width.unwrap_or(100);
    let sort_order = data.sort_order.unwrap_or(0);
    let is_required = data.is_required.unwrap_or(false);
    let show_in_list = data.show_in_list.unwrap_or(true);
    let show_in_export = data.show_in_export.unwrap_or(true);
    let show_in_form = data.show_in_form.unwrap_or(true);

    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO sample_info_columns (field_key, label, data_type, type_key, is_predefined, is_required, is_active, width, sort_order, options, show_in_list, show_in_export, show_in_form) \
         VALUES (?1, ?2, ?3, ?4, 0, ?5, 1, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            field_key, label, data_type, data.type_key,
            is_required as i64, width, sort_order, data.options,
            show_in_list as i64, show_in_export as i64, show_in_form as i64,
        ],
    )?;
    let id = tx.last_insert_rowid();
    // v0.4.72: 新列默认对所有活跃类型可见
    tx.execute(
        "INSERT OR IGNORE INTO sample_info_column_visibility \
         (column_id,type_key,is_visible,is_required,show_in_form,show_in_list,show_in_export,sort_order) \
         SELECT ?1,type_key,1,?2,?3,?4,?5,?6 FROM sample_info_types WHERE is_active=1 AND deleted_at IS NULL",
        params![id,is_required as i64,show_in_form as i64,show_in_list as i64,show_in_export as i64,sort_order],
    )?;
    let created = list_all_on_conn(&tx)?
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| AppError::Internal("创建列失败".into()))?;
    let after = serde_json::to_value(&created).unwrap_or(serde_json::Value::Null);
    audit_repo::log_structured_on_conn(
        &tx,
        "create",
        "sample_info_columns",
        Some(id),
        operator,
        &format!("创建样品自定义字段「{}」", created.label),
        "sample_info",
        "",
        None,
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(created)
}

pub fn update(
    pool: &DbPool,
    id: i64,
    data: &ColumnUpdate,
    operator: &str,
) -> Result<SampleInfoColumn> {
    let mut conn = pool.get()?;

    // 先检查是否存在
    let existing = list_all_on_conn(&conn)?
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| AppError::NotFound("列不存在".into()))?;

    let label = data.label.as_deref().unwrap_or(&existing.label);
    let action_column = existing.data_type == "action" || is_configured_action(&existing.field_key);
    let data_type = if action_column {
        "action"
    } else {
        data.data_type.as_deref().unwrap_or(&existing.data_type)
    };
    let is_active = data.is_active.unwrap_or(existing.is_active) as i64;
    let is_required = if action_column {
        0
    } else {
        data.is_required.unwrap_or(existing.is_required) as i64
    };
    let width = data.width.unwrap_or(existing.width);
    let options = data.options.as_deref().or(existing.options.as_deref());
    let show_in_list = data.show_in_list.unwrap_or(existing.show_in_list) as i64;
    let show_in_export = if action_column {
        0
    } else {
        data.show_in_export.unwrap_or(existing.show_in_export) as i64
    };
    // Action columns are configurable in the management form so their order,
    // width and enabled state can be controlled. The entry UI filters them
    // from data inputs and renders them as buttons in the record list.
    let show_in_form = data.show_in_form.unwrap_or(existing.show_in_form) as i64;

    let before = serde_json::to_value(&existing).unwrap_or(serde_json::Value::Null);
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE sample_info_columns SET label=?1, data_type=?2, is_active=?3, is_required=?4, \
         width=?5, options=?6, show_in_list=?7, show_in_export=?8, show_in_form=?9, \
         updated_at=datetime('now','localtime') WHERE id=?10",
        params![
            label,
            data_type,
            is_active,
            is_required,
            width,
            options,
            show_in_list,
            show_in_export,
            show_in_form,
            id,
        ],
    )?;

    let updated = list_all_on_conn(&tx)?
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| AppError::Internal("更新列失败".into()))?;
    let after = serde_json::to_value(&updated).unwrap_or(serde_json::Value::Null);
    audit_repo::log_structured_on_conn(
        &tx,
        "update",
        "sample_info_columns",
        Some(id),
        operator,
        &format!("修改样品字段「{}」", updated.label),
        "sample_info",
        "",
        Some(&before),
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(updated)
}

pub fn soft_delete(pool: &DbPool, id: i64, operator: &str, reason: &str) -> Result<()> {
    let mut conn = pool.get()?;

    // 检查是否预置字段
    let col = list_all_on_conn(&conn)?
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| AppError::NotFound("列不存在".into()))?;

    if col.is_predefined {
        return Err(AppError::Validation("预置字段不可删除".into()));
    }

    let tx = conn.transaction()?;
    let before = serde_json::to_value(&col).unwrap_or(serde_json::Value::Null);
    tx.execute("UPDATE sample_info_columns SET is_active=0,deleted_at=datetime('now','localtime'),updated_at=datetime('now','localtime') WHERE id=?1", params![id])?;
    trash_repo::move_to_trash_on_conn(
        &tx,
        "样品自定义字段",
        "sample_info_columns",
        id,
        "config",
        "sample_info",
        &col.label,
        "",
        &before,
        reason,
        operator,
        None,
        None,
        "字段值仍保存在历史记录扩展数据中",
        true,
    )?;
    let after = serde_json::json!({"is_active":false,"deleted_at":"now","data":before});
    audit_repo::log_structured_on_conn(
        &tx,
        "delete",
        "sample_info_columns",
        Some(id),
        operator,
        &format!("删除自定义字段「{}」", col.label),
        "sample_info",
        "",
        Some(&before),
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(())
}

pub fn reorder(
    pool: &DbPool,
    data: &ColumnReorder,
    operator: &str,
) -> Result<Vec<SampleInfoColumn>> {
    let mut conn = pool.get()?;
    let ids: std::collections::HashSet<i64> = data.ids.iter().map(|item| item.id).collect();
    let before_items: Vec<SampleInfoColumn> = list_all_on_conn(&conn)?
        .into_iter()
        .filter(|item| ids.contains(&item.id))
        .collect();
    let before = serde_json::to_value(&before_items).unwrap_or(serde_json::Value::Null);
    let tx = conn.transaction()?;
    for item in &data.ids {
        tx.execute(
            "UPDATE sample_info_columns SET sort_order=?1, updated_at=datetime('now','localtime') WHERE id=?2",
            params![item.sort_order, item.id],
        )?;
    }
    let all = list_all_on_conn(&tx)?;
    let after_items: Vec<&SampleInfoColumn> =
        all.iter().filter(|item| ids.contains(&item.id)).collect();
    let after = serde_json::to_value(&after_items).unwrap_or(serde_json::Value::Null);
    audit_repo::log_structured_on_conn(
        &tx,
        "update",
        "sample_info_columns",
        None,
        operator,
        "调整样品字段顺序",
        "sample_info",
        "",
        Some(&before),
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(all)
}

pub fn set_type_rules(
    pool: &DbPool,
    column_id: i64,
    type_rules: &[ColumnTypeRule],
    operator: &str,
) -> Result<SampleInfoColumn> {
    let mut conn = pool.get()?;
    let existing = list_all_on_conn(&conn)?
        .into_iter()
        .find(|item| item.id == column_id)
        .ok_or_else(|| AppError::NotFound("列不存在".into()))?;
    let before = serde_json::to_value(&existing).unwrap_or(serde_json::Value::Null);
    let tx = conn.transaction()?;
    // 清空旧 visibility
    tx.execute(
        "UPDATE sample_info_column_visibility SET is_visible=0,is_required=0,show_in_form=0,show_in_list=0,show_in_export=0 WHERE column_id=?1",
        params![column_id],
    )?;
    // 插入新 visibility
    for rule in type_rules {
        tx.execute(
            "INSERT INTO sample_info_column_visibility \
             (column_id,type_key,is_visible,is_required,show_in_form,show_in_list,show_in_export,sort_order) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8) \
             ON CONFLICT(type_key,column_id) DO UPDATE SET is_visible=excluded.is_visible,is_required=excluded.is_required, \
             show_in_form=excluded.show_in_form,show_in_list=excluded.show_in_list, \
             show_in_export=excluded.show_in_export,sort_order=excluded.sort_order",
            params![
                column_id,
                rule.type_key,
                rule.is_visible as i64,
                (rule.is_visible && rule.show_in_form && rule.is_required) as i64,
                (rule.is_visible && rule.show_in_form) as i64,
                (rule.is_visible && rule.show_in_list) as i64,
                (rule.is_visible && rule.show_in_export) as i64,
                rule.sort_order,
            ],
        )?;
    }
    let updated = list_all_on_conn(&tx)?
        .into_iter()
        .find(|c| c.id == column_id)
        .ok_or_else(|| AppError::NotFound("列不存在".into()))?;
    let after = serde_json::to_value(&updated).unwrap_or(serde_json::Value::Null);
    audit_repo::log_structured_on_conn(
        &tx,
        "update",
        "sample_info_column_visibility",
        Some(column_id),
        operator,
        &format!("修改样品字段「{}」适用类型", updated.label),
        "sample_info",
        "",
        Some(&before),
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_specific_form_list_and_export_flags_are_independent() {
        let pool = crate::db::init_pool("postgres-test");
        crate::db::test_migrations::run(&pool.get().unwrap()).unwrap();
        let conn = pool.get().unwrap();
        let column_id: i64 = conn
            .query_row(
                "SELECT id FROM sample_info_columns WHERE field_key='main_components'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        conn.execute(
            "UPDATE sample_info_column_visibility SET is_visible=1,is_required=0,show_in_form=0,show_in_list=1,show_in_export=0,sort_order=3 WHERE type_key='icp' AND column_id=?1",
            [column_id],
        )
        .unwrap();
        conn.execute(
            "UPDATE sample_info_column_visibility SET is_visible=1,is_required=1,show_in_form=1,show_in_list=0,show_in_export=1,sort_order=7 WHERE type_key='thermal' AND column_id=?1",
            [column_id],
        )
        .unwrap();

        let icp = list_active_by_type(&pool, "icp")
            .unwrap()
            .into_iter()
            .find(|item| item.id == column_id)
            .unwrap();
        assert!(!icp.show_in_form);
        assert!(icp.show_in_list);
        assert!(!icp.show_in_export);
        assert!(!icp.is_required);
        assert_eq!(icp.sort_order, 3);

        let thermal = list_active_by_type(&pool, "thermal")
            .unwrap()
            .into_iter()
            .find(|item| item.id == column_id)
            .unwrap();
        assert!(thermal.show_in_form);
        assert!(!thermal.show_in_list);
        assert!(thermal.show_in_export);
        assert!(thermal.is_required);
        assert_eq!(thermal.sort_order, 7);

        assert!(list_active_common_for_list(&pool)
            .unwrap()
            .iter()
            .all(|item| item.id != column_id));
        assert!(list_active_common_for_export(&pool)
            .unwrap()
            .iter()
            .all(|item| item.id != column_id));
        assert!(list_active_common_for_list(&pool)
            .unwrap()
            .iter()
            .any(|item| item.field_key == "status"));
    }
}
