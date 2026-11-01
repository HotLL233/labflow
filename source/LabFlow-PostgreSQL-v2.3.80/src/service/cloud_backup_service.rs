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
    enabled: bool,
    auto_upload: bool,
    endpoint: String,
    username: String,
    password: String,
}

#[derive(Serialize)]
pub struct SettingsView {
    pub enabled: bool,
    pub auto_upload: bool,
    pub endpoint: String,
    pub username: String,
    pub has_password: bool,
}

#[derive(Deserialize)]
pub struct SettingsInput {
    pub enabled: bool,
    pub auto_upload: bool,
    pub endpoint: String,
    pub username: String,
    pub password: Option<String>,
}

impl Settings {
    fn view(&self) -> SettingsView {
        SettingsView {
            enabled: self.enabled,
            auto_upload: self.auto_upload,
            endpoint: self.endpoint.clone(),
            username: self.username.clone(),
            has_password: !self.password.is_empty(),
        }
    }

    fn target(&self) -> String {
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

fn atomic_json(path: &Path, value: &impl Serialize) -> CloudResult<()> {
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
    value.enabled = input.enabled;
    value.auto_upload = input.auto_upload;
    value.endpoint = input.endpoint.trim().to_string();
    value.username = input.username.trim().to_string();
    if let Some(password) = input.password {
        value.password = password;
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

fn connection(value: &Settings) -> CloudResult<Client> {
    if !value.enabled {
        return Err("云端备份尚未启用".into());
    }
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
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
        return Err(format!(
            "网盘返回 HTTP {}，请检查授权、目录权限、配额或单文件限制",
            result.status().as_u16()
        ));
    }
    Ok(result)
}

pub fn test_connection(cfg: &AppConfig) -> CloudResult<()> {
    let value = settings(cfg)?;
    let client = connection(&value)?;
    let name = format!(".labflow-probe-{}", uuid::Uuid::new_v4());
    let url = file_url(&value, &name)?;
    let probe = uuid::Uuid::new_v4().to_string();
    response(
        request(&client, &value, Method::PUT, url.clone())
            .body(probe.clone())
            .send(),
    )?;
    let result: CloudResult<()> = (|| {
        let mut data = Vec::new();
        response(request(&client, &value, Method::GET, url.clone()).send())?
            .take(1024)
            .read_to_end(&mut data)
            .map_err(|_| "测试文件读取失败")?;
        if data != probe.as_bytes() {
            return Err("测试文件内容不一致，连接未通过".into());
        }
        Ok(())
    })();
    let cleanup = response(request(&client, &value, Method::DELETE, url).send());
    result?;
    cleanup.map(|_| ())
}

pub fn valid_backup_name(name: &str) -> bool {
    name.len() <= 180
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
    let client = connection(&value)?;
    let url = Url::parse(&value.endpoint).map_err(|_| "WebDAV 地址无效")?;
    let mut xml = String::new();
    response(request(&client, &value, Method::from_bytes(b"PROPFIND").unwrap(), url.clone())
        .header("Depth", "1")
        .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
        .body(r#"<?xml version="1.0"?><d:propfind xmlns:d="DAV:"><d:prop><d:getcontentlength/></d:prop></d:propfind>"#)
        .send())?
        .take(4 * 1024 * 1024 + 1)
        .read_to_string(&mut xml)
        .map_err(|_| "无法读取 WebDAV 文件列表")?;
    if xml.len() > 4 * 1024 * 1024 {
        return Err("云端目录过大，请使用独立备份目录".into());
    }
    parse_listing(&xml, &url)
}

fn parse_listing(xml: &str, base: &Url) -> CloudResult<Vec<RemoteBackup>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut in_response = false;
    let mut field = Vec::new();
    let mut href = String::new();
    let mut size = 0;
    let mut objects = Vec::new();
    loop {
        match reader.read_event().map_err(|_| "WebDAV 文件列表格式无效")? {
            Event::Start(event) => {
                field = event.local_name().as_ref().to_vec();
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
            Event::Eof => break,
            _ => {}
        }
    }
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
    Ok(result)
}

fn upload(cfg: &AppConfig, value: &Settings, transfer: &mut Transfer) -> CloudResult<String> {
    let client = connection(value)?;
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
    let metadata_url = file_url(value, &format!("{}.meta.json", transfer.name))?;
    let existing = request(&client, value, Method::GET, metadata_url.clone())
        .send()
        .map_err(|_| "无法检查云端同名备份")?;
    if existing.status().is_success() {
        let previous: RemoteManifest = serde_json::from_reader(existing.take(64 * 1024))
            .map_err(|_| "云端同名备份的校验清单无效，未覆盖")?;
        if previous.name != manifest.name
            || previous.size != size
            || previous.sha256 != manifest.sha256
        {
            return Err("云端存在同名但内容不同的备份，未覆盖".into());
        }
        let existing_file = response(
            request(
                &client,
                value,
                Method::HEAD,
                file_url(value, &transfer.name)?,
            )
            .send(),
        )?;
        if existing_file
            .headers()
            .get(header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            != Some(size)
        {
            return Err("云端同名备份的大小异常，未覆盖，请人工检查".into());
        }
        return Ok("云端已存在相同备份，远端大小已核对；取回时执行完整校验".into());
    }
    if existing.status() != reqwest::StatusCode::NOT_FOUND {
        response(Ok(existing))?;
    }
    // Publish the manifest last; incomplete remote uploads are not offered for restore.
    response(
        request(
            &client,
            value,
            Method::PUT,
            file_url(value, &transfer.name)?,
        )
        .body(reqwest::blocking::Body::sized(
            File::open(path).map_err(|_| "无法读取备份")?,
            size,
        ))
        .send(),
    )?;
    let head = response(
        request(
            &client,
            value,
            Method::HEAD,
            file_url(value, &transfer.name)?,
        )
        .send(),
    )?;
    let remote_size = head
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.parse::<u64>().ok());
    if remote_size != Some(size) {
        return Err("上传后云端文件大小不一致，未发布恢复清单".into());
    }
    response(
        request(&client, value, Method::PUT, metadata_url)
            .json(&manifest)
            .send(),
    )?;
    Ok("已上传，远端大小已核对；取回时将执行完整 SHA-256 和备份内容校验".into())
}

fn download(cfg: &AppConfig, value: &Settings, transfer: &mut Transfer) -> CloudResult<String> {
    let client = connection(value)?;
    let mut bytes = Vec::new();
    response(
        request(
            &client,
            value,
            Method::GET,
            file_url(value, &format!("{}.meta.json", transfer.name))?,
        )
        .send(),
    )?
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
        let mut input = response(
            request(
                &client,
                value,
                Method::GET,
                file_url(value, &transfer.name)?,
            )
            .send(),
        )?;
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
        ] {
            assert!(!valid_backup_name(name));
        }
        assert!(valid_backup_name("workload_full_20261031_001122.zip"));
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
                    password: Some("secret".into())
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
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let mut stored = Vec::new();
            for expected in ["PUT", "GET", "DELETE"] {
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
                password: Some("test-only-secret".into()),
            },
        )
        .unwrap();
        test_connection(&cfg).unwrap();
        server.join().unwrap();
        fs::remove_dir_all(cfg.data_dir()).unwrap();
    }
}
