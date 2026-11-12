use chrono::{DateTime, Utc};
use reqwest::{
    blocking::{Client, Response},
    header, Method, Url,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::Duration,
};

type DriveResult<T> = std::result::Result<T, String>;
const GRAPH: &str = "https://graph.microsoft.com/v1.0/";
const PART_SIZE: u64 = 10 * 1024 * 1024; // A multiple of Graph's 320 KiB fragment requirement.

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Setup {
    pub client_id: String,
    pub tenant: String,
    pub folder: String,
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            client_id: String::new(),
            tenant: "common".into(),
            folder: "LabFlowBackups".into(),
        }
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub(super) struct Auth {
    #[serde(default)]
    pub setup: Setup,
    #[serde(default)]
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: String,
    #[serde(default)]
    pub expires_at: i64,
    #[serde(default)]
    pub account_id: String,
    #[serde(default)]
    pub account: String,
}

impl Auth {
    pub fn connected(&self) -> bool {
        !self.access_token.is_empty()
            && !self.refresh_token.is_empty()
            && !self.account_id.is_empty()
    }
}

#[derive(Serialize, Deserialize)]
pub(super) struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: i64,
    #[serde(default = "poll_interval")]
    pub interval: i64,
}
fn poll_interval() -> i64 {
    5
}

pub(super) enum LoginResult {
    Pending(bool),
    Authorized(Auth),
}

pub fn validate_setup(setup: &mut Setup, enabled: bool) -> DriveResult<()> {
    setup.client_id = setup.client_id.trim().to_string();
    setup.tenant = setup.tenant.trim().to_string();
    setup.folder = setup.folder.trim().trim_matches('/').to_string();
    if setup.tenant.is_empty() {
        setup.tenant = "common".into();
    }
    if setup.folder.is_empty() {
        setup.folder = "LabFlowBackups".into();
    }
    if (enabled && uuid::Uuid::parse_str(&setup.client_id).is_err())
        || setup.tenant.len() > 255
        || setup.tenant.starts_with('.')
        || setup.tenant.ends_with('.')
        || setup.tenant.contains("..")
        || !setup
            .tenant
            .bytes()
            .all(|v| v.is_ascii_alphanumeric() || b".-".contains(&v))
    {
        return Err(
            "请输入微软应用的 Client ID；租户使用 common、consumers、organizations 或有效租户标识"
                .into(),
        );
    }
    if setup.folder.len() > 400
        || setup.folder.split('/').any(|part| {
            part.is_empty()
                || part == "."
                || part == ".."
                || part.ends_with('.')
                || part.ends_with(' ')
                || part
                    .chars()
                    .any(|v| v.is_control() || "\\:*?\"<>|".contains(v))
        })
    {
        return Err("OneDrive 备份目录应为相对路径，不能包含父目录、反斜杠或非法字符".into());
    }
    Ok(())
}

fn client(timeout: u64) -> DriveResult<Client> {
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("LabFlow/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(timeout))
        .build()
        .map_err(|_| "无法创建微软连接".into())
}

fn json_body(response: Response) -> DriveResult<Value> {
    let mut bytes = Vec::new();
    response
        .take(2 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "无法读取微软响应")?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err("微软响应过大".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "微软响应格式无效".into())
}

fn checked(response: reqwest::Result<Response>, operation: &str) -> DriveResult<Response> {
    let response = response.map_err(|_| format!("{operation}连接失败，请检查服务器网络与证书"))?;
    if !response.status().is_success() {
        let detail = match response.status().as_u16() {
            401 => "授权已失效，请重新授权 OneDrive",
            403 => "应用或账号没有所需权限，请检查 Files.ReadWrite 和 User.Read 授权",
            404 => "目录、文件或上传会话不存在",
            409 => "存在同名冲突，请检查目标目录",
            429 => "微软限制了请求频率，请稍后重试",
            507 => "网盘空间不足",
            _ => "微软服务拒绝请求，请稍后重试或检查账号状态",
        };
        return Err(format!(
            "{operation}失败 HTTP {}：{detail}",
            response.status().as_u16()
        ));
    }
    Ok(response)
}

fn authority(setup: &Setup, action: &str) -> String {
    format!(
        "https://login.microsoftonline.com/{}/oauth2/v2.0/{action}",
        setup.tenant
    )
}

pub(super) fn start_device(setup: &Setup) -> DriveResult<DeviceCode> {
    let response = checked(
        client(60)?
            .post(authority(setup, "devicecode"))
            .form(&[
                ("client_id", setup.client_id.as_str()),
                ("scope", "offline_access Files.ReadWrite User.Read"),
            ])
            .send(),
        "发起微软授权",
    )?;
    let device: DeviceCode =
        serde_json::from_value(json_body(response)?).map_err(|_| "微软设备授权响应无效")?;
    let uri = Url::parse(&device.verification_uri).map_err(|_| "微软授权地址无效")?;
    let host = uri.host_str().unwrap_or("");
    if uri.scheme() != "https"
        || !(host == "microsoft.com"
            || host.ends_with(".microsoft.com")
            || host == "microsoftonline.com"
            || host.ends_with(".microsoftonline.com"))
        || !uri.username().is_empty()
        || uri.password().is_some()
        || uri.fragment().is_some()
        || device.device_code.is_empty()
        || device.user_code.is_empty()
        || device.device_code.len() > 8192
        || device.user_code.len() > 64
        || device.interval <= 0
        || device.interval > 300
        || device.expires_in <= 0
        || device.expires_in > 3600
    {
        return Err("微软返回了非预期的授权地址或有效期".into());
    }
    Ok(device)
}

fn apply_tokens(mut auth: Auth, body: &Value) -> DriveResult<Auth> {
    let access = body
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or("微软未返回访问授权")?;
    let lifetime = body
        .get("expires_in")
        .and_then(Value::as_i64)
        .filter(|v| *v > 0 && *v <= 86400)
        .ok_or("微软授权有效期无效")?;
    if body
        .get("token_type")
        .and_then(Value::as_str)
        .is_some_and(|value| !value.eq_ignore_ascii_case("bearer"))
    {
        return Err("不支持微软返回的授权类型".into());
    }
    auth.access_token = access.into();
    if let Some(refresh) = body.get("refresh_token").and_then(Value::as_str) {
        auth.refresh_token = refresh.into();
    }
    if auth.refresh_token.is_empty() {
        return Err("未获得 offline_access 刷新授权，请检查应用权限并重新授权".into());
    }
    auth.expires_at = Utc::now().timestamp() + lifetime;
    Ok(auth)
}

pub(super) fn poll_device(setup: &Setup, code: &str) -> DriveResult<LoginResult> {
    let response = client(60)?
        .post(authority(setup, "token"))
        .form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("client_id", setup.client_id.as_str()),
            ("device_code", code),
        ])
        .send()
        .map_err(|_| "微软授权连接失败")?;
    let success = response.status().is_success();
    let body = json_body(response)?;
    if !success {
        return match body.get("error").and_then(Value::as_str).unwrap_or("") {
            "authorization_pending" => Ok(LoginResult::Pending(false)),
            "slow_down" => Ok(LoginResult::Pending(true)),
            "authorization_declined" | "access_denied" => Err("该授权已被拒绝，请重新发起".into()),
            "expired_token" => Err("授权码已过期，请重新发起".into()),
            "invalid_client" | "unauthorized_client" => {
                Err("微软应用配置不正确，请检查 Client ID、账号类型和公共客户端设备授权".into())
            }
            _ => Err("微软授权未完成，请检查应用设置或重新发起".into()),
        };
    }
    let mut auth = apply_tokens(
        Auth {
            setup: setup.clone(),
            ..Auth::default()
        },
        &body,
    )?;
    let user = json_body(checked(
        client(60)?
            .get(format!(
                "{GRAPH}me?$select=id,displayName,userPrincipalName"
            ))
            .bearer_auth(&auth.access_token)
            .send(),
        "查询授权账号",
    )?)?;
    auth.account_id = user
        .get("id")
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty())
        .ok_or("微软未返回账号标识")?
        .into();
    auth.account = user
        .get("userPrincipalName")
        .and_then(Value::as_str)
        .or_else(|| user.get("displayName").and_then(Value::as_str))
        .unwrap_or("已授权账号")
        .into();
    Ok(LoginResult::Authorized(auth))
}

pub(super) fn refresh(auth: &Auth) -> DriveResult<Auth> {
    let response = checked(
        client(60)?
            .post(authority(&auth.setup, "token"))
            .form(&[
                ("grant_type", "refresh_token"),
                ("client_id", auth.setup.client_id.as_str()),
                ("refresh_token", auth.refresh_token.as_str()),
                ("scope", "offline_access Files.ReadWrite User.Read"),
            ])
            .send(),
        "刷新微软授权",
    )?;
    apply_tokens(auth.clone(), &json_body(response)?)
}

#[derive(Serialize, Deserialize)]
struct UploadSession {
    url: String,
    name: String,
    size: u64,
    sha256: String,
    target: String,
    expires_at: i64,
}

pub(super) struct Connection {
    client: Client,
    api: Url,
    auth: Auth,
    state_dir: PathBuf,
}

impl Connection {
    pub fn new(auth: Auth, state_dir: PathBuf) -> DriveResult<Self> {
        if !auth.connected() {
            return Err("请先完成 OneDrive 账号授权".into());
        }
        Ok(Self {
            client: client(1800)?,
            api: Url::parse(GRAPH).unwrap(),
            auth,
            state_dir,
        })
    }

    fn path_url(&self, parts: &[&str], action: &str) -> Url {
        let mut url = self.api.clone();
        {
            let mut segments = url.path_segments_mut().unwrap();
            segments.pop_if_empty().extend([
                "me",
                "drive",
                if parts.is_empty() { "root" } else { "root:" },
            ]);
            for part in parts {
                segments.push(part);
            }
        }
        if !action.is_empty() {
            url.set_path(&format!(
                "{}{}{action}",
                url.path(),
                if parts.is_empty() { "" } else { ":" }
            ));
        }
        url
    }

    fn file_url(&self, name: &str, action: &str) -> Url {
        let mut parts: Vec<_> = self.auth.setup.folder.split('/').collect();
        if !name.is_empty() {
            parts.push(name);
        }
        self.path_url(&parts, action)
    }

    fn request(&self, method: Method, url: Url) -> reqwest::blocking::RequestBuilder {
        self.client
            .request(method, url)
            .bearer_auth(&self.auth.access_token)
    }

    fn metadata(&self, name: &str) -> DriveResult<Option<Value>> {
        let response = self
            .request(Method::GET, self.file_url(name, ""))
            .send()
            .map_err(|_| "查询 OneDrive 文件失败")?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        json_body(checked(Ok(response), "查询 OneDrive 文件")?).map(Some)
    }

    pub fn ensure_folder(&self) -> DriveResult<()> {
        let mut current = Vec::new();
        for name in self.auth.setup.folder.split('/') {
            let parent = current.clone();
            current.push(name);
            let found = self
                .request(Method::GET, self.path_url(&current, ""))
                .send()
                .map_err(|_| "查询 OneDrive 目录失败")?;
            if found.status().is_success() {
                if !json_body(found)?
                    .get("folder")
                    .is_some_and(Value::is_object)
                {
                    return Err("OneDrive 目标路径已有同名文件，未覆盖".into());
                }
            } else if found.status() == reqwest::StatusCode::NOT_FOUND {
                let created = self.request(Method::POST, self.path_url(&parent, "/children"))
                    .json(&json!({"name":name,"folder":{},"@microsoft.graph.conflictBehavior":"fail"})).send();
                checked(created, "创建 OneDrive 备份目录")?;
            } else {
                checked(Ok(found), "查询 OneDrive 目录")?;
            }
        }
        Ok(())
    }

    pub fn file_size(&self, name: &str) -> DriveResult<Option<u64>> {
        Ok(self
            .metadata(name)?
            .and_then(|value| value.get("size").and_then(Value::as_u64)))
    }

    pub fn list(&self) -> DriveResult<Vec<(String, u64)>> {
        let mut url = self.file_url("", "/children");
        url.query_pairs_mut()
            .append_pair("$select", "name,size,file")
            .append_pair("$top", "200");
        let mut result = Vec::new();
        for _ in 0..100 {
            let value = json_body(checked(
                self.request(Method::GET, url).send(),
                "读取 OneDrive 目录",
            )?)?;
            for item in value
                .get("value")
                .and_then(Value::as_array)
                .ok_or("OneDrive 目录响应无效")?
            {
                if item.get("file").is_some_and(Value::is_object) {
                    if let (Some(name), Some(size)) = (
                        item.get("name").and_then(Value::as_str),
                        item.get("size").and_then(Value::as_u64),
                    ) {
                        result.push((name.to_string(), size));
                    }
                }
            }
            let Some(next) = value.get("@odata.nextLink").and_then(Value::as_str) else {
                return Ok(result);
            };
            url = Url::parse(next).map_err(|_| "OneDrive 分页地址无效")?;
            if url.origin() != self.api.origin() || !url.path().starts_with(self.api.path()) {
                return Err("OneDrive 分页地址不在授权服务范围，已停止".into());
            }
        }
        Err("OneDrive 备份目录过大，请使用专用目录".into())
    }

    fn signed_url(&self, value: &str) -> DriveResult<Url> {
        let url = Url::parse(value).map_err(|_| "微软传输地址无效")?;
        let local_test = cfg!(test) && url.scheme() == "http" && url.origin() == self.api.origin();
        if (!local_test && url.scheme() != "https")
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
        {
            return Err("微软传输地址必须是无内嵌凭据的 HTTPS 地址".into());
        }
        Ok(url)
    }

    pub fn get(&self, name: &str) -> DriveResult<Option<Response>> {
        let Some(item) = self.metadata(name)? else {
            return Ok(None);
        };
        let location = item
            .get("@microsoft.graph.downloadUrl")
            .and_then(Value::as_str)
            .ok_or("OneDrive 未提供文件下载地址")?;
        let mut url = self.signed_url(location)?;
        for _ in 0..5 {
            // Download URLs already carry authorization; never forward the Graph bearer to a CDN.
            let response = self
                .client
                .get(url.clone())
                .send()
                .map_err(|_| "OneDrive 文件下载连接失败")?;
            if response.status().is_redirection() {
                let next = response
                    .headers()
                    .get(header::LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .ok_or("OneDrive 下载跳转无效")?;
                url = self.signed_url(
                    url.join(next)
                        .map_err(|_| "OneDrive 下载地址无效")?
                        .as_str(),
                )?;
            } else {
                return checked(Ok(response), "下载 OneDrive 文件").map(Some);
            }
        }
        Err("OneDrive 下载跳转过多".into())
    }

    pub fn put_bytes(&self, name: &str, data: Vec<u8>) -> DriveResult<()> {
        checked(
            self.request(Method::PUT, self.file_url(name, "/content"))
                .header(header::CONTENT_TYPE, "application/octet-stream")
                .body(data)
                .send(),
            "上传 OneDrive 小文件",
        )?;
        Ok(())
    }

    pub fn delete_probe(&self, name: &str) -> DriveResult<()> {
        checked(
            self.request(Method::DELETE, self.file_url(name, "")).send(),
            "删除 OneDrive 测试文件",
        )?;
        Ok(())
    }

    pub fn put_file(
        &self,
        name: &str,
        file: &Path,
        size: u64,
        sha256: &str,
        id: &str,
    ) -> DriveResult<()> {
        if size <= 4 * 1024 * 1024 {
            checked(
                self.request(Method::PUT, self.file_url(name, "/content"))
                    .header(header::CONTENT_TYPE, "application/octet-stream")
                    .body(reqwest::blocking::Body::sized(
                        File::open(file).map_err(|_| "无法读取备份")?,
                        size,
                    ))
                    .send(),
                "上传 OneDrive 备份",
            )?;
            return Ok(());
        }
        let id = uuid::Uuid::parse_str(id).map_err(|_| "传输任务编号无效")?;
        let session_path = self
            .state_dir
            .join("onedrive-sessions")
            .join(format!("{id}.json"));
        let target = format!(
            "{}\n{}\n{}\n{}",
            self.auth.setup.client_id,
            self.auth.setup.tenant,
            self.auth.account_id,
            self.auth.setup.folder
        );
        let existing: Option<UploadSession> = File::open(&session_path)
            .ok()
            .and_then(|file| serde_json::from_reader(file).ok());
        let mut session = existing.filter(|value| {
            value.name == name
                && value.size == size
                && value.sha256 == sha256
                && value.target == target
                && value.expires_at > Utc::now().timestamp()
        });
        let mut offset = 0;
        if let Some(value) = &session {
            let status = self
                .client
                .get(self.signed_url(&value.url)?)
                .send()
                .map_err(|_| "查询 OneDrive 断点失败")?;
            if matches!(status.status().as_u16(), 404 | 410) {
                session = None;
            } else {
                offset = expected_offset(
                    &json_body(checked(Ok(status), "查询 OneDrive 断点")?)?,
                    size,
                )?;
            }
        }
        let mut session = match session {
            Some(value) => value,
            None => {
                let value = json_body(checked(self.request(Method::POST, self.file_url(name, "/createUploadSession"))
                    .json(&json!({"item":{"@microsoft.graph.conflictBehavior":"replace","name":name}})).send(), "创建 OneDrive 分片会话")?)?;
                let session = UploadSession {
                    url: value
                        .get("uploadUrl")
                        .and_then(Value::as_str)
                        .ok_or("微软未返回分片地址")?
                        .into(),
                    name: name.into(),
                    size,
                    sha256: sha256.into(),
                    target,
                    expires_at: expiry(&value)?,
                };
                self.signed_url(&session.url)?;
                super::cloud_backup_service::atomic_json(&session_path, &session)?;
                session
            }
        };
        let url = self.signed_url(&session.url)?;
        let mut input = File::open(file).map_err(|_| "无法读取分片来源")?;
        while offset < size {
            let length = PART_SIZE.min(size - offset);
            input
                .seek(SeekFrom::Start(offset))
                .map_err(|_| "无法定位备份分片")?;
            let mut bytes = Vec::with_capacity(length as usize);
            input
                .by_ref()
                .take(length)
                .read_to_end(&mut bytes)
                .map_err(|_| "无法读取备份分片")?;
            if bytes.len() as u64 != length {
                return Err("备份文件在传输时发生变化".into());
            }
            // The upload URL is preauthenticated and must not receive a Bearer header.
            let response = checked(
                self.client
                    .put(url.clone())
                    .header(
                        header::CONTENT_RANGE,
                        format!("bytes {}-{}/{}", offset, offset + length - 1, size),
                    )
                    .header(header::CONTENT_LENGTH, length)
                    .header(header::CONTENT_TYPE, "application/octet-stream")
                    .body(bytes)
                    .send(),
                "上传 OneDrive 分片",
            )?;
            if matches!(response.status().as_u16(), 200 | 201) {
                if offset + length != size {
                    return Err("OneDrive 提前结束了分片会话，未发布备份清单".into());
                }
                let _ = fs::remove_file(&session_path);
                return Ok(());
            }
            let value = json_body(response)?;
            let next = expected_offset(&value, size)?;
            if next <= offset || next != offset + length {
                return Err("OneDrive 返回了异常分片范围，请重试任务查询断点".into());
            }
            offset = next;
            session.expires_at = expiry(&value)?;
            super::cloud_backup_service::atomic_json(&session_path, &session)?;
        }
        Err("OneDrive 尚未确认最终分片，请重试任务确认云端状态".into())
    }
}

fn expiry(value: &Value) -> DriveResult<i64> {
    DateTime::parse_from_rfc3339(
        value
            .get("expirationDateTime")
            .and_then(Value::as_str)
            .ok_or("分片会话缺少有效期")?,
    )
    .map(|date| date.timestamp())
    .map_err(|_| "分片会话有效期无效".into())
}

fn expected_offset(value: &Value, size: u64) -> DriveResult<u64> {
    value
        .get("nextExpectedRanges")
        .and_then(Value::as_array)
        .and_then(|ranges| ranges.first())
        .and_then(Value::as_str)
        .and_then(|range| range.split('-').next())
        .and_then(|start| start.parse().ok())
        .filter(|offset| *offset <= size)
        .ok_or_else(|| "OneDrive 断点范围无效".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, net::TcpListener, thread::JoinHandle};

    struct Wire {
        method: String,
        path: String,
        headers: String,
        body: Vec<u8>,
    }

    fn server(
        count: usize,
        mut handler: impl FnMut(usize, &str, &Wire) -> (u16, Vec<u8>, Vec<(String, String)>)
            + Send
            + 'static,
    ) -> (Url, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!(
            "http://127.0.0.1:{}/",
            listener.local_addr().unwrap().port()
        );
        let result = Url::parse(&format!("{base}v1.0/")).unwrap();
        let task = std::thread::spawn(move || {
            for index in 0..count {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(30)))
                    .unwrap();
                let mut header = Vec::new();
                let mut byte = [0u8; 1];
                while !header.ends_with(b"\r\n\r\n") {
                    socket.read_exact(&mut byte).unwrap();
                    header.push(byte[0]);
                }
                let header = String::from_utf8(header).unwrap();
                let mut line = header.lines().next().unwrap().split_whitespace();
                let method = line.next().unwrap().to_string();
                let path = line.next().unwrap().to_string();
                let length = header
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .and_then(|v| v.parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                let mut body = vec![0u8; length];
                socket.read_exact(&mut body).unwrap();
                let request = Wire {
                    method,
                    path,
                    headers: header.to_ascii_lowercase(),
                    body,
                };
                let (status, bytes, headers) = handler(index, &base, &request);
                write!(
                    socket,
                    "HTTP/1.1 {status} OK\r\nContent-Length: {}\r\nConnection: close\r\n",
                    bytes.len()
                )
                .unwrap();
                for (name, value) in headers {
                    write!(socket, "{name}: {value}\r\n").unwrap();
                }
                socket.write_all(b"\r\n").unwrap();
                socket.write_all(&bytes).unwrap();
            }
        });
        (result, task)
    }

    fn auth() -> Auth {
        Auth {
            setup: Setup {
                client_id: "12345678-1234-1234-1234-123456789abc".into(),
                ..Setup::default()
            },
            access_token: "mock-graph-bearer".into(),
            refresh_token: "mock-refresh-private".into(),
            expires_at: Utc::now().timestamp() + 3600,
            account_id: "fixture-account".into(),
            account: "mock-account".into(),
        }
    }

    #[test]
    fn scope_validation_rejects_paths_and_refresh_requires_offline_authorization() {
        for folder in [
            "../other",
            "/../other",
            "folder\\other",
            "folder:bad",
            "one//two",
            "one/./two",
        ] {
            let mut setup = Setup {
                folder: folder.into(),
                ..auth().setup
            };
            assert!(validate_setup(&mut setup, true).is_err(), "{folder}");
        }
        let mut setup = auth().setup;
        validate_setup(&mut setup, true).unwrap();
        assert!(apply_tokens(
            Auth::default(),
            &json!({"access_token":"access","expires_in":3600})
        )
        .is_err());
        let refreshed = apply_tokens(
            auth(),
            &json!({"access_token":"new-access","expires_in":3600,"token_type":"Bearer"}),
        )
        .unwrap();
        assert_eq!(refreshed.refresh_token, "mock-refresh-private");
        assert_eq!(refreshed.account_id, "fixture-account");
        assert!(expected_offset(&json!({"nextExpectedRanges":["999-"]}), 100).is_err());
    }

    #[test]
    fn graph_folder_small_upload_listing_and_cdn_redirect_never_leak_bearer() {
        let (api, task) = server(9, |index, base, request| {
            let graph = !request.path.starts_with("/cdn");
            if request.method == "PUT" {
                assert!(request
                    .headers
                    .contains("content-type: application/octet-stream"));
            }
            assert_eq!(
                request
                    .headers
                    .contains("authorization: bearer mock-graph-bearer"),
                graph
            );
            let value = match index {
                0 => {
                    assert_eq!(request.method, "GET");
                    return (404, Vec::new(), vec![]);
                }
                1 => {
                    assert_eq!(request.method, "POST");
                    assert_eq!(request.path, "/v1.0/me/drive/root/children");
                    let body: Value = serde_json::from_slice(&request.body).unwrap();
                    assert_eq!(body["name"], "LabFlowBackups");
                    json!({"folder":{}})
                }
                2 => {
                    assert_eq!(request.method, "PUT");
                    assert!(request.path.ends_with("probe.txt:/content"));
                    assert_eq!(request.body, b"probe-data");
                    json!({"size":10})
                }
                3 | 7 => {
                    json!({"size":10,"@microsoft.graph.downloadUrl":format!("{base}cdn/redirect")})
                }
                4 => {
                    assert_eq!(request.path, "/cdn/redirect");
                    return (302, vec![], vec![("Location".into(), "/cdn/final".into())]);
                }
                5 => {
                    assert_eq!(request.path, "/cdn/final");
                    return (200, b"probe-data".to_vec(), vec![]);
                }
                6 => {
                    assert_eq!(request.method, "DELETE");
                    return (204, vec![], vec![]);
                }
                8 => {
                    assert!(request.path.contains(":/children?"));
                    json!({"value":[{"name":"workload.zip","size":99,"file":{}},{"name":"workload.zip.meta.json","size":10,"file":{}},{"name":"not-a-file.zip","size":0,"folder":{},"file":null}]})
                }
                _ => unreachable!(),
            };
            (200, serde_json::to_vec(&value).unwrap(), vec![])
        });
        let root = std::env::temp_dir().join(format!("onedrive-test-{}", uuid::Uuid::new_v4()));
        let mut connection = Connection::new(auth(), root).unwrap();
        connection.api = api;
        connection.ensure_folder().unwrap();
        connection
            .put_bytes("probe.txt", b"probe-data".to_vec())
            .unwrap();
        let mut bytes = Vec::new();
        connection
            .get("probe.txt")
            .unwrap()
            .unwrap()
            .read_to_end(&mut bytes)
            .unwrap();
        assert_eq!(bytes, b"probe-data");
        connection.delete_probe("probe.txt").unwrap();
        assert_eq!(connection.file_size("probe.txt").unwrap(), Some(10));
        assert_eq!(connection.list().unwrap().len(), 2);
        task.join().unwrap();
    }

    #[test]
    fn interrupted_large_upload_resumes_from_server_range_without_reuploading_first_part() {
        let expiry = (Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
        let total = PART_SIZE + 123;
        let (api, task) = server(5, move |index, base, request| {
            if index == 0 {
                assert_eq!(request.method, "POST");
                assert!(request
                    .headers
                    .contains("authorization: bearer mock-graph-bearer"));
                return (200, serde_json::to_vec(&json!({"uploadUrl":format!("{base}upload/session?sig=private-test"),"expirationDateTime":expiry})).unwrap(), vec![]);
            }
            assert!(!request.headers.contains("authorization:"));
            if request.method == "PUT" {
                assert!(request
                    .headers
                    .contains("content-type: application/octet-stream"));
            }
            assert!(request.path.starts_with("/upload/session?"));
            let value = match index {
                1 => {
                    assert_eq!(request.body.len() as u64, PART_SIZE);
                    assert!(request
                        .headers
                        .contains(&format!("content-range: bytes 0-{}/{total}", PART_SIZE - 1)));
                    json!({"nextExpectedRanges":[format!("{PART_SIZE}-")],"expirationDateTime":expiry})
                }
                2 => {
                    assert_eq!(request.body.len(), 123);
                    return (500, Vec::new(), vec![]);
                }
                3 => {
                    assert_eq!(request.method, "GET");
                    json!({"nextExpectedRanges":[format!("{PART_SIZE}-")],"expirationDateTime":expiry})
                }
                4 => {
                    assert_eq!(request.body.len(), 123);
                    assert!(request.headers.contains(&format!(
                        "content-range: bytes {PART_SIZE}-{}/{total}",
                        total - 1
                    )));
                    return (201, b"{}".to_vec(), vec![]);
                }
                _ => unreachable!(),
            };
            (202, serde_json::to_vec(&value).unwrap(), vec![])
        });
        let root = std::env::temp_dir().join(format!("onedrive-resume-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let input = root.join("workload_large.zip");
        fs::write(&input, vec![b'x'; total as usize]).unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let mut first = Connection::new(auth(), root.clone()).unwrap();
        first.api = api.clone();
        assert!(first
            .put_file("workload_large.zip", &input, total, "fixture-sha", &id)
            .is_err());
        assert!(root
            .join("onedrive-sessions")
            .join(format!("{id}.json"))
            .exists());
        let mut restarted = Connection::new(auth(), root.clone()).unwrap();
        restarted.api = api;
        restarted
            .put_file("workload_large.zip", &input, total, "fixture-sha", &id)
            .unwrap();
        assert!(!root
            .join("onedrive-sessions")
            .join(format!("{id}.json"))
            .exists());
        task.join().unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}
