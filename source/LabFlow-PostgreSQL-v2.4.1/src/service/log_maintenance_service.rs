use crate::config::AppConfig;
use crate::db::DbPool;
use crate::service::backup_service;
use chrono::{Datelike, Local};
use postgres_compat::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};

const POLICY_KEY: &str = "log_maintenance_policy_v1";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogMaintenancePolicy {
    #[serde(default = "default_session_retention")]
    pub session_retention_days: u32,
    #[serde(default = "default_runtime_online_retention")]
    pub runtime_log_online_retention_days: u32,
    #[serde(default = "default_runtime_archive_retention")]
    pub runtime_log_archive_retention_days: u32,
    #[serde(default = "default_true")]
    pub audit_auto_archive_enabled: bool,
    #[serde(default = "default_audit_online_retention")]
    pub audit_online_retention_days: u32,
    #[serde(default = "default_audit_archive_retention")]
    pub audit_archive_retention_days: u32,
    #[serde(default = "default_archive_time")]
    pub archive_time: String,
    #[serde(default = "default_true")]
    pub database_maintenance_enabled: bool,
    #[serde(default = "default_maintenance_day")]
    pub maintenance_day: u32,
    #[serde(default = "default_maintenance_time")]
    pub maintenance_time: String,
}

fn default_true() -> bool {
    true
}
fn default_session_retention() -> u32 {
    7
}
fn default_runtime_online_retention() -> u32 {
    30
}
fn default_runtime_archive_retention() -> u32 {
    90
}
fn default_audit_online_retention() -> u32 {
    365
}
fn default_audit_archive_retention() -> u32 {
    1825
}
fn default_archive_time() -> String {
    "02:30".into()
}
fn default_maintenance_day() -> u32 {
    1
}
fn default_maintenance_time() -> String {
    "03:30".into()
}

impl Default for LogMaintenancePolicy {
    fn default() -> Self {
        Self {
            session_retention_days: default_session_retention(),
            runtime_log_online_retention_days: default_runtime_online_retention(),
            runtime_log_archive_retention_days: default_runtime_archive_retention(),
            audit_auto_archive_enabled: true,
            audit_online_retention_days: default_audit_online_retention(),
            audit_archive_retention_days: default_audit_archive_retention(),
            archive_time: default_archive_time(),
            database_maintenance_enabled: true,
            maintenance_day: default_maintenance_day(),
            maintenance_time: default_maintenance_time(),
        }
    }
}

impl LogMaintenancePolicy {
    pub fn normalize(mut self) -> Self {
        self.session_retention_days = self.session_retention_days.min(90);
        self.runtime_log_online_retention_days = self.runtime_log_online_retention_days.min(3650);
        self.runtime_log_archive_retention_days = self.runtime_log_archive_retention_days.min(3650);
        self.audit_online_retention_days = self.audit_online_retention_days.clamp(30, 3650);
        self.audit_archive_retention_days = self.audit_archive_retention_days.min(36500);
        self.maintenance_day = self.maintenance_day.clamp(1, 28);
        if !valid_time(&self.archive_time) {
            self.archive_time = default_archive_time();
        }
        if !valid_time(&self.maintenance_time) {
            self.maintenance_time = default_maintenance_time();
        }
        self
    }
}

#[derive(Debug, Serialize)]
pub struct MaintenanceStatus {
    pub policy: LogMaintenancePolicy,
    pub db_size: u64,
    pub audit_online_count: i64,
    pub expired_session_count: i64,
    pub runtime_log_bytes: u64,
    pub archive_count: i64,
    pub archives: Vec<ArchiveBatch>,
    pub recent_jobs: Vec<MaintenanceJob>,
    pub runtime_log_enabled: bool,
    pub runtime_log_max_size_mb: u64,
    pub data_dir: String,
}

#[derive(Debug, Serialize)]
pub struct ArchiveBatch {
    pub id: i64,
    pub archive_type: String,
    pub start_at: Option<String>,
    pub end_at: Option<String>,
    pub file_path: String,
    pub sha256: String,
    pub record_count: i64,
    pub file_size: i64,
    pub status: String,
    pub created_at: String,
    pub verified_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MaintenanceJob {
    pub id: i64,
    pub job_type: String,
    pub status: String,
    pub detail: String,
    pub started_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct JobResult {
    pub detail: String,
    pub affected: i64,
}

#[derive(Serialize, Deserialize)]
struct ArchiveManifestFile {
    path: String,
    sha256: String,
    size: u64,
}

#[derive(Serialize, Deserialize)]
struct ArchiveManifest {
    format_version: u32,
    archive_type: String,
    created_at: String,
    record_count: usize,
    files: Vec<ArchiveManifestFile>,
}

#[derive(Serialize)]
struct AuditLine {
    id: i64,
    action: String,
    table_name: String,
    record_id: Option<i64>,
    user_id: Option<i64>,
    user_name: String,
    detail: String,
    module: String,
    business_no: String,
    before_json: Option<String>,
    after_json: Option<String>,
    source: String,
    created_at: String,
}

fn valid_time(value: &str) -> bool {
    let parts: Vec<_> = value.split(':').collect();
    parts.len() == 2
        && parts[0].parse::<u32>().map(|v| v < 24).unwrap_or(false)
        && parts[1].parse::<u32>().map(|v| v < 60).unwrap_or(false)
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn get_policy(pool: &DbPool) -> std::result::Result<LogMaintenancePolicy, String> {
    let conn = pool.get().map_err(|e| e.to_string())?;
    let value: Option<String> = conn
        .query_row(
            "SELECT value FROM system_settings WHERE key=?1",
            [POLICY_KEY],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(value
        .and_then(|value| serde_json::from_str::<LogMaintenancePolicy>(&value).ok())
        .unwrap_or_default()
        .normalize())
}

pub fn save_policy(
    pool: &DbPool,
    policy: LogMaintenancePolicy,
) -> std::result::Result<LogMaintenancePolicy, String> {
    let policy = policy.normalize();
    let value = serde_json::to_string(&policy).map_err(|e| e.to_string())?;
    pool.get().map_err(|e| e.to_string())?.execute(
        "INSERT INTO system_settings(key,value,updated_at) VALUES(?1,?2,datetime('now','localtime')) ON CONFLICT(key) DO UPDATE SET value=excluded.value,updated_at=excluded.updated_at",
        params![POLICY_KEY, value],
    ).map_err(|e| e.to_string())?;
    Ok(policy)
}

pub fn status(pool: &DbPool, config: &AppConfig) -> std::result::Result<MaintenanceStatus, String> {
    let conn = pool.get().map_err(|e| e.to_string())?;
    let policy = get_policy(pool)?;
    let audit_online_count = conn
        .query_row("SELECT COUNT(*) FROM audit_log", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    let expired_session_count = conn
        .query_row(
            "SELECT COUNT(*) FROM user_sessions WHERE datetime(expires_at) <= datetime('now')",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    let archives = list_archives_on_conn(&conn, 50)?;
    let recent_jobs = list_jobs_on_conn(&conn, 20)?;
    Ok(MaintenanceStatus {
        policy,
        db_size: conn
            .query_row("SELECT pg_database_size(current_database())", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap_or(0)
            .max(0) as u64,
        audit_online_count,
        expired_session_count,
        runtime_log_bytes: directory_bytes(&config.runtime_logs_dir()),
        archive_count: archives.len() as i64,
        archives,
        recent_jobs,
        runtime_log_enabled: config.runtime_log_enabled,
        runtime_log_max_size_mb: config.runtime_log_max_size_mb,
        data_dir: config.data_dir().to_string_lossy().to_string(),
    })
}

pub fn list_archives(pool: &DbPool) -> std::result::Result<Vec<ArchiveBatch>, String> {
    let conn = pool.get().map_err(|e| e.to_string())?;
    list_archives_on_conn(&conn, 200)
}

fn list_archives_on_conn(
    conn: &postgres_compat::Connection,
    limit: i64,
) -> std::result::Result<Vec<ArchiveBatch>, String> {
    let mut stmt = conn.prepare("SELECT id,archive_type,start_at,end_at,file_path,sha256,record_count,file_size,status,created_at,verified_at FROM log_archive_batches WHERE deleted_at IS NULL ORDER BY id DESC LIMIT ?1").map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([limit], |row| {
            Ok(ArchiveBatch {
                id: row.get(0)?,
                archive_type: row.get(1)?,
                start_at: row.get(2)?,
                end_at: row.get(3)?,
                file_path: row.get(4)?,
                sha256: row.get(5)?,
                record_count: row.get(6)?,
                file_size: row.get(7)?,
                status: row.get(8)?,
                created_at: row.get(9)?,
                verified_at: row.get(10)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

fn list_jobs_on_conn(
    conn: &postgres_compat::Connection,
    limit: i64,
) -> std::result::Result<Vec<MaintenanceJob>, String> {
    let mut stmt = conn.prepare("SELECT id,job_type,status,detail,started_at,completed_at FROM maintenance_jobs ORDER BY id DESC LIMIT ?1").map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([limit], |row| {
            Ok(MaintenanceJob {
                id: row.get(0)?,
                job_type: row.get(1)?,
                status: row.get(2)?,
                detail: row.get(3)?,
                started_at: row.get(4)?,
                completed_at: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

fn directory_bytes(dir: &Path) -> u64 {
    fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .map(|entry| {
            let path = entry.path();
            if path.is_dir() {
                directory_bytes(&path)
            } else {
                entry.metadata().map(|m| m.len()).unwrap_or(0)
            }
        })
        .sum()
}

fn with_job<T>(
    pool: &DbPool,
    job_type: &str,
    work: impl FnOnce() -> std::result::Result<(T, String), String>,
) -> std::result::Result<T, String> {
    let _guard = backup_service::DATA_MAINTENANCE_LOCK
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .map_err(|_| "maintenance lock poisoned".to_string())?;
    let conn = pool.get().map_err(|e| e.to_string())?;
    conn.execute("INSERT INTO maintenance_jobs(job_type,status,detail,created_by) VALUES(?1,'running','', 'system')", [job_type]).map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    match work() {
        Ok((value, detail)) => {
            conn.execute("UPDATE maintenance_jobs SET status='completed',detail=?1,completed_at=datetime('now','localtime') WHERE id=?2", params![detail, id]).map_err(|e| e.to_string())?;
            Ok(value)
        }
        Err(error) => {
            let _ = conn.execute("UPDATE maintenance_jobs SET status='failed',detail=?1,completed_at=datetime('now','localtime') WHERE id=?2", params![error, id]);
            Err(error)
        }
    }
}

pub fn cleanup_sessions(
    pool: &DbPool,
    policy: &LogMaintenancePolicy,
) -> std::result::Result<JobResult, String> {
    let days = policy.session_retention_days;
    with_job(pool, "session_cleanup", || {
        let conn = pool.get().map_err(|e| e.to_string())?;
        let cutoff = (Local::now() - chrono::Duration::days(days as i64))
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let affected = conn
            .execute("DELETE FROM user_sessions WHERE expires_at <= ?1", [cutoff])
            .map_err(|e| e.to_string())? as i64;
        Ok((
            JobResult {
                detail: format!("removed {affected} expired sessions retained for {days} days"),
                affected,
            },
            format!("removed {affected} expired sessions retained for {days} days"),
        ))
    })
}

pub fn archive_runtime_logs(
    pool: &DbPool,
    config: &AppConfig,
    policy: &LogMaintenancePolicy,
) -> std::result::Result<JobResult, String> {
    let config = config.clone();
    let policy = policy.clone();
    with_job(pool, "runtime_log_archive", || {
        let logs_dir = config.runtime_logs_dir();
        let archive_dir = logs_dir.join("archive");
        fs::create_dir_all(&archive_dir).map_err(|e| e.to_string())?;
        let now = std::time::SystemTime::now();
        let mut archived = 0i64;
        for entry in fs::read_dir(&logs_dir)
            .map_err(|e| e.to_string())?
            .flatten()
        {
            let path = entry.path();
            if path.is_dir() || path.extension().and_then(|v| v.to_str()) != Some("log") {
                continue;
            }
            let age = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|m| now.duration_since(m).ok())
                .map(|d| d.as_secs() / 86400)
                .unwrap_or(0);
            if age < policy.runtime_log_online_retention_days as u64 {
                continue;
            }
            let bytes = fs::read(&path).map_err(|e| e.to_string())?;
            let name = path
                .file_name()
                .and_then(|v| v.to_str())
                .unwrap_or("runtime.log");
            let destination = archive_dir.join(format!("{}.zip", name));
            if destination.exists() {
                fs::remove_file(&path).map_err(|e| e.to_string())?;
                continue;
            }
            create_simple_archive(&destination, "runtime", name, &bytes, 1)?;
            let file_bytes = fs::read(&destination).map_err(|e| e.to_string())?;
            pool.get().map_err(|e| e.to_string())?.execute("INSERT INTO log_archive_batches(archive_type,file_path,sha256,record_count,file_size,status,verified_at) VALUES('runtime',?1,?2,1,?3,'completed',datetime('now','localtime'))", params![destination.to_string_lossy(), sha256(&file_bytes), file_bytes.len() as i64]).map_err(|e| e.to_string())?;
            fs::remove_file(&path).map_err(|e| e.to_string())?;
            archived += 1;
        }
        mark_removed_archives(
            pool,
            cleanup_old_files(&archive_dir, policy.runtime_log_archive_retention_days)?,
        )?;
        Ok((
            JobResult {
                detail: format!("已归档 {archived} 个运行日志文件"),
                affected: archived,
            },
            format!("已归档 {archived} 个运行日志文件"),
        ))
    })
}

fn cleanup_old_files(dir: &Path, retention_days: u32) -> std::result::Result<Vec<String>, String> {
    if retention_days == 0 {
        return Ok(Vec::new());
    }
    let now = std::time::SystemTime::now();
    let mut removed = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())?.flatten() {
        if entry.path().extension().and_then(|v| v.to_str()) != Some("zip") {
            continue;
        }
        let age = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|m| now.duration_since(m).ok())
            .map(|d| d.as_secs() / 86400)
            .unwrap_or(0);
        if age >= retention_days as u64 {
            let path = entry.path();
            fs::remove_file(&path).map_err(|e| e.to_string())?;
            removed.push(path.to_string_lossy().to_string());
        }
    }
    Ok(removed)
}

fn mark_removed_archives(pool: &DbPool, removed: Vec<String>) -> std::result::Result<(), String> {
    if removed.is_empty() {
        return Ok(());
    }
    let conn = pool.get().map_err(|e| e.to_string())?;
    for path in removed {
        conn.execute("UPDATE log_archive_batches SET deleted_at=datetime('now','localtime'),status='expired' WHERE file_path=?1", [path]).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn create_simple_archive(
    destination: &Path,
    archive_type: &str,
    name: &str,
    bytes: &[u8],
    record_count: usize,
) -> std::result::Result<(), String> {
    let partial = destination.with_extension("partial");
    let _ = fs::remove_file(&partial);
    let file = fs::File::create(&partial).map_err(|e| e.to_string())?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file(name, options).map_err(|e| e.to_string())?;
    zip.write_all(bytes).map_err(|e| e.to_string())?;
    let manifest = ArchiveManifest {
        format_version: 1,
        archive_type: archive_type.into(),
        created_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        record_count,
        files: vec![ArchiveManifestFile {
            path: name.into(),
            sha256: sha256(bytes),
            size: bytes.len() as u64,
        }],
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
    zip.start_file("manifest.json", options)
        .map_err(|e| e.to_string())?;
    zip.write_all(&manifest_bytes).map_err(|e| e.to_string())?;
    zip.start_file("sha256.txt", options)
        .map_err(|e| e.to_string())?;
    zip.write_all(format!("{}  {}\n", sha256(bytes), name).as_bytes())
        .map_err(|e| e.to_string())?;
    zip.finish().map_err(|e| e.to_string())?;
    verify_archive_path(&partial)?;
    fs::rename(partial, destination).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn archive_audit(
    pool: &DbPool,
    config: &AppConfig,
    policy: &LogMaintenancePolicy,
) -> std::result::Result<JobResult, String> {
    let config = config.clone();
    let policy = policy.clone();
    with_job(pool, "audit_archive", || {
        let conn = pool.get().map_err(|e| e.to_string())?;
        let cutoff = (Local::now()
            - chrono::Duration::days(policy.audit_online_retention_days as i64))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
        let mut stmt = conn.prepare("SELECT id,action,table_name,record_id,user_id,user_name,detail,module,business_no,before_json,after_json,source,created_at FROM audit_log WHERE created_at < ?1 ORDER BY id").map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([cutoff], |row| {
                Ok(AuditLine {
                    id: row.get(0)?,
                    action: row.get(1)?,
                    table_name: row.get(2)?,
                    record_id: row.get(3)?,
                    user_id: row.get(4)?,
                    user_name: row.get::<_, String>(5).unwrap_or_default(),
                    detail: row.get::<_, String>(6).unwrap_or_default(),
                    module: row.get::<_, String>(7).unwrap_or_else(|_| "shared".into()),
                    business_no: row.get::<_, String>(8).unwrap_or_default(),
                    before_json: row.get(9)?,
                    after_json: row.get(10)?,
                    source: row
                        .get::<_, String>(11)
                        .unwrap_or_else(|_| "application".into()),
                    created_at: row.get(12)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        if rows.is_empty() {
            return Ok((
                JobResult {
                    detail: "当前没有达到归档条件的审计日志".into(),
                    affected: 0,
                },
                "当前没有达到归档条件的审计日志".into(),
            ));
        }
        let start_at = rows
            .first()
            .map(|row| row.created_at.clone())
            .unwrap_or_default();
        let end_at = rows
            .last()
            .map(|row| row.created_at.clone())
            .unwrap_or_default();
        let jsonl = rows
            .iter()
            .map(|row| {
                serde_json::to_string(row)
                    .map(|line| format!("{line}\n"))
                    .map_err(|e| e.to_string())
            })
            .collect::<std::result::Result<String, _>>()?;
        let summary = format!(
            "start_at,end_at,record_count\n{start_at},{end_at},{}\n",
            rows.len()
        );
        let directory = config.data_dir().join("archive").join("audit");
        fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let filename = format!(
            "audit_{}_{}_{}.zip",
            start_at.replace(['-', ' ', ':'], ""),
            end_at.replace(['-', ' ', ':'], ""),
            Local::now().format("%H%M%S")
        );
        let destination = directory.join(filename);
        create_audit_archive(&destination, &jsonl, &summary, rows.len())?;
        let bytes = fs::read(&destination).map_err(|e| e.to_string())?;
        let digest = sha256(&bytes);
        let min_id = rows.first().map(|row| row.id).unwrap_or(0);
        let max_id = rows.last().map(|row| row.id).unwrap_or(0);
        let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
        tx.execute("INSERT INTO log_archive_batches(archive_type,start_at,end_at,file_path,sha256,record_count,file_size,status,verified_at) VALUES('audit',?1,?2,?3,?4,?5,?6,'completed',datetime('now','localtime'))", params![start_at, end_at, destination.to_string_lossy(), digest, rows.len() as i64, bytes.len() as i64]).map_err(|e| e.to_string())?;
        tx.execute(
            "DELETE FROM audit_log WHERE id>=?1 AND id<=?2",
            params![min_id, max_id],
        )
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        mark_removed_archives(
            pool,
            cleanup_old_files(&directory, policy.audit_archive_retention_days)?,
        )?;
        Ok((
            JobResult {
                detail: format!("已归档 {} 条审计日志", rows.len()),
                affected: rows.len() as i64,
            },
            format!("已归档 {} 条审计日志", rows.len()),
        ))
    })
}

fn create_audit_archive(
    destination: &Path,
    jsonl: &str,
    summary: &str,
    record_count: usize,
) -> std::result::Result<(), String> {
    let partial = destination.with_extension("partial");
    let _ = fs::remove_file(&partial);
    let file = fs::File::create(&partial).map_err(|e| e.to_string())?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let files = [
        ("audit_log.jsonl", jsonl.as_bytes()),
        ("summary.csv", summary.as_bytes()),
    ];
    let mut manifest_files = Vec::new();
    for (name, bytes) in files {
        zip.start_file(name, options).map_err(|e| e.to_string())?;
        zip.write_all(bytes).map_err(|e| e.to_string())?;
        manifest_files.push(ArchiveManifestFile {
            path: name.into(),
            sha256: sha256(bytes),
            size: bytes.len() as u64,
        });
    }
    let manifest = ArchiveManifest {
        format_version: 1,
        archive_type: "audit".into(),
        created_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        record_count,
        files: manifest_files,
    };
    zip.start_file("manifest.json", options)
        .map_err(|e| e.to_string())?;
    zip.write_all(&serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    zip.start_file("sha256.txt", options)
        .map_err(|e| e.to_string())?;
    zip.write_all(
        manifest
            .files
            .iter()
            .map(|f| format!("{}  {}\n", f.sha256, f.path))
            .collect::<String>()
            .as_bytes(),
    )
    .map_err(|e| e.to_string())?;
    zip.finish().map_err(|e| e.to_string())?;
    verify_archive_path(&partial)?;
    fs::rename(partial, destination).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn verify_archive(pool: &DbPool, id: i64) -> std::result::Result<ArchiveBatch, String> {
    let conn = pool.get().map_err(|e| e.to_string())?;
    let path: String = conn
        .query_row(
            "SELECT file_path FROM log_archive_batches WHERE id=?1",
            [id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    verify_archive_path(Path::new(&path))?;
    conn.execute("UPDATE log_archive_batches SET verified_at=datetime('now','localtime'),status='completed' WHERE id=?1", [id]).map_err(|e| e.to_string())?;
    list_archives_on_conn(&conn, 200)?
        .into_iter()
        .find(|batch| batch.id == id)
        .ok_or_else(|| "归档包验证后未找到".into())
}

fn verify_archive_path(path: &Path) -> std::result::Result<(), String> {
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|e| e.to_string())?;
    let manifest: ArchiveManifest = {
        let mut item = archive
            .by_name("manifest.json")
            .map_err(|_| "archive manifest missing".to_string())?;
        let mut bytes = Vec::new();
        item.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        serde_json::from_slice(&bytes).map_err(|e| e.to_string())?
    };
    let mut line_count = 0usize;
    for expected in &manifest.files {
        let mut item = archive
            .by_name(&expected.path)
            .map_err(|_| format!("archive file missing: {}", expected.path))?;
        let mut bytes = Vec::new();
        item.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        if bytes.len() as u64 != expected.size || sha256(&bytes) != expected.sha256 {
            return Err(format!("archive checksum failed: {}", expected.path));
        }
        if expected.path == "audit_log.jsonl" {
            line_count = bytes.iter().filter(|byte| **byte == b'\n').count();
        }
    }
    if manifest.archive_type == "audit" && line_count != manifest.record_count {
        return Err("audit archive record count verification failed".into());
    }
    Ok(())
}

pub fn database_maintenance(
    pool: &DbPool,
    config: &AppConfig,
) -> std::result::Result<JobResult, String> {
    let config = config.clone();
    with_job(pool, "database_maintenance", || {
        let backup = backup_service::create_database_backup(&config, true)?;
        let conn = pool.get().map_err(|e| e.to_string())?;
        conn.execute_batch("VACUUM (ANALYZE);")
            .map_err(|e| e.to_string())?;
        let integrity: i64 = conn
            .query_row("SELECT 1", [], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        if integrity != 1 {
            return Err(format!("integrity_check failed: {integrity}"));
        }
        Ok((
            JobResult {
                detail: format!(
                    "数据库备份 {}、VACUUM ANALYZE 和连接检查已完成",
                    backup.name
                ),
                affected: 1,
            },
            format!(
                "数据库备份 {}、VACUUM ANALYZE 和连接检查已完成",
                backup.name
            ),
        ))
    })
}

fn completed_today(conn: &postgres_compat::Connection, job_type: &str) -> bool {
    let today = Local::now().format("%Y-%m-%d").to_string();
    conn.query_row("SELECT EXISTS(SELECT 1 FROM maintenance_jobs WHERE job_type=?1 AND LEFT(started_at,10)=?2)", [job_type, today.as_str()], |row| row.get(0)).unwrap_or(false)
}

pub fn run_scheduled(pool: &DbPool, config: &AppConfig) {
    let policy = match get_policy(pool) {
        Ok(value) => value,
        Err(error) => {
            tracing::error!("load log maintenance policy failed: {error}");
            return;
        }
    };
    let now = Local::now();
    let current = now.format("%H:%M").to_string();
    let conn = match pool.get() {
        Ok(value) => value,
        Err(error) => {
            tracing::error!("maintenance connection failed: {error}");
            return;
        }
    };
    if current >= policy.archive_time && !completed_today(&conn, "session_cleanup") {
        let _ = cleanup_sessions(pool, &policy);
    }
    if current >= policy.archive_time && !completed_today(&conn, "runtime_log_archive") {
        let _ = archive_runtime_logs(pool, config, &policy);
    }
    if policy.audit_auto_archive_enabled
        && current >= policy.archive_time
        && !completed_today(&conn, "audit_archive")
    {
        let _ = archive_audit(pool, config, &policy);
    }
    if policy.database_maintenance_enabled
        && now.day() == policy.maintenance_day
        && current >= policy.maintenance_time
        && !completed_today(&conn, "database_maintenance")
    {
        let _ = database_maintenance(pool, config);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn audit_archive_is_verified_before_online_rows_are_removed() {
        let root =
            std::env::temp_dir().join(format!("workload_log_maintenance_{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let cfg = AppConfig {
            db_dir: root.to_string_lossy().to_string(),
            ..AppConfig::default()
        };
        let pool = crate::db::init_pool("postgres-test");
        let conn = pool.get().unwrap();
        crate::db::test_migrations::run(&conn).unwrap();
        conn.execute("INSERT INTO audit_log(action,table_name,user_name,detail,created_at) VALUES('update','tests','tester','archive test','2020-01-01 00:00:00')", []).unwrap();
        drop(conn);

        let policy = LogMaintenancePolicy {
            audit_online_retention_days: 30,
            ..LogMaintenancePolicy::default()
        };
        let result = archive_audit(&pool, &cfg, &policy).unwrap();
        assert_eq!(result.affected, 1);
        let conn = pool.get().unwrap();
        let online: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM audit_log WHERE table_name='tests'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(online, 0);
        let archive: String = conn.query_row("SELECT file_path FROM log_archive_batches WHERE archive_type='audit' ORDER BY id DESC LIMIT 1", [], |row| row.get(0)).unwrap();
        drop(conn);
        verify_archive_path(Path::new(&archive)).unwrap();
        let _ = fs::remove_dir_all(root);
    }
}
