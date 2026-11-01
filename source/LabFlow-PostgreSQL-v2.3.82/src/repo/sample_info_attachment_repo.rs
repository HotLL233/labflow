use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::sample_info_attachment::SampleInfoAttachment;
use crate::repo::{audit_repo, trash_repo};

/// 获取某条记录的所有附件
pub fn list_by_record(pool: &DbPool, record_id: i64) -> Result<Vec<SampleInfoAttachment>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT id, record_id, file_name, stored_name, file_size, file_type, created_at \
         FROM sample_info_attachments WHERE record_id=?1 AND deleted_at IS NULL ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([record_id], |row| {
        Ok(SampleInfoAttachment {
            id: row.get(0)?,
            record_id: row.get(1)?,
            file_name: row.get(2)?,
            stored_name: row.get(3)?,
            file_size: row.get(4)?,
            file_type: row.get(5)?,
            created_at: row.get(6)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

pub fn list_by_records(
    pool: &DbPool,
    record_ids: &[i64],
) -> Result<std::collections::HashMap<i64, Vec<SampleInfoAttachment>>> {
    let mut result: std::collections::HashMap<i64, Vec<SampleInfoAttachment>> =
        record_ids.iter().map(|id| (*id, Vec::new())).collect();
    if record_ids.is_empty() {
        return Ok(result);
    }
    let placeholders = (1..=record_ids.len())
        .map(|index| format!("?{}", index))
        .collect::<Vec<_>>()
        .join(",");
    let conn = pool.get()?;
    let mut stmt = conn.prepare(&format!(
        "SELECT id, record_id, file_name, stored_name, file_size, file_type, created_at
         FROM sample_info_attachments WHERE record_id IN ({}) AND deleted_at IS NULL ORDER BY created_at DESC",
        placeholders
    ))?;
    let rows = stmt.query_map(
        postgres_compat::params_from_iter(record_ids.iter()),
        |row| {
            Ok(SampleInfoAttachment {
                id: row.get(0)?,
                record_id: row.get(1)?,
                file_name: row.get(2)?,
                stored_name: row.get(3)?,
                file_size: row.get(4)?,
                file_type: row.get(5)?,
                created_at: row.get(6)?,
            })
        },
    )?;
    for attachment in rows {
        let attachment = attachment?;
        result
            .entry(attachment.record_id)
            .or_default()
            .push(attachment);
    }
    Ok(result)
}

/// 按 id 查找附件
pub fn find_by_id(pool: &DbPool, att_id: i64) -> Result<SampleInfoAttachment> {
    let conn = pool.get()?;
    conn.query_row(
        "SELECT id, record_id, file_name, stored_name, file_size, file_type, created_at \
         FROM sample_info_attachments WHERE id = ?1",
        [att_id],
        |row| {
            Ok(SampleInfoAttachment {
                id: row.get(0)?,
                record_id: row.get(1)?,
                file_name: row.get(2)?,
                stored_name: row.get(3)?,
                file_size: row.get(4)?,
                file_type: row.get(5)?,
                created_at: row.get(6)?,
            })
        },
    )
    .map_err(|e| match e {
        postgres_compat::Error::QueryReturnedNoRows => AppError::NotFound("附件不存在".into()),
        _ => e.into(),
    })
}

/// 创建附件记录
pub fn create(
    pool: &DbPool,
    record_id: i64,
    file_name: &str,
    stored_name: &str,
    file_size: i64,
    file_type: &str,
) -> Result<SampleInfoAttachment> {
    let conn = pool.get()?;
    conn.execute(
        "INSERT INTO sample_info_attachments (record_id, file_name, stored_name, file_size, file_type) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        postgres_compat::params![record_id, file_name, stored_name, file_size, file_type],
    )?;
    let id = conn.last_insert_rowid();
    find_by_id_on_conn(&conn, id)
}

/// 按 id 查找附件（连接上）
fn find_by_id_on_conn(
    conn: &postgres_compat::Connection,
    att_id: i64,
) -> Result<SampleInfoAttachment> {
    conn.query_row(
        "SELECT id, record_id, file_name, stored_name, file_size, file_type, created_at \
         FROM sample_info_attachments WHERE id = ?1",
        [att_id],
        |row| {
            Ok(SampleInfoAttachment {
                id: row.get(0)?,
                record_id: row.get(1)?,
                file_name: row.get(2)?,
                stored_name: row.get(3)?,
                file_size: row.get(4)?,
                file_type: row.get(5)?,
                created_at: row.get(6)?,
            })
        },
    )
    .map_err(|e| match e {
        postgres_compat::Error::QueryReturnedNoRows => AppError::NotFound("附件不存在".into()),
        _ => e.into(),
    })
}

/// v0.4.28: 获取某条记录的下一个附件序号
pub fn next_seq_for_record(pool: &DbPool, record_id: i64) -> Result<i64> {
    let conn = pool.get()?;
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sample_info_attachments WHERE record_id = ?1",
        [record_id],
        |row| row.get(0),
    )?;
    Ok(count + 1)
}

/// 删除附件（同时返回文件名以便删除物理文件）
pub fn delete(
    pool: &DbPool,
    att_id: i64,
    user_name: &str,
    reason: &str,
) -> Result<SampleInfoAttachment> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;

    let att = find_by_id_on_conn(&tx, att_id)?;
    tx.execute("UPDATE sample_info_attachments SET deleted_at=datetime('now','localtime') WHERE id=?1 AND deleted_at IS NULL", [att_id])?;

    let detail = format!(
        "删除附件#{}：「{}」({} bytes)",
        att.id, att.file_name, att.file_size
    );
    let before = serde_json::to_value(&att).unwrap_or(serde_json::Value::Null);
    let owner: (Option<i64>, Option<i64>) = tx
        .query_row(
            "SELECT created_by_user_id,group_id FROM sample_info_records WHERE id=?1",
            [att.record_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap_or((None, None));
    trash_repo::move_to_trash_on_conn(
        &tx,
        "样品附件",
        "sample_info_attachments",
        att_id,
        "files",
        "sample_info",
        &att.file_name,
        "",
        &before,
        reason,
        user_name,
        owner.0,
        owner.1,
        "原始文件保留至永久清理",
        true,
    )?;
    let after = serde_json::json!({"deleted_at":"now","data":before});
    audit_repo::log_structured_on_conn(
        &tx,
        "delete",
        "sample_info_attachments",
        Some(att_id),
        user_name,
        &detail,
        "sample_info",
        "",
        Some(&before),
        Some(&after),
        "record",
    )?;
    tx.commit()?;
    Ok(att)
}
