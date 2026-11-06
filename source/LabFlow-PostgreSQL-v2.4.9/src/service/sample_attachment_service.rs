use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::sample_info_attachment::SampleInfoAttachment;
use crate::repo::sample_info_attachment_repo;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const MAX_ATTACHMENT_SIZE: usize = 100 * 1024 * 1024;

fn write_upload_file(path: &Path, data: &[u8]) -> Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| AppError::Internal(format!("创建附件文件失败: {error}")))?;
    if let Err(error) = file.write_all(data) {
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(AppError::Internal(format!("保存文件失败: {error}")));
    }
    Ok(())
}

pub fn validate_upload(file_name: &str, file_data: &[u8]) -> Result<()> {
    let ext = Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !["pdf", "doc", "docx"].contains(&ext.as_str()) {
        return Err(AppError::Validation(
            "仅支持 PDF、Word 文档（.pdf/.doc/.docx）".into(),
        ));
    }
    if file_data.is_empty() {
        return Err(AppError::Validation("未选择文件".into()));
    }
    if file_data.len() > MAX_ATTACHMENT_SIZE {
        return Err(AppError::Validation("单个附件不能超过 100MB".into()));
    }
    Ok(())
}

pub fn save_upload(
    pool: &DbPool,
    attachments_dir: &Path,
    record_id: i64,
    file_name: &str,
    file_type: &str,
    file_data: &[u8],
) -> Result<(SampleInfoAttachment, PathBuf)> {
    let conn = pool.get()?;
    save_upload_on_conn(
        &conn,
        attachments_dir,
        record_id,
        file_name,
        file_type,
        file_data,
    )
}

pub(crate) fn save_upload_on_conn(
    conn: &postgres_compat::Connection,
    attachments_dir: &Path,
    record_id: i64,
    file_name: &str,
    file_type: &str,
    file_data: &[u8],
) -> Result<(SampleInfoAttachment, PathBuf)> {
    validate_upload(file_name, file_data)?;

    let ext = Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("bin");
    let seq = sample_info_attachment_repo::next_seq_for_record_on_conn(conn, record_id)?;
    let now = chrono::Local::now().format("%Y%m%d%H%M%S");
    let stored_name = format!("seq_{}_{}_{}_{}.{}", seq, record_id, now, file_name, ext);
    std::fs::create_dir_all(attachments_dir)
        .map_err(|error| AppError::Internal(format!("创建附件目录失败: {error}")))?;
    let file_path = attachments_dir.join(&stored_name);
    write_upload_file(&file_path, file_data)?;

    match sample_info_attachment_repo::create_on_conn(
        conn,
        record_id,
        file_name,
        &stored_name,
        file_data.len() as i64,
        file_type,
    ) {
        Ok(attachment) => Ok((attachment, file_path)),
        Err(error) => {
            let _ = std::fs::remove_file(&file_path);
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_supported_document_extensions() {
        assert!(validate_upload("report.pdf", b"data").is_ok());
        assert!(validate_upload("report.DOCX", b"data").is_ok());
        assert!(validate_upload("report.xlsx", b"data").is_err());
    }

    #[test]
    fn attachment_file_collision_preserves_existing_contents() {
        let dir =
            std::env::temp_dir().join(format!("labflow-upload-file-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let existing = dir.join("existing.pdf");
        std::fs::write(&existing, b"original document").unwrap();
        assert!(write_upload_file(&existing, b"replacement").is_err());
        assert_eq!(std::fs::read(&existing).unwrap(), b"original document");
        let fresh = dir.join("fresh.pdf");
        write_upload_file(&fresh, b"new document").unwrap();
        assert_eq!(std::fs::read(&fresh).unwrap(), b"new document");
        std::fs::remove_file(existing).unwrap();
        std::fs::remove_file(fresh).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn attachment_submission_keeps_business_metadata_and_outbox_in_one_transaction() {
        let pool = crate::db::init_pool("postgres-test");
        crate::db::test_migrations::run(&pool.get().unwrap()).unwrap();
        let data: crate::models::sample_info::SampleInfoCreate = serde_json::from_value(serde_json::json!({
            "batch_no":"B001","user_name":"sender01","lab_name":"lab01","project_name":"project01",
            "main_components":"component01","detection_type":"ICP","type_key":"icp","quantity":1
        })).unwrap();
        let dir = std::env::temp_dir().join(format!(
            "labflow-attachment-outbox-{}",
            uuid::Uuid::new_v4()
        ));
        let mut conn = pool.get().unwrap();
        let observer = pool.get().unwrap();
        let tx = conn.transaction().unwrap();
        let record = crate::repo::sample_info_repo::create_on_conn(&tx, &data, "sender01").unwrap();
        let (_, path) = save_upload_on_conn(
            &tx,
            &dir,
            record.id,
            "report.pdf",
            "application/pdf",
            b"document",
        )
        .unwrap();
        assert!(save_upload_on_conn(
            &tx,
            &dir,
            record.id,
            "invalid.xlsx",
            "application/octet-stream",
            b"document"
        )
        .is_err());
        assert_eq!(
            observer
                .query_row("SELECT COUNT(*) FROM notification_events", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
        drop(tx);
        assert_eq!(observer.query_row("SELECT (SELECT COUNT(*) FROM sample_info_records)+(SELECT COUNT(*) FROM sample_info_attachments)+(SELECT COUNT(*) FROM notification_events)",[],|row|row.get::<_,i64>(0)).unwrap(),0);
        std::fs::remove_file(path).unwrap();
        let tx = conn.transaction().unwrap();
        let record = crate::repo::sample_info_repo::create_on_conn(&tx, &data, "sender01").unwrap();
        let (_, path) = save_upload_on_conn(
            &tx,
            &dir,
            record.id,
            "report.pdf",
            "application/pdf",
            b"document",
        )
        .unwrap();
        tx.commit().unwrap();
        assert_eq!(
            observer
                .query_row(
                    "SELECT COUNT(*) FROM sample_info_attachments WHERE record_id=?1",
                    [record.id],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
        assert_eq!(observer.query_row("SELECT COUNT(*) FROM notification_events WHERE resource_type='sample_info' AND resource_id=?1",[record.id],|row|row.get::<_,i64>(0)).unwrap(),1);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
