use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    #[serde(default = "default_port")]
    pub server_port: u16,
    #[serde(default = "default_db_dir")]
    pub db_dir: String,
    #[serde(default)]
    pub database_url: String,
    #[serde(default = "default_db_pool_max_size")]
    pub db_pool_max_size: u32,
    #[serde(default = "default_db_pool_min_idle")]
    pub db_pool_min_idle: u32,
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default)]
    pub log_file: Option<String>,
    #[serde(default = "default_runtime_log_enabled")]
    pub runtime_log_enabled: bool,
    #[serde(default = "default_runtime_log_max_size_mb")]
    pub runtime_log_max_size_mb: u64,
    #[serde(default = "default_admin_user")]
    pub admin_user: String,
    #[serde(default = "default_admin_pass")]
    pub admin_pass: String,
    #[serde(default = "default_backup_enabled")]
    pub backup_enabled: bool,
    #[serde(default = "default_backup_interval")]
    pub backup_interval_hours: u64,
    #[serde(default = "default_max_backup_count")]
    pub max_backup_count: u64,
    #[serde(default = "default_backup_mode")]
    pub backup_mode: String,
    #[serde(default)]
    pub backup_sync_dir: Option<String>,
}
fn default_port() -> u16 {
    8000
}
fn default_db_dir() -> String {
    "data".to_string()
}
fn default_db_pool_max_size() -> u32 {
    12
}
fn default_db_pool_min_idle() -> u32 {
    1
}
fn default_log_level() -> String {
    "info".to_string()
}
fn default_runtime_log_enabled() -> bool {
    true
}
fn default_runtime_log_max_size_mb() -> u64 {
    50
}
fn default_admin_user() -> String {
    "admin".to_string()
}
fn default_admin_pass() -> String {
    "admin123".to_string()
}
fn default_backup_enabled() -> bool {
    false
}
fn default_backup_interval() -> u64 {
    24
}
fn default_max_backup_count() -> u64 {
    10
}
fn default_backup_mode() -> String {
    "full".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server_port: default_port(),
            db_dir: default_db_dir(),
            database_url: String::new(),
            db_pool_max_size: default_db_pool_max_size(),
            db_pool_min_idle: default_db_pool_min_idle(),
            log_level: default_log_level(),
            log_file: None,
            runtime_log_enabled: default_runtime_log_enabled(),
            runtime_log_max_size_mb: default_runtime_log_max_size_mb(),
            admin_user: default_admin_user(),
            admin_pass: default_admin_pass(),
            backup_enabled: default_backup_enabled(),
            backup_interval_hours: default_backup_interval(),
            max_backup_count: default_max_backup_count(),
            backup_mode: default_backup_mode(),
            backup_sync_dir: None,
        }
    }
}

impl AppConfig {
    pub fn set_server_port(port: u16) -> Result<(), String> {
        write_server_port(&Self::config_path(), port)
    }

    pub fn config_path() -> PathBuf {
        for name in ["LABFLOW_CONFIG_PATH", "WORKLOAD_CONFIG_PATH"] {
            if let Ok(path) = std::env::var(name) {
                if !path.trim().is_empty() {
                    return PathBuf::from(path);
                }
            }
        }
        let legacy_path = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_default()
            .join("config.toml");
        #[cfg(target_os = "windows")]
        if let Ok(program_data) = std::env::var("ProgramData") {
            for name in ["labflow", "WorkloadTool"] {
                let shared_path = PathBuf::from(&program_data)
                    .join(name)
                    .join("server-config.toml");
                if shared_path.exists() {
                    return shared_path;
                }
            }
            if !legacy_path.exists() {
                return PathBuf::from(program_data)
                    .join("labflow")
                    .join("server-config.toml");
            }
        }
        legacy_path
    }

    pub fn load() -> Self {
        let mut config = Self::default();
        let cp = Self::config_path();
        if cp.exists() {
            let c = std::fs::read_to_string(&cp).unwrap_or_default();
            if let Ok(toml_cfg) = toml::from_str::<Self>(&c) {
                config.admin_pass = toml_cfg.admin_pass;
                config.server_port = toml_cfg.server_port;
                config.db_dir = toml_cfg.db_dir;
                config.database_url = toml_cfg.database_url;
                config.db_pool_max_size = toml_cfg.db_pool_max_size;
                config.db_pool_min_idle = toml_cfg.db_pool_min_idle;
                config.log_level = toml_cfg.log_level;
                config.log_file = toml_cfg.log_file;
                config.runtime_log_enabled = toml_cfg.runtime_log_enabled;
                config.runtime_log_max_size_mb = toml_cfg.runtime_log_max_size_mb;
                config.admin_user = toml_cfg.admin_user;
                config.backup_enabled = toml_cfg.backup_enabled;
                config.backup_interval_hours = toml_cfg.backup_interval_hours;
                config.max_backup_count = toml_cfg.max_backup_count;
                config.backup_mode = toml_cfg.backup_mode;
                config.backup_sync_dir = toml_cfg.backup_sync_dir;
            }
        }
        if let Ok(port) =
            std::env::var("LABFLOW_SERVER_PORT").or_else(|_| std::env::var("WORKLOAD_SERVER_PORT"))
        {
            if let Ok(port) = port.parse() {
                config.server_port = port;
            }
        }
        // Environment variables are deployment overrides for Docker/systemd.
        if let Ok(password) = std::env::var("ADMIN_PASSWORD") {
            if !password.trim().is_empty() {
                config.admin_pass = password;
            }
        }
        config
    }
    pub fn save(&self) {
        if let Err(error) = self.try_save() {
            tracing::error!(%error, "服务器配置保存失败");
        }
    }

    pub fn try_save(&self) -> Result<(), String> {
        self.try_save_to(&Self::config_path())
    }

    fn try_save_to(&self, path: &std::path::Path) -> Result<(), String> {
        let mut value = if path.exists() {
            let text = std::fs::read_to_string(path)
                .map_err(|error| format!("无法读取原服务器配置: {error}"))?;
            toml::from_str::<toml::Value>(&text)
                .map_err(|error| format!("原服务器配置格式无效，未覆盖: {error}"))?
        } else {
            toml::Value::Table(toml::Table::new())
        };
        let updates =
            toml::Value::try_from(self).map_err(|error| format!("无法生成服务器配置: {error}"))?;
        let table = value.as_table_mut().ok_or("服务器配置必须是对象")?;
        table.extend(updates.as_table().ok_or("服务器配置必须是对象")?.clone());
        let text = toml::to_string_pretty(&value)
            .map_err(|error| format!("无法生成服务器配置: {error}"))?;
        let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4().simple()));
        std::fs::write(&temporary, text)
            .map_err(|error| format!("无法写入服务器配置，请检查权限与可用空间: {error}"))?;
        if let Err(error) = std::fs::rename(&temporary, path) {
            let _ = std::fs::remove_file(temporary);
            return Err(format!("无法保存服务器配置，原配置已保留: {error}"));
        }
        Ok(())
    }

    /// 数据目录：支持 WORKLOAD_DATA_DIR 环境变量（Docker/Linux），fallback 到 exe 同级目录下的 db_dir
    pub fn data_dir(&self) -> PathBuf {
        if let Ok(d) =
            std::env::var("LABFLOW_DATA_DIR").or_else(|_| std::env::var("WORKLOAD_DATA_DIR"))
        {
            PathBuf::from(d)
        } else {
            let configured = PathBuf::from(&self.db_dir);
            if configured.is_absolute() {
                return configured;
            }
            let exe_dir = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                .unwrap_or_default();
            exe_dir.join(configured)
        }
    }
    pub fn postgres_url(&self) -> String {
        std::env::var("DATABASE_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| self.database_url.clone())
    }
    pub fn backup_dir(&self) -> PathBuf {
        self.data_dir().join("backups")
    }
    /// 附件存储目录（data/attachments/）
    pub fn attachments_dir(&self) -> PathBuf {
        self.data_dir().join("attachments")
    }
    /// Generated previews are disposable and must stay on a short Windows path.
    /// Do not derive this from the attachment path: WinRT PDF APIs reject long paths.
    pub fn attachment_preview_dir(&self) -> PathBuf {
        if let Ok(path) = std::env::var("WORKLOAD_ATTACHMENT_PREVIEW_DIR") {
            if !path.trim().is_empty() {
                return PathBuf::from(path);
            }
        }
        std::env::var_os("ProgramData")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("LabFlow")
            .join("pv")
    }
    /// Private help sources and reader previews are application data and must be backed up together.
    pub fn help_docs_dir(&self) -> PathBuf {
        self.data_dir().join("help_docs")
    }
    pub fn runtime_logs_dir(&self) -> PathBuf {
        self.data_dir().join("logs")
    }
}

fn write_server_port(path: &std::path::Path, port: u16) -> Result<(), String> {
    if port == 0 {
        return Err("请输入1到65535之间的端口".into());
    }
    let resolved = path
        .canonicalize()
        .map_err(|_| "无法定位服务器配置，未修改端口")?;
    let path = resolved.as_path();
    let original = std::fs::read_to_string(path).map_err(|_| "无法读取服务器配置，未修改端口")?;
    let mut value: toml::Value =
        toml::from_str(&original).map_err(|_| "服务器配置格式无效，未修改端口")?;
    value
        .as_table_mut()
        .ok_or("服务器配置格式无效，未修改端口")?
        .insert("server_port".into(), toml::Value::Integer(port.into()));
    let text = toml::to_string_pretty(&value).map_err(|_| "无法生成服务器配置")?;
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4().simple()));
    std::fs::write(&temporary, text).map_err(|_| "无法写入服务器配置，请检查文件权限")?;
    if let Err(error) = std::fs::rename(&temporary, path) {
        let _ = std::fs::remove_file(temporary);
        return Err(format!("无法保存端口，原配置已保留：{error}"));
    }
    Ok(())
}

#[cfg(test)]
mod desktop_port_tests {
    #[test]
    fn saving_config_preserves_unknown_keys_and_reports_write_errors() {
        let root = std::env::temp_dir().join(format!("labflow-config-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("config.toml");
        std::fs::write(&path, "custom_setting = \"preserve\"\n").unwrap();
        let config = super::AppConfig {
            runtime_log_max_size_mb: 37,
            ..super::AppConfig::default()
        };
        config.try_save_to(&path).unwrap();
        let value: toml::Value = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(value["custom_setting"].as_str(), Some("preserve"));
        assert_eq!(value["runtime_log_max_size_mb"].as_integer(), Some(37));
        assert!(config
            .try_save_to(&root.join("missing/config.toml"))
            .is_err());
        std::fs::write(&path, "invalid = [").unwrap();
        assert!(config.try_save_to(&path).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "invalid = [");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn editing_port_preserves_custom_settings_and_rejects_zero() {
        let path =
            std::env::temp_dir().join(format!("labflow-config-{}.toml", uuid::Uuid::new_v4()));
        let original = "server_port = 8000\nbackup_enabled = true\ncustom_setting = \"preserve\"\n";
        std::fs::write(&path, original).unwrap();
        assert!(super::write_server_port(&path, 0).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        super::write_server_port(&path, 8123).unwrap();
        let updated: toml::Value =
            toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(updated["server_port"].as_integer(), Some(8123));
        assert_eq!(updated["backup_enabled"].as_bool(), Some(true));
        assert_eq!(updated["custom_setting"].as_str(), Some("preserve"));
        std::fs::remove_file(path).unwrap();
    }
}
