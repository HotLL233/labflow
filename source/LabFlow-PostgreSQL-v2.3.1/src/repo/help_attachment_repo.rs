use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::help_attachment::{HelpAttachment, HelpAttachmentUpdate};

const COLS: &str =
    "id,title,filename,file_path,file_type,file_size,is_visible,sort_order,created_at,updated_at";

fn row_to_item(
    row: &postgres_compat::Row,
) -> std::result::Result<HelpAttachment, postgres_compat::Error> {
    Ok(HelpAttachment {
        id: row.get(0)?,
        title: row.get(1)?,
        filename: row.get(2)?,
        file_path: row.get(3)?,
        file_type: row.get(4)?,
        file_size: row.get(5)?,
        is_visible: row.get::<_, i64>(6).unwrap_or(1) != 0,
        sort_order: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

pub fn list(pool: &DbPool, visible_only: bool) -> Result<Vec<HelpAttachment>> {
    let conn = pool.get()?;
    let sql = if visible_only {
        format!("SELECT {COLS} FROM help_attachments WHERE is_visible=1 AND deleted_at IS NULL ORDER BY sort_order,id")
    } else {
        format!(
            "SELECT {COLS} FROM help_attachments WHERE deleted_at IS NULL ORDER BY sort_order,id"
        )
    };
    let rows = conn.prepare(&sql)?.query_map([], row_to_item)?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

pub fn get(pool: &DbPool, id: i64) -> Result<HelpAttachment> {
    let conn = pool.get()?;
    conn.query_row(
        &format!("SELECT {COLS} FROM help_attachments WHERE id=?1 AND deleted_at IS NULL"),
        [id],
        row_to_item,
    )
    .map_err(|e| match e {
        postgres_compat::Error::QueryReturnedNoRows => AppError::NotFound("教程附件不存在".into()),
        _ => e.into(),
    })
}

pub fn create(
    pool: &DbPool,
    title: &str,
    filename: &str,
    path: &str,
    file_type: &str,
    size: i64,
) -> Result<HelpAttachment> {
    let conn = pool.get()?;
    conn.execute("INSERT INTO help_attachments(title,filename,file_path,file_type,file_size) VALUES(?1,?2,?3,?4,?5)", postgres_compat::params![title,filename,path,file_type,size])?;
    get(pool, conn.last_insert_rowid())
}

pub fn update(pool: &DbPool, id: i64, body: &HelpAttachmentUpdate) -> Result<HelpAttachment> {
    let conn = pool.get()?;
    if let Some(v) = body.title.as_deref() {
        conn.execute("UPDATE help_attachments SET title=?1,updated_at=datetime('now','localtime') WHERE id=?2", postgres_compat::params![v,id])?;
    }
    if let Some(v) = body.is_visible {
        conn.execute("UPDATE help_attachments SET is_visible=?1,updated_at=datetime('now','localtime') WHERE id=?2", postgres_compat::params![v as i64,id])?;
    }
    if let Some(v) = body.sort_order {
        conn.execute("UPDATE help_attachments SET sort_order=?1,updated_at=datetime('now','localtime') WHERE id=?2", postgres_compat::params![v,id])?;
    }
    get(pool, id)
}

pub fn delete(pool: &DbPool, id: i64) -> Result<HelpAttachment> {
    let item = get(pool, id)?;
    let conn = pool.get()?;
    conn.execute("UPDATE help_attachments SET deleted_at=datetime('now','localtime'),updated_at=datetime('now','localtime') WHERE id=?1", [id])?;
    Ok(item)
}
