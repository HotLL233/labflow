use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::sample_info_draft::{
    SampleInfoDraft, SampleInfoDraftAttachment, SampleInfoDraftInput,
};
use postgres_compat::params;

fn map_row(row: &postgres_compat::Row<'_>) -> postgres_compat::Result<SampleInfoDraft> {
    let payload: String = row.get(5)?;
    Ok(SampleInfoDraft {
        id: row.get(0)?,
        user_id: row.get(1)?,
        username_snapshot: row.get(2)?,
        type_key: row.get(3)?,
        title: row.get(4)?,
        payload: serde_json::from_str(&payload)
            .unwrap_or(serde_json::Value::Object(Default::default())),
        status: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        submitted_at: row.get(9)?,
    })
}

pub fn list(pool: &DbPool, user_id: i64) -> Result<Vec<SampleInfoDraft>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare("SELECT id,user_id,username_snapshot,type_key,title,payload_json,status,created_at,updated_at,submitted_at FROM sample_info_drafts WHERE user_id=?1 AND status='active' AND deleted_at IS NULL ORDER BY updated_at DESC")?;
    Ok(stmt
        .query_map([user_id], map_row)?
        .collect::<std::result::Result<Vec<_>, _>>()?)
}

pub fn create(
    pool: &DbPool,
    user_id: i64,
    username: &str,
    input: &SampleInfoDraftInput,
) -> Result<SampleInfoDraft> {
    let conn = pool.get()?;
    conn.execute("INSERT INTO sample_info_drafts(user_id,username_snapshot,type_key,title,payload_json) VALUES(?1,?2,?3,?4,?5)", params![user_id,username,input.type_key,input.title.trim(),input.payload.to_string()])?;
    let id = conn.last_insert_rowid();
    get(pool, id, user_id)
}

pub fn update(
    pool: &DbPool,
    id: i64,
    user_id: i64,
    input: &SampleInfoDraftInput,
) -> Result<SampleInfoDraft> {
    let conn = pool.get()?;
    let changed = conn.execute("UPDATE sample_info_drafts SET type_key=?1,title=?2,payload_json=?3,updated_at=to_char(CURRENT_TIMESTAMP,'YYYY-MM-DD HH24:MI:SS') WHERE id=?4 AND user_id=?5 AND status='active' AND deleted_at IS NULL", params![input.type_key,input.title.trim(),input.payload.to_string(),id,user_id])?;
    if changed == 0 {
        return Err(AppError::NotFound("草稿不存在或无权修改".into()));
    }
    get(pool, id, user_id)
}

pub fn get(pool: &DbPool, id: i64, user_id: i64) -> Result<SampleInfoDraft> {
    let conn = pool.get()?;
    conn.query_row("SELECT id,user_id,username_snapshot,type_key,title,payload_json,status,created_at,updated_at,submitted_at FROM sample_info_drafts WHERE id=?1 AND user_id=?2 AND deleted_at IS NULL", params![id,user_id], map_row).map_err(|e| match e { postgres_compat::Error::QueryReturnedNoRows => AppError::NotFound("草稿不存在或无权访问".into()), other => other.into() })
}

pub fn remove(pool: &DbPool, id: i64, user_id: i64) -> Result<()> {
    let conn = pool.get()?;
    let changed = conn.execute("UPDATE sample_info_drafts SET status='deleted',deleted_at=to_char(CURRENT_TIMESTAMP,'YYYY-MM-DD HH24:MI:SS'),updated_at=to_char(CURRENT_TIMESTAMP,'YYYY-MM-DD HH24:MI:SS') WHERE id=?1 AND user_id=?2 AND status='active'", params![id,user_id])?;
    if changed == 0 {
        return Err(AppError::NotFound("草稿不存在或无权删除".into()));
    }
    Ok(())
}

fn map_attachment(
    row: &postgres_compat::Row<'_>,
) -> postgres_compat::Result<SampleInfoDraftAttachment> {
    Ok(SampleInfoDraftAttachment {
        id: row.get(0)?,
        draft_id: row.get(1)?,
        row_index: row.get(2)?,
        file_name: row.get(3)?,
        stored_name: row.get(4)?,
        file_size: row.get(5)?,
        file_type: row.get(6)?,
        created_at: row.get(7)?,
    })
}

pub fn list_attachments(
    pool: &DbPool,
    draft_id: i64,
    user_id: i64,
) -> Result<Vec<SampleInfoDraftAttachment>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare("SELECT a.id,a.draft_id,a.row_index,a.file_name,a.stored_name,a.file_size,a.file_type,a.created_at FROM sample_info_draft_attachments a JOIN sample_info_drafts d ON d.id=a.draft_id WHERE a.draft_id=?1 AND d.user_id=?2 AND d.status='active' AND d.deleted_at IS NULL ORDER BY a.row_index,a.created_at")?;
    Ok(stmt
        .query_map(params![draft_id, user_id], map_attachment)?
        .collect::<std::result::Result<Vec<_>, _>>()?)
}

pub fn create_attachment(
    pool: &DbPool,
    draft_id: i64,
    user_id: i64,
    row_index: i64,
    file_name: &str,
    stored_name: &str,
    file_size: i64,
    file_type: &str,
) -> Result<SampleInfoDraftAttachment> {
    get(pool, draft_id, user_id)?;
    let conn = pool.get()?;
    conn.execute("INSERT INTO sample_info_draft_attachments(draft_id,row_index,file_name,stored_name,file_size,file_type) VALUES(?1,?2,?3,?4,?5,?6)", params![draft_id,row_index,file_name,stored_name,file_size,file_type])?;
    let id = conn.last_insert_rowid();
    find_attachment(pool, id, user_id)
}

pub fn find_attachment(
    pool: &DbPool,
    attachment_id: i64,
    user_id: i64,
) -> Result<SampleInfoDraftAttachment> {
    let conn = pool.get()?;
    conn.query_row("SELECT a.id,a.draft_id,a.row_index,a.file_name,a.stored_name,a.file_size,a.file_type,a.created_at FROM sample_info_draft_attachments a JOIN sample_info_drafts d ON d.id=a.draft_id WHERE a.id=?1 AND d.user_id=?2 AND d.status='active' AND d.deleted_at IS NULL", params![attachment_id,user_id], map_attachment).map_err(|e| match e { postgres_compat::Error::QueryReturnedNoRows => AppError::NotFound("草稿附件不存在或无权访问".into()), other => other.into() })
}

pub fn delete_attachment(
    pool: &DbPool,
    attachment_id: i64,
    user_id: i64,
) -> Result<SampleInfoDraftAttachment> {
    let attachment = find_attachment(pool, attachment_id, user_id)?;
    let conn = pool.get()?;
    conn.execute(
        "DELETE FROM sample_info_draft_attachments WHERE id=?1",
        params![attachment_id],
    )?;
    Ok(attachment)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft_attachments_are_scoped_to_the_draft_and_keep_the_row_index() {
        let pool = crate::db::init_pool("postgres-test");
        crate::db::test_migrations::run(&pool.get().unwrap()).unwrap();
        let draft = create(
            &pool,
            101,
            "draft-user",
            &SampleInfoDraftInput {
                type_key: "icp".into(),
                title: "ICP 草稿".into(),
                payload: serde_json::json!({"rows": [{"batch_no": "B001"}]}),
            },
        )
        .unwrap();
        let attachment = create_attachment(
            &pool,
            draft.id,
            101,
            0,
            "report.pdf",
            "draft_test_report.pdf",
            12,
            "application/pdf",
        )
        .unwrap();
        assert_eq!(attachment.row_index, 0);
        assert_eq!(attachment.file_name, "report.pdf");
        let restored = list_attachments(&pool, draft.id, 101).unwrap();
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].id, attachment.id);
        delete_attachment(&pool, attachment.id, 101).unwrap();
        assert!(list_attachments(&pool, draft.id, 101).unwrap().is_empty());
    }
}
