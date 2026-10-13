use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::sample_info_attachment::SampleInfoAttachment;
use crate::repo::sample_info_attachment_repo;
use std::path::{Path, PathBuf};

pub const MAX_ATTACHMENT_SIZE: usize = 100 * 1024 * 1024;

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
    validate_upload(file_name, file_data)?;

    let ext = Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("bin");
    let seq = sample_info_attachment_repo::next_seq_for_record(pool, record_id)?;
    let now = chrono::Local::now().format("%Y%m%d%H%M%S");
    let stored_name = format!("seq_{}_{}_{}_{}.{}", seq, record_id, now, file_name, ext);
    std::fs::create_dir_all(attachments_dir)
        .map_err(|error| AppError::Internal(format!("创建附件目录失败: {error}")))?;
    let file_path = attachments_dir.join(&stored_name);
    std::fs::write(&file_path, file_data)
        .map_err(|error| AppError::Internal(format!("保存文件失败: {error}")))?;

    match sample_info_attachment_repo::create(
        pool,
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
    use super::validate_upload;

    #[test]
    fn accepts_supported_document_extensions() {
        assert!(validate_upload("report.pdf", b"data").is_ok());
        assert!(validate_upload("report.DOCX", b"data").is_ok());
        assert!(validate_upload("report.xlsx", b"data").is_err());
    }
}
