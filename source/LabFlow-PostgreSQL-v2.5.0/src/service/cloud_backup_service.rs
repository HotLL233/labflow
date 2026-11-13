use super::onedrive_backup_service as onedrive;
use crate::{config::AppConfig, service::backup_service};
use chrono::Utc;
use quick_xml::{events::Event, Reader};
use reqwest::{blocking::Client, header, Method, Url};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::Duration,
};

type CloudResult<T> = std::result::Result<T, String>;
static STATE_LOCK: Mutex<()> = Mutex::new(());
static TRANSFER_LOCK: Mutex<()> = Mutex::new(());
static INITIALIZED: OnceLock<()> = OnceLock::new();

#[derive(Clone, Default, Serialize, Deserialize)]
struct Settings {
    #[serde(default = "default_provider")]
    provider: String,
    enabled: bool,
    auto_upload: bool,
    endpoint: String,
    username: String,
    password: String,
    #[serde(default)]
    onedrive: onedrive::Auth,
}

fn default_provider() -> String {
    "webdav".into()
}

#[derive(Serialize)]
pub struct SettingsView {
    pub provider: String,
    pub enabled: bool,
    pub auto_upload: bool,
    pub endpoint: String,
    pub username: String,
    pub has_password: bool,
    pub onedrive: OneDriveView,
}

#[derive(Serialize)]
pub struct OneDriveView {
    pub client_id: String,
    pub tenant: String,
    pub folder: String,
    pub connected: bool,
    pub account: String,
}

#[derive(Default, Deserialize)]
pub struct SettingsInput {
    #[serde(default)]
    pub provider: Option<String>,
    pub enabled: bool,
    pub auto_upload: bool,
    pub endpoint: String,
    pub username: String,
    pub password: Option<String>,
    #[serde(default)]
    pub onedrive: Option<onedrive::Setup>,
}

impl Settings {
    fn view(&self) -> SettingsView {
        SettingsView {
            provider: if self.provider.is_empty() {
                default_provider()
            } else {
                self.provider.clone()
            },
            enabled: self.enabled,
            auto_upload: self.auto_upload,
            endpoint: self.endpoint.clone(),
            username: self.username.clone(),
            has_password: !self.password.is_empty(),
            onedrive: OneDriveView {
                client_id: self.onedrive.setup.client_id.clone(),
                tenant: self.onedrive.setup.tenant.clone(),
                folder: self.onedrive.setup.folder.clone(),
                connected: self.onedrive.connected(),
                account: self.onedrive.account.clone(),
            },
        }
    }

    fn target(&self) -> String {
        if self.provider == "onedrive" {
            return format!(
                "{:x}",
                Sha256::digest(
                    format!(
                        "onedrive\n{}\n{}\n{}\n{}",
                        self.onedrive.setup.client_id,
                        self.onedrive.setup.tenant,
                        self.onedrive.setup.folder,
                        self.onedrive.account_id
                    )
                    .as_bytes()
                )
            );
        }
        format!(
            "{:x}",
            Sha256::digest(format!("{}\n{}", self.endpoint, self.username).as_bytes())
        )
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Transfer {
    pub id: String,
    pub direction: String,
    pub name: String,
    pub size: u64,
    pub status: String,
    pub attempts: u32,
    pub detail: String,
    pub created_at: i64,
    pub updated_at: i64,
    next_attempt_at: i64,
    target: String,
}

#[derive(Serialize)]
pub struct CloudStatus {
    pub settings: SettingsView,
    pub transfers: Vec<Transfer>,
}

#[derive(Serialize)]
pub struct RemoteBackup {
    pub name: String,
    pub size: u64,
    pub has_manifest: bool,
}

#[derive(Serialize, Deserialize)]
struct RemoteManifest {
    name: String,
    size: u64,
    sha256: String,
    app_version: String,
    created_at: i64,
}

fn root(cfg: &AppConfig) -> PathBuf {
    // This directory is deliberately outside backup_sources: credentials and jobs must not be restored from a dump.
    cfg.data_dir().join("cloud-backup")
}

fn settings(cfg: &AppConfig) -> CloudResult<Settings> {
    let path = root(cfg).join("settings.json");
    if !path.exists() {
        return Ok(Settings::default());
    }
    serde_json::from_reader(File::open(path).map_err(|_| "无法读取云端备份配置")?)
        .map_err(|_| "云端备份配置损坏，请检查服务器文件".into())
}

pub(super) fn atomic_json(path: &Path, value: &impl Serialize) -> CloudResult<()> {
    let parent = path.parent().ok_or("无效状态目录")?;
    fs::create_dir_all(parent).map_err(|_| "无法创建云端备份状态目录")?;
    #[cfg(windows)]
    if path
        .file_name()
        .is_some_and(|value| value == "settings.json")
    {
        let identity = std::process::Command::new(r"C:\Windows\System32\whoami.exe")
            .args(["/user", "/fo", "csv", "/nh"])
            .output()
            .map_err(|_| "无法确认云端凭据的服务账号")?;
        let identity = String::from_utf8_lossy(&identity.stdout);
        let sid = identity
            .trim()
            .split(',')
            .next_back()
            .unwrap_or("")
            .trim_matches('"');
        if !sid.starts_with("S-1-")
            || !sid
                .bytes()
                .all(|value| value.is_ascii_digit() || b"S-".contains(&value))
        {
            return Err("无法确认云端凭据的服务账号".into());
        }
        let access = std::process::Command::new(r"C:\Windows\System32\icacls.exe")
            .arg(parent)
            .args([
                "/inheritance:r",
                "/grant:r",
                "*S-1-5-18:(OI)(CI)(F)",
                "/grant:r",
                "*S-1-5-32-544:(OI)(CI)(F)",
                "/grant:r",
            ])
            .arg(format!("*{sid}:(OI)(CI)(F)"))
            .output()
            .map_err(|_| "无法保护网盘凭据目录")?;
        if !access.status.success() {
            return Err("网盘凭据目录权限设置失败，配置未保存".into());
        }
    }
    let temporary = parent.join(format!(".{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut output = options
            .open(&temporary)
            .map_err(|_| "无法保存云端备份状态")?;
        serde_json::to_writer(&mut output, value).map_err(|_| "云端备份状态编码失败")?;
        output.flush().map_err(|_| "云端备份状态写入失败")?;
        output.sync_all().map_err(|_| "云端备份状态写入失败")?;
        fs::rename(&temporary, path).map_err(|_| "无法提交云端备份状态")?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

fn read_transfers(cfg: &AppConfig) -> CloudResult<Vec<Transfer>> {
    read_transfers_from(&root(cfg).join("jobs"))
}

fn read_transfers_from(dir: &Path) -> CloudResult<Vec<Transfer>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut transfers = Vec::new();
    for entry in fs::read_dir(dir).map_err(|_| "无法读取传输任务目录")? {
        let path = entry.map_err(|_| "无法读取传输任务")?.path();
        if path.extension().is_some_and(|value| value == "json") {
            let transfer =
                serde_json::from_reader(File::open(path).map_err(|_| "无法打开传输任务")?)
                    .map_err(|_| "传输任务文件损坏，已停止自动处理以保护备份")?;
            transfers.push(transfer);
        }
    }
    transfers.sort_by_key(|item: &Transfer| std::cmp::Reverse(item.created_at));
    Ok(transfers)
}

fn save_transfer(cfg: &AppConfig, transfer: &Transfer) -> CloudResult<()> {
    let id = uuid::Uuid::parse_str(&transfer.id).map_err(|_| "无效任务编号")?;
    atomic_json(&root(cfg).join("jobs").join(format!("{id}.json")), transfer)
}

pub fn status(cfg: &AppConfig) -> CloudResult<CloudStatus> {
    let _guard = STATE_LOCK.lock().map_err(|_| "云端状态锁失败")?;
    let mut transfers = read_transfers(cfg)?;
    let mut finished = 0;
    transfers.retain(|job| {
        if protects(job) {
            true
        } else {
            finished += 1;
            finished <= 100
        }
    });
    Ok(CloudStatus {
        settings: settings(cfg)?.view(),
        transfers,
    })
}

fn normalize_settings(cfg: &AppConfig, input: SettingsInput) -> CloudResult<Settings> {
    let mut value = settings(cfg)?;
    let previous_target = value.target();
    let had_password = !value.password.is_empty();
    let supplied_password = input
        .password
        .as_ref()
        .is_some_and(|value| !value.is_empty());
    if let Some(provider) = input.provider {
        if !matches!(
            provider.as_str(),
            "webdav"
                | "nutstore"
                | "nextcloud"
                | "owncloud"
                | "openlist"
                | "baidu_gateway"
                | "aliyun_gateway"
                | "quark_gateway"
                | "onedrive"
        ) {
            return Err("不支持的网盘类型".into());
        }
        value.provider = provider;
    }
    if value.provider.is_empty() {
        value.provider = default_provider();
    }
    value.enabled = input.enabled;
    value.auto_upload = input.auto_upload;
    value.endpoint = input.endpoint.trim().to_string();
    value.username = input.username.trim().to_string();
    if let Some(password) = input.password {
        value.password = password;
    }
    if let Some(mut setup) = input.onedrive {
        onedrive::validate_setup(&mut setup, value.enabled && value.provider == "onedrive")?;
        if setup.client_id != value.onedrive.setup.client_id
            || setup.tenant != value.onedrive.setup.tenant
        {
            value.onedrive = onedrive::Auth::default();
        }
        value.onedrive.setup = setup;
    }
    if value.provider == "onedrive" {
        onedrive::validate_setup(&mut value.onedrive.setup, value.enabled)?;
        value.password.clear();
        value.username.clear();
        value.endpoint.clear();
        return Ok(value);
    }
    if !value.endpoint.is_empty() {
        let mut url = Url::parse(&value.endpoint).map_err(|_| "请输入完整 WebDAV 目录地址")?;
        let loopback = matches!(
            url.host_str(),
            Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
        );
        if (url.scheme() != "https" && !(url.scheme() == "http" && loopback))
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err("WebDAV 必须使用 HTTPS，账号密码请填写在独立输入框中".into());
        }
        if !url.path().ends_with('/') {
            url.set_path(&format!("{}/", url.path()));
        }
        value.endpoint = url.to_string();
        if value.provider == "nutstore" {
            if url.host_str() != Some("dav.jianguoyun.com") || !url.path().starts_with("/dav/") {
                return Err("坚果云地址应使用 https://dav.jianguoyun.com/dav/ 下的备份目录".into());
            }
            if url.path() == "/dav/" {
                url.set_path("/dav/LabFlowBackups/");
                value.endpoint = url.to_string();
            }
        }
    }
    if had_password && value.target() != previous_target && !supplied_password {
        if value.enabled {
            return Err("更换网盘目录或账号时，请重新输入应用密码，原密码不会发送给新目标".into());
        }
        value.password.clear();
    }
    if value.enabled
        && (value.endpoint.is_empty() || value.username.is_empty() || value.password.is_empty())
    {
        return Err("启用云端备份需要目录地址、账号和应用密码".into());
    }
    Ok(value)
}

pub fn update_settings(cfg: &AppConfig, input: SettingsInput) -> CloudResult<SettingsView> {
    let _guard = STATE_LOCK.lock().map_err(|_| "云端状态锁失败")?;
    let value = normalize_settings(cfg, input)?;
    atomic_json(&root(cfg).join("settings.json"), &value)?;
    Ok(value.view())
}

#[derive(Serialize, Deserialize)]
struct PendingLogin {
    id: String,
    actor_id: i64,
    target: String,
    device: onedrive::DeviceCode,
    expires_at: i64,
    next_poll_at: i64,
}

#[derive(Serialize)]
pub struct LoginView {
    pub id: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_at: i64,
    pub interval: i64,
}

pub fn start_login(cfg: &AppConfig, actor_id: i64) -> CloudResult<LoginView> {
    let value = settings(cfg)?;
    if value.provider != "onedrive" || !value.enabled {
        return Err("请先保存并启用 OneDrive 配置".into());
    }
    let mut setup = value.onedrive.setup.clone();
    onedrive::validate_setup(&mut setup, true)?;
    let device = onedrive::start_device(&setup)?;
    let view = LoginView {
        id: uuid::Uuid::new_v4().to_string(),
        user_code: device.user_code.clone(),
        verification_uri: device.verification_uri.clone(),
        expires_at: Utc::now().timestamp() + device.expires_in,
        interval: device.interval.max(5),
    };
    let _guard = STATE_LOCK.lock().map_err(|_| "授权状态锁失败")?;
    if settings(cfg)?.target() != value.target() {
        return Err("配置已变更，请重新发起授权".into());
    }
    atomic_json(
        &root(cfg).join("onedrive-login.json"),
        &PendingLogin {
            id: view.id.clone(),
            actor_id,
            target: value.target(),
            device,
            expires_at: view.expires_at,
            next_poll_at: Utc::now().timestamp() + view.interval,
        },
    )?;
    Ok(view)
}

pub fn poll_login(cfg: &AppConfig, actor_id: i64, id: &str) -> CloudResult<bool> {
    let (mut pending, value) = {
        let _guard = STATE_LOCK.lock().map_err(|_| "授权状态锁失败")?;
        let mut pending: PendingLogin = serde_json::from_reader(
            File::open(root(cfg).join("onedrive-login.json"))
                .map_err(|_| "授权任务不存在，请重新发起")?,
        )
        .map_err(|_| "授权任务无效")?;
        let value = settings(cfg)?;
        if pending.id != id
            || pending.actor_id != actor_id
            || value.provider != "onedrive"
            || pending.target != value.target()
        {
            return Err("授权任务与当前操作者或配置不匹配，请重新发起".into());
        }
        let now = Utc::now().timestamp();
        if now >= pending.expires_at {
            return Err("微软授权码已过期，请重新发起".into());
        }
        if now < pending.next_poll_at {
            return Ok(false);
        }
        pending.next_poll_at = now + pending.device.interval.max(5);
        atomic_json(&root(cfg).join("onedrive-login.json"), &pending)?;
        (pending, value)
    };
    let result = onedrive::poll_device(&value.onedrive.setup, &pending.device.device_code)?;
    let _guard = STATE_LOCK.lock().map_err(|_| "授权状态锁失败")?;
    let mut latest = settings(cfg)?;
    let current: PendingLogin = serde_json::from_reader(
        File::open(root(cfg).join("onedrive-login.json")).map_err(|_| "授权任务已结束")?,
    )
    .map_err(|_| "授权任务无效")?;
    if current.id != id || latest.target() != pending.target {
        return Err("授权期间配置已变更，请重新发起".into());
    }
    match result {
        onedrive::LoginResult::Pending(slow) => {
            if slow {
                if pending.device.interval >= 300 {
                    return Err("微软授权服务持续限流，请稍后重新发起".into());
                }
                pending.device.interval += 5;
                pending.next_poll_at = Utc::now().timestamp() + pending.device.interval;
                atomic_json(&root(cfg).join("onedrive-login.json"), &pending)?;
            }
            Ok(false)
        }
        onedrive::LoginResult::Authorized(auth) => {
            latest.onedrive = auth;
            atomic_json(&root(cfg).join("settings.json"), &latest)?;
            let _ = fs::remove_file(root(cfg).join("onedrive-login.json"));
            Ok(true)
        }
    }
}

pub fn disconnect_onedrive(cfg: &AppConfig) -> CloudResult<()> {
    let _guard = STATE_LOCK.lock().map_err(|_| "授权状态锁失败")?;
    let mut value = settings(cfg)?;
    value.onedrive.access_token.clear();
    value.onedrive.refresh_token.clear();
    value.onedrive.expires_at = 0;
    atomic_json(&root(cfg).join("settings.json"), &value)?;
    let _ = fs::remove_file(root(cfg).join("onedrive-login.json"));
    Ok(())
}

fn connection(value: &Settings) -> CloudResult<Client> {
    if !value.enabled {
        return Err("云端备份尚未启用".into());
    }
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("LabFlow/", env!("CARGO_PKG_VERSION"), " WebDAV"))
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(1800))
        .build()
        .map_err(|_| "无法初始化 WebDAV 连接".into())
}

fn file_url(value: &Settings, name: &str) -> CloudResult<Url> {
    let mut url = Url::parse(&value.endpoint).map_err(|_| "WebDAV 地址无效")?;
    url.path_segments_mut()
        .map_err(|_| "WebDAV 地址无效")?
        .pop_if_empty()
        .push(name);
    Ok(url)
}

fn request(
    client: &Client,
    value: &Settings,
    method: Method,
    url: Url,
) -> reqwest::blocking::RequestBuilder {
    client
        .request(method, url)
        .basic_auth(&value.username, Some(&value.password))
}

fn response(
    result: reqwest::Result<reqwest::blocking::Response>,
) -> CloudResult<reqwest::blocking::Response> {
    let result = result.map_err(|_| "网盘连接失败，请检查网络、证书和目录地址")?;
    if !result.status().is_success() {
        let reason = match result.status().as_u16() {
            401 => "服务器未接受账号或应用密码，请确认当前配置已保存且授权仍有效",
            403 => "当前账号无权执行此操作，请检查备份目录的读写与删除权限",
            404 => "目录或文件不存在；请检查完整目录层级，服务根集合通常不能直接上传文件",
            405 => "该位置不支持此操作，请确认填写的是可写 WebDAV 目录",
            413 => "文件超过网盘单文件大小限制",
            423 => "目标文件被锁定，请稍后重试",
            429 => "请求过于频繁，请稍后重试",
            507 => "网盘存储空间不足",
            301 | 302 | 307 | 308 => {
                "服务要求跳转，请填写最终 HTTPS WebDAV 地址；凭据不会自动转发到其他地址"
            }
            _ => "网盘服务拒绝请求，请稍后重试或检查服务状态",
        };
        return Err(format!(
            "网盘返回 HTTP {}：{reason}",
            result.status().as_u16()
        ));
    }
    Ok(result)
}

pub fn test_connection(cfg: &AppConfig) -> CloudResult<()> {
    test_current_connection(cfg, None)
}

pub fn test_current_connection(cfg: &AppConfig, input: Option<SettingsInput>) -> CloudResult<()> {
    let value = match input {
        Some(input) => normalize_settings(cfg, input)?,
        None => settings(cfg)?,
    };
    let storage = Storage::open(cfg, &value)?;
    storage.ensure_folder()?;
    let name = format!("LabFlowProbe-{}.txt", uuid::Uuid::new_v4());
    let probe = uuid::Uuid::new_v4().to_string();
    storage
        .put_bytes(&name, probe.as_bytes().to_vec())
        .map_err(|e| format!("上传测试文件：{e}"))?;
    let result: CloudResult<()> = (|| {
        let mut data = Vec::new();
        storage
            .get(&name)?
            .ok_or("测试文件上传后不可读取")?
            .take(1024)
            .read_to_end(&mut data)
            .map_err(|_| "测试文件读取失败")?;
        if data != probe.as_bytes() {
            return Err("测试文件内容不一致，连接未通过".into());
        }
        Ok(())
    })();
    let cleanup = storage.delete_probe(&name);
    result?;
    cleanup
}

enum Storage {
    WebDav(Settings, Client),
    OneDrive(onedrive::Connection),
}

impl Storage {
    fn open(cfg: &AppConfig, value: &Settings) -> CloudResult<Self> {
        if !value.enabled {
            return Err("云端备份尚未启用".into());
        }
        if value.provider != "onedrive" {
            return Ok(Self::WebDav(value.clone(), connection(value)?));
        }
        let mut auth = value.onedrive.clone();
        if !auth.connected() {
            return Err("请先保存配置并完成 OneDrive 账号授权".into());
        }
        if auth.expires_at <= Utc::now().timestamp() + 120 {
            auth = onedrive::refresh(&auth)?;
            auth = store_refreshed_auth(cfg, value, auth)?;
        }
        onedrive::Connection::new(auth, root(cfg)).map(Self::OneDrive)
    }

    fn ensure_folder(&self) -> CloudResult<()> {
        match self {
            Self::OneDrive(value) => value.ensure_folder(),
            Self::WebDav(value, client) => ensure_dav_folder(value, client),
        }
    }

    fn get(&self, name: &str) -> CloudResult<Option<reqwest::blocking::Response>> {
        match self {
            Self::OneDrive(value) => value.get(name),
            Self::WebDav(value, client) => dav_get(value, client, name),
        }
    }

    fn put_bytes(&self, name: &str, bytes: Vec<u8>) -> CloudResult<()> {
        match self {
            Self::OneDrive(value) => value.put_bytes(name, bytes),
            Self::WebDav(value, client) => response(
                request(client, value, Method::PUT, file_url(value, name)?)
                    .body(bytes)
                    .send(),
            )
            .map(|_| ()),
        }
    }

    fn put_file(
        &self,
        name: &str,
        path: &Path,
        size: u64,
        sha256: &str,
        id: &str,
    ) -> CloudResult<()> {
        match self {
            Self::OneDrive(value) => value.put_file(name, path, size, sha256, id),
            Self::WebDav(value, client) => response(
                request(client, value, Method::PUT, file_url(value, name)?)
                    .body(reqwest::blocking::Body::sized(
                        File::open(path).map_err(|_| "无法读取备份")?,
                        size,
                    ))
                    .send(),
            )
            .map(|_| ()),
        }
    }

    fn size(&self, name: &str) -> CloudResult<Option<u64>> {
        match self {
            Self::OneDrive(value) => value.file_size(name),
            Self::WebDav(value, client) => {
                Ok(dav_listing(value, client, file_url(value, name)?, "0")?
                    .into_iter()
                    .find(|item| item.name == name)
                    .map(|item| item.size))
            }
        }
    }

    fn delete_probe(&self, name: &str) -> CloudResult<()> {
        match self {
            Self::OneDrive(value) => value.delete_probe(name),
            Self::WebDav(value, client) => {
                response(request(client, value, Method::DELETE, file_url(value, name)?).send())
                    .map(|_| ())
            }
        }
    }
}

fn store_refreshed_auth(
    cfg: &AppConfig,
    snapshot: &Settings,
    mut auth: onedrive::Auth,
) -> CloudResult<onedrive::Auth> {
    let _guard = STATE_LOCK.lock().map_err(|_| "授权状态锁失败")?;
    let mut current = settings(cfg)?;
    if current.provider != "onedrive"
        || !current.onedrive.connected()
        || current.onedrive.setup.client_id != auth.setup.client_id
        || current.onedrive.setup.tenant != auth.setup.tenant
        || current.onedrive.account_id != auth.account_id
    {
        return Err("刷新期间授权已断开或账号配置已改变，请重新授权".into());
    }
    if current.onedrive.refresh_token != snapshot.onedrive.refresh_token {
        // Never overwrite a newer login/refresh or resurrect a locally disconnected account.
        auth.access_token = current.onedrive.access_token;
        auth.refresh_token = current.onedrive.refresh_token;
        auth.expires_at = current.onedrive.expires_at;
    } else {
        current.onedrive.access_token = auth.access_token.clone();
        current.onedrive.refresh_token = auth.refresh_token.clone();
        current.onedrive.expires_at = auth.expires_at;
        atomic_json(&root(cfg).join("settings.json"), &current)?;
    }
    Ok(auth)
}

fn dav_get(
    value: &Settings,
    client: &Client,
    name: &str,
) -> CloudResult<Option<reqwest::blocking::Response>> {
    let mut url = file_url(value, name)?;
    let origin = url.origin();
    let local = url.scheme() == "http"
        && matches!(
            url.host_str(),
            Some("127.0.0.1" | "localhost" | "[::1]" | "::1")
        );
    let mut forward_auth = true;
    for _ in 0..5 {
        let mut builder = client.get(url.clone());
        if forward_auth {
            builder = builder.basic_auth(&value.username, Some(&value.password));
        }
        let result = builder.send().map_err(|_| "网盘文件读取连接失败")?;
        if result.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !result.status().is_redirection() {
            return response(Ok(result)).map(Some);
        }
        let location = result
            .headers()
            .get(header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .ok_or("网盘下载跳转缺少地址")?;
        let next = url.join(location).map_err(|_| "网盘下载跳转地址无效")?;
        let local_next = local
            && next.scheme() == "http"
            && matches!(
                next.host_str(),
                Some("127.0.0.1" | "localhost" | "[::1]" | "::1")
            );
        if (next.scheme() != "https" && !local_next)
            || !next.username().is_empty()
            || next.password().is_some()
            || next.fragment().is_some()
        {
            return Err("已阻止不安全的网盘下载跳转".into());
        }
        forward_auth &= next.origin() == origin;
        url = next;
    }
    Err("网盘下载跳转过多".into())
}

fn ensure_dav_folder(value: &Settings, client: &Client) -> CloudResult<()> {
    let url = Url::parse(&value.endpoint).map_err(|_| "WebDAV 地址无效")?;
    let probe = |url: Url| {
        request(client, value, Method::from_bytes(b"PROPFIND").unwrap(), url)
            .header("Depth", "0")
            .send()
    };
    let found = probe(url.clone()).map_err(|_| "目录鉴权连接失败")?;
    let missing = found.status() == reqwest::StatusCode::NOT_FOUND;
    if !missing {
        response(Ok(found)).map_err(|e| format!("目录鉴权：{e}"))?;
    }
    if url.host_str() == Some("dav.jianguoyun.com") && url.path() == "/dav/" {
        return Err(
            "坚果云 /dav/ 是同步空间入口，不能直接上传。请选择坚果云类型并使用具体备份目录".into(),
        );
    }
    if matches!(
        value.provider.as_str(),
        "openlist" | "baidu_gateway" | "aliyun_gateway" | "quark_gateway"
    ) && url.path().trim_end_matches('/') == "/dav"
    {
        return Err("网关 /dav/ 是挂载入口，请配置 /dav/挂载名/备份目录/".into());
    }
    if missing {
        let mut parent = url.clone();
        parent
            .path_segments_mut()
            .map_err(|_| "备份目录地址无效")?
            .pop_if_empty()
            .pop()
            .push("");
        if parent == url {
            return Err("WebDAV 服务根地址不存在".into());
        }
        response(probe(parent)).map_err(|e| format!("上级目录不可用，请填写实际层级：{e}"))?;
        let created = request(
            client,
            value,
            Method::from_bytes(b"MKCOL").unwrap(),
            url.clone(),
        )
        .send()
        .map_err(|_| "创建备份目录失败")?;
        if !matches!(created.status().as_u16(), 201 | 405) {
            response(Ok(created)).map_err(|e| format!("创建备份目录：{e}"))?;
        }
        response(probe(url)).map_err(|e| format!("创建后的目录不可读取：{e}"))?;
    }
    Ok(())
}

pub fn valid_backup_name(name: &str) -> bool {
    let base = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved = matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (base.len() == 4
            && (base.starts_with("COM") || base.starts_with("LPT"))
            && matches!(base.as_bytes()[3], b'1'..=b'9'));
    name.len() <= 180
        && !reserved
        && name
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && (name.ends_with(".zip") || name.ends_with(".dump"))
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-.".contains(&c))
        && !name.contains("..")
}

pub fn local_file(cfg: &AppConfig, name: &str) -> CloudResult<PathBuf> {
    if !valid_backup_name(name) {
        return Err("无效备份文件名".into());
    }
    let path = cfg.backup_dir().join(name);
    let metadata = fs::symlink_metadata(&path).map_err(|_| "备份文件不存在")?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("备份文件必须是备份目录中的普通文件".into());
    }
    let resolved = path.canonicalize().map_err(|_| "无法读取备份文件")?;
    if resolved.parent()
        != Some(
            cfg.backup_dir()
                .canonicalize()
                .map_err(|_| "备份目录不存在")?
                .as_path(),
        )
    {
        return Err("备份文件不在允许目录中".into());
    }
    Ok(resolved)
}

fn protects(transfer: &Transfer) -> bool {
    matches!(transfer.status.as_str(), "queued" | "running" | "failed")
}

pub fn protected_names(cfg: &AppConfig) -> CloudResult<HashSet<String>> {
    let _guard = STATE_LOCK.lock().map_err(|_| "云端状态锁失败")?;
    Ok(read_transfers(cfg)?
        .into_iter()
        .filter(protects)
        .map(|item| item.name)
        .collect())
}

pub fn protected_names_for_directory(directory: &Path) -> CloudResult<HashSet<String>> {
    let _guard = STATE_LOCK.lock().map_err(|_| "云端状态锁失败")?;
    let Some(parent) = directory.parent() else {
        return Ok(HashSet::new());
    };
    Ok(read_transfers_from(&parent.join("cloud-backup/jobs"))?
        .into_iter()
        .filter(protects)
        .map(|item| item.name)
        .collect())
}

pub fn enqueue(cfg: &AppConfig, name: &str, direction: &str) -> CloudResult<String> {
    if !valid_backup_name(name) || !matches!(direction, "upload" | "download") {
        return Err("无效备份传输请求".into());
    }
    let _guard = STATE_LOCK.lock().map_err(|_| "云端状态锁失败")?;
    let value = settings(cfg)?;
    if !value.enabled {
        return Err("请先启用并保存云端备份配置".into());
    }
    if value.provider == "onedrive" && !value.onedrive.connected() {
        return Err("请先完成 OneDrive 账号授权，再上传或取回备份".into());
    }
    let transfers = read_transfers(cfg)?;
    if let Some(existing) = transfers.iter().find(|job| {
        job.name == name
            && job.direction == direction
            && job.target == value.target()
            && protects(job)
    }) {
        return Ok(existing.id.clone());
    }
    let size = if direction == "upload" {
        fs::metadata(local_file(cfg, name)?)
            .map_err(|_| "无法读取备份大小")?
            .len()
    } else {
        0
    };
    let now = Utc::now().timestamp();
    let transfer = Transfer {
        id: uuid::Uuid::new_v4().to_string(),
        direction: direction.into(),
        name: name.into(),
        size,
        status: "queued".into(),
        attempts: 0,
        detail: "等待后台传输".into(),
        created_at: now,
        updated_at: now,
        next_attempt_at: now,
        target: value.target(),
    };
    save_transfer(cfg, &transfer)?;
    Ok(transfer.id)
}

pub fn enqueue_automatic(cfg: &AppConfig, name: &str) -> CloudResult<()> {
    let value = settings(cfg)?;
    if value.enabled && value.auto_upload {
        enqueue(cfg, name, "upload")?;
    }
    Ok(())
}

pub fn change_transfer(cfg: &AppConfig, id: &str, retry: bool) -> CloudResult<()> {
    let _guard = STATE_LOCK.lock().map_err(|_| "云端状态锁失败")?;
    let mut transfer = read_transfers(cfg)?
        .into_iter()
        .find(|job| job.id == id)
        .ok_or("任务不存在")?;
    if transfer.status == "running" || transfer.status == "succeeded" {
        return Err("进行中或已完成的任务不能重试/取消".into());
    }
    if retry && transfer.target != settings(cfg)?.target() {
        return Err("目标网盘已变更，请恢复原目标配置或取消此任务".into());
    }
    transfer.status = if retry { "queued" } else { "cancelled" }.into();
    transfer.detail = if retry {
        "等待重新传输"
    } else {
        "已取消，未删除任何本地或云端备份"
    }
    .into();
    transfer.attempts = 0;
    transfer.next_attempt_at = Utc::now().timestamp();
    transfer.updated_at = Utc::now().timestamp();
    save_transfer(cfg, &transfer)
}

pub fn hash_file(path: &Path) -> CloudResult<(u64, String)> {
    let mut input = File::open(path).map_err(|_| "无法读取备份文件")?;
    let mut hasher = Sha256::new();
    let mut size = 0;
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let count = input.read(&mut buffer).map_err(|_| "读取备份失败")?;
        if count == 0 {
            break;
        }
        size += count as u64;
        hasher.update(&buffer[..count]);
    }
    Ok((size, format!("{:x}", hasher.finalize())))
}

pub fn list_remote(cfg: &AppConfig) -> CloudResult<Vec<RemoteBackup>> {
    let value = settings(cfg)?;
    let (value, client) = match Storage::open(cfg, &value)? {
        Storage::OneDrive(drive) => return Ok(remote_objects(drive.list()?)),
        Storage::WebDav(value, client) => (value, client),
    };
    let url = Url::parse(&value.endpoint).map_err(|_| "WebDAV 地址无效")?;
    dav_listing(&value, &client, url, "1")
}

fn dav_listing(
    value: &Settings,
    client: &Client,
    url: Url,
    depth: &str,
) -> CloudResult<Vec<RemoteBackup>> {
    let mut xml = String::new();
    response(request(client, value, Method::from_bytes(b"PROPFIND").unwrap(), url)
        .header("Depth", depth)
        .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
        .body(r#"<?xml version="1.0"?><d:propfind xmlns:d="DAV:"><d:prop><d:getcontentlength/></d:prop></d:propfind>"#)
        .send())?
        .take(4 * 1024 * 1024 + 1)
        .read_to_string(&mut xml)
        .map_err(|_| "无法读取 WebDAV 文件列表")?;
    if xml.len() > 4 * 1024 * 1024 {
        return Err("云端目录过大，请使用独立备份目录".into());
    }
    parse_listing(
        &xml,
        &Url::parse(&value.endpoint).map_err(|_| "WebDAV 地址无效")?,
    )
}

fn parse_listing(xml: &str, base: &Url) -> CloudResult<Vec<RemoteBackup>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut in_response = false;
    let mut field = Vec::new();
    let mut href = String::new();
    let mut size = 0;
    let mut objects = Vec::new();
    let mut root_seen = false;
    loop {
        match reader.read_event().map_err(|_| "WebDAV 文件列表格式无效")? {
            Event::Start(event) => {
                field = event.local_name().as_ref().to_vec();
                if !root_seen {
                    if field != b"multistatus" {
                        return Err(
                            "服务响应不是 WebDAV 目录，请检查是否误填了网页登录或分享链接".into(),
                        );
                    }
                    root_seen = true;
                }
                if field == b"response" {
                    in_response = true;
                    href.clear();
                    size = 0;
                }
            }
            Event::Text(event) if in_response => {
                let value = event.unescape().map_err(|_| "WebDAV 文件名无效")?;
                if field == b"href" {
                    href.push_str(&value);
                }
                if field == b"getcontentlength" {
                    size = value.parse().unwrap_or(0);
                }
            }
            Event::End(event) => {
                if event.local_name().as_ref() == b"response" {
                    if let Ok(url) = base.join(&href) {
                        if url.origin() == base.origin()
                            && url.path().rsplit_once('/').map(|(p, _)| format!("{p}/"))
                                == Some(base.path().to_string())
                        {
                            let encoded = url
                                .path_segments()
                                .and_then(|mut values| values.next_back())
                                .unwrap_or("");
                            let name = url_escape::decode(encoded).into_owned();
                            if valid_backup_name(&name)
                                || name
                                    .strip_suffix(".meta.json")
                                    .is_some_and(valid_backup_name)
                            {
                                objects.push((name, size));
                            }
                        }
                    }
                    in_response = false;
                }
                field.clear();
            }
            Event::Empty(event) if !root_seen => {
                if event.local_name().as_ref() != b"multistatus" {
                    return Err("服务响应不是 WebDAV 目录".into());
                }
                root_seen = true;
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if !root_seen {
        return Err("WebDAV 目录响应为空".into());
    }
    Ok(remote_objects(objects))
}

fn remote_objects(objects: Vec<(String, u64)>) -> Vec<RemoteBackup> {
    let names: HashSet<_> = objects.iter().map(|(name, _)| name.clone()).collect();
    let mut result: Vec<_> = objects
        .into_iter()
        .filter(|(name, _)| valid_backup_name(name))
        .map(|(name, size)| RemoteBackup {
            has_manifest: names.contains(&format!("{name}.meta.json")),
            name,
            size,
        })
        .collect();
    result.sort_by(|a, b| b.name.cmp(&a.name));
    result
}

fn upload(cfg: &AppConfig, value: &Settings, transfer: &mut Transfer) -> CloudResult<String> {
    let storage = Storage::open(cfg, value)?;
    storage.ensure_folder()?;
    let path = local_file(cfg, &transfer.name)?;
    backup_service::validate_backup_file(&path)?;
    let (size, sha256) = hash_file(&path)?;
    transfer.size = size;
    let manifest = RemoteManifest {
        name: transfer.name.clone(),
        size,
        sha256,
        app_version: backup_service::backup_version(&path).unwrap_or_else(|| "unknown".into()),
        created_at: Utc::now().timestamp(),
    };
    let metadata_name = format!("{}.meta.json", transfer.name);
    if let Some(existing) = storage.get(&metadata_name)? {
        let previous: RemoteManifest = serde_json::from_reader(existing.take(64 * 1024))
            .map_err(|_| "云端同名备份的校验清单无效，未覆盖")?;
        if previous.name != manifest.name
            || previous.size != size
            || previous.sha256 != manifest.sha256
        {
            return Err("云端存在同名但内容不同的备份，未覆盖".into());
        }
        if storage.size(&transfer.name)? != Some(size) {
            return Err("云端同名备份的大小异常，未覆盖，请人工检查".into());
        }
        return Ok("云端已存在相同备份，远端大小已核对；取回时执行完整校验".into());
    }
    // Publish the manifest last; incomplete remote uploads are not offered for restore.
    storage.put_file(&transfer.name, &path, size, &manifest.sha256, &transfer.id)?;
    let remote_size = storage.size(&transfer.name)?;
    if remote_size != Some(size) {
        return Err("上传后云端文件大小不一致，未发布恢复清单".into());
    }
    storage.put_bytes(
        &metadata_name,
        serde_json::to_vec(&manifest).map_err(|_| "校验清单编码失败")?,
    )?;
    Ok("已上传，远端大小已核对；取回时将执行完整 SHA-256 和备份内容校验".into())
}

fn download(cfg: &AppConfig, value: &Settings, transfer: &mut Transfer) -> CloudResult<String> {
    let storage = Storage::open(cfg, value)?;
    let mut bytes = Vec::new();
    storage
        .get(&format!("{}.meta.json", transfer.name))?
        .ok_or("云端备份缺少校验清单，不能取回")?
        .take(64 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "无法读取云端校验清单")?;
    if bytes.len() > 64 * 1024 {
        return Err("云端校验清单过大".into());
    }
    let manifest: RemoteManifest =
        serde_json::from_slice(&bytes).map_err(|_| "云端校验清单无效")?;
    if manifest.name != transfer.name
        || manifest.size == 0
        || manifest.size == u64::MAX
        || manifest.sha256.len() != 64
        || !manifest.sha256.bytes().all(|v| v.is_ascii_hexdigit())
    {
        return Err("云端校验清单与备份不匹配".into());
    }
    let temporary_dir = cfg.backup_dir().join(".cloud-downloads").join(&transfer.id);
    fs::create_dir_all(&temporary_dir).map_err(|_| "无法创建取回暂存目录")?;
    let temporary = temporary_dir.join(&transfer.name);
    let result = (|| {
        let mut input = storage.get(&transfer.name)?.ok_or("云端备份文件不存在")?;
        let mut output =
            File::create(&temporary).map_err(|_| "无法创建取回文件，请检查磁盘空间")?;
        let copied = std::io::copy(&mut input.by_ref().take(manifest.size + 1), &mut output)
            .map_err(|_| "取回文件失败，请检查网络和磁盘空间")?;
        output.sync_all().map_err(|_| "取回文件落盘失败")?;
        if copied != manifest.size {
            return Err("云端备份大小不匹配".into());
        }
        if hash_file(&temporary)?.1 != manifest.sha256 {
            return Err("云端备份 SHA-256 校验失败".into());
        }
        backup_service::validate_backup_file(&temporary)?;
        let _guard = backup_service::DATA_MAINTENANCE_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .map_err(|_| "备份锁失败")?;
        let destination = cfg.backup_dir().join(&transfer.name);
        if destination.exists() {
            if hash_file(&local_file(cfg, &transfer.name)?)?.1 != manifest.sha256 {
                return Err("本地存在同名但内容不同的备份，未覆盖".into());
            }
        } else {
            fs::rename(&temporary, destination).map_err(|_| "无法发布已取回的备份")?;
        }
        transfer.size = manifest.size;
        Ok("取回成功，SHA-256 和备份内容校验通过；可在本地列表下载或恢复".into())
    })();
    let _ = fs::remove_file(temporary);
    let _ = fs::remove_dir(temporary_dir);
    result
}

pub fn process_one(cfg: &AppConfig) -> CloudResult<()> {
    let Ok(_running) = TRANSFER_LOCK.try_lock() else {
        return Ok(());
    };
    let value = settings(cfg)?;
    if !value.enabled {
        return Ok(());
    }
    let mut transfer = {
        let _guard = STATE_LOCK.lock().map_err(|_| "云端状态锁失败")?;
        let mut jobs = read_transfers(cfg)?;
        if INITIALIZED.set(()).is_ok() {
            for job in &mut jobs {
                if job.status == "running" {
                    job.status = "queued".into();
                    job.detail = "程序重启，等待重新传输".into();
                    save_transfer(cfg, job)?;
                }
            }
        }
        let now = Utc::now().timestamp();
        let Some(mut job) = jobs
            .into_iter()
            .rev()
            .find(|j| j.status == "queued" && j.next_attempt_at <= now)
        else {
            return Ok(());
        };
        job.status = "running".into();
        job.attempts += 1;
        job.updated_at = now;
        job.detail = "正在后台传输".into();
        save_transfer(cfg, &job)?;
        job
    };
    let result = if transfer.target != value.target() {
        Err("目标网盘已变更，请恢复原目标配置或取消任务".into())
    } else if transfer.direction == "upload" {
        upload(cfg, &value, &mut transfer)
    } else {
        download(cfg, &value, &mut transfer)
    };
    let _guard = STATE_LOCK.lock().map_err(|_| "云端状态锁失败")?;
    transfer.updated_at = Utc::now().timestamp();
    match result {
        Ok(detail) => {
            transfer.status = "succeeded".into();
            transfer.detail = detail;
        }
        Err(error) => {
            transfer.status = if transfer.attempts < 3 {
                "queued"
            } else {
                "failed"
            }
            .into();
            transfer.next_attempt_at = transfer.updated_at + i64::from(transfer.attempts) * 60;
            transfer.detail = error;
        }
    }
    save_transfer(cfg, &transfer)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> AppConfig {
        let mut cfg = AppConfig::default();
        cfg.db_dir = std::env::temp_dir()
            .join(format!("labflow-cloud-{}", uuid::Uuid::new_v4()))
            .to_string_lossy()
            .into();
        cfg
    }

    #[test]
    fn names_and_urls_cannot_escape_backup_scope_or_expose_credentials() {
        for name in [
            "../workload.zip",
            "workload/evil.zip",
            "jwt_secret",
            "workload.zip?x",
            "workload..zip",
            "CON.zip",
            "lpt1.dump",
        ] {
            assert!(!valid_backup_name(name));
        }
        for name in [
            "labflow_full_20261101_103637.zip",
            "labflow_auto_full_20261101_103637_2.zip",
            "labflow_20261101_103637.dump",
            "workload_full_20261101_103637.zip",
            "workload_full_20261101_103637_2.zip",
            "workload_auto_full_20261101_103637.zip",
            "workload_20261101_103637.dump",
            "workload_auto_20261101_103637_2.dump",
            "workload_full_20261101_103637_2a55814b0e9c4a6fa4021aed962b1c23.zip",
        ] {
            assert!(valid_backup_name(name), "{name}");
        }
        let cfg = config();
        for endpoint in [
            "http://example.com/backup/",
            "https://user:secret@example.com/",
            "https://example.com/?token=secret",
        ] {
            assert!(normalize_settings(
                &cfg,
                SettingsInput {
                    enabled: true,
                    auto_upload: true,
                    endpoint: endpoint.into(),
                    username: "u".into(),
                    password: Some("secret".into()),
                    ..SettingsInput::default()
                }
            )
            .is_err());
        }
    }

    #[test]
    fn webdav_listing_accepts_only_direct_backup_children_and_marks_manifests() {
        let xml = r#"<d:multistatus xmlns:d="DAV:"><d:response><d:href>/dav/workload_full_1.zip</d:href><d:propstat><d:prop><d:getcontentlength>123</d:getcontentlength></d:prop></d:propstat></d:response><d:response><d:href>/dav/workload_full_1.zip.meta.json</d:href></d:response><d:response><d:href>https://evil.test/dav/workload_2.zip</d:href></d:response><d:response><d:href>/dav/sub/workload_3.zip</d:href></d:response></d:multistatus>"#;
        let values = parse_listing(xml, &Url::parse("https://example.com/dav/").unwrap()).unwrap();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].size, 123);
        assert!(values[0].has_manifest);
        assert!(parse_listing(
            "<html><body>login</body></html>",
            &Url::parse("https://example.com/dav/").unwrap()
        )
        .is_err());
    }

    #[test]
    fn jobs_are_durable_deduplicated_and_cancel_releases_local_retention() {
        let cfg = config();
        fs::create_dir_all(cfg.backup_dir()).unwrap();
        fs::write(cfg.backup_dir().join("workload_1.dump"), b"fixture").unwrap();
        update_settings(
            &cfg,
            SettingsInput {
                enabled: true,
                auto_upload: true,
                endpoint: "https://example.com/dav/".into(),
                username: "u".into(),
                password: Some("secret-not-in-view".into()),
                ..SettingsInput::default()
            },
        )
        .unwrap();
        let id = enqueue(&cfg, "workload_1.dump", "upload").unwrap();
        assert!(update_settings(
            &cfg,
            SettingsInput {
                enabled: true,
                auto_upload: true,
                endpoint: "https://other.example.com/backup/".into(),
                username: "u".into(),
                password: None,
                ..SettingsInput::default()
            }
        )
        .is_err());
        assert_eq!(settings(&cfg).unwrap().endpoint, "https://example.com/dav/");
        assert_eq!(id, enqueue(&cfg, "workload_1.dump", "upload").unwrap());
        assert!(protected_names(&cfg).unwrap().contains("workload_1.dump"));
        let later = cfg.backup_dir().join("workload_2.dump");
        fs::write(&later, b"newer").unwrap();
        OpenOptions::new()
            .write(true)
            .open(cfg.backup_dir().join("workload_1.dump"))
            .unwrap()
            .set_times(
                fs::FileTimes::new().set_modified(std::time::UNIX_EPOCH + Duration::from_secs(100)),
            )
            .unwrap();
        OpenOptions::new()
            .write(true)
            .open(&later)
            .unwrap()
            .set_times(
                fs::FileTimes::new().set_modified(std::time::UNIX_EPOCH + Duration::from_secs(200)),
            )
            .unwrap();
        backup_service::cleanup_old_backups(&cfg.backup_dir(), 1);
        assert!(cfg.backup_dir().join("workload_1.dump").exists());
        let view = serde_json::to_string(&status(&cfg).unwrap()).unwrap();
        assert!(!view.contains("secret-not-in-view"));
        change_transfer(&cfg, &id, false).unwrap();
        assert!(!protected_names(&cfg).unwrap().contains("workload_1.dump"));
        backup_service::cleanup_old_backups(&cfg.backup_dir(), 1);
        assert!(!cfg.backup_dir().join("workload_1.dump").exists());
        assert!(later.exists());
        fs::remove_dir_all(cfg.data_dir()).unwrap();
    }

    #[test]
    fn real_http_probe_uses_basic_auth_and_cleans_up_without_exposing_secrets() {
        use base64::Engine;
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let mut stored = Vec::new();
            for expected in ["PROPFIND", "PUT", "GET", "DELETE"] {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut byte = [0u8; 1];
                while !bytes.ends_with(b"\r\n\r\n") {
                    socket.read_exact(&mut byte).unwrap();
                    bytes.push(byte[0]);
                }
                let headers = String::from_utf8(bytes).unwrap();
                assert!(headers.starts_with(expected));
                assert!(headers
                    .to_ascii_lowercase()
                    .contains("authorization: basic "));
                let expected_auth =
                    base64::engine::general_purpose::STANDARD.encode("fixture:test-only-secret");
                assert!(headers.contains(&format!("Basic {expected_auth}")));
                if expected == "PUT" {
                    let length: usize = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .map(|value| value.parse().unwrap())
                        })
                        .unwrap();
                    stored.resize(length, 0);
                    socket.read_exact(&mut stored).unwrap();
                }
                let data = if expected == "GET" {
                    stored.as_slice()
                } else {
                    &[]
                };
                write!(
                    socket,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    data.len()
                )
                .unwrap();
                socket.write_all(data).unwrap();
            }
        });
        let cfg = config();
        update_settings(
            &cfg,
            SettingsInput {
                enabled: true,
                auto_upload: false,
                endpoint: format!("http://127.0.0.1:{port}/backups/"),
                username: "fixture".into(),
                password: Some("saved-old-secret".into()),
                ..SettingsInput::default()
            },
        )
        .unwrap();
        test_current_connection(
            &cfg,
            Some(SettingsInput {
                enabled: true,
                auto_upload: false,
                endpoint: format!("http://127.0.0.1:{port}/backups/"),
                username: "fixture".into(),
                password: Some("test-only-secret".into()),
                ..SettingsInput::default()
            }),
        )
        .unwrap();
        assert_eq!(settings(&cfg).unwrap().password, "saved-old-secret");
        server.join().unwrap();
        fs::remove_dir_all(cfg.data_dir()).unwrap();
    }

    #[test]
    fn old_webdav_configuration_keeps_its_target_and_nutstore_preset_selects_a_child_directory() {
        let cfg = config();
        atomic_json(&root(&cfg).join("settings.json"), &serde_json::json!({
            "enabled":true,"auto_upload":false,"endpoint":"https://example.com/dav/backup/","username":"fixture","password":"old-secret"
        })).unwrap();
        let legacy = settings(&cfg).unwrap();
        assert_eq!(legacy.provider, "webdav");
        assert_eq!(
            legacy.target(),
            format!(
                "{:x}",
                Sha256::digest(b"https://example.com/dav/backup/\nfixture")
            )
        );
        let nutstore = normalize_settings(
            &cfg,
            SettingsInput {
                provider: Some("nutstore".into()),
                enabled: true,
                auto_upload: false,
                endpoint: "https://dav.jianguoyun.com/dav/".into(),
                username: "fixture".into(),
                password: Some("fresh-secret".into()),
                ..SettingsInput::default()
            },
        )
        .unwrap();
        assert_eq!(
            nutstore.endpoint,
            "https://dav.jianguoyun.com/dav/LabFlowBackups/"
        );
        assert_ne!(legacy.target(), nutstore.target());
        fs::remove_dir_all(cfg.data_dir()).unwrap();
    }

    #[test]
    fn gateway_cross_origin_download_redirect_never_forwards_basic_credentials() {
        use std::net::TcpListener;
        fn headers(socket: &mut std::net::TcpStream) -> String {
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut byte = [0u8; 1];
            while !bytes.ends_with(b"\r\n\r\n") {
                socket.read_exact(&mut byte).unwrap();
                bytes.push(byte[0]);
            }
            String::from_utf8(bytes).unwrap().to_ascii_lowercase()
        }
        let cdn = TcpListener::bind("127.0.0.1:0").unwrap();
        let cdn_port = cdn.local_addr().unwrap().port();
        let cdn_server = std::thread::spawn(move || {
            let (mut socket, _) = cdn.accept().unwrap();
            assert!(!headers(&mut socket).contains("authorization:"));
            socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\nmock-data",
                )
                .unwrap();
        });
        let gateway = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = gateway.local_addr().unwrap().port();
        let gateway_server = std::thread::spawn(move || {
            let (mut socket, _) = gateway.accept().unwrap();
            assert!(headers(&mut socket).contains("authorization: basic "));
            write!(socket, "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{cdn_port}/signed-file?sig=mock\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
        });
        let cfg = config();
        update_settings(
            &cfg,
            SettingsInput {
                enabled: true,
                auto_upload: false,
                provider: Some("openlist".into()),
                endpoint: format!("http://127.0.0.1:{port}/backups/"),
                username: "fixture".into(),
                password: Some("private-gateway-password".into()),
                ..SettingsInput::default()
            },
        )
        .unwrap();
        let value = settings(&cfg).unwrap();
        let mut output = Vec::new();
        Storage::open(&cfg, &value)
            .unwrap()
            .get("workload_1.zip")
            .unwrap()
            .unwrap()
            .read_to_end(&mut output)
            .unwrap();
        assert_eq!(output, b"mock-data");
        gateway_server.join().unwrap();
        cdn_server.join().unwrap();
        fs::remove_dir_all(cfg.data_dir()).unwrap();
    }

    #[test]
    fn onedrive_tokens_stay_private_and_device_authorization_is_bound_to_actor_and_setup() {
        let cfg = config();
        update_settings(
            &cfg,
            SettingsInput {
                provider: Some("onedrive".into()),
                enabled: true,
                auto_upload: false,
                endpoint: String::new(),
                username: String::new(),
                onedrive: Some(onedrive::Setup {
                    client_id: "12345678-1234-1234-1234-123456789abc".into(),
                    ..onedrive::Setup::default()
                }),
                ..SettingsInput::default()
            },
        )
        .unwrap();
        let mut value = settings(&cfg).unwrap();
        value.onedrive.access_token = "never-return-access".into();
        value.onedrive.refresh_token = "never-return-refresh".into();
        value.onedrive.account_id = "mock-account-id".into();
        value.onedrive.account = "mock-account".into();
        let identity = value.target();
        value.onedrive.expires_at += 3600;
        assert_eq!(identity, value.target());
        atomic_json(&root(&cfg).join("settings.json"), &value).unwrap();
        let public = serde_json::to_string(&status(&cfg).unwrap()).unwrap();
        assert!(!public.contains("never-return"));
        assert!(public.contains("mock-account"));
        let id = uuid::Uuid::new_v4().to_string();
        let pending = PendingLogin {
            id: id.clone(),
            actor_id: 7,
            target: identity.clone(),
            device: onedrive::DeviceCode {
                device_code: "private-device".into(),
                user_code: "USER-CODE".into(),
                verification_uri: "https://microsoft.com/devicelogin".into(),
                expires_in: 600,
                interval: 5,
            },
            expires_at: Utc::now().timestamp() + 600,
            next_poll_at: Utc::now().timestamp() + 60,
        };
        atomic_json(&root(&cfg).join("onedrive-login.json"), &pending).unwrap();
        assert!(poll_login(&cfg, 8, &id).is_err());
        assert!(!poll_login(&cfg, 7, &id).unwrap());
        value.onedrive.setup.folder = "ChangedDestination".into();
        atomic_json(&root(&cfg).join("settings.json"), &value).unwrap();
        assert!(poll_login(&cfg, 7, &id).is_err());
        assert_ne!(identity, value.target());
        disconnect_onedrive(&cfg).unwrap();
        assert!(!settings(&cfg).unwrap().onedrive.connected());
        let mut refreshed = value.onedrive.clone();
        refreshed.access_token = "stale-inflight-access".into();
        refreshed.refresh_token = "stale-inflight-refresh".into();
        assert!(store_refreshed_auth(&cfg, &value, refreshed.clone()).is_err());
        assert!(settings(&cfg).unwrap().onedrive.refresh_token.is_empty());
        let mut newer = value.clone();
        newer.onedrive.access_token = "newer-access".into();
        newer.onedrive.refresh_token = "newer-refresh".into();
        atomic_json(&root(&cfg).join("settings.json"), &newer).unwrap();
        assert_eq!(
            store_refreshed_auth(&cfg, &value, refreshed)
                .unwrap()
                .refresh_token,
            "newer-refresh"
        );
        assert_eq!(
            settings(&cfg).unwrap().onedrive.refresh_token,
            "newer-refresh"
        );
        fs::remove_dir_all(cfg.data_dir()).unwrap();
    }
}
