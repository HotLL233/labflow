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
    "database".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server_port: default_port(),
            db_dir: default_db_dir(),
            database_url: String::new(),
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
    pub fn config_path() -> PathBuf {
        if let Ok(path) = std::env::var("WORKLOAD_CONFIG_PATH") {
            if !path.trim().is_empty() {
                return PathBuf::from(path);
            }
        }
        let legacy_path = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_default()
            .join("config.toml");
        #[cfg(target_os = "windows")]
        if let Ok(program_data) = std::env::var("ProgramData") {
            let shared_path = PathBuf::from(program_data)
                .join("WorkloadTool")
                .join("server-config.toml");
            if shared_path.exists() || !legacy_path.exists() {
                return shared_path;
            }
        }
        legacy_path
    }

    pub fn load() -> Self {
        let mut config = Self::default();
        // 优先从环境变量读取 ADMIN_PASSWORD（Docker/Linux）
        if let Ok(p) = std::env::var("ADMIN_PASSWORD") {
            config.admin_pass = p;
        }
        if let Ok(port) = std::env::var("WORKLOAD_SERVER_PORT") {
            if let Ok(port) = port.parse() {
                config.server_port = port;
            }
        }
        let cp = Self::config_path();
        if cp.exists() {
            let c = std::fs::read_to_string(&cp).unwrap_or_default();
            if let Ok(toml_cfg) = toml::from_str::<Self>(&c) {
                config.admin_pass = toml_cfg.admin_pass;
                config.server_port = toml_cfg.server_port;
                config.db_dir = toml_cfg.db_dir;
                config.database_url = toml_cfg.database_url;
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
        if let Ok(port) = std::env::var("WORKLOAD_SERVER_PORT") {
            if let Ok(port) = port.parse() {
                config.server_port = port;
            }
        }
        config
    }
    pub fn save(&self) {
        let cp = Self::config_path();
        if let Ok(s) = toml::to_string_pretty(self) {
            let _ = std::fs::write(&cp, s);
        }
    }

    /// 数据目录：支持 WORKLOAD_DATA_DIR 环境变量（Docker/Linux），fallback 到 exe 同级目录下的 db_dir
    pub fn data_dir(&self) -> PathBuf {
        if let Ok(d) = std::env::var("WORKLOAD_DATA_DIR") {
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
    /// Private help sources and reader previews are application data and must be backed up together.
    pub fn help_docs_dir(&self) -> PathBuf {
        self.data_dir().join("help_docs")
    }
    pub fn runtime_logs_dir(&self) -> PathBuf {
        self.data_dir().join("logs")
    }
}
