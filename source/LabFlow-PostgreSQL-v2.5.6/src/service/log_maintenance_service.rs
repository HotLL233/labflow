use crate::config::AppConfig;
use crate::db::DbPool;
use crate::service::backup_service;
use chrono::{Datelike, Local};
use postgres_compat::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
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

#[derive(Serialize, Deserialize)]
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
    #[serde(default)]
    operator_user_id: Option<i64>,
    #[serde(default)]
    operator_username_snapshot: String,
    #[serde(default)]
    business_user_id: Option<i64>,
    #[serde(default)]
    business_username_snapshot: String,
    #[serde(default)]
    business_division_id: Option<i64>,
    #[serde(default)]
    business_division_name_snapshot: String,
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
    let archive_count = conn
        .query_row(
            "SELECT COUNT(*) FROM log_archive_batches WHERE deleted_at IS NULL",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
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
        archive_count,
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

pub fn archives_page(
    pool: &DbPool,
    keyword: &str,
    page: i64,
    page_size: i64,
) -> std::result::Result<crate::models::PaginatedResponse<ArchiveBatch>, String> {
    let keyword = keyword.trim();
    if keyword.chars().count() > 200 {
        return Err("归档查询关键词不能超过200个字符".into());
    }
    let page = page.max(1);
    let page_size = page_size.clamp(1, 200);
    let conn = pool.get().map_err(|error| error.to_string())?;
    let condition = "WHERE deleted_at IS NULL AND (?1='' OR file_path ILIKE '%'||?1||'%' OR archive_type ILIKE '%'||?1||'%' OR COALESCE(start_at,'') ILIKE '%'||?1||'%' OR COALESCE(end_at,'') ILIKE '%'||?1||'%')";
    let total: i64 = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM log_archive_batches {condition}"),
            [keyword],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let mut stmt = conn.prepare(
        &format!("SELECT id,archive_type,start_at,end_at,file_path,sha256,record_count,file_size,status,created_at,verified_at FROM log_archive_batches {condition} ORDER BY id DESC LIMIT ?2 OFFSET ?3"),
    ).map_err(|error| error.to_string())?;
    let items = stmt
        .query_map(
            params![keyword, page_size, (page - 1).saturating_mul(page_size)],
            archive_from_row,
        )
        .map_err(|error| error.to_string())?
        .collect::<postgres_compat::Result<Vec<_>>>()
        .map_err(|error| error.to_string())?;
    Ok(crate::models::PaginatedResponse {
        items,
        total,
        page,
        page_size,
    })
}

fn stored_archive_path(config: &AppConfig, path: &Path) -> std::result::Result<String, String> {
    path.strip_prefix(config.data_dir())
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .map_err(|_| "归档路径不在数据目录中".into())
}

pub fn archive_relative_path(
    archive_type: &str,
    stored: &str,
) -> std::result::Result<String, String> {
    let prefix = match archive_type {
        "audit" => "archive/audit/",
        "runtime" => "logs/archive/",
        _ => return Err("不支持的归档类型".into()),
    };
    let normalized = stored.replace('\\', "/");
    let legacy_absolute = normalized.starts_with('/')
        || (normalized.as_bytes().get(1) == Some(&b':')
            && normalized.as_bytes().get(2) == Some(&b'/'));
    let name = normalized
        .strip_prefix(prefix)
        .or_else(|| {
            legacy_absolute
                .then(|| {
                    normalized
                        .rsplit_once(&format!("/{prefix}"))
                        .map(|(_, name)| name)
                })
                .flatten()
        })
        .ok_or("归档路径不属于指定的日志目录")?;
    if name.is_empty()
        || !name.ends_with(".zip")
        || name.contains(['/', '\\', ':', '\0'])
        || name == "."
        || name == ".."
    {
        return Err("归档文件路径无效".into());
    }
    Ok(format!("{prefix}{name}"))
}

pub(crate) fn resolved_archive_path(
    config: &AppConfig,
    archive_type: &str,
    stored: &str,
) -> std::result::Result<PathBuf, String> {
    let relative = archive_relative_path(archive_type, stored)?;
    let path = config.data_dir().join(relative);
    let root = match archive_type {
        "audit" => config.data_dir().join("archive/audit"),
        "runtime" => config.runtime_logs_dir().join("archive"),
        _ => return Err("不支持的归档类型".into()),
    }
    .canonicalize()
    .map_err(|_| "归档目录不存在，请确认历史归档文件已随备份恢复".to_string())?;
    let resolved = path
        .canonicalize()
        .map_err(|_| "归档文件不存在，请从原主机补充恢复历史归档".to_string())?;
    if !resolved.starts_with(root) || !resolved.is_file() {
        return Err("归档文件不能越过日志目录".into());
    }
    Ok(resolved)
}

fn runtime_archive_matches(path: &Path, name: &str, bytes: &[u8]) -> bool {
    if verify_archive_path(path).is_err() {
        return false;
    }
    let Ok(file) = fs::File::open(path) else {
        return false;
    };
    let Ok(mut archive) = ZipArchive::new(file) else {
        return false;
    };
    let Ok(mut entry) = archive.by_name(name) else {
        return false;
    };
    let mut existing = Vec::new();
    entry.read_to_end(&mut existing).is_ok() && existing == bytes
}

fn list_archives_on_conn(
    conn: &postgres_compat::Connection,
    limit: i64,
) -> std::result::Result<Vec<ArchiveBatch>, String> {
    let mut stmt = conn.prepare("SELECT id,archive_type,start_at,end_at,file_path,sha256,record_count,file_size,status,created_at,verified_at FROM log_archive_batches WHERE deleted_at IS NULL ORDER BY id DESC LIMIT ?1").map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([limit], archive_from_row)
        .map_err(|e| e.to_string())?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

fn archive_from_row(row: &postgres_compat::Row<'_>) -> postgres_compat::Result<ArchiveBatch> {
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
            let name = entry.file_name().to_string_lossy().to_string();
            // Today's file may still be held by SizeRollingWriter, including when retention is zero.
            if path.is_dir()
                || path.extension().and_then(|v| v.to_str()) != Some("log")
                || name.starts_with(&format!("app-{}", Local::now().format("%Y-%m-%d")))
            {
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
            let mut destination = archive_dir.join(format!("{name}.zip"));
            if destination.exists() && !runtime_archive_matches(&destination, &name, &bytes) {
                destination =
                    archive_dir.join(format!("{name}.{}.zip", uuid::Uuid::new_v4().simple()));
            }
            if !destination.exists() {
                create_simple_archive(&destination, "runtime", &name, &bytes, 1)?;
            }
            let file_bytes = fs::read(&destination).map_err(|e| e.to_string())?;
            let stored_path = stored_archive_path(&config, &destination)?;
            pool.get().map_err(|e| e.to_string())?.execute("INSERT INTO log_archive_batches(archive_type,file_path,sha256,record_count,file_size,status,verified_at) SELECT 'runtime',?1,?2,1,?3,'completed',datetime('now','localtime') WHERE NOT EXISTS(SELECT 1 FROM log_archive_batches WHERE file_path=?1 AND deleted_at IS NULL)", params![stored_path, sha256(&file_bytes), file_bytes.len() as i64]).map_err(|e| e.to_string())?;
            fs::remove_file(&path).map_err(|e| e.to_string())?;
            archived += 1;
        }
        mark_removed_archives(
            pool,
            &config,
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

fn mark_removed_archives(
    pool: &DbPool,
    config: &AppConfig,
    removed: Vec<String>,
) -> std::result::Result<(), String> {
    if removed.is_empty() {
        return Ok(());
    }
    let conn = pool.get().map_err(|e| e.to_string())?;
    for path in removed {
        let relative = stored_archive_path(config, Path::new(&path))?;
        conn.execute("UPDATE log_archive_batches SET deleted_at=datetime('now','localtime'),status='expired' WHERE file_path=?1 OR file_path=?2", params![path, relative]).map_err(|e| e.to_string())?;
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
        let mut stmt = conn.prepare("SELECT id,action,table_name,record_id,user_id,user_name,detail,module,business_no,before_json,after_json,source,created_at,operator_user_id,operator_username_snapshot,business_user_id,business_username_snapshot,business_division_id,business_division_name_snapshot FROM audit_log WHERE created_at < ?1 ORDER BY id").map_err(|e| e.to_string())?;
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
                    operator_user_id: row.get(13)?,
                    operator_username_snapshot: row.get(14)?,
                    business_user_id: row.get(15)?,
                    business_username_snapshot: row.get(16)?,
                    business_division_id: row.get(17)?,
                    business_division_name_snapshot: row.get(18)?,
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
            .iter()
            .map(|row| row.created_at.as_str())
            .min()
            .unwrap_or_default();
        let end_at = rows
            .iter()
            .map(|row| row.created_at.as_str())
            .max()
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
            uuid::Uuid::new_v4().simple()
        );
        let destination = directory.join(filename);
        create_audit_archive(&destination, &jsonl, &summary, rows.len())?;
        let bytes = fs::read(&destination).map_err(|e| e.to_string())?;
        let digest = sha256(&bytes);
        let stored_path = stored_archive_path(&config, &destination)?;
        let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
        tx.execute("INSERT INTO log_archive_batches(archive_type,start_at,end_at,file_path,sha256,record_count,file_size,status,verified_at) VALUES('audit',?1,?2,?3,?4,?5,?6,'completed',datetime('now','localtime'))", params![start_at, end_at, stored_path, digest, rows.len() as i64, bytes.len() as i64]).map_err(|e| e.to_string())?;
        for batch in rows.chunks(500) {
            let placeholders = (1..=batch.len())
                .map(|index| format!("?{index}"))
                .collect::<Vec<_>>()
                .join(",");
            tx.execute(
                &format!("DELETE FROM audit_log WHERE id IN ({placeholders})"),
                postgres_compat::params_from_iter(batch.iter().map(|row| row.id)),
            )
            .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())?;
        mark_removed_archives(
            pool,
            &config,
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
        format_version: 2,
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

fn archive_by_id(pool: &DbPool, id: i64) -> std::result::Result<ArchiveBatch, String> {
    let conn = pool.get().map_err(|e| e.to_string())?;
    conn
        .query_row(
            "SELECT id,archive_type,start_at,end_at,file_path,sha256,record_count,file_size,status,created_at,verified_at FROM log_archive_batches WHERE id=?1 AND deleted_at IS NULL",
            [id],
            archive_from_row,
        )
        .optional()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "归档批次不存在或已过期".into())
}

pub fn verified_archive(
    pool: &DbPool,
    config: &AppConfig,
    id: i64,
) -> std::result::Result<(ArchiveBatch, PathBuf), String> {
    let batch = archive_by_id(pool, id)?;
    let path = resolved_archive_path(config, &batch.archive_type, &batch.file_path)?;
    let mut file = fs::File::open(&path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 65536];
    let mut size = 0_u64;
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
        size = size.saturating_add(count as u64);
    }
    if size != batch.file_size.max(0) as u64 || format!("{:x}", hasher.finalize()) != batch.sha256 {
        return Err("归档文件与数据库校验值不一致，已拒绝读取".into());
    }
    verify_archive_path(&path)?;
    Ok((batch, path))
}

pub fn verify_archive(
    pool: &DbPool,
    config: &AppConfig,
    id: i64,
) -> std::result::Result<ArchiveBatch, String> {
    let (batch, _) = verified_archive(pool, config, id)?;
    let conn = pool.get().map_err(|e| e.to_string())?;
    let stored = archive_relative_path(&batch.archive_type, &batch.file_path)?;
    conn.execute("UPDATE log_archive_batches SET verified_at=datetime('now','localtime'),status='completed',file_path=?2 WHERE id=?1", params![id, stored]).map_err(|e| e.to_string())?;
    drop(conn);
    archive_by_id(pool, id)
}

pub fn archive_entries(
    pool: &DbPool,
    config: &AppConfig,
    id: i64,
    keyword: &str,
    start_date: Option<&str>,
    end_date: Option<&str>,
    page: i64,
    page_size: i64,
) -> std::result::Result<
    crate::models::PaginatedResponse<crate::models::audit::AuditLogResponse>,
    String,
> {
    for date in [start_date, end_date].into_iter().flatten() {
        chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .map_err(|_| "归档查询日期无效，请使用YYYY-MM-DD".to_string())?;
    }
    if start_date
        .zip(end_date)
        .is_some_and(|(start, end)| start > end)
    {
        return Err("开始日期不能晚于结束日期".into());
    }
    if keyword.chars().count() > 200 {
        return Err("搜索关键词不能超过200个字符".into());
    }
    let (batch, path) = verified_archive(pool, config, id)?;
    if batch.archive_type != "audit" {
        return Err("仅审计归档支持记录检索，运行日志请下载查看".into());
    }
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|e| e.to_string())?;
    let entry = archive
        .by_name("audit_log.jsonl")
        .map_err(|e| e.to_string())?;
    let page = page.max(1);
    let page_size = page_size.clamp(1, 500);
    let offset = (page - 1).saturating_mul(page_size);
    let keyword = keyword.to_lowercase();
    let mut total = 0_i64;
    let mut items = Vec::new();
    // shortcut: searches scan one archive; add an index if archive searches become frequent.
    for line in BufReader::new(entry).lines() {
        let line = line.map_err(|e| e.to_string())?;
        let record: AuditLine =
            serde_json::from_str(&line).map_err(|_| "审计归档记录格式无效".to_string())?;
        let date = record.created_at.get(..10).unwrap_or("");
        if (!keyword.is_empty() && !line.to_lowercase().contains(&keyword))
            || start_date.is_some_and(|start| date < start)
            || end_date.is_some_and(|end| date > end)
        {
            continue;
        }
        if total >= offset && items.len() < page_size as usize {
            items.push(crate::models::audit::AuditLogResponse {
                id: record.id,
                action: record.action,
                table_name: record.table_name,
                record_id: record.record_id,
                user_id: record.user_id,
                user_name: record.user_name,
                detail: record.detail,
                module: record.module,
                business_no: record.business_no,
                before_data: record
                    .before_json
                    .and_then(|value| serde_json::from_str(&value).ok()),
                after_data: record
                    .after_json
                    .and_then(|value| serde_json::from_str(&value).ok()),
                source: record.source,
                operator_user_id: record.operator_user_id,
                operator_username_snapshot: record.operator_username_snapshot,
                business_user_id: record.business_user_id,
                business_username_snapshot: record.business_username_snapshot,
                business_division_id: record.business_division_id,
                business_division_name_snapshot: record.business_division_name_snapshot,
                created_at: record.created_at,
            });
        }
        total += 1;
    }
    Ok(crate::models::PaginatedResponse {
        items,
        total,
        page,
        page_size,
    })
}

fn verify_archive_path(path: &Path) -> std::result::Result<(), String> {
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|e| e.to_string())?;
    let manifest: ArchiveManifest = {
        let item = archive
            .by_name("manifest.json")
            .map_err(|_| "archive manifest missing".to_string())?;
        let mut bytes = Vec::new();
        item.take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 1024 * 1024 {
            return Err("归档清单过大".into());
        }
        serde_json::from_slice(&bytes).map_err(|e| e.to_string())?
    };
    let mut line_count = 0usize;
    for expected in &manifest.files {
        let mut item = archive
            .by_name(&expected.path)
            .map_err(|_| format!("archive file missing: {}", expected.path))?;
        let mut size = 0_u64;
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; 65536];
        loop {
            let count = item.read(&mut buffer).map_err(|e| e.to_string())?;
            if count == 0 {
                break;
            }
            size = size.saturating_add(count as u64);
            if size > expected.size {
                return Err(format!(
                    "archive size verification failed: {}",
                    expected.path
                ));
            }
            hasher.update(&buffer[..count]);
            if expected.path == "audit_log.jsonl" {
                line_count += buffer[..count]
                    .iter()
                    .filter(|byte| **byte == b'\n')
                    .count();
            }
        }
        if size != expected.size || format!("{:x}", hasher.finalize()) != expected.sha256 {
            return Err(format!("archive checksum failed: {}", expected.path));
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

fn scheduled_job_due(
    conn: &postgres_compat::Connection,
    job_type: &str,
) -> std::result::Result<bool, String> {
    scheduled_job_due_at(conn, job_type, Local::now().naive_local())
}

fn scheduled_job_due_at(
    conn: &postgres_compat::Connection,
    job_type: &str,
    now: chrono::NaiveDateTime,
) -> std::result::Result<bool, String> {
    let today = now.format("%Y-%m-%d").to_string();
    let stale = (now - chrono::Duration::minutes(30))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    conn.execute("UPDATE maintenance_jobs SET status='failed',detail='上次维护任务超时或程序已重启，允许有限重试',completed_at=datetime('now','localtime') WHERE job_type=?1 AND status='running' AND started_at<=?2", params![job_type, stale]).map_err(|e| e.to_string())?;
    let (attempts, completed, running, latest): (i64, i64, i64, Option<String>) = conn.query_row(
        "SELECT COUNT(*),COUNT(*) FILTER(WHERE status='completed'),COUNT(*) FILTER(WHERE status='running'),MAX(started_at) FROM maintenance_jobs WHERE job_type=?1 AND LEFT(started_at,10)=?2",
        params![job_type, today],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    ).map_err(|e| e.to_string())?;
    if completed > 0 || running > 0 || attempts >= 3 {
        return Ok(false);
    }
    let retry_cutoff = (now - chrono::Duration::minutes(5))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    Ok(latest.is_none_or(|started| started <= retry_cutoff))
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
    for (job, enabled) in [
        ("session_cleanup", current >= policy.archive_time),
        ("runtime_log_archive", current >= policy.archive_time),
        (
            "audit_archive",
            policy.audit_auto_archive_enabled && current >= policy.archive_time,
        ),
        (
            "database_maintenance",
            policy.database_maintenance_enabled
                && now.day() == policy.maintenance_day
                && current >= policy.maintenance_time,
        ),
    ] {
        if !enabled {
            continue;
        }
        let result = scheduled_job_due(&conn, job).and_then(|due| {
            if !due {
                return Ok(());
            }
            match job {
                "session_cleanup" => cleanup_sessions(pool, &policy),
                "runtime_log_archive" => archive_runtime_logs(pool, config, &policy),
                "audit_archive" => archive_audit(pool, config, &policy),
                _ => database_maintenance(pool, config),
            }
            .map(|_| ())
        });
        if let Err(error) = result {
            tracing::error!(job, %error, "计划维护未完成，将按有限重试策略处理");
        }
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
        conn.execute("INSERT INTO audit_log(action,table_name,user_name,detail,operator_user_id,operator_username_snapshot,business_user_id,business_username_snapshot,business_division_id,business_division_name_snapshot,created_at) VALUES('update','tests','tester','archive test',101,'operator',202,'subject',303,'department','2020-01-01 00:00:00')", []).unwrap();
        conn.execute("INSERT INTO audit_log(action,table_name,user_name,detail) VALUES('update','tests','tester','keep recent audit')", []).unwrap();
        let recent_id = conn.last_insert_rowid();
        conn.execute("INSERT INTO audit_log(action,table_name,user_name,detail,created_at) VALUES('update','tests','tester','archive test 2','2021-01-01 00:00:00')", []).unwrap();
        drop(conn);

        let policy = LogMaintenancePolicy {
            audit_online_retention_days: 30,
            ..LogMaintenancePolicy::default()
        };
        let result = archive_audit(&pool, &cfg, &policy).unwrap();
        assert_eq!(result.affected, 2);
        let conn = pool.get().unwrap();
        let online: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM audit_log WHERE table_name='tests'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(online, 1);
        let kept: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM audit_log WHERE id=?1",
                [recent_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(kept, 1);
        let id: i64 = conn.query_row("SELECT id FROM log_archive_batches WHERE archive_type='audit' ORDER BY id DESC LIMIT 1", [], |row| row.get(0)).unwrap();
        let stored: String = conn
            .query_row(
                "SELECT file_path FROM log_archive_batches WHERE id=?1",
                [id],
                |row| row.get(0),
            )
            .unwrap();
        let legacy = format!("C:\\old-host\\data\\{}", stored.replace('/', "\\"));
        conn.execute(
            "UPDATE log_archive_batches SET file_path=?1 WHERE id=?2",
            params![legacy, id],
        )
        .unwrap();
        drop(conn);
        let entries = archive_entries(&pool, &cfg, id, "archive test", None, None, 1, 1).unwrap();
        assert_eq!(entries.total, 2);
        assert_eq!(entries.items.len(), 1);
        let archived = &entries.items[0];
        assert_eq!(archived.operator_user_id, Some(101));
        assert_eq!(archived.operator_username_snapshot, "operator");
        assert_eq!(archived.business_user_id, Some(202));
        assert_eq!(archived.business_username_snapshot, "subject");
        assert_eq!(archived.business_division_id, Some(303));
        assert_eq!(archived.business_division_name_snapshot, "department");
        assert_eq!(verify_archive(&pool, &cfg, id).unwrap().file_path, stored);
        let second = archive_entries(
            &pool,
            &cfg,
            id,
            "",
            Some("2021-01-01"),
            Some("2021-01-01"),
            1,
            50,
        )
        .unwrap();
        assert_eq!(second.total, 1);
        let (_, archive) = verified_archive(&pool, &cfg, id).unwrap();
        fs::write(&archive, b"tampered").unwrap();
        assert!(verified_archive(&pool, &cfg, id).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn archive_paths_relocate_only_known_log_roots() {
        assert_eq!(
            archive_relative_path("audit", "C:\\old\\data\\archive\\audit\\audit.zip").unwrap(),
            "archive/audit/audit.zip"
        );
        assert_eq!(
            archive_relative_path("runtime", "/old/data/logs/archive/app.log.zip").unwrap(),
            "logs/archive/app.log.zip"
        );
        for value in [
            "../outside.zip",
            "other/archive/audit/audit.zip",
            "C:/other/secret.zip",
            "archive/audit/../../secret.zip",
            "archive/audit/C:secret.zip",
        ] {
            assert!(archive_relative_path("audit", value).is_err(), "{value}");
        }
        let legacy = r#"{"id":1,"action":"update","table_name":"tests","record_id":null,"user_id":null,"user_name":"tester","detail":"","module":"shared","business_no":"","before_json":null,"after_json":null,"source":"application","created_at":"2020-01-01 00:00:00"}"#;
        let record: AuditLine = serde_json::from_str(legacy).unwrap();
        assert_eq!(record.operator_user_id, None);
        assert_eq!(record.business_username_snapshot, "");
    }

    #[test]
    fn archive_pagination_reaches_batches_older_than_two_hundred() {
        let pool = crate::db::init_pool("archive-pagination");
        let conn = pool.get().unwrap();
        crate::db::test_migrations::run(&conn).unwrap();
        conn.execute("INSERT INTO log_archive_batches(archive_type,file_path,sha256) SELECT 'audit','archive/audit/old-'||n||'.zip','' FROM generate_series(1,225) AS source(n) ORDER BY n", []).unwrap();
        drop(conn);
        let page = archives_page(&pool, "", 5, 50).unwrap();
        assert_eq!(page.total, 225);
        assert_eq!(page.items.len(), 25);
        assert_eq!(
            page.items.last().unwrap().file_path,
            "archive/audit/old-1.zip"
        );
        let search = archives_page(&pool, "old-1.zip", 1, 50).unwrap();
        assert_eq!(search.total, 1);
        assert_eq!(search.items[0].file_path, "archive/audit/old-1.zip");
        pool.get().unwrap().execute("UPDATE log_archive_batches SET deleted_at=datetime('now','localtime') WHERE file_path='archive/audit/old-1.zip'", []).unwrap();
        assert_eq!(archives_page(&pool, "old-1.zip", 1, 50).unwrap().total, 0);
    }

    #[test]
    fn runtime_archive_keeps_active_logs_and_does_not_trust_existing_zip() {
        let root = std::env::temp_dir().join(format!("labflow-log-archive-{}", Uuid::new_v4()));
        let cfg = AppConfig {
            db_dir: root.to_string_lossy().to_string(),
            ..AppConfig::default()
        };
        let pool = crate::db::init_pool("postgres-test");
        crate::db::test_migrations::run(&pool.get().unwrap()).unwrap();
        let logs = cfg.runtime_logs_dir();
        fs::create_dir_all(logs.join("archive")).unwrap();
        let mut writer = crate::runtime_log::SizeRollingWriter::new(logs.clone(), 1);
        writer.write_all(b"before archive\n").unwrap();
        let old = logs.join("app-2000-01-01.log");
        fs::write(&old, b"old log\n").unwrap();
        let existing = logs.join("archive/app-2000-01-01.log.zip");
        fs::write(&existing, b"broken archive").unwrap();
        let policy = LogMaintenancePolicy {
            runtime_log_online_retention_days: 0,
            ..LogMaintenancePolicy::default()
        };
        assert_eq!(
            archive_runtime_logs(&pool, &cfg, &policy).unwrap().affected,
            1
        );
        writer.write_all(b"after archive\n").unwrap();
        writer.flush().unwrap();
        let current = logs.join(format!("app-{}.log", Local::now().format("%Y-%m-%d")));
        assert_eq!(
            fs::read_to_string(current).unwrap(),
            "before archive\nafter archive\n"
        );
        assert_eq!(fs::read(&existing).unwrap(), b"broken archive");
        assert!(!old.exists());
        let id: i64 = pool
            .get()
            .unwrap()
            .query_row("SELECT id FROM log_archive_batches LIMIT 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(verified_archive(&pool, &cfg, id).is_ok());
        drop(writer);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_scheduled_maintenance_has_delayed_bounded_retries() {
        let pool = crate::db::init_pool("postgres-test");
        let conn = pool.get().unwrap();
        crate::db::test_migrations::run(&conn).unwrap();
        let now = chrono::NaiveDate::from_ymd_opt(2026, 11, 2)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap();
        assert!(scheduled_job_due_at(&conn, "retry-test", now).unwrap());
        conn.execute("INSERT INTO maintenance_jobs(job_type,status,started_at) VALUES('retry-test','failed','2026-11-02 11:59:00')", []).unwrap();
        assert!(!scheduled_job_due_at(&conn, "retry-test", now).unwrap());
        conn.execute("UPDATE maintenance_jobs SET started_at='2026-11-02 11:00:00' WHERE job_type='retry-test'", []).unwrap();
        assert!(scheduled_job_due_at(&conn, "retry-test", now).unwrap());
        for _ in 0..2 {
            conn.execute("INSERT INTO maintenance_jobs(job_type,status,started_at) VALUES('retry-test','failed','2026-11-02 11:00:00')", []).unwrap();
        }
        assert!(!scheduled_job_due_at(&conn, "retry-test", now).unwrap());
        conn.execute("INSERT INTO maintenance_jobs(job_type,status,started_at) VALUES('running-test','running','2026-11-02 11:00:00')", []).unwrap();
        assert!(scheduled_job_due_at(&conn, "running-test", now).unwrap());
        conn.execute("INSERT INTO maintenance_jobs(job_type,status,started_at) VALUES('completed-test','completed','2026-11-02 11:00:00')", []).unwrap();
        assert!(!scheduled_job_due_at(&conn, "completed-test", now).unwrap());
    }
}
