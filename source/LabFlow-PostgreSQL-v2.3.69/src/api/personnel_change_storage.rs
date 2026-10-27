//! Recoverable coordination of the independent personnel files and their DB audit/outbox.
use super::{ensure_feature_dir, AppError, AuthContext, DbPool, Result};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path};
use std::sync::{Mutex, MutexGuard};

static FILES: Mutex<()> = Mutex::new(());
const JOURNAL: &str = ".personnel-operation.json";

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct FileChange {
    pub name: String,
    #[serde(deserialize_with = "required_before")]
    before: Option<Vec<u8>>,
    after: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
struct Journal {
    operation: String,
    files: Vec<FileChange>,
    checksum: String,
}

fn required_before<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<Vec<u8>>, D::Error> {
    Option::<Vec<u8>>::deserialize(deserializer)
}

fn journal_checksum(journal: &Journal) -> Result<String> {
    Ok(super::digest(
        &serde_json::to_vec(&(&journal.operation, &journal.files))
            .map_err(|e| AppError::Internal(e.to_string()))?,
    ))
}

fn validate_journal(journal: &Journal) -> Result<()> {
    let valid_operation = journal
        .operation
        .strip_prefix("personnel-journal:")
        .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok());
    let mut names = BTreeSet::new();
    if !valid_operation
        || journal.files.is_empty()
        || journal
            .files
            .iter()
            .any(|file| !safe_name(&file.name) || !names.insert(&file.name))
        || journal.checksum != journal_checksum(journal)?
    {
        return Err(AppError::Internal(
            "人员操作恢复日志校验失败，已阻止所有恢复写入，请管理员核对备份".into(),
        ));
    }
    Ok(())
}

fn io(error: std::io::Error) -> AppError {
    AppError::Internal(format!("人员台账文件操作失败: {error}"))
}

pub(super) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(io)?;
        file.write_all(bytes).map_err(io)?;
        file.sync_all().map_err(io)?;
        fs::rename(&temporary, path).map_err(io)?;
        #[cfg(unix)]
        if let Some(parent) = path.parent() {
            fs::File::open(parent)
                .and_then(|f| f.sync_all())
                .map_err(io)?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

pub(super) fn change(name: &str, after: Vec<u8>) -> Result<FileChange> {
    if !safe_name(name) {
        return Err(AppError::Validation("人员台账文件名无效".into()));
    }
    let path = ensure_feature_dir()?.join(name);
    let before = match fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(io(error)),
    };
    Ok(FileChange {
        name: name.into(),
        before,
        after,
    })
}

fn restore(journal: &Journal, committed: bool) -> Result<()> {
    let dir = ensure_feature_dir()?;
    restore_at(&dir, journal, committed)
}
fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains(['/', '\\', ':'])
        && Path::new(name).components().count() == 1
        && Path::new(name)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}
fn restore_at(dir: &Path, journal: &Journal, committed: bool) -> Result<()> {
    // Validate every entry before changing the first file. A valid JSON shape
    // alone must never turn a damaged before-image into a deletion request.
    validate_journal(journal)?;
    for item in &journal.files {
        let path = dir.join(&item.name);
        if committed {
            atomic_write(&path, &item.after)?;
        } else if let Some(before) = &item.before {
            atomic_write(&path, before)?;
        } else if path.exists() {
            fs::remove_file(path).map_err(io)?;
        }
    }
    fs::remove_file(dir.join(JOURNAL)).map_err(io)?;
    Ok(())
}

fn committed(pool: &DbPool, operation: &str) -> Result<bool> {
    Ok(pool.get()?.query_row(
        "SELECT EXISTS(SELECT 1 FROM audit_log WHERE module='personnel_change' AND source=?1)",
        [operation],
        |row| row.get(0),
    )?)
}

pub(super) fn lock(pool: &DbPool) -> Result<MutexGuard<'static, ()>> {
    let guard = FILES
        .lock()
        .map_err(|_| AppError::Internal("人员台账锁异常，请重启服务后重试".into()))?;
    let path = ensure_feature_dir()?.join(JOURNAL);
    if path.exists() {
        let journal: Journal =
            serde_json::from_slice(&fs::read(path).map_err(io)?).map_err(|e| {
                AppError::Internal(format!("人员操作恢复日志损坏，已阻止继续写入: {e}"))
            })?;
        validate_journal(&journal)?;
        restore(&journal, committed(pool, &journal.operation)?)?;
    }
    Ok(guard)
}

pub(super) fn mutate(
    pool: &DbPool,
    ctx: &AuthContext,
    action: &str,
    business_no: &str,
    reason: &str,
    before: Value,
    after: Value,
    files: Vec<FileChange>,
    notification: Option<(&str, &Value)>,
) -> Result<()> {
    let dir = ensure_feature_dir()?;
    let mut journal = Journal {
        operation: format!("personnel-journal:{}", uuid::Uuid::new_v4()),
        files,
        checksum: String::new(),
    };
    journal.checksum = journal_checksum(&journal)?;
    validate_journal(&journal)?;
    // A backup is retained even when an operation subsequently rolls back.
    let history = dir.join("summary-history");
    for item in &journal.files {
        if item.name == super::SUMMARY_FILE {
            if let Some(bytes) = &item.before {
                fs::create_dir_all(&history).map_err(io)?;
                atomic_write(
                    &history.join(format!(
                        "{}-{}.xlsx",
                        chrono::Local::now().format("%Y%m%d%H%M%S"),
                        uuid::Uuid::new_v4()
                    )),
                    bytes,
                )?;
            }
        }
    }
    atomic_write(
        &dir.join(JOURNAL),
        &serde_json::to_vec(&journal).map_err(|e| AppError::Internal(e.to_string()))?,
    )?;
    let result = (|| {
        let mut conn = pool.get()?;
        let tx = conn.transaction()?;
        crate::repo::audit_repo::log_structured_actor_on_conn(
            &tx,
            action,
            "personnel_change_notices",
            None,
            ctx.user.id,
            &ctx.user.username,
            reason,
            "personnel_change",
            business_no,
            Some(&before),
            Some(&after),
            &journal.operation,
        )?;
        if let Some((kind, payload)) = notification {
            match kind {
                "submitted" => {
                    crate::service::notification_service::enqueue_personnel_change_on_conn(
                        &tx,
                        business_no,
                        payload,
                    )?
                }
                "approved" => {
                    crate::service::notification_service::enqueue_personnel_change_approved_on_conn(
                        &tx,
                        business_no,
                        payload,
                    )?
                }
                "rejected" => {
                    crate::service::notification_service::enqueue_personnel_change_rejected_on_conn(
                        &tx,
                        business_no,
                        payload,
                    )?
                }
                _ => return Err(AppError::Internal("未知人员通知事件".into())),
            }
        }
        for item in &journal.files {
            atomic_write(&dir.join(&item.name), &item.after)?;
        }
        tx.commit()?;
        Ok(())
    })();
    match result {
        Ok(()) => restore(&journal, true),
        Err(error) => {
            // A commit response can be lost. Re-read the durable audit marker before
            // deciding whether to finish or roll back the file side of the operation.
            let was_committed = committed(pool, &journal.operation)?;
            restore(&journal, was_committed)?;
            if was_committed {
                Ok(())
            } else {
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sealed(files: Vec<FileChange>) -> Journal {
        let mut journal = Journal {
            operation: format!("personnel-journal:{}", uuid::Uuid::new_v4()),
            files,
            checksum: String::new(),
        };
        journal.checksum = journal_checksum(&journal).unwrap();
        journal
    }
    #[test]
    fn recovery_rolls_back_uncommitted_files_or_finishes_committed_files() {
        let root = std::env::temp_dir().join(format!(
            "labflow-personnel-recovery-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir(&root).unwrap();
        for committed in [false, true] {
            let journal = sealed(vec![
                FileChange {
                    name: "summary.xlsx".into(),
                    before: Some(b"before".to_vec()),
                    after: b"after".to_vec(),
                },
                FileChange {
                    name: "new.jsonl".into(),
                    before: None,
                    after: b"new".to_vec(),
                },
            ]);
            atomic_write(&root.join("summary.xlsx"), b"interrupted").unwrap();
            atomic_write(&root.join("new.jsonl"), b"partial").unwrap();
            atomic_write(&root.join(JOURNAL), b"journal").unwrap();
            restore_at(&root, &journal, committed).unwrap();
            assert_eq!(
                fs::read(root.join("summary.xlsx")).unwrap(),
                if committed {
                    b"after".as_slice()
                } else {
                    b"before".as_slice()
                }
            );
            assert_eq!(root.join("new.jsonl").exists(), committed);
            assert!(!root.join(JOURNAL).exists());
            if committed {
                fs::remove_file(root.join("new.jsonl")).unwrap();
            }
        }
        fs::remove_file(root.join("summary.xlsx")).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn damaged_journals_cannot_delete_existing_data_or_partially_restore_files() {
        let root = std::env::temp_dir().join(format!(
            "labflow-journal-validation-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir(&root).unwrap();
        let target = root.join("summary.xlsx");
        atomic_write(&target, b"unchanged").unwrap();
        let valid = sealed(vec![FileChange {
            name: "summary.xlsx".into(),
            before: Some(b"before".to_vec()),
            after: b"after".to_vec(),
        }]);
        let mut missing_before = serde_json::to_value(&valid).unwrap();
        missing_before["files"][0]
            .as_object_mut()
            .unwrap()
            .remove("before");
        assert!(serde_json::from_value::<Journal>(missing_before).is_err());
        for defect in ["operation", "later-path", "duplicate", "checksum", "empty"] {
            let mut journal = sealed(valid.files.clone());
            match defect {
                "operation" => journal.operation = "bad".into(),
                "later-path" => journal.files.push(FileChange {
                    name: "../escape".into(),
                    before: None,
                    after: vec![],
                }),
                "duplicate" => journal.files.push(journal.files[0].clone()),
                "empty" => journal.files.clear(),
                _ => {}
            }
            journal.checksum = journal_checksum(&journal).unwrap();
            if defect == "checksum" {
                journal.files[0].after.push(1);
            }
            atomic_write(&root.join(JOURNAL), &serde_json::to_vec(&journal).unwrap()).unwrap();
            assert!(restore_at(&root, &journal, false).is_err());
            assert_eq!(fs::read(&target).unwrap(), b"unchanged");
            assert!(root.join(JOURNAL).exists());
        }
        let nullable = sealed(vec![FileChange {
            name: "new.json".into(),
            before: None,
            after: vec![],
        }]);
        assert!(serde_json::from_slice::<Journal>(&serde_json::to_vec(&nullable).unwrap()).is_ok());
        fs::remove_file(root.join(JOURNAL)).unwrap();
        fs::remove_file(target).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
