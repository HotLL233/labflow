use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::{
    user::{LoginResponse, User},
    ApiResponse,
};
use crate::repo::{oidc_repo, user_repo};
use crate::service::{
    auth_service::{self, LoginSource},
    authz_service,
};
use axum::{
    extract::{DefaultBodyLimit, Path, Query, Request, State},
    http::{header, HeaderMap, HeaderValue},
    middleware::{self, Next},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use openidconnect::{
    core::{
        CoreAuthDisplay, CoreAuthPrompt, CoreAuthenticationFlow, CoreErrorResponseType,
        CoreGenderClaim, CoreIdTokenVerifier, CoreJsonWebKey, CoreJweContentEncryptionAlgorithm,
        CoreJwsSigningAlgorithm, CoreProviderMetadata, CoreRevocableToken,
        CoreRevocationErrorResponse, CoreTokenIntrospectionResponse, CoreTokenType,
    },
    AdditionalClaims, AuthorizationCode, Client, ClientId, ClientSecret, CsrfToken,
    EmptyExtraTokenFields, EndpointMaybeSet, EndpointNotSet, EndpointSet, IdToken, IdTokenClaims,
    IdTokenFields, IssuerUrl, Nonce, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl,
    StandardErrorResponse, StandardTokenResponse, TokenResponse,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const CALLBACK: &str = "/api/auth/oidc/callback";
const COOKIE: &str = "__Host-labflow-oidc";
const FLOW_SECONDS: u64 = 300;
const HANDOFF_SECONDS: u64 = 60;
const SESSION_SECONDS: i64 = 15 * 60;

#[derive(Debug, Deserialize, Serialize)]
struct AdmissionClaims {
    #[serde(default)]
    labflow_roles: Vec<String>,
}
impl AdditionalClaims for AdmissionClaims {}

type AdmissionToken = IdToken<
    AdmissionClaims,
    CoreGenderClaim,
    CoreJweContentEncryptionAlgorithm,
    CoreJwsSigningAlgorithm,
>;

fn verify_claims<'token>(
    token: &'token AdmissionToken,
    verifier: CoreIdTokenVerifier<'_>,
    nonce: &Nonce,
) -> Option<&'token IdTokenClaims<AdmissionClaims, CoreGenderClaim>> {
    token
        .claims(
            &verifier.set_allowed_algs([CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256]),
            nonce,
        )
        .ok()
}

type ClientWithAdmission = Client<
    AdmissionClaims,
    CoreAuthDisplay,
    CoreGenderClaim,
    CoreJweContentEncryptionAlgorithm,
    CoreJsonWebKey,
    CoreAuthPrompt,
    StandardErrorResponse<CoreErrorResponseType>,
    StandardTokenResponse<
        IdTokenFields<
            AdmissionClaims,
            EmptyExtraTokenFields,
            CoreGenderClaim,
            CoreJweContentEncryptionAlgorithm,
            CoreJwsSigningAlgorithm,
        >,
        CoreTokenType,
    >,
    CoreTokenIntrospectionResponse,
    CoreRevocableToken,
    CoreRevocationErrorResponse,
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

struct Config {
    issuer: IssuerUrl,
    client_id: ClientId,
    secret: ClientSecret,
    redirect: RedirectUrl,
    origin: String,
    http: reqwest::Client,
}

fn https_url(value: &reqwest::Url) -> bool {
    value.scheme() == "https"
        && value.host().is_some()
        && value.username().is_empty()
        && value.password().is_none()
        && value.query().is_none()
        && value.fragment().is_none()
}

impl Config {
    fn load() -> Option<Self> {
        let issuer = IssuerUrl::new(auth_service::configured_oidc_issuer()?).ok()?;
        let redirect = RedirectUrl::new(std::env::var("LABFLOW_OIDC_REDIRECT_URI").ok()?).ok()?;
        let client_id = std::env::var("LABFLOW_OIDC_CLIENT_ID").ok()?;
        let secret = std::env::var("LABFLOW_OIDC_CLIENT_SECRET").ok()?;
        if !https_url(issuer.url())
            || !https_url(redirect.url())
            || redirect.url().path() != CALLBACK
            || client_id.trim().is_empty()
            || secret.trim().is_empty()
        {
            return None;
        }
        let origin = redirect.url().origin().ascii_serialization();
        let mut http = reqwest::Client::builder()
            .https_only(true)
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none());
        if let Some(file) = std::env::var_os("LABFLOW_OIDC_CA_FILE") {
            let pem = std::fs::read(file).ok()?;
            http = http.add_root_certificate(reqwest::Certificate::from_pem(&pem).ok()?);
        }
        Some(Self {
            issuer,
            client_id: ClientId::new(client_id),
            secret: ClientSecret::new(secret),
            redirect,
            origin,
            http: http.build().ok()?,
        })
    }

    fn host_matches(&self, headers: &HeaderMap) -> bool {
        headers
            .get(header::HOST)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|host| {
                host.eq_ignore_ascii_case(self.origin.trim_start_matches("https://"))
            })
    }

    fn same_origin(&self, headers: &HeaderMap) -> bool {
        self.host_matches(headers)
            && headers
                .get(header::ORIGIN)
                .and_then(|value| value.to_str().ok())
                == Some(&self.origin)
    }

    fn client(&self, metadata: CoreProviderMetadata) -> ClientWithAdmission {
        ClientWithAdmission::from_provider_metadata(
            metadata,
            self.client_id.clone(),
            Some(self.secret.clone()),
        )
        .set_redirect_uri(self.redirect.clone())
    }
}

#[derive(Deserialize, Default)]
struct Device {
    device_id: Option<String>,
    device_name: Option<String>,
}

#[derive(Deserialize)]
struct Begin {
    #[serde(flatten)]
    device: Device,
    invitation: Option<String>,
}

struct Awaiting {
    metadata: CoreProviderMetadata,
    state: CsrfToken,
    nonce: Nonce,
    pkce: PkceCodeVerifier,
    device: Device,
    invitation: Option<String>,
}

struct Handoff {
    subject: String,
    deadline: i64,
    device: Device,
}

enum Phase {
    Awaiting(Box<Awaiting>),
    Handoff(Handoff),
}

struct Pilot {
    pool: DbPool,
    config: Option<Config>,
    flows: Mutex<HashMap<String, (Instant, Phase)>>,
}

impl Pilot {
    fn configured(&self) -> Result<&Config> {
        self.config
            .as_ref()
            .ok_or_else(|| AppError::Forbidden("统一登录试点未开启".into()))
    }

    fn insert(
        &self,
        key: String,
        phase: Phase,
        seconds: u64,
        previous: Option<&str>,
    ) -> Result<()> {
        let mut flows = self.flows.lock().map_err(|_| unavailable())?;
        let now = Instant::now();
        flows.retain(|_, (expires, _)| *expires > now);
        if let Some(previous) = previous {
            flows.remove(previous);
        }
        if flows.len() >= 256 {
            return Err(unavailable());
        }
        flows.insert(key, (now + Duration::from_secs(seconds), phase));
        Ok(())
    }

    fn take(&self, key: &str, matches: impl FnOnce(&Phase) -> bool) -> Option<Phase> {
        let mut flows = self.flows.lock().ok()?;
        if !flows
            .get(key)
            .is_some_and(|(expires, phase)| *expires > Instant::now() && matches(phase))
        {
            return None;
        }
        flows.remove(key).map(|(_, phase)| phase)
    }
}

pub fn router(pool: DbPool) -> Router {
    let config = Config::load();
    if auth_service::oidc_enabled() && config.is_none() {
        tracing::error!("统一登录配置无效，入口未启用；试点人员不回退至密码登录");
    }
    Router::new()
        .route("/api/auth/oidc/status", get(status))
        .route("/api/auth/oidc/start", post(start))
        .route(CALLBACK, get(callback))
        .route("/api/auth/oidc/redeem", post(redeem))
        .route("/api/auth/oidc/bindings", get(bindings))
        .route("/api/auth/oidc/bindings/:id/invite", post(invite))
        .route("/api/auth/oidc/bindings/:id/confirm", post(confirm))
        .route("/api/auth/oidc/bindings/:id/revoke", post(revoke))
        .layer(DefaultBodyLimit::max(4096))
        .layer(middleware::from_fn(private_response))
        .with_state(Arc::new(Pilot {
            pool,
            config,
            flows: Mutex::default(),
        }))
}

async fn private_response(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response
}

fn browser_cookie(headers: &HeaderMap) -> Option<&str> {
    let mut values = headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().strip_prefix("__Host-labflow-oidc="));
    let first = values.next()?;
    (values.next().is_none()
        && first.len() == 64
        && first.bytes().all(|byte| byte.is_ascii_hexdigit()))
    .then_some(first)
}

fn with_cookie(mut response: Response, value: &str, seconds: u64) -> Response {
    response.headers_mut().append(
        header::SET_COOKIE,
        HeaderValue::from_str(&format!(
            "{COOKIE}={value}; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age={seconds}"
        ))
        .expect("generated OIDC cookie"),
    );
    response
}

fn unavailable() -> AppError {
    AppError::Forbidden("统一登录暂不可用，请稍后重试".into())
}

async fn status(State(pilot): State<Arc<Pilot>>, headers: HeaderMap) -> Json<ApiResponse<bool>> {
    Json(ApiResponse::ok(
        pilot
            .config
            .as_ref()
            .is_some_and(|config| config.host_matches(&headers)),
    ))
}

async fn start(
    State(pilot): State<Arc<Pilot>>,
    headers: HeaderMap,
    Json(body): Json<Begin>,
) -> Result<Response> {
    let config = pilot.configured()?;
    if !config.same_origin(&headers) {
        return Err(AppError::Forbidden("只能从本应用启动统一登录".into()));
    }
    if body
        .device
        .device_id
        .as_ref()
        .is_some_and(|value| value.len() > 128)
        || body
            .device
            .device_name
            .as_ref()
            .is_some_and(|value| value.len() > 256)
    {
        return Err(AppError::Validation("设备标识过长".into()));
    }
    let invitation = body
        .invitation
        .as_deref()
        .map(oidc_repo::invite_digest)
        .transpose()?;
    let metadata = CoreProviderMetadata::discover_async(config.issuer.clone(), &config.http)
        .await
        .map_err(|_| unavailable())?;
    if !https_url(metadata.authorization_endpoint().url())
        || metadata.authorization_endpoint().url().origin() != config.issuer.url().origin()
    {
        return Err(unavailable());
    }
    let client = config.client(metadata.clone());
    let (challenge, pkce) = PkceCodeChallenge::new_random_sha256();
    let (url, state, nonce) = client
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .set_pkce_challenge(challenge)
        .url();
    let browser = oidc_repo::new_secret();
    pilot.insert(
        browser.clone(),
        Phase::Awaiting(Box::new(Awaiting {
            metadata,
            state,
            nonce,
            pkce,
            device: body.device,
            invitation,
        })),
        FLOW_SECONDS,
        browser_cookie(&headers),
    )?;
    Ok(with_cookie(
        Json(ApiResponse::ok(url.to_string())).into_response(),
        &browser,
        FLOW_SECONDS,
    ))
}

#[derive(Deserialize)]
struct AuthorizationResult {
    code: Option<String>,
    state: Option<String>,
    iss: Option<String>,
    error: Option<String>,
}

async fn callback(
    State(pilot): State<Arc<Pilot>>,
    headers: HeaderMap,
    Query(query): Query<AuthorizationResult>,
) -> Response {
    const FAILED: &str = "/login?oidc=failed";
    let (Ok(config), Some(browser), Some(returned_state)) =
        (pilot.configured(), browser_cookie(&headers), query.state)
    else {
        return Redirect::to(FAILED).into_response();
    };
    if !config.host_matches(&headers) {
        return Redirect::to(FAILED).into_response();
    }
    let Some(Phase::Awaiting(pending)) = pilot.take(browser, |phase| {
        matches!(phase, Phase::Awaiting(pending) if pending.state.secret() == &returned_state)
    }) else { return Redirect::to(FAILED).into_response(); };
    let finish = |destination| with_cookie(Redirect::to(destination).into_response(), "", 0);
    if query.error.is_some()
        || query
            .iss
            .as_deref()
            .is_some_and(|issuer| issuer != config.issuer.as_str())
    {
        return finish(FAILED);
    }
    let Some(code) = query.code.filter(|code| !code.is_empty()) else {
        return finish(FAILED);
    };
    let Awaiting {
        metadata,
        nonce,
        pkce,
        invitation,
        device,
        ..
    } = *pending;
    let client = config.client(metadata);
    let deadline = Utc::now().timestamp() + SESSION_SECONDS;
    let Ok(exchange) = client.exchange_code(AuthorizationCode::new(code)) else {
        return finish(FAILED);
    };
    let Ok(tokens) = exchange
        .set_pkce_verifier(pkce)
        .request_async(&config.http)
        .await
    else {
        return finish(FAILED);
    };
    let Some(claims) = tokens
        .id_token()
        .and_then(|token| verify_claims(token, client.id_token_verifier(), &nonce))
    else {
        return finish(FAILED);
    };
    if !claims
        .additional_claims()
        .labflow_roles
        .iter()
        .any(|role| role == "pilot-access")
    {
        return finish("/login?oidc=no_access");
    }
    let subject = claims.subject().as_str();
    if subject.is_empty() || subject.len() > 255 {
        return finish(FAILED);
    }
    if let Some(digest) = invitation {
        return match oidc_repo::stage(&pilot.pool, config.issuer.as_str(), subject, &digest) {
            Ok(()) => finish("/login?oidc=pending"),
            Err(_) => finish("/login?oidc=invite_failed"),
        };
    }
    let key = oidc_repo::new_secret();
    if pilot
        .insert(
            key.clone(),
            Phase::Handoff(Handoff {
                subject: subject.to_string(),
                deadline,
                device,
            }),
            HANDOFF_SECONDS,
            None,
        )
        .is_err()
    {
        return finish(FAILED);
    }
    with_cookie(
        Redirect::to("/login?oidc=complete").into_response(),
        &key,
        HANDOFF_SECONDS,
    )
}

async fn redeem(
    State(pilot): State<Arc<Pilot>>,
    headers: HeaderMap,
    Json(()): Json<()>,
) -> Result<Response> {
    let config = pilot.configured()?;
    if !config.same_origin(&headers) {
        return Err(AppError::Forbidden("只能从本应用兑换统一登录会话".into()));
    }
    let Some(Phase::Handoff(session)) = browser_cookie(&headers)
        .and_then(|key| pilot.take(key, |phase| matches!(phase, Phase::Handoff(_))))
    else {
        return Err(AppError::Forbidden(
            "统一登录交接已过期或已使用，请重新登录".into(),
        ));
    };
    let result = || -> Result<LoginResponse> {
        let issuer = config.issuer.as_str();
        let id = {
            let conn = pilot.pool.get()?;
            oidc_repo::find_user(&conn, issuer, &session.subject)?
        }
        .ok_or_else(|| AppError::Forbidden("中心身份尚未绑定或本地账号不允许统一登录".into()))?;
        let user = user_repo::find_by_id(&pilot.pool, id)?
            .ok_or_else(|| AppError::Forbidden("本地人员不可用".into()))?;
        auth_service::create_login_response(
            &pilot.pool,
            user,
            LoginSource::Oidc {
                issuer,
                subject: &session.subject,
                deadline: session.deadline,
            },
            session.device.device_id.as_deref(),
            session.device.device_name.as_deref(),
        )
    };
    let response = match result() {
        Ok(login) => Json(ApiResponse::ok(login)).into_response(),
        Err(error) => error.into_response(),
    };
    Ok(with_cookie(response, "", 0))
}

fn operator(pilot: &Pilot, headers: &HeaderMap) -> Result<User> {
    let ctx = authz_service::authenticate(&pilot.pool, headers)?;
    if !ctx.is_system_admin() {
        return Err(AppError::Forbidden("仅系统管理员可管理统一登录绑定".into()));
    }
    Ok(ctx.user)
}

async fn bindings(
    State(pilot): State<Arc<Pilot>>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<oidc_repo::Binding>>>> {
    operator(&pilot, &headers)?;
    Ok(Json(ApiResponse::ok(oidc_repo::list(&pilot.pool)?)))
}

#[derive(Deserialize)]
struct Invite {
    employee_no: String,
    reason: String,
}

async fn invite(
    State(pilot): State<Arc<Pilot>>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<Invite>,
) -> Result<Json<ApiResponse<String>>> {
    let actor = operator(&pilot, &headers)?;
    let config = pilot.configured()?;
    if actor.id == id {
        return Err(AppError::Forbidden(
            "请由另一位系统管理员核对并预选试点人员".into(),
        ));
    }
    Ok(Json(ApiResponse::ok(oidc_repo::invite(
        &pilot.pool,
        &actor,
        id,
        config.issuer.as_str(),
        &body.employee_no,
        &body.reason,
    )?)))
}

#[derive(Deserialize)]
struct Confirm {
    issuer: String,
    subject: String,
    reason: String,
}

async fn confirm(
    State(pilot): State<Arc<Pilot>>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<Confirm>,
) -> Result<Json<ApiResponse<()>>> {
    let actor = operator(&pilot, &headers)?;
    let config = pilot.configured()?;
    if body.issuer != config.issuer.as_str() {
        return Err(AppError::Conflict("发证方与当前试点配置不符".into()));
    }
    oidc_repo::confirm(
        &pilot.pool,
        &actor,
        id,
        &body.issuer,
        &body.subject,
        &body.reason,
    )?;
    Ok(Json(ApiResponse::ok_msg("已确认统一登录身份")))
}

#[derive(Deserialize)]
struct Revoke {
    reason: String,
}

async fn revoke(
    State(pilot): State<Arc<Pilot>>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<Revoke>,
) -> Result<Json<ApiResponse<()>>> {
    let actor = operator(&pilot, &headers)?;
    oidc_repo::revoke(&pilot.pool, &actor, id, &body.reason)?;
    Ok(Json(ApiResponse::ok_msg(
        "绑定与应用会话已撤销；本地密码登录仍禁止",
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
    use openidconnect::{core::CoreJsonWebKeySet, JsonWebKeyId};
    use rsa::{
        pkcs8::{EncodePrivateKey, LineEnding},
        traits::PublicKeyParts,
        RsaPrivateKey,
    };
    use serde_json::json;

    #[test]
    fn signed_id_token_must_match_issuer_client_nonce_and_expiry() {
        let private = RsaPrivateKey::new(&mut rand::rngs::OsRng, 2048).unwrap();
        let private_pem = private.to_pkcs8_pem(LineEnding::LF).unwrap();
        let encoding = EncodingKey::from_rsa_pem(private_pem.as_bytes()).unwrap();
        let kid = "ephemeral-unit-test";
        let key = CoreJsonWebKey::new_rsa(
            private.n().to_bytes_be(),
            private.e().to_bytes_be(),
            Some(JsonWebKeyId::new(kid.into())),
        );
        let issuer = "https://issuer.example.invalid/realms/pilot";
        let nonce = Nonce::new_random();
        let now = Utc::now().timestamp();
        let claims = json!({"sub":"case-id", "iss":issuer, "aud":"labflow-pilot", "nonce":nonce.secret(),
            "iat":now-120, "exp":now+60, "labflow_roles":["pilot-access"]});
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(kid.into());
        let check = |claims: &serde_json::Value| {
            let raw = encode(&header, claims, &encoding).unwrap();
            let token: AdmissionToken = raw.parse().unwrap();
            let verifier = CoreIdTokenVerifier::new_public_client(
                ClientId::new("labflow-pilot".into()),
                IssuerUrl::new(issuer.into()).unwrap(),
                CoreJsonWebKeySet::new(vec![key.clone()]),
            );
            verify_claims(&token, verifier, &nonce)
                .map(|claims| claims.subject().as_str().to_owned())
        };
        assert_eq!(check(&claims).as_deref(), Some("case-id"));
        for (field, value) in [
            ("iss", json!("https://other.example.invalid/realms/pilot")),
            ("aud", json!("other-client")),
            ("nonce", json!("wrong-flow")),
            ("exp", json!(now - 1)),
        ] {
            let mut altered = claims.clone();
            altered[field] = value;
            assert!(check(&altered).is_none(), "unexpectedly accepted {field}");
        }
    }
}
