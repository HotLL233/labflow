use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::help::{HelpDocUpdateRequest, HelpDocument};
use crate::repo::{audit_repo, trash_repo};

const COLS: &str = "id, title, filename, file_path, file_type, file_size, is_visible, sort_order, page_count, created_at, updated_at";

fn row_to_doc(
    row: &postgres_compat::Row,
) -> std::result::Result<HelpDocument, postgres_compat::Error> {
    Ok(HelpDocument {
        id: row.get(0)?,
        title: row.get(1)?,
        filename: row.get(2)?,
        file_path: row.get(3)?,
        file_type: row.get(4)?,
        file_size: row.get(5)?,
        is_visible: row.get::<_, i64>(6).unwrap_or(1) != 0,
        sort_order: row.get(7)?,
        page_count: row.get::<_, i64>(8).unwrap_or(0),
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

pub fn list(pool: &DbPool, visible_only: bool) -> Result<Vec<HelpDocument>> {
    let conn = pool.get()?;
    let sql = if visible_only {
        &format!("SELECT {COLS} FROM help_documents WHERE is_visible=1 AND deleted_at IS NULL ORDER BY sort_order,id")
    } else {
        &format!(
            "SELECT {COLS} FROM help_documents WHERE deleted_at IS NULL ORDER BY sort_order,id"
        )
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |row| row_to_doc(row))?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

pub fn get_by_id(pool: &DbPool, id: i64) -> Result<HelpDocument> {
    let conn = pool.get()?;
    get_by_id_on_conn(&conn, id)
}

fn get_by_id_on_conn(conn: &postgres_compat::Connection, id: i64) -> Result<HelpDocument> {
    conn.query_row(
        &format!("SELECT {COLS} FROM help_documents WHERE id = ?1"),
        [id],
        |row| row_to_doc(row),
    )
    .map_err(|e| match e {
        postgres_compat::Error::QueryReturnedNoRows => AppError::NotFound("帮助文档不存在".into()),
        _ => e.into(),
    })
}

pub fn create(
    pool: &DbPool,
    title: &str,
    filename: &str,
    file_path: &str,
    file_type: &str,
    file_size: i64,
    operator: &str,
) -> Result<HelpDocument> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO help_documents (title, filename, file_path, file_type, file_size)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        (title, filename, file_path, file_type, file_size),
    )?;
    let id = tx.last_insert_rowid();
    let doc = get_by_id_on_conn(&tx, id)?;
    let after = serde_json::to_value(&doc).unwrap_or(serde_json::Value::Null);
    audit_repo::log_structured_on_conn(
        &tx,
        "create",
        "help_documents",
        Some(id),
        operator,
        &format!("创建帮助文档「{}」", doc.title),
        "shared",
        "",
        None,
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(doc)
}

/// v2.2.0: only one active tutorial is shown to readers; old source files remain recoverable.
pub fn hide_visible(pool: &DbPool) -> Result<()> {
    let conn = pool.get()?;
    conn.execute("UPDATE help_documents SET is_visible=0,updated_at=datetime('now','localtime') WHERE is_visible<>0 AND deleted_at IS NULL", [])?;
    Ok(())
}

pub fn set_page_count(pool: &DbPool, id: i64, page_count: i64) -> Result<()> {
    let conn = pool.get()?;
    conn.execute(
        "UPDATE help_documents SET page_count=?1, updated_at=datetime('now','localtime') WHERE id=?2",
        (page_count, id),
    )?;
    Ok(())
}

pub fn update(
    pool: &DbPool,
    id: i64,
    body: &HelpDocUpdateRequest,
    operator: &str,
) -> Result<HelpDocument> {
    let mut conn = pool.get()?;
    let current = get_by_id_on_conn(&conn, id)?;
    let before = serde_json::to_value(&current).unwrap_or(serde_json::Value::Null);
    let tx = conn.transaction()?;
    if let Some(ref title) = body.title {
        tx.execute(
            "UPDATE help_documents SET title=?1, updated_at=datetime('now','localtime') WHERE id=?2",
            (title, id),
        )?;
    }
    if let Some(v) = body.is_visible {
        tx.execute(
            "UPDATE help_documents SET is_visible=?1, updated_at=datetime('now','localtime') WHERE id=?2",
            (v, id),
        )?;
    }
    if let Some(so) = body.sort_order {
        tx.execute(
            "UPDATE help_documents SET sort_order=?1, updated_at=datetime('now','localtime') WHERE id=?2",
            (so, id),
        )?;
    }
    let updated = get_by_id_on_conn(&tx, id)?;
    let after = serde_json::to_value(&updated).unwrap_or(serde_json::Value::Null);
    audit_repo::log_structured_on_conn(
        &tx,
        "update",
        "help_documents",
        Some(id),
        operator,
        &format!("修改帮助文档「{}」", updated.title),
        "shared",
        "",
        Some(&before),
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(updated)
}

/// v0.4.74: 批量更新排序
pub fn reorder_documents(pool: &DbPool, ids: &[(i64, i64)], operator: &str) -> Result<()> {
    let mut conn = pool.get()?;
    let ids_set: std::collections::HashSet<i64> = ids.iter().map(|(id, _)| *id).collect();
    let before_items: Vec<HelpDocument> = list(pool, false)?
        .into_iter()
        .filter(|doc| ids_set.contains(&doc.id))
        .collect();
    let before = serde_json::to_value(&before_items).unwrap_or(serde_json::Value::Null);
    let tx = conn.transaction()?;
    for (id, sort_order) in ids {
        tx.execute(
            "UPDATE help_documents SET sort_order=?1, updated_at=datetime('now','localtime') WHERE id=?2",
            (sort_order, id),
        )?;
    }
    let mut stmt = tx.prepare(&format!(
        "SELECT {COLS} FROM help_documents WHERE deleted_at IS NULL ORDER BY sort_order,id"
    ))?;
    let after_items = stmt
        .query_map([], row_to_doc)?
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|doc| ids_set.contains(&doc.id))
        .collect::<Vec<_>>();
    drop(stmt);
    let after = serde_json::to_value(&after_items).unwrap_or(serde_json::Value::Null);
    audit_repo::log_structured_on_conn(
        &tx,
        "update",
        "help_documents",
        None,
        operator,
        "调整帮助文档排序",
        "shared",
        "",
        Some(&before),
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(())
}

pub fn delete(pool: &DbPool, id: i64, operator: &str, reason: &str) -> Result<HelpDocument> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let doc = get_by_id_on_conn(&tx, id)?;
    let before = serde_json::to_value(&doc).unwrap_or(serde_json::Value::Null);
    tx.execute("UPDATE help_documents SET deleted_at=datetime('now','localtime'),updated_at=datetime('now','localtime') WHERE id=?1", [id])?;
    trash_repo::move_to_trash_on_conn(
        &tx,
        "帮助文档",
        "help_documents",
        id,
        "files",
        "shared",
        &doc.title,
        "",
        &before,
        reason,
        operator,
        None,
        None,
        "原始文件保留至永久清理",
        true,
    )?;
    let after = serde_json::json!({"deleted_at":"now","data":before});
    audit_repo::log_structured_on_conn(
        &tx,
        "delete",
        "help_documents",
        Some(id),
        operator,
        &format!("删除帮助文档「{}」", doc.title),
        "shared",
        "",
        Some(&before),
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(doc)
}
