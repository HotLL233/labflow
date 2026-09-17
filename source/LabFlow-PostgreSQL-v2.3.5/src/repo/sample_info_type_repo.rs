use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::sample_info_type::{SampleInfoType, SampleInfoTypeCreate, SampleInfoTypeUpdate};
use crate::repo::{audit_repo, trash_repo};

fn row_to_type(row: &postgres_compat::Row) -> postgres_compat::Result<SampleInfoType> {
    Ok(SampleInfoType {
        id: row.get(0)?,
        type_key: row.get(1)?,
        label: row.get(2)?,
        description: row.get::<_, String>(3).unwrap_or_default(),
        color: row
            .get::<_, String>(4)
            .unwrap_or_else(|_| "#2e7d32".to_string()),
        sort_order: row.get::<_, i64>(5).unwrap_or(0),
        is_active: row.get::<_, i64>(6).unwrap_or(1),
        created_at: row.get(7)?,
    })
}

/// 列表（仅启用 is_active=1），供门户使用
pub fn list(pool: &DbPool) -> Result<Vec<SampleInfoType>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT id, type_key, label, description, color, sort_order, is_active, created_at \
         FROM sample_info_types WHERE is_active=1 AND deleted_at IS NULL ORDER BY sort_order ASC, id ASC",
    )?;
    let rows = stmt.query_map([], |row| row_to_type(row))?;
    let items = rows.collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(items)
}

/// 列表（含软删 is_active=0），供管理页使用
pub fn list_all(pool: &DbPool) -> Result<Vec<SampleInfoType>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT id, type_key, label, description, color, sort_order, is_active, created_at \
         FROM sample_info_types WHERE deleted_at IS NULL ORDER BY sort_order ASC, id ASC",
    )?;
    let rows = stmt.query_map([], |row| row_to_type(row))?;
    let items = rows.collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(items)
}

/// 创建（事务 + 审计）；type_key 唯一冲突返回 Conflict
pub fn create(
    pool: &DbPool,
    data: &SampleInfoTypeCreate,
    operator: &str,
) -> Result<SampleInfoType> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;

    let label = data.label.trim().to_string();
    let type_key = data.type_key.trim().to_string();
    if type_key.is_empty() || label.is_empty() {
        return Err(AppError::Validation("type_key 与 label 不能为空".into()));
    }

    // 唯一性校验
    let exists: i64 = tx.query_row(
        "SELECT COUNT(*) FROM sample_info_types WHERE type_key=?1 AND is_active=1",
        [&type_key],
        |r| r.get(0),
    )?;
    if exists > 0 {
        return Err(AppError::Conflict(format!(
            "类型标识「{}」已存在",
            type_key
        )));
    }

    tx.execute(
        "INSERT INTO sample_info_types (type_key, label, description, color, sort_order) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        postgres_compat::params![
            type_key,
            label,
            data.description.clone().unwrap_or_default(),
            data.color.clone().unwrap_or_else(|| "#2e7d32".to_string()),
            data.sort_order.unwrap_or(0),
        ],
    )?;

    let id = tx.last_insert_rowid();
    let detail = format!("创建检测类型#{}：{}（{}）", id, label, type_key);
    let item = get_by_id(&tx, id)?;
    let after = serde_json::to_value(&item).unwrap_or(serde_json::Value::Null);
    audit_repo::log_structured_on_conn(
        &tx,
        "create",
        "sample_info_types",
        Some(id),
        operator,
        &detail,
        "sample_info",
        "",
        None,
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(item)
}

/// 更新（事务 + 审计）
pub fn update(
    pool: &DbPool,
    id: i64,
    data: &SampleInfoTypeUpdate,
    operator: &str,
) -> Result<SampleInfoType> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let existing = get_by_id(&tx, id)?;
    let before = serde_json::to_value(&existing).unwrap_or(serde_json::Value::Null);

    let mut changes: Vec<String> = vec![];

    if let Some(ref tk) = data.type_key {
        let tk = tk.trim().to_string();
        if !tk.is_empty() && tk != existing.type_key {
            // 唯一性校验（排除自身）
            let conflict: i64 = tx.query_row(
                "SELECT COUNT(*) FROM sample_info_types WHERE type_key=?1 AND id<>?2",
                postgres_compat::params![tk, id],
                |r| r.get(0),
            )?;
            if conflict > 0 {
                return Err(AppError::Conflict(format!("类型标识「{}」已存在", tk)));
            }
            changes.push(format!("标识 {} → {}", existing.type_key, tk));
            tx.execute(
                "UPDATE sample_info_types SET type_key=?1 WHERE id=?2",
                postgres_compat::params![tk, id],
            )?;
        }
    }
    if let Some(ref l) = data.label {
        let l = l.trim().to_string();
        if !l.is_empty() && l != existing.label {
            changes.push(format!("名称 {} → {}", existing.label, l));
            tx.execute(
                "UPDATE sample_info_types SET label=?1 WHERE id=?2",
                postgres_compat::params![l, id],
            )?;
        }
    }
    if let Some(ref d) = data.description {
        if d != &existing.description {
            tx.execute(
                "UPDATE sample_info_types SET description=?1 WHERE id=?2",
                postgres_compat::params![d, id],
            )?;
            changes.push("描述已更新".into());
        }
    }
    if let Some(ref c) = data.color {
        if c != &existing.color {
            tx.execute(
                "UPDATE sample_info_types SET color=?1 WHERE id=?2",
                postgres_compat::params![c, id],
            )?;
            changes.push("颜色已更新".into());
        }
    }
    if let Some(so) = data.sort_order {
        if so != existing.sort_order {
            tx.execute(
                "UPDATE sample_info_types SET sort_order=?1 WHERE id=?2",
                postgres_compat::params![so, id],
            )?;
            changes.push(format!("排序 {} → {}", existing.sort_order, so));
        }
    }
    if let Some(ia) = data.is_active {
        if ia != existing.is_active {
            tx.execute(
                "UPDATE sample_info_types SET is_active=?1 WHERE id=?2",
                postgres_compat::params![ia, id],
            )?;
            changes.push(format!("启用 {}", ia));
        }
    }

    if changes.is_empty() {
        drop(tx);
    } else {
        let detail = format!("修改检测类型#{}：{}", id, changes.join("，"));
        let item = get_by_id(&tx, id)?;
        let after = serde_json::to_value(&item).unwrap_or(serde_json::Value::Null);
        audit_repo::log_structured_on_conn(
            &tx,
            "update",
            "sample_info_types",
            Some(id),
            operator,
            &detail,
            "sample_info",
            "",
            Some(&before),
            Some(&after),
            "management",
        )?;
        tx.commit()?;
    }

    let item = get_by_id(&conn, id)?;
    Ok(item)
}

/// 软删除：is_active=0（事务 + 审计）
pub fn soft_delete(pool: &DbPool, id: i64, operator: &str, reason: &str) -> Result<()> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let existing = get_by_id(&tx, id)?;

    tx.execute("UPDATE sample_info_types SET is_active=0,deleted_at=datetime('now','localtime') WHERE id=?1 AND deleted_at IS NULL", [id])?;
    let detail = format!(
        "删除检测类型#{}：{}（{}）",
        id, existing.label, existing.type_key
    );
    let before = serde_json::to_value(&existing).unwrap_or(serde_json::Value::Null);
    let record_count: i64 = tx.query_row(
        "SELECT COUNT(*) FROM sample_info_records WHERE type_key=?1",
        [&existing.type_key],
        |row| row.get(0),
    )?;
    let dependency = format!("关联样品记录 {record_count} 条");
    trash_repo::move_to_trash_on_conn(
        &tx,
        "样品检测类型",
        "sample_info_types",
        id,
        "config",
        "sample_info",
        &existing.label,
        "",
        &before,
        reason,
        operator,
        None,
        None,
        &dependency,
        record_count == 0,
    )?;
    let after = serde_json::json!({"is_active":false,"deleted_at":"now","data":before});
    audit_repo::log_structured_on_conn(
        &tx,
        "delete",
        "sample_info_types",
        Some(id),
        operator,
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

fn get_by_id(conn: &postgres_compat::Connection, id: i64) -> Result<SampleInfoType> {
    conn.query_row(
        "SELECT id, type_key, label, description, color, sort_order, is_active, created_at \
         FROM sample_info_types WHERE id=?1",
        [id],
        |row| row_to_type(row),
    )
    .map_err(|e| match e {
        postgres_compat::Error::QueryReturnedNoRows => AppError::NotFound("检测类型不存在".into()),
        _ => e.into(),
    })
}
