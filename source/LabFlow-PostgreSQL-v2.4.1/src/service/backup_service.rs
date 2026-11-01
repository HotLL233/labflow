use crate::config::AppConfig;
use chrono::Local;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime};
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};

const PENDING_MARKER: &str = "restore_pending.json";
const PENDING_DB: &str = "restore_pending.dump";
const PENDING_ATTACHMENTS: &str = "restore_pending_attachments";
const PENDING_PREVIEWS: &str = "restore_pending_previews";
const PENDING_HELP_DOCS: &str = "restore_pending_help_docs";
const PENDING_PERSONNEL_CHANGE: &str = "restore_pending_personnel_change_data";
const PENDING_CONFIG: &str = "restore_pending_config.toml";

/// Backup, archive and PostgreSQL maintenance operations must never overlap.
pub static DATA_MAINTENANCE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Debug, Clone, Serialize)]
pub struct BackupResult {
    pub name: String,
    pub size: u64,
    pub sync_warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ManifestFile {
    path: String,
    sha256: String,
    size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BackupManifest {
    format_version: u32,
    app_version: String,
    created_at: String,
    mode: String,
    #[serde(default)]
    included_sources: Vec<String>,
    #[serde(default)]
    file_count: u64,
    #[serde(default)]
    total_bytes: u64,
    files: Vec<ManifestFile>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PendingRestore {
    source_name: String,
    full: bool,
    staged_at: String,
    #[serde(default)]
    attempts: u32,
    #[serde(default)]
    last_error: Option<String>,
}

pub fn ensure_backup_directories(cfg: &AppConfig) -> std::result::Result<(), String> {
    fs::create_dir_all(cfg.backup_dir()).map_err(|e| format!("无法创建备份目录: {e}"))?;
    fs::create_dir_all(cfg.backup_dir().join(".trash"))
        .map_err(|e| format!("无法创建备份回收目录: {e}"))?;
    Ok(())
}

pub fn repair_backup_permissions(cfg: &AppConfig) -> std::result::Result<(), String> {
    ensure_backup_directories(cfg)?;
    #[cfg(windows)]
    {
        for directory in [cfg.backup_dir(), cfg.backup_dir().join(".trash")] {
            let output = Command::new("icacls")
                .arg(&directory)
                .args([
                    "/inheritance:e",
                    "/grant",
                    "*S-1-5-18:(OI)(CI)(F)",
                    "/grant",
                    "*S-1-5-32-544:(OI)(CI)(F)",
                    "/grant",
                    "*S-1-5-32-545:(OI)(CI)(M)",
                ])
                .output()
                .map_err(|e| format!("无法执行备份目录权限修复: {e}"))?;
            if !output.status.success() {
                return Err(format!(
                    "备份目录权限修复失败: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
        }
    }
    Ok(())
}

fn rename_with_retry(source: &Path, destination: &Path) -> std::result::Result<(), String> {
    if let Ok(metadata) = fs::metadata(source) {
        let mut permissions = metadata.permissions();
        if permissions.readonly() {
            permissions.set_readonly(false);
            let _ = fs::set_permissions(source, permissions);
        }
    }
    let mut last_error = None;
    for delay in [0_u64, 200, 500, 1000] {
        if delay > 0 {
            std::thread::sleep(Duration::from_millis(delay));
        }
        match fs::rename(source, destination) {
            Ok(()) => return Ok(()),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error
        .map(|error| error.to_string())
        .unwrap_or_else(|| "文件移动失败".to_string()))
}

pub fn move_path_with_retry(source: &Path, destination: &Path) -> std::result::Result<(), String> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("无法创建目标目录: {e}"))?;
    }
    rename_with_retry(source, destination)
}

pub fn pending_restore_error(cfg: &AppConfig) -> Option<String> {
    let bytes = read_bytes(&cfg.data_dir().join(PENDING_MARKER)).ok()?;
    serde_json::from_slice::<PendingRestore>(&bytes)
        .ok()
        .and_then(|marker| marker.last_error)
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn read_bytes(path: &Path) -> std::result::Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    Ok(bytes)
}

fn write_zip_file(
    zip: &mut ZipWriter<fs::File>,
    zip_path: &str,
    bytes: &[u8],
    options: SimpleFileOptions,
    manifest_files: &mut Vec<ManifestFile>,
) -> std::result::Result<(), String> {
    zip.start_file(zip_path, options)
        .map_err(|e| e.to_string())?;
    zip.write_all(bytes).map_err(|e| e.to_string())?;
    manifest_files.push(ManifestFile {
        path: zip_path.to_string(),
        sha256: sha256(bytes),
        size: bytes.len() as u64,
    });
    Ok(())
}

fn write_zip_stream(
    zip: &mut ZipWriter<fs::File>,
    zip_path: &str,
    source: &Path,
    options: SimpleFileOptions,
    manifest_files: &mut Vec<ManifestFile>,
) -> std::result::Result<(), String> {
    zip.start_file(zip_path, options)
        .map_err(|e| e.to_string())?;
    let mut input = fs::File::open(source).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 1024 * 1024];
    let mut size = 0_u64;
    loop {
        let count = input.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        zip.write_all(&buffer[..count]).map_err(|e| e.to_string())?;
        hasher.update(&buffer[..count]);
        size += count as u64;
    }
    manifest_files.push(ManifestFile {
        path: zip_path.to_string(),
        sha256: format!("{:x}", hasher.finalize()),
        size,
    });
    Ok(())
}

fn add_directory(
    zip: &mut ZipWriter<fs::File>,
    base: &Path,
    current: &Path,
    zip_root: &str,
    options: SimpleFileOptions,
    manifest_files: &mut Vec<ManifestFile>,
) -> std::result::Result<(), String> {
    if !current.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(current).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            add_directory(zip, base, &path, zip_root, options, manifest_files)?;
        } else {
            let rel = path
                .strip_prefix(base)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            let zip_path = format!("{zip_root}/{rel}");
            write_zip_stream(zip, &zip_path, &path, options, manifest_files)?;
        }
    }
    Ok(())
}

fn personnel_change_data_dir(cfg: &AppConfig) -> PathBuf {
    std::env::var("PERSONNEL_CHANGE_DATA_DIR")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| cfg.data_dir().join("personnel-change-data"))
}

fn backup_sources(cfg: &AppConfig) -> Vec<(&'static str, PathBuf)> {
    vec![
        ("attachments", cfg.attachments_dir()),
        ("attachment_previews", cfg.attachment_preview_dir()),
        ("help_docs", cfg.help_docs_dir()),
        ("personnel-change-data", personnel_change_data_dir(cfg)),
    ]
}

fn postgres_tool(name: &str) -> std::result::Result<PathBuf, String> {
    let file_name = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };

    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            let candidate = directory.join("postgresql").join("bin").join(&file_name);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }
    if let Some(program_data) = std::env::var_os("PROGRAMDATA") {
        for application in ["labflow", "WorkloadTool"] {
            let postgres_root = PathBuf::from(&program_data)
                .join(application)
                .join("PostgreSQL18");
            let package_version = env!("CARGO_PKG_VERSION").trim_end_matches("-server");
            let candidate = postgres_root
                .join(format!("runtime-v{package_version}"))
                .join("bin")
                .join(&file_name);
            if candidate.exists() {
                return Ok(candidate);
            }

            let mut runtimes = fs::read_dir(&postgres_root)
                .into_iter()
                .flatten()
                .flatten()
                .filter(|entry| {
                    entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false)
                        && entry.file_name().to_string_lossy().starts_with("runtime-v")
                })
                .collect::<Vec<_>>();
            runtimes.sort_by_key(|entry| {
                entry
                    .metadata()
                    .and_then(|metadata| metadata.modified())
                    .ok()
            });
            for runtime in runtimes.into_iter().rev() {
                let candidate = runtime.path().join("bin").join(&file_name);
                if candidate.exists() {
                    return Ok(candidate);
                }
            }
        }
    }
    if let Ok(bin) = std::env::var("POSTGRES_BIN") {
        let candidate = PathBuf::from(bin).join(&file_name);
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    for version in ["18", "17", "16", "15", "14"] {
        let candidate = PathBuf::from(format!(
            r"C:\Program Files\PostgreSQL\{version}\bin\{name}.exe"
        ));
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    if !cfg!(windows) {
        return Ok(PathBuf::from(name));
    }
    Err(format!(
        "未找到 PostgreSQL 工具 {file_name}，请检查私有 PostgreSQL 运行库是否完整"
    ))
}

#[cfg(test)]
mod postgres_tool_tests {
    use super::postgres_tool;

    #[test]
    #[cfg(windows)]
    fn locates_private_postgres_backup_tools() {
        for tool in ["pg_dump", "pg_restore", "psql"] {
            let path = postgres_tool(tool).expect("private PostgreSQL tool should be found");
            assert!(path.exists(), "{} does not exist", path.display());
            assert_eq!(
                path.file_stem().and_then(|value| value.to_str()),
                Some(tool)
            );
        }
    }
}

fn command_error(name: &str, output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        format!("{name} 执行失败，退出码: {}", output.status)
    } else {
        format!("{name} 执行失败: {stderr}")
    }
}

pub fn verify_database(path: &Path) -> std::result::Result<String, String> {
    if !path.exists() || fs::metadata(path).map(|item| item.len()).unwrap_or(0) == 0 {
        return Err("PostgreSQL 备份文件为空或不存在".into());
    }
    let output = Command::new(postgres_tool("pg_restore")?)
        .arg("--list")
        .arg(path)
        .output()
        .map_err(|error| format!("无法启动 pg_restore: {error}"))?;
    if !output.status.success() {
        return Err(command_error("pg_restore --list", &output));
    }
    Ok("ok".into())
}

fn database_snapshot(cfg: &AppConfig, destination: &Path) -> std::result::Result<(), String> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if destination.exists() {
        fs::remove_file(destination).map_err(|e| e.to_string())?;
    }
    let database_url = cfg.postgres_url();
    if database_url.trim().is_empty() {
        return Err("未配置 PostgreSQL DATABASE_URL".into());
    }
    let output = Command::new(postgres_tool("pg_dump")?)
        .args(["--format=custom", "--no-owner", "--no-privileges"])
        .arg("--dbname")
        .arg(database_url)
        .arg("--file")
        .arg(destination)
        .output()
        .map_err(|error| format!("无法启动 pg_dump: {error}"))?;
    if !output.status.success() {
        let _ = fs::remove_file(destination);
        return Err(command_error("pg_dump", &output));
    }
    verify_database(destination)?;
    Ok(())
}

fn available_backup_name(dir: &Path, stem: &str, extension: &str) -> String {
    let mut name = format!("{stem}.{extension}");
    let mut suffix = 2;
    while dir.join(&name).exists() || dir.join(format!(".{name}.part")).exists() {
        name = format!("{stem}_{suffix}.{extension}");
        suffix += 1;
    }
    name
}

fn create_database_backup_file(
    cfg: &AppConfig,
    automatic: bool,
) -> std::result::Result<(String, u64), String> {
    let dir = cfg.backup_dir();
    ensure_backup_directories(cfg)?;
    let prefix = if automatic { "labflow_auto" } else { "labflow" };
    let name = available_backup_name(
        &dir,
        &format!("{prefix}_{}", Local::now().format("%Y%m%d_%H%M%S")),
        "dump",
    );
    let destination = dir.join(&name);
    let temporary = dir.join(format!(".{name}.part"));
    database_snapshot(cfg, &temporary)?;
    fs::rename(&temporary, &destination).map_err(|e| e.to_string())?;
    let size = fs::metadata(destination).map(|m| m.len()).unwrap_or(0);
    Ok((name, size))
}

/// Maintenance always uses a database-only snapshot, regardless of the user's
/// normal backup mode, so a scheduled VACUUM cannot unexpectedly package files.
pub fn create_database_backup(
    cfg: &AppConfig,
    automatic: bool,
) -> std::result::Result<BackupResult, String> {
    let (name, size) = create_database_backup_file(cfg, automatic)?;
    cleanup_old_backups(&cfg.backup_dir(), cfg.max_backup_count);
    Ok(BackupResult {
        name,
        size,
        sync_warning: None,
    })
}

fn create_full_backup(
    cfg: &AppConfig,
    automatic: bool,
) -> std::result::Result<(String, u64), String> {
    let dir = cfg.backup_dir();
    ensure_backup_directories(cfg)?;
    let prefix = if automatic {
        "labflow_auto_full"
    } else {
        "labflow_full"
    };
    let name = available_backup_name(
        &dir,
        &format!("{prefix}_{}", Local::now().format("%Y%m%d_%H%M%S")),
        "zip",
    );
    let destination = dir.join(&name);
    let temporary = dir.join(format!(".{name}.part"));
    let snapshot = dir.join(format!("snapshot_{}.dump", uuid::Uuid::new_v4()));
    database_snapshot(cfg, &snapshot)?;

    let result = (|| {
        let file = fs::File::create(&temporary).map_err(|e| e.to_string())?;
        let mut zip = ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let mut files = Vec::new();
        // Preserve the archive entry name for existing backup readers.
        write_zip_stream(&mut zip, "workload.dump", &snapshot, options, &mut files)?;
        let mut included_sources = vec!["database".to_string()];
        for (name, source) in backup_sources(cfg) {
            add_directory(&mut zip, &source, &source, name, options, &mut files)?;
            if source.exists() {
                included_sources.push(name.to_string());
            }
        }
        let config_path = AppConfig::config_path();
        if config_path.exists() {
            let config_bytes = read_bytes(&config_path)?;
            write_zip_file(
                &mut zip,
                "config.snapshot.toml",
                &config_bytes,
                options,
                &mut files,
            )?;
            included_sources.push("config".to_string());
        }
        let manifest = BackupManifest {
            format_version: 3,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            created_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            mode: "full".to_string(),
            included_sources,
            file_count: files.len() as u64,
            total_bytes: files.iter().map(|file| file.size).sum(),
            files,
        };
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
        zip.start_file("backup_manifest.json", options)
            .map_err(|e| e.to_string())?;
        zip.write_all(&manifest_bytes).map_err(|e| e.to_string())?;
        zip.finish().map_err(|e| e.to_string())?;
        validate_full_archive(&temporary)?;
        fs::rename(&temporary, &destination).map_err(|e| e.to_string())?;
        Ok::<(), String>(())
    })();
    let _ = fs::remove_file(snapshot);
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result?;
    let size = fs::metadata(destination).map(|m| m.len()).unwrap_or(0);
    Ok((name, size))
}

fn copy_to_sync_dir(cfg: &AppConfig, source: &Path) -> std::result::Result<(), String> {
    let Some(sync_dir) = cfg
        .backup_sync_dir
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    else {
        return Ok(());
    };
    let destination_dir = PathBuf::from(sync_dir);
    fs::create_dir_all(&destination_dir).map_err(|e| format!("无法创建同步目录: {e}"))?;
    let file_name = source
        .file_name()
        .ok_or_else(|| "无效备份文件名".to_string())?;
    fs::copy(source, destination_dir.join(file_name))
        .map_err(|e| format!("复制到同步目录失败: {e}"))?;
    cleanup_old_backups(&destination_dir, cfg.max_backup_count);
    Ok(())
}

pub fn create_backup(
    cfg: &AppConfig,
    automatic: bool,
) -> std::result::Result<BackupResult, String> {
    let _guard = DATA_MAINTENANCE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| "backup and maintenance lock poisoned".to_string())?;
    ensure_backup_directories(cfg)?;
    let (name, size) = if cfg.backup_mode.eq_ignore_ascii_case("full") {
        create_full_backup(cfg, automatic)?
    } else {
        create_database_backup_file(cfg, automatic)?
    };
    // Queue before retention: a cloud upload must pin its immutable local source.
    let cloud_warning = super::cloud_backup_service::enqueue_automatic(cfg, &name).err();
    cleanup_old_backups(&cfg.backup_dir(), cfg.max_backup_count);
    let sync_error = copy_to_sync_dir(cfg, &cfg.backup_dir().join(&name)).err();
    let warnings: Vec<_> = [
        sync_error.map(|value| format!("同步目录：{value}")),
        cloud_warning.map(|value| format!("云端任务：{value}")),
    ]
    .into_iter()
    .flatten()
    .collect();
    let sync_warning = if warnings.is_empty() {
        None
    } else {
        Some(warnings.join("；"))
    };
    Ok(BackupResult {
        name,
        size,
        sync_warning,
    })
}

pub fn cleanup_old_backups(dir: &Path, max_count: u64) {
    if max_count == 0 {
        return;
    }
    let protected = match super::cloud_backup_service::protected_names_for_directory(dir) {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!("备份保留状态不可读，跳过自动清理: {error}");
            return;
        }
    };
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<_> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            if !name.ends_with(".dump") && !name.ends_with(".zip") {
                return None;
            }
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((entry.path(), modified))
        })
        .collect();
    files.sort_by(|a, b| b.1.cmp(&a.1));
    for (path, _) in files.into_iter().skip(max_count as usize) {
        if !path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|name| protected.contains(name))
        {
            let _ = fs::remove_file(path);
        }
    }
}

fn latest_automatic_backup(dir: &Path) -> Option<SystemTime> {
    fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            if !name.starts_with("labflow_auto_") && !name.starts_with("workload_auto_") {
                return None;
            }
            entry.metadata().ok()?.modified().ok()
        })
        .max()
}

pub fn automatic_backup_due(cfg: &AppConfig) -> bool {
    if !cfg.backup_enabled {
        return false;
    }
    let interval = Duration::from_secs(cfg.backup_interval_hours.max(1) * 3600);
    latest_automatic_backup(&cfg.backup_dir())
        .and_then(|time| time.elapsed().ok())
        .map(|elapsed| elapsed >= interval)
        .unwrap_or(true)
}

fn safe_relative_path(name: &str, prefix: &str) -> Option<PathBuf> {
    let normalized = name.replace('\\', "/");
    if !normalized.starts_with(prefix) {
        return None;
    }
    let relative = normalized.trim_start_matches(prefix);
    if relative.is_empty()
        || relative.split('/').any(|part| {
            part == ".."
                || part == "."
                || part.is_empty()
                || part.contains(':')
                || part.contains('\0')
        })
    {
        return None;
    }
    let path = PathBuf::from(relative);
    if path.is_absolute() {
        return None;
    }
    Some(path)
}

fn validate_full_archive(path: &Path) -> std::result::Result<BackupManifest, String> {
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("无法打开全量备份: {e}"))?;
    let manifest: BackupManifest = {
        let entry = archive
            .by_name("backup_manifest.json")
            .map_err(|_| "全量备份缺少校验清单".to_string())?;
        let mut bytes = Vec::new();
        entry
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 16 * 1024 * 1024 {
            return Err("备份清单过大".into());
        }
        serde_json::from_slice(&bytes).map_err(|e| format!("备份清单无效: {e}"))?
    };
    let mut declared = HashSet::new();
    let mut normalized = HashSet::new();
    for entry in &manifest.files {
        let path = entry.path.replace('\\', "/");
        if !normalized.insert(path.clone()) {
            return Err("备份清单包含重复的目标路径".into());
        }
        if !matches!(
            path.as_str(),
            "workload.dump" | "config.toml" | "config.snapshot.toml"
        ) && ![
            "attachments/",
            "attachment_previews/",
            "help_docs/",
            "personnel-change-data/",
        ]
        .iter()
        .any(|prefix| safe_relative_path(&path, prefix).is_some())
        {
            return Err("备份清单包含越界或不支持的文件路径".into());
        }
        if !declared.insert(entry.path.clone()) {
            return Err("备份清单包含重复文件".into());
        }
    }
    if !declared.contains("workload.dump") {
        return Err("备份清单缺少数据库".into());
    }
    let mut names = HashSet::new();
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|e| e.to_string())?;
        if !names.insert(entry.name().to_string()) {
            return Err("备份包含重复文件".into());
        }
        if !entry.is_dir()
            && entry.name() != "backup_manifest.json"
            && !declared.contains(entry.name())
        {
            return Err("备份包含清单未声明的文件".into());
        }
    }
    for expected in &manifest.files {
        let mut entry = archive
            .by_name(&expected.path)
            .map_err(|_| format!("备份缺少文件: {}", expected.path))?;
        let mut hasher = Sha256::new();
        let mut size = 0_u64;
        let mut buffer = [0_u8; 1024 * 1024];
        loop {
            let count = entry.read(&mut buffer).map_err(|e| e.to_string())?;
            if count == 0 {
                break;
            }
            size = size.saturating_add(count as u64);
            if size > expected.size {
                return Err(format!("备份文件大小异常: {}", expected.path));
            }
            hasher.update(&buffer[..count]);
        }
        if size != expected.size || format!("{:x}", hasher.finalize()) != expected.sha256 {
            return Err(format!("备份文件校验失败: {}", expected.path));
        }
    }
    Ok(manifest)
}

pub fn validate_backup_file(path: &Path) -> std::result::Result<(), String> {
    if path.extension().is_some_and(|value| value == "zip") {
        validate_full_archive(path).map(|_| ())
    } else {
        verify_database(path).map(|_| ())
    }
}

pub fn backup_version(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    let mut archive = ZipArchive::new(file).ok()?;
    let entry = archive.by_name("backup_manifest.json").ok()?;
    serde_json::from_reader::<_, BackupManifest>(entry.take(16 * 1024 * 1024))
        .ok()
        .map(|value| value.app_version)
}

pub fn stage_restore(cfg: &AppConfig, source: &Path) -> std::result::Result<String, String> {
    let _guard = DATA_MAINTENANCE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| "backup lock poisoned".to_string())?;
    if cfg.data_dir().join(PENDING_MARKER).exists() {
        return Err("已有待恢复任务，请先完成恢复后再上传备份".into());
    }
    if !source.exists() {
        return Err("备份文件不存在".into());
    }
    fs::create_dir_all(cfg.data_dir()).map_err(|e| e.to_string())?;
    ensure_backup_directories(cfg)?;
    let full = source
        .extension()
        .and_then(|x| x.to_str())
        .map(|x| x.eq_ignore_ascii_case("zip"))
        .unwrap_or(false);
    let pending_db = cfg.data_dir().join(PENDING_DB);
    let pending_attachments = cfg.data_dir().join(PENDING_ATTACHMENTS);
    let pending_previews = cfg.data_dir().join(PENDING_PREVIEWS);
    let pending_help_docs = cfg.data_dir().join(PENDING_HELP_DOCS);
    let pending_personnel_change = cfg.data_dir().join(PENDING_PERSONNEL_CHANGE);
    let pending_config = cfg.data_dir().join(PENDING_CONFIG);
    let _ = fs::remove_file(&pending_db);
    let _ = fs::remove_file(&pending_config);
    let _ = fs::remove_dir_all(&pending_attachments);
    let _ = fs::remove_dir_all(&pending_previews);
    let _ = fs::remove_dir_all(&pending_help_docs);
    let _ = fs::remove_dir_all(&pending_personnel_change);

    if full {
        validate_full_archive(source)?;
        let file = fs::File::open(source).map_err(|e| e.to_string())?;
        let mut archive = ZipArchive::new(file).map_err(|e| e.to_string())?;
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).map_err(|e| e.to_string())?;
            let name = entry.name().replace('\\', "/");
            if name == "workload.dump" {
                let mut out = fs::File::create(&pending_db).map_err(|e| e.to_string())?;
                std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
            } else if name == "config.toml" || name == "config.snapshot.toml" {
                let mut out = fs::File::create(&pending_config).map_err(|e| e.to_string())?;
                std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
            } else if let Some(relative) = safe_relative_path(&name, "attachments/") {
                let destination = pending_attachments.join(relative);
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                let mut out = fs::File::create(destination).map_err(|e| e.to_string())?;
                std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
            } else if let Some(relative) = safe_relative_path(&name, "attachment_previews/") {
                let destination = pending_previews.join(relative);
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                let mut out = fs::File::create(destination).map_err(|e| e.to_string())?;
                std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
            } else if let Some(relative) = safe_relative_path(&name, "help_docs/") {
                let destination = pending_help_docs.join(relative);
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                let mut out = fs::File::create(destination).map_err(|e| e.to_string())?;
                std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
            } else if let Some(relative) = safe_relative_path(&name, "personnel-change-data/") {
                let destination = pending_personnel_change.join(relative);
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                let mut out = fs::File::create(destination).map_err(|e| e.to_string())?;
                std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
            }
        }
    } else {
        verify_database(source)?;
        fs::copy(source, &pending_db).map_err(|e| e.to_string())?;
    }
    verify_database(&pending_db)?;
    let safety = create_database_backup(cfg, false)?.name;
    let marker = PendingRestore {
        source_name: source
            .file_name()
            .and_then(|x| x.to_str())
            .unwrap_or("backup")
            .to_string(),
        full,
        staged_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        attempts: 0,
        last_error: None,
    };
    fs::write(
        cfg.data_dir().join(PENDING_MARKER),
        serde_json::to_vec_pretty(&marker).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(safety)
}

fn copy_restore_directory(source: &Path, destination: &Path) -> std::result::Result<(), String> {
    fs::create_dir_all(destination).map_err(|e| e.to_string())?;
    if source.exists() {
        for entry in fs::read_dir(source).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let target = destination.join(entry.file_name());
            let kind = entry.file_type().map_err(|e| e.to_string())?;
            if kind.is_symlink() {
                return Err("恢复目录中不能包含符号链接".into());
            }
            if kind.is_dir() {
                copy_restore_directory(&entry.path(), &target)?;
            } else {
                fs::copy(entry.path(), target).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

fn replace_directory(source: &Path, destination: &Path) -> std::result::Result<PathBuf, String> {
    let token = uuid::Uuid::new_v4();
    let previous = destination.with_extension(format!("restore_previous_{token}"));
    let next = destination.with_extension(format!("restore_next_{token}"));
    // Keep staging intact: retry must never replace restored files with an empty directory.
    if let Err(error) = copy_restore_directory(source, &next) {
        let _ = fs::remove_dir_all(&next);
        return Err(error);
    }
    if destination.exists() {
        move_path_with_retry(destination, &previous)
            .map_err(|e| format!("无法暂存现有附件: {e}"))?;
    }
    if let Err(error) = move_path_with_retry(&next, destination) {
        if previous.exists() {
            let _ = move_path_with_retry(&previous, destination);
        }
        return Err(format!("无法恢复附件: {error}"));
    }
    Ok(previous)
}

fn restore_database_atomically(cfg: &AppConfig, dump: &Path) -> std::result::Result<(), String> {
    let sql = cfg.data_dir().join("restore_pending.sql");
    let output = Command::new(postgres_tool("pg_restore")?)
        .args(["--no-owner", "--no-privileges", "--file"])
        .arg(&sql)
        .arg(dump)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(command_error("展开 PostgreSQL 备份", &output));
    }
    let result = apply_restore_sql(cfg, &sql);
    let _ = fs::remove_file(sql);
    result
}

fn apply_restore_sql(cfg: &AppConfig, sql: &Path) -> std::result::Result<(), String> {
    let output = Command::new(postgres_tool("psql")?)
        .args(["--no-psqlrc", "--dbname"]).arg(cfg.postgres_url())
        .args(["--set=ON_ERROR_STOP=1", "--single-transaction", "--command",
            "DROP SCHEMA IF EXISTS public CASCADE; CREATE SCHEMA public;", "--file"])
        .arg(sql)
        .args(["--command", "SELECT COUNT(*) FROM public.users; SELECT COUNT(*) FROM public.work_records; SELECT COUNT(*) FROM public.rd_work_records; SELECT COUNT(*) FROM public.sample_info_records; SELECT COUNT(*) FROM public.audit_log;"])
        .output()
        .map_err(|error| format!("无法启动 psql: {error}"))?;
    if !output.status.success() {
        return Err(command_error("事务恢复 PostgreSQL", &output));
    }
    Ok(())
}

pub fn apply_pending_restore(cfg: &AppConfig) -> std::result::Result<Option<String>, String> {
    let marker_path = cfg.data_dir().join(PENDING_MARKER);
    if !marker_path.exists() {
        return Ok(None);
    }
    let mut marker: PendingRestore =
        serde_json::from_slice(&read_bytes(&marker_path)?).map_err(|e| e.to_string())?;
    marker.attempts = marker.attempts.saturating_add(1);
    let mut replaced: Vec<(PathBuf, PathBuf)> = Vec::new();
    let result: std::result::Result<String, String> = (|| {
        let pending_db = cfg.data_dir().join(PENDING_DB);
        verify_database(&pending_db)?;
        restore_database_atomically(cfg, &pending_db)?;
        if marker.full {
            for (staged, destination, optional) in [
                (PENDING_ATTACHMENTS, cfg.attachments_dir(), false),
                (PENDING_HELP_DOCS, cfg.help_docs_dir(), false),
                (PENDING_PREVIEWS, cfg.attachment_preview_dir(), true),
                (
                    PENDING_PERSONNEL_CHANGE,
                    personnel_change_data_dir(cfg),
                    true,
                ),
            ] {
                let source = cfg.data_dir().join(staged);
                if optional && !source.exists() {
                    continue;
                }
                let previous = replace_directory(&source, &destination)?;
                replaced.push((destination, previous));
            }
            // config.toml belongs to the current installation. The staged copy is
            // intentionally not applied so a restore cannot break the local runtime.
        }
        verify_restored_database(cfg)?;
        fs::remove_file(&marker_path).map_err(|e| e.to_string())?;
        let _ = fs::remove_file(&pending_db);
        let _ = fs::remove_file(cfg.data_dir().join(PENDING_CONFIG));
        for (_, previous) in &replaced {
            let _ = fs::remove_dir_all(previous);
        }
        for staged in [
            PENDING_ATTACHMENTS,
            PENDING_HELP_DOCS,
            PENDING_PREVIEWS,
            PENDING_PERSONNEL_CHANGE,
        ] {
            let _ = fs::remove_dir_all(cfg.data_dir().join(staged));
        }
        Ok(marker.source_name.clone())
    })();
    if let Err(error) = &result {
        for (destination, previous) in replaced.iter().rev() {
            if fs::remove_dir_all(destination).is_ok() && previous.exists() {
                let _ = move_path_with_retry(previous, destination);
            }
        }
        marker.last_error = Some(error.clone());
        let _ = fs::write(
            &marker_path,
            serde_json::to_vec_pretty(&marker).unwrap_or_default(),
        );
    }
    result.map(Some)
}

fn verify_restored_database(cfg: &AppConfig) -> std::result::Result<(), String> {
    let connection = postgres_compat::Connection::connect(&cfg.postgres_url())
        .map_err(|error| format!("恢复后数据库连接失败: {error}"))?;
    for table in [
        "users",
        "work_records",
        "rd_work_records",
        "sample_info_records",
        "audit_log",
    ] {
        connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(|error| format!("恢复后校验表 {table} 失败: {error}"))?;
    }
    Ok(())
}

pub fn test_sync_directory(path: &str) -> std::result::Result<(), String> {
    let directory = PathBuf::from(path.trim());
    if path.trim().is_empty() {
        return Err("同步目录不能为空".into());
    }
    fs::create_dir_all(&directory).map_err(|e| format!("无法创建同步目录: {e}"))?;
    let probe = directory.join(format!(".workload_backup_test_{}", uuid::Uuid::new_v4()));
    fs::write(&probe, b"ok").map_err(|e| format!("同步目录不可写: {e}"))?;
    fs::remove_file(probe).map_err(|e| format!("同步目录清理失败: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_backup_interval_recognizes_new_and_legacy_files() {
        let root = std::env::temp_dir().join(format!("labflow_auto_{}", uuid::Uuid::new_v4()));
        let cfg = AppConfig {
            db_dir: root.to_string_lossy().into(),
            backup_enabled: true,
            ..AppConfig::default()
        };
        fs::create_dir_all(cfg.backup_dir()).unwrap();
        assert!(automatic_backup_due(&cfg));
        for name in [
            "labflow_auto_full_20261101_103637.zip",
            "workload_auto_20261101_103637.dump",
        ] {
            let path = cfg.backup_dir().join(name);
            fs::write(&path, b"fixture").unwrap();
            assert!(!automatic_backup_due(&cfg), "{name}");
            fs::remove_file(path).unwrap();
        }
        fs::remove_dir(cfg.backup_dir()).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn backup_names_are_short_and_skip_existing_files_and_partial_writes() {
        let root = std::env::temp_dir().join(format!("backup_names_{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let stem = "workload_full_20261101_103637";
        let legacy = root.join(format!("{stem}_2a55814b0e9c4a6fa4021aed962b1c23.zip"));
        fs::write(&legacy, b"legacy").unwrap();
        assert_eq!(
            available_backup_name(&root, stem, "zip"),
            "workload_full_20261101_103637.zip"
        );
        let existing = root.join(format!("{stem}.zip"));
        let partial = root.join(format!(".{stem}_2.zip.part"));
        fs::write(&existing, b"existing").unwrap();
        fs::write(&partial, b"in progress").unwrap();
        assert_eq!(
            available_backup_name(&root, stem, "zip"),
            "workload_full_20261101_103637_3.zip"
        );
        for (prefix, extension) in [
            ("workload", "dump"),
            ("workload_auto", "dump"),
            ("workload_auto_full", "zip"),
        ] {
            let stem = format!("{prefix}_20261101_103637");
            assert_eq!(
                available_backup_name(&root, &stem, extension),
                format!("{stem}.{extension}")
            );
        }
        assert_eq!(fs::read(&legacy).unwrap(), b"legacy");
        assert_eq!(fs::read(&existing).unwrap(), b"existing");
        assert_eq!(fs::read(&partial).unwrap(), b"in progress");
        for path in [legacy, existing, partial] {
            fs::remove_file(path).unwrap();
        }
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn full_backup_checksum_rejects_tampering_and_unlisted_entries() {
        let root = std::env::temp_dir().join(format!("backup_manifest_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("workload_fixture.zip");
        let options = SimpleFileOptions::default();
        for (declared_hash, extra, valid) in [
            (sha256(b"dump"), false, true),
            ("0".repeat(64), false, false),
            (sha256(b"dump"), true, false),
        ] {
            let file = fs::File::create(&path).unwrap();
            let mut zip = ZipWriter::new(file);
            zip.start_file("workload.dump", options).unwrap();
            zip.write_all(b"dump").unwrap();
            zip.start_file("backup_manifest.json", options).unwrap();
            zip.write_all(
                serde_json::to_string(&BackupManifest {
                    format_version: 3,
                    app_version: "2.3.79".into(),
                    created_at: "".into(),
                    mode: "full".into(),
                    included_sources: vec!["database".into()],
                    file_count: 1,
                    total_bytes: 4,
                    files: vec![ManifestFile {
                        path: "workload.dump".into(),
                        sha256: declared_hash,
                        size: 4,
                    }],
                })
                .unwrap()
                .as_bytes(),
            )
            .unwrap();
            if extra {
                zip.start_file("attachments/unlisted.txt", options).unwrap();
                zip.write_all(b"extra").unwrap();
            }
            zip.finish().unwrap();
            assert_eq!(validate_full_archive(&path).is_ok(), valid);
            assert_eq!(backup_version(&path).as_deref(), Some("2.3.79"));
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn restore_relative_paths_reject_windows_drives_and_parent_traversal() {
        for name in [
            "attachments/../outside",
            "attachments/C:/outside",
            "attachments/./file",
            "attachments//file",
            "attachments/\\file",
        ] {
            assert!(safe_relative_path(name, "attachments/").is_none(), "{name}");
        }
        assert_eq!(
            safe_relative_path("attachments/2026/报告.pdf", "attachments/"),
            Some(PathBuf::from("2026/报告.pdf"))
        );
    }

    #[test]
    fn directory_restore_preserves_staging_for_retries() {
        let root = std::env::temp_dir().join(format!("restore_retry_{}", uuid::Uuid::new_v4()));
        let staged = root.join("staged");
        let live = root.join("live");
        fs::create_dir_all(&staged).unwrap();
        fs::create_dir_all(&live).unwrap();
        fs::write(staged.join("sample.txt"), b"restored").unwrap();
        fs::write(live.join("original.txt"), b"original").unwrap();
        let original = replace_directory(&staged, &live).unwrap();
        assert_eq!(
            fs::read(original.join("original.txt")).unwrap(),
            b"original"
        );
        replace_directory(&staged, &live).unwrap();
        assert_eq!(fs::read(live.join("sample.txt")).unwrap(), b"restored");
        assert_eq!(fs::read(staged.join("sample.txt")).unwrap(), b"restored");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "requires dedicated labflow_restore_* database and PostgreSQL tools"]
    fn atomic_database_restore_rolls_back_failure_and_restores_dump() {
        assert!(
            std::env::var("DATABASE_URL").unwrap_or_default().is_empty(),
            "unset DATABASE_URL so it cannot override the isolated test database"
        );
        let database_url = std::env::var("WORKLOAD_RESTORE_TEST_DATABASE_URL")
            .expect("dedicated restore database URL");
        let name = database_url.rsplit('/').next().unwrap();
        assert!(
            name.starts_with("labflow_restore_")
                && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        );
        let root = std::env::temp_dir().join(format!("restore_atomic_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let cfg = AppConfig {
            database_url,
            db_dir: root.to_string_lossy().into_owned(),
            ..AppConfig::default()
        };
        let conn = postgres_compat::Connection::connect(&cfg.postgres_url()).unwrap();
        conn.execute_batch(
            "DROP SCHEMA public CASCADE; CREATE SCHEMA public;
            CREATE TABLE users(id INTEGER); INSERT INTO users VALUES(42);
            CREATE TABLE work_records(id INTEGER); CREATE TABLE rd_work_records(id INTEGER);
            CREATE TABLE sample_info_records(id INTEGER); CREATE TABLE audit_log(id INTEGER);",
        )
        .unwrap();
        let invalid = root.join("invalid.sql");
        fs::write(&invalid, b"CREATE TABLE public.users(id INTEGER); INSERT INTO public.users VALUES(99); SELECT * FROM public.nonexistent_restore_test_table;").unwrap();
        assert!(apply_restore_sql(&cfg, &invalid).is_err());
        assert_eq!(
            conn.query_row("SELECT id FROM users", [], |row| row.get::<_, i32>(0))
                .unwrap(),
            42
        );
        let backup = create_database_backup(&cfg, false).unwrap();
        conn.execute("UPDATE users SET id=99", []).unwrap();
        restore_database_atomically(&cfg, &cfg.backup_dir().join(backup.name)).unwrap();
        assert_eq!(
            conn.query_row("SELECT id FROM users", [], |row| row.get::<_, i32>(0))
                .unwrap(),
            42
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn backup_file_move_round_trip_works_with_retry_helper() {
        let root = std::env::temp_dir().join(format!("backup_move_{}", uuid::Uuid::new_v4()));
        let cfg = AppConfig {
            db_dir: root.to_string_lossy().to_string(),
            ..AppConfig::default()
        };
        ensure_backup_directories(&cfg).unwrap();
        let source = cfg.backup_dir().join("test.dump");
        let trash = cfg.backup_dir().join(".trash").join("stored_test.dump");
        fs::write(&source, b"backup-test").unwrap();
        move_path_with_retry(&source, &trash).unwrap();
        assert!(!source.exists());
        assert_eq!(fs::read(&trash).unwrap(), b"backup-test");
        move_path_with_retry(&trash, &source).unwrap();
        assert_eq!(fs::read(&source).unwrap(), b"backup-test");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn backup_directories_are_created_before_use() {
        let root = std::env::temp_dir().join(format!("backup_dirs_{}", uuid::Uuid::new_v4()));
        let cfg = AppConfig {
            db_dir: root.to_string_lossy().to_string(),
            ..AppConfig::default()
        };
        ensure_backup_directories(&cfg).unwrap();
        assert!(cfg.backup_dir().is_dir());
        assert!(cfg.backup_dir().join(".trash").is_dir());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[ignore = "requires an isolated PostgreSQL DATABASE_URL and PostgreSQL client tools"]
    fn stage_restore_creates_pending_state_without_overwriting_runtime_config() {
        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
        let root = std::env::temp_dir().join(format!("backup_stage_{}", uuid::Uuid::new_v4()));
        let runtime_config = AppConfig::config_path();
        let runtime_config_before = fs::read(&runtime_config).ok();
        let cfg = AppConfig {
            db_dir: root.to_string_lossy().to_string(),
            database_url,
            ..AppConfig::default()
        };
        let backup = create_database_backup(&cfg, false).unwrap();
        let source = cfg.backup_dir().join(backup.name);
        stage_restore(&cfg, &source).unwrap();
        assert!(cfg.data_dir().join(PENDING_MARKER).exists());
        assert!(cfg.data_dir().join(PENDING_DB).exists());
        assert!(!cfg.data_dir().join(PENDING_CONFIG).exists());
        if let Some(before) = runtime_config_before {
            assert_eq!(fs::read(runtime_config).unwrap(), before);
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[ignore = "requires an isolated PostgreSQL DATABASE_URL and PostgreSQL client tools"]
    fn database_and_full_backup_are_valid() {
        let root = std::env::temp_dir().join(format!("backup_service_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(root.join("attachments")).unwrap();
        fs::write(root.join("attachments/test.txt"), b"attachment").unwrap();
        let cfg = AppConfig {
            db_dir: root.to_string_lossy().to_string(),
            backup_mode: "database".into(),
            ..AppConfig::default()
        };
        let db_result = create_backup(&cfg, false).unwrap();
        assert!(db_result.name.starts_with("labflow_"));
        assert!(db_result.name.ends_with(".dump"));
        verify_database(&cfg.backup_dir().join(db_result.name)).unwrap();
        let full_cfg = AppConfig {
            backup_mode: "full".into(),
            ..cfg.clone()
        };
        let full_result = create_backup(&full_cfg, false).unwrap();
        assert!(full_result.name.starts_with("labflow_full_"));
        assert!(full_result.name.ends_with(".zip"));
        validate_full_archive(&full_cfg.backup_dir().join(full_result.name)).unwrap();
        let _ = fs::remove_dir_all(root);
    }
}
