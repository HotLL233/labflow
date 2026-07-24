use crate::config::AppConfig;
use chrono::Local;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
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
const PENDING_HELP_DOCS: &str = "restore_pending_help_docs";
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
    files: Vec<ManifestFile>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PendingRestore {
    source_name: String,
    full: bool,
    staged_at: String,
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
            let bytes = read_bytes(&path)?;
            write_zip_file(zip, &zip_path, &bytes, options, manifest_files)?;
        }
    }
    Ok(())
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
        let postgres_root = PathBuf::from(program_data)
            .join("WorkloadTool")
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

fn create_database_backup_file(
    cfg: &AppConfig,
    automatic: bool,
) -> std::result::Result<(String, u64), String> {
    let dir = cfg.backup_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let prefix = if automatic {
        "workload_auto"
    } else {
        "workload"
    };
    let name = format!("{prefix}_{}.dump", Local::now().format("%Y%m%d_%H%M%S"));
    let destination = dir.join(&name);
    database_snapshot(cfg, &destination)?;
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
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let prefix = if automatic {
        "workload_auto_full"
    } else {
        "workload_full"
    };
    let name = format!("{prefix}_{}.zip", Local::now().format("%Y%m%d_%H%M%S"));
    let destination = dir.join(&name);
    let snapshot = dir.join(format!("snapshot_{}.dump", uuid::Uuid::new_v4()));
    database_snapshot(cfg, &snapshot)?;

    let result = (|| {
        let file = fs::File::create(&destination).map_err(|e| e.to_string())?;
        let mut zip = ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let mut files = Vec::new();
        let db_bytes = read_bytes(&snapshot)?;
        write_zip_file(&mut zip, "workload.dump", &db_bytes, options, &mut files)?;
        add_directory(
            &mut zip,
            &cfg.attachments_dir(),
            &cfg.attachments_dir(),
            "attachments",
            options,
            &mut files,
        )?;
        add_directory(
            &mut zip,
            &cfg.help_docs_dir(),
            &cfg.help_docs_dir(),
            "help_docs",
            options,
            &mut files,
        )?;
        let config_path = AppConfig::config_path();
        if config_path.exists() {
            let config_bytes = read_bytes(&config_path)?;
            write_zip_file(&mut zip, "config.toml", &config_bytes, options, &mut files)?;
        }
        let manifest = BackupManifest {
            format_version: 2,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            created_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            mode: "full".to_string(),
            files,
        };
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
        zip.start_file("backup_manifest.json", options)
            .map_err(|e| e.to_string())?;
        zip.write_all(&manifest_bytes).map_err(|e| e.to_string())?;
        zip.finish().map_err(|e| e.to_string())?;
        Ok::<(), String>(())
    })();
    let _ = fs::remove_file(snapshot);
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
    let (name, size) = if cfg.backup_mode.eq_ignore_ascii_case("full") {
        create_full_backup(cfg, automatic)?
    } else {
        create_database_backup_file(cfg, automatic)?
    };
    cleanup_old_backups(&cfg.backup_dir(), cfg.max_backup_count);
    let sync_warning = copy_to_sync_dir(cfg, &cfg.backup_dir().join(&name)).err();
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
        let _ = fs::remove_file(path);
    }
}

fn latest_automatic_backup(dir: &Path) -> Option<SystemTime> {
    fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            if !name.starts_with("workload_auto_") {
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
        || relative
            .split('/')
            .any(|part| part == ".." || part.is_empty())
    {
        return None;
    }
    Some(PathBuf::from(relative))
}

fn validate_full_archive(path: &Path) -> std::result::Result<BackupManifest, String> {
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("无法打开全量备份: {e}"))?;
    let manifest: BackupManifest = {
        let mut entry = archive
            .by_name("backup_manifest.json")
            .map_err(|_| "全量备份缺少校验清单".to_string())?;
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        serde_json::from_slice(&bytes).map_err(|e| format!("备份清单无效: {e}"))?
    };
    for expected in &manifest.files {
        let mut entry = archive
            .by_name(&expected.path)
            .map_err(|_| format!("备份缺少文件: {}", expected.path))?;
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        if bytes.len() as u64 != expected.size || sha256(&bytes) != expected.sha256 {
            return Err(format!("备份文件校验失败: {}", expected.path));
        }
    }
    Ok(manifest)
}

pub fn stage_restore(cfg: &AppConfig, source: &Path) -> std::result::Result<String, String> {
    if !source.exists() {
        return Err("备份文件不存在".into());
    }
    fs::create_dir_all(cfg.data_dir()).map_err(|e| e.to_string())?;
    let full = source
        .extension()
        .and_then(|x| x.to_str())
        .map(|x| x.eq_ignore_ascii_case("zip"))
        .unwrap_or(false);
    let pending_db = cfg.data_dir().join(PENDING_DB);
    let pending_attachments = cfg.data_dir().join(PENDING_ATTACHMENTS);
    let pending_help_docs = cfg.data_dir().join(PENDING_HELP_DOCS);
    let pending_config = cfg.data_dir().join(PENDING_CONFIG);
    let _ = fs::remove_file(&pending_db);
    let _ = fs::remove_file(&pending_config);
    let _ = fs::remove_dir_all(&pending_attachments);
    let _ = fs::remove_dir_all(&pending_help_docs);

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
            } else if name == "config.toml" {
                let mut out = fs::File::create(&pending_config).map_err(|e| e.to_string())?;
                std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
            } else if let Some(relative) = safe_relative_path(&name, "attachments/") {
                let destination = pending_attachments.join(relative);
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
    };
    fs::write(
        cfg.data_dir().join(PENDING_MARKER),
        serde_json::to_vec_pretty(&marker).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(safety)
}

fn replace_directory(source: &Path, destination: &Path) -> std::result::Result<(), String> {
    let previous = destination.with_extension(format!(
        "restore_previous_{}",
        Local::now().format("%Y%m%d_%H%M%S")
    ));
    if destination.exists() {
        fs::rename(destination, &previous).map_err(|e| format!("无法暂存现有附件: {e}"))?;
    }
    if source.exists() {
        if let Err(error) = fs::rename(source, destination) {
            if previous.exists() {
                let _ = fs::rename(&previous, destination);
            }
            return Err(format!("无法恢复附件: {error}"));
        }
    } else {
        fs::create_dir_all(destination).map_err(|e| e.to_string())?;
    }
    let _ = fs::remove_dir_all(previous);
    Ok(())
}

pub fn apply_pending_restore(cfg: &AppConfig) -> std::result::Result<Option<String>, String> {
    let marker_path = cfg.data_dir().join(PENDING_MARKER);
    if !marker_path.exists() {
        return Ok(None);
    }
    let marker: PendingRestore =
        serde_json::from_slice(&read_bytes(&marker_path)?).map_err(|e| e.to_string())?;
    let pending_db = cfg.data_dir().join(PENDING_DB);
    verify_database(&pending_db)?;
    let output = Command::new(postgres_tool("pg_restore")?)
        .args([
            "--clean",
            "--if-exists",
            "--no-owner",
            "--no-privileges",
            "--exit-on-error",
        ])
        .arg("--dbname")
        .arg(cfg.postgres_url())
        .arg(&pending_db)
        .output()
        .map_err(|error| format!("无法启动 pg_restore: {error}"))?;
    if !output.status.success() {
        return Err(command_error("pg_restore", &output));
    }
    if marker.full {
        replace_directory(
            &cfg.data_dir().join(PENDING_ATTACHMENTS),
            &cfg.attachments_dir(),
        )?;
        replace_directory(
            &cfg.data_dir().join(PENDING_HELP_DOCS),
            &cfg.help_docs_dir(),
        )?;
        let pending_config = cfg.data_dir().join(PENDING_CONFIG);
        if pending_config.exists() {
            fs::copy(&pending_config, AppConfig::config_path())
                .map_err(|e| format!("无法恢复配置: {e}"))?;
        }
    }
    let _ = fs::remove_file(pending_db);
    let _ = fs::remove_file(cfg.data_dir().join(PENDING_CONFIG));
    let _ = fs::remove_file(marker_path);
    Ok(Some(marker.source_name))
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
        assert!(db_result.name.ends_with(".dump"));
        verify_database(&cfg.backup_dir().join(db_result.name)).unwrap();
        let full_cfg = AppConfig {
            backup_mode: "full".into(),
            ..cfg.clone()
        };
        let full_result = create_backup(&full_cfg, false).unwrap();
        assert!(full_result.name.ends_with(".zip"));
        validate_full_archive(&full_cfg.backup_dir().join(full_result.name)).unwrap();
        let _ = fs::remove_dir_all(root);
    }
}
