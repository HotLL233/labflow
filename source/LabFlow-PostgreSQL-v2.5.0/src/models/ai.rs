use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AiCapabilities {
    pub help: bool,
    pub query: bool,
    pub draft: bool,
    pub report: bool,
    pub voice: bool,
}
impl Default for AiCapabilities {
    fn default() -> Self {
        Self {
            help: true,
            query: true,
            draft: true,
            report: true,
            voice: false,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AiModel {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Deserialize)]
pub struct AiProviderInput {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub url: String,
    #[serde(default)]
    pub no_key: bool,
    #[serde(default)]
    pub secret: Option<String>,
    #[serde(default)]
    pub clear_secret: bool,
    #[serde(default)]
    pub models: Vec<AiModel>,
    #[serde(default)]
    pub default_model: String,
}

#[derive(Clone, Serialize)]
pub struct AiProviderView {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub url: String,
    pub no_key: bool,
    pub has_secret: bool,
    pub models: Vec<AiModel>,
    pub default_model: String,
}

#[derive(Clone, Default, Deserialize)]
#[serde(default)]
pub struct AiSpeechInput {
    pub enabled: bool,
    pub url: String,
    pub model: String,
    pub no_key: bool,
    pub secret: Option<String>,
    pub clear_secret: bool,
}

#[derive(Clone, Serialize)]
pub struct AiSpeechView {
    pub enabled: bool,
    pub url: String,
    pub model: String,
    pub no_key: bool,
    pub has_secret: bool,
}

#[derive(Deserialize)]
pub struct AiConfigInput {
    pub revision: u64,
    pub enabled: bool,
    pub name: String,
    #[serde(default)]
    pub default_provider: String,
    #[serde(default)]
    pub capabilities: AiCapabilities,
    #[serde(default)]
    pub providers: Vec<AiProviderInput>,
    #[serde(default)]
    pub speech: AiSpeechInput,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
    #[serde(default = "default_rate")]
    pub max_requests_per_minute: usize,
}
pub fn default_timeout() -> u64 {
    90
}
pub fn default_rate() -> usize {
    20
}

#[derive(Serialize)]
pub struct AiConfigView {
    pub revision: u64,
    pub enabled: bool,
    pub name: String,
    pub default_provider: String,
    pub capabilities: AiCapabilities,
    pub providers: Vec<AiProviderView>,
    pub speech: AiSpeechView,
    pub tests: Vec<AiTestResult>,
    pub timeout_seconds: u64,
    pub max_requests_per_minute: usize,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AiTestResult {
    pub provider_id: String,
    pub model: String,
    pub kind: String,
    pub passed: bool,
    pub reachable: bool,
    pub usable: bool,
    pub message: String,
    pub latency_ms: u64,
    pub checked_at: String,
}

#[derive(Deserialize)]
pub struct AiModelRequest {
    pub provider_id: String,
}
#[derive(Serialize)]
pub struct AiModelList {
    pub models: Vec<AiModel>,
    pub truncated: bool,
}
#[derive(Deserialize)]
pub struct AiTestRequest {
    pub provider_id: String,
    #[serde(default)]
    pub model: Option<String>,
    pub kind: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct AiMessage {
    pub role: String,
    pub content: String,
}
#[derive(Default, Deserialize)]
#[serde(default)]
pub struct AiChatContext {
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub include_business: Option<bool>,
}
#[derive(Deserialize)]
pub struct AiChatRequest {
    pub messages: Vec<AiMessage>,
    #[serde(default)]
    pub provider_id: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub context: AiChatContext,
}

#[derive(Serialize)]
pub struct AiPublicModel {
    pub id: String,
    pub name: String,
    pub provider_id: String,
    pub provider_name: String,
    pub chat_ready: bool,
    pub tools_ready: bool,
}
#[derive(Serialize)]
pub struct AiPublicCapabilities {
    pub enabled: bool,
    pub can_use: bool,
    pub can_manage: bool,
    pub name: String,
    pub provider_label: String,
    pub capabilities: AiCapabilities,
    pub speech_enabled: bool,
    pub speech_destination: String,
    pub can_sample: bool,
    pub can_work: bool,
    pub can_aux: bool,
    pub can_report: bool,
    pub models: Vec<AiPublicModel>,
    pub default_provider: String,
    pub default_model: String,
}
