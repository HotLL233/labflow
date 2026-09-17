use crate::db::DbPool;
use crate::error::Result;
use crate::models::sample_info_column_visibility::{
    SampleInfoColumnVisibility, VisibilityUpdateRequest,
};
use crate::repo::audit_repo;

fn snapshot_on_conn(
    conn: &postgres_compat::Connection,
    type_key: &str,
) -> Result<serde_json::Value> {
    let mut stmt = conn.prepare(
        "SELECT column_id,is_visible,is_required,show_in_form,show_in_list,show_in_export,sort_order FROM sample_info_column_visibility WHERE type_key=?1 ORDER BY sort_order,column_id",
    )?;
    let items = stmt
        .query_map([type_key], |row| {
            Ok(serde_json::json!({
                "column_id": row.get::<_, i64>(0)?,
                "is_visible": row.get::<_, i64>(1)? != 0,
                "is_required": row.get::<_, i64>(2)? != 0,
                "show_in_form": row.get::<_, i64>(3)? != 0,
                "show_in_list": row.get::<_, i64>(4)? != 0,
                "show_in_export": row.get::<_, i64>(5)? != 0,
                "sort_order": row.get::<_, i64>(6)?,
            }))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(serde_json::json!({"type_key":type_key,"items":items}))
}

/// 获取某个类型对所有预置列的可见性（用于管理页）
pub fn list_by_type(pool: &DbPool, type_key: &str) -> Result<Vec<SampleInfoColumnVisibility>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT id,type_key,column_id,is_visible,is_required,show_in_form,show_in_list,show_in_export,sort_order \
         FROM sample_info_column_visibility WHERE type_key = ?1 ORDER BY sort_order,column_id",
    )?;
    let rows = stmt.query_map([type_key], |row| {
        Ok(SampleInfoColumnVisibility {
            id: row.get(0)?,
            type_key: row.get(1)?,
            column_id: row.get(2)?,
            is_visible: row.get::<_, i64>(3)? != 0,
            is_required: row.get::<_, i64>(4)? != 0,
            show_in_form: row.get::<_, i64>(5)? != 0,
            show_in_list: row.get::<_, i64>(6)? != 0,
            show_in_export: row.get::<_, i64>(7)? != 0,
            sort_order: row.get(8)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

/// 为新类型初始化所有预置列的可见性（默认全部可见）
pub fn init_for_type(conn: &postgres_compat::Connection, type_key: &str) -> Result<usize> {
    let count = conn.execute(
        "INSERT OR IGNORE INTO sample_info_column_visibility \
         (type_key,column_id,is_visible,is_required,show_in_form,show_in_list,show_in_export,sort_order) \
         SELECT ?1,id,1,is_required,show_in_form,show_in_list,show_in_export,sort_order \
         FROM sample_info_columns WHERE deleted_at IS NULL",
        [type_key],
    )?;
    Ok(count)
}

/// 批量更新预置列可见性
pub fn batch_update(pool: &DbPool, req: &VisibilityUpdateRequest, user_name: &str) -> Result<()> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;

    let type_key = &req.type_key;
    let before = snapshot_on_conn(&tx, type_key)?;
    for item in &req.items {
        let data_type: String = tx
            .query_row(
                "SELECT data_type FROM sample_info_columns WHERE id=?1 AND deleted_at IS NULL",
                [item.column_id],
                |row| row.get(0),
            )
            .unwrap_or_default();
        let is_action = data_type == "action";
        tx.execute(
            "INSERT INTO sample_info_column_visibility \
             (type_key,column_id,is_visible,is_required,show_in_form,show_in_list,show_in_export,sort_order) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8) \
             ON CONFLICT(type_key,column_id) DO UPDATE SET \
             is_visible=excluded.is_visible,is_required=excluded.is_required,show_in_form=excluded.show_in_form, \
             show_in_list=excluded.show_in_list,show_in_export=excluded.show_in_export,sort_order=excluded.sort_order",
            postgres_compat::params![
                type_key,
                item.column_id,
                item.is_visible as i64,
                (item.is_visible && !is_action && item.show_in_form && item.is_required) as i64,
                (item.is_visible && !is_action && item.show_in_form) as i64,
                (item.is_visible && item.show_in_list) as i64,
                (item.is_visible && !is_action && item.show_in_export) as i64,
                item.sort_order,
            ],
        )?;
    }

    let count = req.items.len();
    let detail = format!("批量更新「{}」类型 {} 条预置列可见性", type_key, count);
    let after = snapshot_on_conn(&tx, type_key)?;
    audit_repo::log_structured_on_conn(
        &tx,
        "update",
        "sample_info_column_visibility",
        None,
        user_name,
        &detail,
        "sample_info",
        "",
        Some(&before),
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(())
}
