//! Explicitly configured upstreams only. Secrets never leave this module's private store.
use crate::{config::AppConfig, models::ai::*};
use futures_util::StreamExt;
use reqwest::{Client, Response, Url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::Write,
    path::Path,
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::mpsc;

type AiResult<T> = Result<T, String>;
const MAX_RESPONSE: usize = 4 * 1024 * 1024;
pub const MAX_AUDIO: usize = 10 * 1024 * 1024;
static STORE_LOCK: Mutex<()> = Mutex::new(());
static CALLS: LazyLock<Mutex<HashMap<i64, UserCalls>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
struct UserCalls {
    active: bool,
    started: Instant,
    count: usize,
}
pub struct CallLease(i64);
impl Drop for CallLease {
    fn drop(&mut self) {
        if let Ok(mut calls) = CALLS.lock() {
            if let Some(v) = calls.get_mut(&self.0) {
                v.active = false;
            }
        }
    }
}
pub fn acquire_call(user_id: i64, rate: usize) -> AiResult<CallLease> {
    let mut calls = CALLS.lock().map_err(|_| "AI 调用状态不可用")?;
    calls.retain(|_, v| v.active || v.started.elapsed() < Duration::from_secs(60));
    if calls.values().filter(|v| v.active).count() >= 8 {
        return Err("AI 服务正在处理其他请求，请稍后重试".into());
    }
    let v = calls.entry(user_id).or_insert_with(|| UserCalls {
        active: false,
        started: Instant::now(),
        count: 0,
    });
    if v.active {
        return Err("当前账号已有 AI 请求进行中，请等待或取消后重试".into());
    }
    if v.started.elapsed() >= Duration::from_secs(60) {
        v.started = Instant::now();
        v.count = 0;
    }
    if v.count >= rate {
        return Err("当前账号调用过于频繁，请一分钟后重试".into());
    }
    v.count += 1;
    v.active = true;
    Ok(CallLease(user_id))
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Provider {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub url: String,
    pub no_key: bool,
    secret: String,
    pub models: Vec<AiModel>,
    pub default_model: String,
    secret_revision: String,
}
impl Provider {
    fn view(&self) -> AiProviderView {
        AiProviderView {
            id: self.id.clone(),
            name: self.name.clone(),
            protocol: self.protocol.clone(),
            url: self.url.clone(),
            no_key: self.no_key,
            has_secret: !self.secret.is_empty(),
            models: self.models.clone(),
            default_model: self.default_model.clone(),
        }
    }
    fn binding(&self, model: &str) -> String {
        format!(
            "{:x}",
            Sha256::digest(
                format!(
                    "{}\n{}\n{}\n{}\n{}",
                    self.protocol, self.url, model, self.secret_revision, self.no_key
                )
                .as_bytes()
            )
        )
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
struct Speech {
    enabled: bool,
    url: String,
    model: String,
    no_key: bool,
    secret: String,
}
#[derive(Clone, Serialize, Deserialize)]
struct StoredTest {
    binding: String,
    result: AiTestResult,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Settings {
    pub revision: u64,
    pub enabled: bool,
    pub name: String,
    pub default_provider: String,
    pub capabilities: AiCapabilities,
    pub providers: Vec<Provider>,
    speech: Speech,
    tests: Vec<StoredTest>,
    pub timeout_seconds: u64,
    pub max_requests_per_minute: usize,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            revision: 0,
            enabled: false,
            name: "AI 助手".into(),
            default_provider: String::new(),
            capabilities: AiCapabilities::default(),
            providers: vec![],
            speech: Speech::default(),
            tests: vec![],
            timeout_seconds: default_timeout(),
            max_requests_per_minute: default_rate(),
        }
    }
}
impl Settings {
    pub fn view(&self) -> AiConfigView {
        AiConfigView {
            revision: self.revision,
            enabled: self.enabled,
            name: self.name.clone(),
            default_provider: self.default_provider.clone(),
            capabilities: self.capabilities.clone(),
            providers: self.providers.iter().map(Provider::view).collect(),
            speech: AiSpeechView {
                enabled: self.speech.enabled,
                url: self.speech.url.clone(),
                model: self.speech.model.clone(),
                no_key: self.speech.no_key,
                has_secret: !self.speech.secret.is_empty(),
            },
            tests: self
                .tests
                .iter()
                .filter(|t| {
                    self.providers.iter().any(|p| {
                        p.id == t.result.provider_id && p.binding(&t.result.model) == t.binding
                    })
                })
                .map(|t| t.result.clone())
                .collect(),
            timeout_seconds: self.timeout_seconds,
            max_requests_per_minute: self.max_requests_per_minute,
        }
    }
    pub fn provider(&self, id: Option<&str>) -> AiResult<Provider> {
        let id = id.unwrap_or(&self.default_provider);
        self.providers
            .iter()
            .find(|p| p.id == id)
            .cloned()
            .ok_or_else(|| "请先配置并选择模型服务".into())
    }
    pub fn model(&self, p: &Provider, model: Option<&str>) -> AiResult<String> {
        let id = model.unwrap_or(&p.default_model);
        if id.is_empty() || (!p.models.iter().any(|m| m.id == id) && id != p.default_model) {
            return Err("请选择管理员配置的可用模型".into());
        }
        Ok(id.into())
    }
    pub fn tested(&self, p: &Provider, model: &str, kind: &str) -> bool {
        self.tests.iter().any(|t| {
            t.binding == p.binding(model)
                && t.result.provider_id == p.id
                && t.result.kind == kind
                && t.result.passed
                && t.result.usable
        })
    }
    fn speech_config_ready(&self) -> bool {
        self.speech.enabled
            && !self.speech.url.is_empty()
            && !self.speech.model.is_empty()
            && (self.speech.no_key || !self.speech.secret.is_empty())
    }
    pub fn speech_ready(&self) -> bool {
        self.capabilities.voice && self.speech_config_ready()
    }
    pub fn speech_destination(&self) -> String {
        Url::parse(&self.speech.url)
            .ok()
            .and_then(|u| {
                u.host_str().map(|host| match u.port() {
                    Some(port) => format!("{host}:{port}"),
                    None => host.into(),
                })
            })
            .unwrap_or_default()
    }
}
fn config_path(cfg: &AppConfig) -> std::path::PathBuf {
    cfg.data_dir().join("ai-assistant").join("settings.json")
}
fn read_settings(cfg: &AppConfig) -> AiResult<Settings> {
    let path = config_path(cfg);
    if !path.exists() {
        return Ok(Settings::default());
    }
    serde_json::from_reader(File::open(path).map_err(|_| "无法读取 AI 私密配置")?)
        .map_err(|_| "AI 私密配置损坏，请检查服务端文件；为保护配置已停止写入".into())
}
pub(crate) fn settings(cfg: &AppConfig) -> AiResult<Settings> {
    let _guard = STORE_LOCK.lock().map_err(|_| "AI 配置锁不可用")?;
    read_settings(cfg)
}
fn atomic_private_json(path: &Path, value: &impl Serialize) -> AiResult<()> {
    let parent = path.parent().ok_or("无效 AI 配置目录")?;
    fs::create_dir_all(parent).map_err(|_| "无法创建 AI 私密配置目录")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
            .map_err(|_| "无法保护 AI 私密目录")?;
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let system = std::env::var_os("SystemRoot")
            .map(std::path::PathBuf::from)
            .ok_or("无法定位系统权限工具")?
            .join("System32");
        let identity = std::process::Command::new(system.join("whoami.exe"))
            .args(["/user", "/fo", "csv", "/nh"])
            .creation_flags(0x08000000)
            .output()
            .map_err(|_| "无法确认 AI 服务账号")?;
        if !identity.status.success() {
            return Err("无法确认 AI 服务账号".into());
        }
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
                .all(|v| v.is_ascii_digit() || b"S-".contains(&v))
        {
            return Err("无法确认 AI 服务账号".into());
        }
        let access = std::process::Command::new(system.join("icacls.exe"))
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
            .creation_flags(0x08000000)
            .output()
            .map_err(|_| "无法保护 AI 私密目录")?;
        if !access.status.success() {
            return Err("AI 私密目录权限设置失败，配置未保存".into());
        }
    }
    let temp = parent.join(format!(".{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut opts = OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&temp).map_err(|_| "无法保存 AI 私密配置")?;
        serde_json::to_writer(&mut f, value).map_err(|_| "AI 配置编码失败")?;
        f.flush().map_err(|_| "AI 配置写入失败")?;
        f.sync_all().map_err(|_| "AI 配置写入失败")?;
        fs::rename(&temp, path).map_err(|_| "无法提交 AI 私密配置")?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}
fn normalized_url(raw: &str, optional: bool) -> AiResult<String> {
    let raw = raw.trim();
    if raw.is_empty() && optional {
        return Ok(String::new());
    }
    if raw.len() > 2048 {
        return Err("接口地址过长".into());
    }
    let url = Url::parse(raw).map_err(|_| "请输入完整的 HTTP 或 HTTPS 接口地址")?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("接口地址须使用 HTTP/HTTPS，不能包含账号、密钥、查询参数或片段".into());
    }
    Ok(url.as_str().trim_end_matches('/').into())
}
fn valid_text(value: &str, max: usize, label: &str) -> AiResult<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > max || value.chars().any(char::is_control) {
        return Err(format!("{label}为空、过长或包含控制字符"));
    }
    Ok(value.into())
}
fn merge_secret(
    old: &str,
    input: Option<String>,
    clear: bool,
    changed: bool,
    no_key: bool,
) -> AiResult<String> {
    if clear && input.as_ref().is_some_and(|s| !s.is_empty()) {
        return Err("替换密钥和清除密钥不能同时选择".into());
    }
    if no_key || clear {
        return Ok(String::new());
    }
    if let Some(s) = input.filter(|s| !s.is_empty()) {
        if s.len() > 8192 || s.chars().any(char::is_control) {
            return Err("密钥过长或包含控制字符".into());
        }
        return Ok(s);
    }
    if changed && !old.is_empty() {
        return Err("接口地址或协议已变更，请重新填写密钥，或明确选择无密钥".into());
    }
    Ok(old.into())
}
fn merged_settings(old: &Settings, input: AiConfigInput) -> AiResult<Settings> {
    if input.revision != old.revision {
        return Err("AI 配置已被其他管理员修改，请刷新后重试".into());
    }
    if input.providers.len() > 16
        || !(10..=180).contains(&input.timeout_seconds)
        || !(1..=120).contains(&input.max_requests_per_minute)
    {
        return Err("最多配置 16 个服务，超时范围 10–180 秒，每分钟调用上限 1–120".into());
    }
    let mut ids = HashSet::new();
    let mut providers = Vec::new();
    for p in input.providers {
        let id = valid_text(&p.id, 80, "服务标识")?;
        if !id
            .bytes()
            .all(|v| v.is_ascii_alphanumeric() || b"_-".contains(&v))
            || !ids.insert(id.clone())
        {
            return Err("服务标识只能使用字母、数字、下划线和短横线，且不能重复".into());
        }
        if !matches!(p.protocol.as_str(), "openai" | "anthropic" | "ollama") {
            return Err("不支持的模型协议".into());
        }
        let url = normalized_url(&p.url, false)?;
        let previous = old.providers.iter().find(|v| v.id == id);
        let changed = previous.is_some_and(|v| v.url != url || v.protocol != p.protocol);
        let secret = merge_secret(
            previous.map_or("", |v| &v.secret),
            p.secret,
            p.clear_secret,
            changed,
            p.no_key,
        )?;
        let mut model_ids = HashSet::new();
        let mut models = Vec::new();
        if p.models.len() > 500 {
            return Err("每个服务最多配置 500 个可用模型".into());
        }
        for m in p.models {
            let mid = valid_text(&m.id, 256, "模型 ID")?;
            if !model_ids.insert(mid.clone()) {
                continue;
            }
            models.push(AiModel {
                id: mid.clone(),
                name: if m.name.trim().is_empty() {
                    mid
                } else {
                    valid_text(&m.name, 256, "模型名称")?
                },
            });
        }
        let default_model = p.default_model.trim().to_string();
        if !default_model.is_empty() {
            valid_text(&default_model, 256, "默认模型")?;
            if !model_ids.contains(&default_model) {
                models.push(AiModel {
                    id: default_model.clone(),
                    name: default_model.clone(),
                });
            }
        }
        let secret_revision = previous
            .filter(|v| v.secret == secret)
            .map(|v| v.secret_revision.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        providers.push(Provider {
            id,
            name: valid_text(&p.name, 100, "服务名称")?,
            protocol: p.protocol,
            url,
            no_key: p.no_key,
            secret,
            models,
            default_model,
            secret_revision,
        });
    }
    let sp = input.speech;
    let speech_url = normalized_url(&sp.url, !sp.enabled)?;
    let speech = Speech {
        enabled: sp.enabled,
        url: speech_url.clone(),
        model: sp.model.trim().into(),
        no_key: sp.no_key,
        secret: merge_secret(
            &old.speech.secret,
            sp.secret,
            sp.clear_secret,
            speech_url != old.speech.url,
            sp.no_key,
        )?,
    };
    if speech.enabled
        && (speech.model.is_empty()
            || speech.model.len() > 256
            || (!speech.no_key && speech.secret.is_empty()))
    {
        return Err("启用语音前请配置语音模型和密钥，或明确选择无密钥".into());
    }
    let result = Settings {
        revision: old.revision.saturating_add(1),
        enabled: input.enabled,
        name: valid_text(&input.name, 80, "助手名称")?,
        default_provider: input.default_provider,
        capabilities: input.capabilities,
        providers,
        speech,
        tests: old.tests.clone(),
        timeout_seconds: input.timeout_seconds,
        max_requests_per_minute: input.max_requests_per_minute,
    };
    if result.enabled {
        let p = result.provider(None)?;
        result.model(&p, None)?;
        if !p.no_key && p.secret.is_empty() {
            return Err("启用助手前请配置密钥，或明确选择无密钥".into());
        }
    }
    Ok(result)
}
pub fn save_config(cfg: &AppConfig, input: AiConfigInput) -> AiResult<AiConfigView> {
    let _guard = STORE_LOCK.lock().map_err(|_| "AI 配置锁不可用")?;
    let value = merged_settings(&read_settings(cfg)?, input)?;
    atomic_private_json(&config_path(cfg), &value)?;
    Ok(value.view())
}
fn client(timeout: u64) -> AiResult<Client> {
    Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(timeout))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "无法初始化模型连接".into())
}
fn endpoint(base: &str, path: &str) -> String {
    let base = base.trim_end_matches('/');
    let prefix = if path.starts_with("/v1/") && base.ends_with("/v1") {
        "/v1"
    } else if path.starts_with("/api/") && base.ends_with("/api") {
        "/api"
    } else {
        ""
    };
    format!("{base}{}", path.strip_prefix(prefix).unwrap_or(path))
}
fn authorized(p: &Provider, b: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    if p.protocol == "anthropic" {
        let b = b.header("anthropic-version", "2023-06-01");
        if p.no_key || p.secret.is_empty() {
            b
        } else {
            b.header("x-api-key", &p.secret)
        }
    } else if p.no_key || p.secret.is_empty() {
        b
    } else {
        b.bearer_auth(&p.secret)
    }
}
fn response_error(code: u16) -> String {
    match code {
        401 | 403 => "上游拒绝鉴权，请检查密钥、模型授权和账号余额".into(),
        404 | 405 => "上游接口不存在或不支持该操作，请检查协议和基础地址".into(),
        429 => "上游请求限流或额度不足，请稍后重试".into(),
        300..=399 => "上游返回重定向，出于密钥保护未跟随；请填写最终接口地址".into(),
        _ => format!("上游返回 HTTP {code}，未返回其原始正文以保护配置"),
    }
}
fn transport_error(e: reqwest::Error) -> String {
    if e.is_timeout() {
        "上游请求超时，请检查连接或稍后重试".into()
    } else if e.is_connect() {
        "无法连接上游，请检查地址、网络和 TLS 证书".into()
    } else {
        "上游通信失败，请检查服务状态".into()
    }
}
async fn bounded_json(mut r: Response) -> AiResult<Value> {
    if !r.status().is_success() {
        return Err(response_error(r.status().as_u16()));
    }
    let mut body = Vec::new();
    while let Some(chunk) = r.chunk().await.map_err(transport_error)? {
        if body.len() + chunk.len() > MAX_RESPONSE {
            return Err("上游响应超过 4MB 限制".into());
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|_| "上游返回的 JSON 格式不符合所选协议".into())
}
pub async fn discover_models(cfg: &AppConfig, id: &str) -> AiResult<AiModelList> {
    let s = settings(cfg)?;
    let p = s.provider(Some(id))?;
    discover_provider(&p, s.timeout_seconds).await
}
async fn discover_provider(p: &Provider, timeout: u64) -> AiResult<AiModelList> {
    let c = client(timeout)?;
    let mut models = Vec::new();
    let mut ids = HashSet::new();
    let mut cursor = None::<String>;
    let mut truncated = false;
    for page in 0..10 {
        let path = if p.protocol == "ollama" {
            "/api/tags"
        } else {
            "/v1/models"
        };
        let mut request = authorized(p, c.get(endpoint(&p.url, path)));
        if p.protocol == "anthropic" {
            request = request.query(&[("limit", "100")]);
            if let Some(value) = cursor.as_ref() {
                request = request.query(&[("after_id", value)]);
            }
        } else if let Some(value) = cursor.as_ref() {
            request = request.query(&[("after", value)]);
        }
        let value = bounded_json(request.send().await.map_err(transport_error)?).await?;
        let rows = value
            .get(if p.protocol == "ollama" {
                "models"
            } else {
                "data"
            })
            .and_then(Value::as_array)
            .ok_or("上游模型列表结构不符合所选协议")?;
        for m in rows {
            let id = m
                .get(if p.protocol == "ollama" { "name" } else { "id" })
                .and_then(Value::as_str)
                .ok_or("上游模型列表缺少模型标识")?;
            if id.len() > 256 || id.is_empty() {
                continue;
            }
            if ids.insert(id.to_string()) {
                models.push(AiModel {
                    id: id.into(),
                    name: m
                        .get("display_name")
                        .or_else(|| m.get("name"))
                        .and_then(Value::as_str)
                        .unwrap_or(id)
                        .chars()
                        .take(256)
                        .collect(),
                });
            }
            if models.len() >= 500 {
                truncated = true;
                break;
            }
        }
        if truncated {
            break;
        }
        let has_more = value
            .get("has_more")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if !has_more {
            break;
        }
        let next = value.get("last_id").and_then(Value::as_str).or_else(|| {
            rows.last()
                .and_then(|v| v.get("id"))
                .and_then(Value::as_str)
        });
        let Some(next) = next else {
            truncated = true;
            break;
        };
        if cursor.as_deref() == Some(next) || next.len() > 256 || page == 9 {
            truncated = true;
            break;
        }
        cursor = Some(next.into());
    }
    Ok(AiModelList { models, truncated })
}

#[derive(Clone)]
pub(crate) struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}
pub(crate) struct Completion {
    pub text: String,
    pub tools: Vec<ToolCall>,
}
fn tools_for_protocol(protocol: &str, tools: &[Value]) -> Vec<Value> {
    tools.iter().map(|t|if protocol=="anthropic"{let f=&t["function"];json!({"name":f["name"],"description":f["description"],"input_schema":f["parameters"]})}else{t.clone()}).collect()
}
pub(crate) fn request_body(
    p: &Provider,
    model: &str,
    messages: &[Value],
    system: &str,
    tools: &[Value],
    stream: bool,
) -> Value {
    let tools = tools_for_protocol(&p.protocol, tools);
    if p.protocol == "anthropic" {
        let mut v = json!({"model":model,"max_tokens":2048,"system":system,"messages":messages,"stream":stream});
        if !tools.is_empty() {
            v["tools"] = json!(tools);
        }
        v
    } else {
        let mut all = vec![json!({"role":"system","content":system})];
        all.extend_from_slice(messages);
        let mut v = json!({"model":model,"messages":all,"stream":stream});
        if p.protocol == "openai" {
            v["max_tokens"] = json!(2048);
        }
        if !tools.is_empty() {
            v["tools"] = json!(tools);
        }
        v
    }
}
fn chat_path(p: &Provider) -> &'static str {
    match p.protocol.as_str() {
        "anthropic" => "/v1/messages",
        "ollama" => "/api/chat",
        _ => "/v1/chat/completions",
    }
}
fn parse_completion(p: &Provider, v: Value) -> AiResult<Completion> {
    let mut out = Completion {
        text: String::new(),
        tools: vec![],
    };
    if p.protocol == "anthropic" {
        let content = v["content"].as_array().ok_or("上游未返回有效消息")?;
        for c in content {
            match c["type"].as_str() {
                Some("text") => out.text.push_str(c["text"].as_str().unwrap_or("")),
                Some("tool_use") => out.tools.push(ToolCall {
                    id: c["id"].as_str().unwrap_or("").into(),
                    name: c["name"].as_str().unwrap_or("").into(),
                    arguments: c["input"].clone(),
                }),
                _ => {}
            }
        }
    } else {
        let message = if p.protocol == "ollama" {
            &v["message"]
        } else {
            &v["choices"][0]["message"]
        };
        out.text = message["content"].as_str().unwrap_or("").into();
        if let Some(calls) = message["tool_calls"].as_array() {
            for (i, c) in calls.iter().enumerate() {
                let a = &c["function"]["arguments"];
                out.tools.push(ToolCall {
                    id: c["id"]
                        .as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("call_{i}")),
                    name: c["function"]["name"].as_str().unwrap_or("").into(),
                    arguments: if let Some(s) = a.as_str() {
                        serde_json::from_str(s).map_err(|_| "模型工具参数不是有效 JSON")?
                    } else {
                        a.clone()
                    },
                });
            }
        }
    }
    validate_completion(&out)?;
    Ok(out)
}
fn validate_completion(out: &Completion) -> AiResult<()> {
    if out.text.trim().is_empty() && out.tools.is_empty() {
        return Err("上游返回空回复，模型对话未通过".into());
    }
    if out.tools.len() > 6
        || out.tools.iter().any(|t| {
            t.name.is_empty() || t.name.len() > 100 || t.id.len() > 200 || !t.arguments.is_object()
        })
    {
        return Err("模型工具调用结构无效或调用数量超限".into());
    }
    Ok(())
}
async fn complete(
    p: &Provider,
    model: &str,
    messages: &[Value],
    system: &str,
    tools: &[Value],
    timeout: u64,
) -> AiResult<Completion> {
    let c = client(timeout)?;
    let v = bounded_json(
        authorized(p, c.post(endpoint(&p.url, chat_path(p))))
            .json(&request_body(p, model, messages, system, tools, false))
            .send()
            .await
            .map_err(transport_error)?,
    )
    .await?;
    parse_completion(p, v)
}
#[derive(Default)]
struct PartialTool {
    id: String,
    name: String,
    args: String,
    input: Option<Value>,
}
#[derive(Default)]
struct StreamState {
    text: String,
    tools: HashMap<usize, PartialTool>,
    ended: bool,
}
fn parse_stream_line(
    p: &Provider,
    line: &[u8],
    state: &mut StreamState,
) -> AiResult<Option<String>> {
    let line = std::str::from_utf8(line)
        .map_err(|_| "上游流式响应不是有效 UTF-8")?
        .trim();
    if line.is_empty() || line.starts_with(':') || line.starts_with("event:") {
        return Ok(None);
    }
    let data = if p.protocol == "ollama" {
        line
    } else {
        match line.strip_prefix("data:") {
            Some(v) => v.trim(),
            None => return Ok(None),
        }
    };
    if data == "[DONE]" {
        state.ended = true;
        return Ok(None);
    }
    let v: Value = serde_json::from_str(data).map_err(|_| "上游流式分块不是有效 JSON")?;
    if v.get("error").is_some() || v["type"] == "error" {
        return Err("上游在流式输出中返回错误".into());
    }
    let mut delta = String::new();
    match p.protocol.as_str() {
        "anthropic" => match v["type"].as_str() {
            Some("content_block_start") => {
                if v["content_block"]["type"] == "tool_use" {
                    let i = v["index"].as_u64().unwrap_or(0) as usize;
                    let t = state.tools.entry(i).or_default();
                    t.id = v["content_block"]["id"].as_str().unwrap_or("").into();
                    t.name = v["content_block"]["name"].as_str().unwrap_or("").into();
                    t.input = Some(v["content_block"]["input"].clone());
                }
            }
            Some("content_block_delta") => {
                if v["delta"]["type"] == "text_delta" {
                    delta = v["delta"]["text"].as_str().unwrap_or("").into();
                } else if v["delta"]["type"] == "input_json_delta" {
                    state
                        .tools
                        .entry(v["index"].as_u64().unwrap_or(0) as usize)
                        .or_default()
                        .args
                        .push_str(v["delta"]["partial_json"].as_str().unwrap_or(""));
                }
            }
            Some("message_stop") => state.ended = true,
            _ => {}
        },
        "ollama" => {
            delta = v["message"]["content"].as_str().unwrap_or("").into();
            if let Some(calls) = v["message"]["tool_calls"].as_array() {
                for c in calls {
                    let index = state.tools.len();
                    state.tools.insert(
                        index,
                        PartialTool {
                            id: format!("call_{index}"),
                            name: c["function"]["name"].as_str().unwrap_or("").into(),
                            input: Some(c["function"]["arguments"].clone()),
                            args: String::new(),
                        },
                    );
                }
            }
            if v["done"] == true {
                state.ended = true;
            }
        }
        _ => {
            let d = &v["choices"][0]["delta"];
            delta = d["content"].as_str().unwrap_or("").into();
            if let Some(calls) = d["tool_calls"].as_array() {
                for c in calls {
                    let t = state
                        .tools
                        .entry(c["index"].as_u64().unwrap_or(0) as usize)
                        .or_default();
                    t.id.push_str(c["id"].as_str().unwrap_or(""));
                    t.name
                        .push_str(c["function"]["name"].as_str().unwrap_or(""));
                    t.args
                        .push_str(c["function"]["arguments"].as_str().unwrap_or(""));
                }
            }
        }
    }
    if state.tools.len() > 6 {
        return Err("模型单轮工具调用数量超限".into());
    }
    state.text.push_str(&delta);
    if delta.is_empty() {
        Ok(None)
    } else {
        Ok(Some(delta))
    }
}
pub(crate) async fn stream_complete(
    p: &Provider,
    model: &str,
    messages: &[Value],
    system: &str,
    tools: &[Value],
    timeout: u64,
    events: Option<&mpsc::Sender<AiEvent>>,
) -> AiResult<Completion> {
    let c = client(timeout)?;
    let response = authorized(p, c.post(endpoint(&p.url, chat_path(p))))
        .json(&request_body(p, model, messages, system, tools, true))
        .send()
        .await
        .map_err(transport_error)?;
    if !response.status().is_success() {
        return Err(response_error(response.status().as_u16()));
    }
    let mut stream = response.bytes_stream();
    let mut pending = Vec::new();
    let mut total = 0;
    let mut state = StreamState::default();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(transport_error)?;
        total += chunk.len();
        if total > MAX_RESPONSE {
            return Err("上游流式响应超过 4MB 限制".into());
        }
        pending.extend_from_slice(&chunk);
        while let Some(i) = pending.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = pending.drain(..=i).collect();
            if let Some(delta) = parse_stream_line(p, &line, &mut state)? {
                if let Some(tx) = events {
                    tx.send(AiEvent::new("delta", json!({"text":delta})))
                        .await
                        .map_err(|_| "会话已取消")?;
                }
            }
        }
        if pending.len() > 512 * 1024 {
            return Err("上游单个流式分块过大".into());
        }
    }
    if !pending.is_empty() {
        if let Some(delta) = parse_stream_line(p, &pending, &mut state)? {
            if let Some(tx) = events {
                tx.send(AiEvent::new("delta", json!({"text":delta})))
                    .await
                    .map_err(|_| "会话已取消")?;
            }
        }
    }
    if !state.ended {
        return Err("上游流式响应未正常结束，请重试或检查协议".into());
    }
    let mut tools: Vec<_> = state.tools.into_iter().collect();
    tools.sort_by_key(|(i, _)| *i);
    let tools = tools
        .into_iter()
        .map(|(_, t)| {
            Ok(ToolCall {
                id: t.id,
                name: t.name,
                arguments: if t.args.is_empty() {
                    t.input.unwrap_or_else(|| json!({}))
                } else {
                    serde_json::from_str(&t.args).map_err(|_| "模型工具参数不是有效 JSON")?
                },
            })
        })
        .collect::<AiResult<Vec<_>>>()?;
    let out = Completion {
        text: state.text,
        tools,
    };
    validate_completion(&out)?;
    Ok(out)
}
pub(crate) fn append_tool_turn(
    protocol: &str,
    messages: &mut Vec<Value>,
    completion: &Completion,
    results: &[Value],
) {
    if protocol == "anthropic" {
        let mut content = Vec::new();
        if !completion.text.is_empty() {
            content.push(json!({"type":"text","text":completion.text}));
        }
        content.extend(
            completion
                .tools
                .iter()
                .map(|t| json!({"type":"tool_use","id":t.id,"name":t.name,"input":t.arguments})),
        );
        messages.push(json!({"role":"assistant","content":content}));
        messages.push(json!({"role":"user","content":completion.tools.iter().zip(results).map(|(t,r)|json!({"type":"tool_result","tool_use_id":t.id,"content":r.to_string()})).collect::<Vec<_>>()}));
    } else {
        let calls:Vec<_>=completion.tools.iter().map(|t|if protocol=="ollama"{json!({"function":{"name":t.name,"arguments":t.arguments}})}else{json!({"id":t.id,"type":"function","function":{"name":t.name,"arguments":t.arguments.to_string()}})}).collect();
        messages.push(json!({"role":"assistant","content":completion.text,"tool_calls":calls}));
        for (t, r) in completion.tools.iter().zip(results) {
            messages.push(if protocol == "ollama" {
                json!({"role":"tool","tool_name":t.name,"content":r.to_string()})
            } else {
                json!({"role":"tool","tool_call_id":t.id,"content":r.to_string()})
            });
        }
    }
}
pub async fn run_test(cfg: &AppConfig, input: AiTestRequest) -> AiResult<AiTestResult> {
    if !matches!(
        input.kind.as_str(),
        "connectivity" | "hi" | "stream" | "tools"
    ) {
        return Err("未知的测试类型".into());
    }
    let s = settings(cfg)?;
    let p = s.provider(Some(&input.provider_id))?;
    let model = if input.kind == "connectivity" {
        input.model.unwrap_or_else(|| p.default_model.clone())
    } else {
        s.model(&p, input.model.as_deref())?
    };
    let started = Instant::now();
    let mut reachable = false;
    let result: AiResult<String> = if input.kind == "connectivity" {
        let path = if p.protocol == "ollama" {
            "/api/tags"
        } else {
            "/v1/models"
        };
        match authorized(&p, client(s.timeout_seconds)?.get(endpoint(&p.url, path)))
            .send()
            .await
        {
            Ok(r) => {
                reachable = true;
                Ok(format!(
                    "地址可达（HTTP {}）；这不代表密钥或模型可用",
                    r.status().as_u16()
                ))
            }
            Err(e) => Err(transport_error(e)),
        }
    } else if input.kind == "tools" {
        let tools = vec![
            json!({"type":"function","function":{"name":"labflow_ping","description":"Harmless connection test. Call this once with value hi, then report the returned value.","parameters":{"type":"object","properties":{"value":{"type":"string","enum":["hi"]}},"required":["value"],"additionalProperties":false}}}),
        ];
        let mut messages = vec![
            json!({"role":"user","content":"Call labflow_ping with value hi. After its result, reply with its returned status."}),
        ];
        match complete(
            &p,
            &model,
            &messages,
            "This is a harmless tool protocol test.",
            &tools,
            s.timeout_seconds,
        )
        .await
        {
            Ok(first) => {
                reachable = true;
                if first.tools.len() != 1
                    || first.tools[0].name != "labflow_ping"
                    || first.tools[0].arguments != json!({"value":"hi"})
                {
                    Err("模型未正确调用无副作用测试工具".into())
                } else {
                    append_tool_turn(
                        &p.protocol,
                        &mut messages,
                        &first,
                        &[json!({"status":"labflow_tool_ok"})],
                    );
                    match complete(
                        &p,
                        &model,
                        &messages,
                        "Report the tool's returned status.",
                        &[],
                        s.timeout_seconds,
                    )
                    .await
                    {
                        Ok(last)
                            if last.tools.is_empty() && last.text.contains("labflow_tool_ok") =>
                        {
                            Ok("工具调用、参数校验及返回后继续回答已通过".into())
                        }
                        Ok(_) => Err("模型未正确读取工具返回并继续回答".into()),
                        Err(e) => Err(e),
                    }
                }
            }
            Err(e) => Err(e),
        }
    } else {
        let messages = vec![json!({"role":"user","content":"hi"})];
        let result = if input.kind == "stream" {
            stream_complete(
                &p,
                &model,
                &messages,
                "Reply briefly to hi.",
                &[],
                s.timeout_seconds,
                None,
            )
            .await
        } else {
            complete(
                &p,
                &model,
                &messages,
                "Reply briefly to hi.",
                &[],
                s.timeout_seconds,
            )
            .await
        };
        match result {
            Ok(v) => {
                reachable = true;
                Ok(v.text.chars().take(300).collect())
            }
            Err(e) => Err(e),
        }
    };
    let passed = result.is_ok();
    if let Err(ref e) = result {
        reachable |= e.starts_with("上游拒绝")
            || e.starts_with("上游接口")
            || e.starts_with("上游返回")
            || e.starts_with("上游请求限流")
            || e.starts_with("上游在流式");
    }
    let test = AiTestResult {
        provider_id: p.id.clone(),
        model: model.clone(),
        kind: input.kind,
        passed,
        reachable,
        usable: passed && !model.is_empty(),
        message: result.unwrap_or_else(|e| e),
        latency_ms: started.elapsed().as_millis().min(u64::MAX as u128) as u64,
        checked_at: chrono::Utc::now().to_rfc3339(),
    };
    // Connectivity is deliberately never a model usability result.
    let mut test = test;
    if test.kind == "connectivity" {
        test.usable = false;
    }
    let _guard = STORE_LOCK.lock().map_err(|_| "AI 配置锁不可用")?;
    let mut current = read_settings(cfg)?;
    if !current
        .providers
        .iter()
        .any(|v| v.id == p.id && v.binding(&model) == p.binding(&model))
    {
        return Err("测试期间配置发生变更，请使用新配置重测".into());
    }
    current.tests.retain(|v| {
        !(v.result.provider_id == p.id && v.result.model == model && v.result.kind == test.kind)
    });
    current.tests.push(StoredTest {
        binding: p.binding(&model),
        result: test.clone(),
    });
    if current.tests.len() > 300 {
        current.tests.drain(..current.tests.len() - 300);
    }
    atomic_private_json(&config_path(cfg), &current)?;
    Ok(test)
}

pub(crate) struct AiEvent {
    pub name: &'static str,
    pub data: Value,
}
impl AiEvent {
    pub fn new(name: &'static str, data: Value) -> Self {
        Self { name, data }
    }
}

pub async fn transcribe(
    cfg: &AppConfig,
    data: Vec<u8>,
    mime: &str,
    filename: &str,
    admin_test: bool,
) -> AiResult<String> {
    if data.is_empty() || data.len() > MAX_AUDIO {
        return Err("音频为空或超过 10MB，请上传不超过 60 秒的短音频".into());
    }
    validate_audio(&data, mime)?;
    let s = settings(cfg)?;
    if (!s.enabled || !s.speech_ready()) && !admin_test || !s.speech_config_ready() {
        return Err("管理员尚未启用语音输入".into());
    }
    let ext = filename.rsplit('.').next().unwrap_or("webm");
    let ext = if matches!(ext, "webm" | "wav" | "mp3" | "mp4" | "m4a" | "ogg" | "flac") {
        ext
    } else {
        "webm"
    };
    let part = reqwest::multipart::Part::bytes(data)
        .file_name(format!("audio.{ext}"))
        .mime_str(mime)
        .map_err(|_| "不支持的音频类型")?;
    let form = reqwest::multipart::Form::new()
        .text("model", s.speech.model.clone())
        .text("response_format", "json")
        .part("file", part);
    let mut request = client(s.timeout_seconds)?
        .post(endpoint(&s.speech.url, "/v1/audio/transcriptions"))
        .multipart(form);
    if !s.speech.no_key && !s.speech.secret.is_empty() {
        request = request.bearer_auth(&s.speech.secret);
    }
    let v = bounded_json(request.send().await.map_err(transport_error)?).await?;
    let text = v["text"]
        .as_str()
        .filter(|v| !v.trim().is_empty())
        .ok_or("语音服务未返回有效识别文本")?;
    if text.len() > 32000 {
        return Err("语音识别文本过长".into());
    }
    Ok(text.into())
}
fn validate_audio(data: &[u8], mime: &str) -> AiResult<()> {
    let mime = mime.split(';').next().unwrap_or("").trim();
    let valid = match mime {
        "audio/webm" | "video/webm" => data.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]),
        "audio/wav" | "audio/x-wav" | "audio/wave" => {
            data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WAVE")
        }
        "audio/mpeg" | "audio/mp3" => {
            data.starts_with(b"ID3")
                || data
                    .get(..2)
                    .is_some_and(|v| v[0] == 0xff && v[1] & 0xe0 == 0xe0)
        }
        "audio/mp4" | "video/mp4" | "audio/x-m4a" => data.get(4..8) == Some(b"ftyp"),
        "audio/ogg" | "application/ogg" => data.starts_with(b"OggS"),
        "audio/flac" | "audio/x-flac" => data.starts_with(b"fLaC"),
        _ => false,
    };
    if valid {
        if audio_duration_seconds(data)
            .is_some_and(|seconds| !seconds.is_finite() || seconds > 60.5)
        {
            return Err("音频超过 60 秒，请截取短音频后重试".into());
        }
        Ok(())
    } else {
        Err("音频类型或文件内容不受支持，请使用 WebM、WAV、MP3、MP4/M4A、OGG 或 FLAC".into())
    }
}

// Read container metadata without decoding or executing an external program.
// Some streaming WebM/MP3 files have no duration metadata; their 10MB bound and
// the browser's recording timer remain enforced independently.
fn audio_duration_seconds(data: &[u8]) -> Option<f64> {
    if data.starts_with(b"fLaC") && data.get(4).is_some_and(|v| v & 0x7f == 0) {
        let packed = u64::from_be_bytes(data.get(18..26)?.try_into().ok()?);
        let rate = packed >> 44;
        let samples = packed & 0x0f_ffff_ffff;
        if rate > 0 && samples > 0 {
            return Some(samples as f64 / rate as f64);
        }
    }
    if data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WAVE") {
        let mut pos = 12usize;
        let mut rate = None;
        let mut payload = None;
        while pos.checked_add(8)? <= data.len() {
            let length = u32::from_le_bytes(data.get(pos + 4..pos + 8)?.try_into().ok()?) as usize;
            if data.get(pos..pos + 4) == Some(b"fmt ") && length >= 16 {
                rate =
                    Some(u32::from_le_bytes(data.get(pos + 16..pos + 20)?.try_into().ok()?) as f64);
            } else if data.get(pos..pos + 4) == Some(b"data") {
                payload = Some(length as f64);
            }
            pos = pos
                .checked_add(8)?
                .checked_add(length)?
                .checked_add(length % 2)?;
        }
        if let (Some(rate), Some(length)) = (rate, payload) {
            if rate > 0.0 {
                return Some(length / rate);
            }
        }
    }
    if data.get(4..8) == Some(b"ftyp") {
        // mvhd is nested in moov; checking its version and fixed fields is enough
        // to reject known long recordings without trusting a client duration.
        for (i, _) in data.windows(4).enumerate().filter(|(_, v)| *v == b"mvhd") {
            let version = *data.get(i + 4)?;
            let (scale, duration) = if version == 0 {
                (
                    u32::from_be_bytes(data.get(i + 16..i + 20)?.try_into().ok()?) as f64,
                    u32::from_be_bytes(data.get(i + 20..i + 24)?.try_into().ok()?) as f64,
                )
            } else if version == 1 {
                (
                    u32::from_be_bytes(data.get(i + 24..i + 28)?.try_into().ok()?) as f64,
                    u64::from_be_bytes(data.get(i + 28..i + 36)?.try_into().ok()?) as f64,
                )
            } else {
                continue;
            };
            if scale > 0.0 {
                return Some(duration / scale);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    fn provider(protocol: &str) -> Provider {
        Provider {
            id: "test".into(),
            name: "测试".into(),
            protocol: protocol.into(),
            url: "http://127.0.0.1:1/v1".into(),
            no_key: false,
            secret: "never_return_this".into(),
            models: vec![],
            default_model: "model".into(),
            secret_revision: "revision".into(),
        }
    }
    #[test]
    fn secret_replacement_is_explicit() {
        assert_eq!(
            merge_secret("old", Some(String::new()), false, false, false).unwrap(),
            "old"
        );
        assert!(merge_secret("old", None, false, true, false).is_err());
        assert_eq!(merge_secret("old", None, true, true, false).unwrap(), "");
        assert_eq!(
            merge_secret("old", Some("new".into()), false, true, false).unwrap(),
            "new"
        );
        assert!(merge_secret("old", Some("new".into()), true, false, false).is_err());
    }
    #[test]
    fn public_config_does_not_expose_credentials() {
        let p = provider("openai");
        let view = serde_json::to_string(&p.view()).unwrap();
        assert!(!view.contains("never_return_this"));
        assert!(!view.contains("revision"));
        assert!(view.contains("has_secret"));
    }
    #[test]
    fn urls_and_endpoints_are_bounded() {
        assert!(normalized_url("https://user:pass@x/v1", false).is_err());
        assert!(normalized_url("https://x?secret=test", false).is_err());
        assert!(normalized_url("file:///etc/passwd", false).is_err());
        assert_eq!(
            endpoint("https://x/v1/", "/v1/models"),
            "https://x/v1/models"
        );
        assert_eq!(
            endpoint("http://localhost:11434/api", "/api/chat"),
            "http://localhost:11434/api/chat"
        );
    }
    #[test]
    fn streams_require_real_completion_and_tool_json() {
        let p = provider("openai");
        let mut s = StreamState::default();
        parse_stream_line(
            &p,
            b"data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}",
            &mut s,
        )
        .unwrap();
        assert_eq!(s.text, "hi");
        assert!(!s.ended);
        parse_stream_line(&p, b"data: [DONE]", &mut s).unwrap();
        assert!(s.ended);
        assert!(parse_completion(&p, json!({"choices":[{"message":{"content":""}}]})).is_err());
        assert!(parse_completion(&p,json!({"choices":[{"message":{"tool_calls":[{"function":{"name":"x","arguments":"[]"}}]}}]})).is_err());
    }
    #[test]
    fn protocol_parsing_and_private_binding() {
        let mut p = provider("anthropic");
        let v = parse_completion(
            &p,
            json!({"content":[{"type":"tool_use","id":"a","name":"safe","input":{"x":1}}]}),
        )
        .unwrap();
        assert_eq!(v.tools[0].name, "safe");
        let old = p.binding("model");
        p.secret_revision = "changed".into();
        assert_ne!(old, p.binding("model"));
        assert_ne!(p.binding("model"), p.binding("other"));
    }
    #[test]
    fn tool_results_use_only_each_protocols_fields() {
        let completion = Completion {
            text: String::new(),
            tools: vec![ToolCall {
                id: "a".into(),
                name: "ping".into(),
                arguments: json!({"value":"hi"}),
            }],
        };
        let mut messages = vec![];
        append_tool_turn("openai", &mut messages, &completion, &[json!({"ok":true})]);
        assert_eq!(messages[1]["tool_call_id"], "a");
        assert!(messages[1].get("tool_name").is_none());
        assert!(messages[0]["tool_calls"][0]["function"]["arguments"].is_string());
        messages.clear();
        append_tool_turn("ollama", &mut messages, &completion, &[json!({"ok":true})]);
        assert_eq!(messages[1]["tool_name"], "ping");
        assert!(messages[1].get("tool_call_id").is_none());
        assert!(messages[0]["tool_calls"][0].get("id").is_none());
        assert!(messages[0]["tool_calls"][0]["function"]["arguments"].is_object());
    }
    #[test]
    fn audio_rejects_claimed_type_with_wrong_content() {
        assert!(validate_audio(b"<html>not audio</html>", "audio/webm").is_err());
        assert!(validate_audio(&[0x1a, 0x45, 0xdf, 0xa3, 0], "audio/webm;codecs=opus").is_ok());
    }
    #[test]
    fn known_long_wav_is_rejected_without_decoding() {
        let mut data = Vec::new();
        data.extend_from_slice(b"RIFF");
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(b"WAVEfmt ");
        data.extend_from_slice(&16u32.to_le_bytes());
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&8000u32.to_le_bytes());
        data.extend_from_slice(&16000u32.to_le_bytes());
        data.extend_from_slice(&2u16.to_le_bytes());
        data.extend_from_slice(&16u16.to_le_bytes());
        data.extend_from_slice(b"data");
        data.extend_from_slice(&(16000u32 * 61).to_le_bytes());
        assert_eq!(audio_duration_seconds(&data), Some(61.0));
        assert!(validate_audio(&data, "audio/wav")
            .err()
            .unwrap()
            .contains("60 秒"));
    }
    #[test]
    fn rate_limiter_releases_active_request() {
        let lease = acquire_call(-7901, 1).unwrap();
        assert!(acquire_call(-7901, 1).is_err());
        drop(lease);
        assert!(acquire_call(-7901, 1).is_err());
    }
    #[test]
    fn restart_preserves_binding_and_changes_invalidate_it() {
        let p = provider("openai");
        let result = AiTestResult {
            provider_id: p.id.clone(),
            model: "model".into(),
            kind: "tools".into(),
            passed: true,
            reachable: true,
            usable: true,
            message: "通过".into(),
            latency_ms: 10,
            checked_at: "2026-11-13T00:00:00Z".into(),
        };
        let s = Settings {
            providers: vec![p.clone()],
            tests: vec![StoredTest {
                binding: p.binding("model"),
                result,
            }],
            ..Settings::default()
        };
        let mut reload: Settings =
            serde_json::from_slice(&serde_json::to_vec(&s).unwrap()).unwrap();
        assert!(reload.tested(&p, "model", "tools"));
        assert_eq!(reload.view().tests.len(), 1);
        reload.providers[0].url = "https://changed.example/v1".into();
        assert!(reload.view().tests.is_empty());
        assert!(!serde_json::to_string(&s.view())
            .unwrap()
            .contains(&p.binding("model")));
    }
    #[test]
    fn stale_revision_cannot_overwrite_configuration() {
        let old = Settings {
            revision: 2,
            ..Settings::default()
        };
        let input: AiConfigInput =
            serde_json::from_value(json!({"revision":1,"enabled":false,"name":"AI 助手"})).unwrap();
        assert!(merged_settings(&old, input)
            .err()
            .unwrap()
            .contains("其他管理员"));
    }
    async fn fake_upstream(
        responses: Vec<(u16, &'static str, String)>,
    ) -> (String, tokio::task::JoinHandle<Vec<String>>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            let mut captured = Vec::new();
            for (status, content_type, body) in responses {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                let mut buffer = [0u8; 4096];
                loop {
                    let n = socket.read(&mut buffer).await.unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&buffer[..n]);
                    assert!(bytes.len() < 256 * 1024);
                    if let Some(i) = bytes.windows(4).position(|s| s == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&bytes[..i]);
                        let len = head
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|v| v.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        if bytes.len() >= i + 4 + len {
                            break;
                        }
                    }
                }
                captured.push(String::from_utf8(bytes).unwrap());
                let redirect = if status == 302 {
                    "Location: http://127.0.0.1:1/private\r\n"
                } else {
                    ""
                };
                let head=format!("HTTP/1.1 {status} Test\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n{redirect}Connection: close\r\n\r\n",body.len());
                socket.write_all(head.as_bytes()).await.unwrap();
                socket.write_all(body.as_bytes()).await.unwrap();
            }
            captured
        });
        (url, task)
    }
    #[tokio::test]
    async fn fake_models_follow_anthropic_cursor_and_deduplicate() {
        let (url,task)=fake_upstream(vec![(200,"application/json",json!({"data":[{"id":"one"}],"has_more":true,"last_id":"one"}).to_string()),(200,"application/json",json!({"data":[{"id":"one"},{"id":"two","display_name":"第二个"}],"has_more":false}).to_string())]).await;
        let mut p = provider("anthropic");
        p.url = url;
        let list = discover_provider(&p, 5).await.unwrap();
        assert_eq!(list.models.len(), 2);
        assert!(!list.truncated);
        let requests = task.await.unwrap();
        assert!(requests[1].contains("after_id=one"));
        assert!(requests[0].contains("x-api-key: never_return_this"));
    }
    #[tokio::test]
    async fn fake_stream_requires_end_marker_and_rejects_empty_body() {
        let good = "data: {\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\n\ndata: [DONE]\n\n";
        let broken = "data: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\n";
        let (url, task) = fake_upstream(vec![
            (200, "text/event-stream", good.into()),
            (200, "text/event-stream", broken.into()),
            (
                200,
                "application/json",
                "{\"choices\":[{\"message\":{\"content\":\"\"}}]}".into(),
            ),
        ])
        .await;
        let mut p = provider("openai");
        p.url = url;
        let result = stream_complete(&p, "model", &[], "test", &[], 5, None)
            .await
            .unwrap();
        assert_eq!(result.text, "hello");
        assert!(stream_complete(&p, "model", &[], "test", &[], 5, None)
            .await
            .err()
            .unwrap()
            .contains("未正常结束"));
        assert!(complete(&p, "model", &[], "test", &[], 5)
            .await
            .err()
            .unwrap()
            .contains("空回复"));
        task.await.unwrap();
    }
    #[tokio::test]
    async fn fake_errors_do_not_return_upstream_body_or_follow_redirect() {
        let (url, task) = fake_upstream(vec![
            (
                401,
                "application/json",
                "{\"error\":\"never_return_this secret traceback\"}".into(),
            ),
            (302, "text/plain", "redirect secret".into()),
        ])
        .await;
        let mut p = provider("openai");
        p.url = url;
        let error = complete(&p, "model", &[], "test", &[], 5)
            .await
            .err()
            .unwrap();
        assert!(error.contains("鉴权"));
        assert!(!error.contains("never_return_this"));
        assert!(complete(&p, "model", &[], "test", &[], 5)
            .await
            .err()
            .unwrap()
            .contains("重定向"));
        task.await.unwrap();
    }
    #[tokio::test]
    async fn cancelling_stream_drops_the_upstream_connection() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let upstream = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let mut buffer = [0u8; 4096];
            loop {
                let n = socket.read(&mut buffer).await.unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buffer[..n]);
                if let Some(i) = bytes.windows(4).position(|s| s == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&bytes[..i]);
                    let length = header
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|s| s.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    if bytes.len() >= i + 4 + length {
                        break;
                    }
                }
            }
            let chunk = b"data: {\"choices\":[{\"delta\":{\"content\":\"begin\"}}]}\n\n";
            socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n").await.unwrap();
            socket
                .write_all(format!("{:x}\r\n", chunk.len()).as_bytes())
                .await
                .unwrap();
            socket.write_all(chunk).await.unwrap();
            socket.write_all(b"\r\n").await.unwrap();
            let _ = ready_tx.send(());
            tokio::time::timeout(Duration::from_secs(3), socket.read(&mut buffer))
                .await
                .unwrap()
                .unwrap()
        });
        let mut p = provider("openai");
        p.url = url;
        let request =
            tokio::spawn(
                async move { stream_complete(&p, "model", &[], "test", &[], 30, None).await },
            );
        tokio::time::timeout(Duration::from_secs(3), ready_rx)
            .await
            .unwrap()
            .unwrap();
        request.abort();
        let _ = request.await;
        assert_eq!(upstream.await.unwrap(), 0);
    }
}
