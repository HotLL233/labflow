use super::workload_report_handler::{integration_workload_records, IntegrationWorkloadRecord};
use crate::{
    db::DbPool,
    error::{AppError, Result},
    models::ApiResponse,
    repo::audit_repo,
    service::authz_service::{self, AuthContext},
};
use axum::{
    extract::{DefaultBodyLimit, Path, Query, Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use chrono::{Datelike, Local, Months, NaiveDate};
use postgres_compat::{Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant},
};
use tokio::sync::Semaphore;
use uuid::Uuid;

const ENDPOINT: &str = "/api/lab/integration/v1/workload-records";
const MONTH_MAX_ROWS: usize = 50_000;
const MAX_SNAPSHOTS: usize = 4;
const SNAPSHOT_TTL: Duration = Duration::from_secs(300);

#[derive(Clone)]
struct IntegrationState {
    pool: DbPool,
    snapshots: Arc<Mutex<HashMap<i64, Snapshot>>>,
    read_slots: Arc<Semaphore>,
}

pub(super) fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/admin/workload-api-links", get(list).post(create))
        .route(
            "/api/admin/workload-api-links/:id",
            put(update).delete(delete_link),
        )
        .route("/api/admin/workload-api-links/:id/rotate", post(rotate))
        .route("/api/lab/integration/v1/manifest", get(manifest))
        .route(ENDPOINT, get(pull))
        .layer(DefaultBodyLimit::max(32 * 1024))
        .layer(middleware::from_fn(no_store))
        .with_state(IntegrationState {
            pool,
            snapshots: Arc::default(),
            read_slots: Arc::new(Semaphore::new(2)),
        })
}

async fn no_store(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Detection,
    Auxiliary,
    Both,
}

impl Kind {
    fn filter(self) -> Option<&'static str> {
        match self {
            Self::Detection => Some("detection"),
            Self::Auxiliary => Some("auxiliary"),
            Self::Both => None,
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
enum Departments {
    All,
    Selected { ids: Vec<i64> },
}

impl Departments {
    fn ids(&self) -> Option<&[i64]> {
        match self {
            Self::All => None,
            Self::Selected { ids } => Some(ids),
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct LinkConfig {
    name: String,
    kind: Kind,
    departments: Departments,
    from_month: String,
    through_month: Option<String>,
    expires_on: Option<NaiveDate>,
    enabled: bool,
}

impl LinkConfig {
    fn is_expired(&self) -> bool {
        self.expires_on
            .is_some_and(|date| date < Local::now().date_naive())
    }

    fn validate(&mut self, conn: &Connection, previous: Option<&Self>) -> Result<()> {
        self.name = self.name.trim().to_owned();
        if self.name.is_empty()
            || self.name.chars().count() > 80
            || self.name.chars().any(char::is_control)
        {
            return Err(AppError::Validation("接口名称须填写 1–80 个字符".into()));
        }
        parse_month(&self.from_month)?;
        if let Some(end) = &self.through_month {
            parse_month(end)?;
            if end < &self.from_month {
                return Err(AppError::Validation("结束月份不能早于起始月份".into()));
            }
        }
        if self
            .expires_on
            .is_some_and(|date| !(1..=9999).contains(&date.year()))
            || (self.enabled && self.is_expired())
        {
            return Err(AppError::Validation(
                "启用接口的令牌到期日不能早于今天".into(),
            ));
        }
        if let Departments::Selected { ids } = &mut self.departments {
            if ids.is_empty() || ids.len() > 200 || ids.iter().any(|id| *id <= 0) {
                return Err(AppError::Validation("请选择 1–200 个检测部门".into()));
            }
            ids.sort_unstable();
            ids.dedup();
            let old_ids = previous
                .and_then(|config| config.departments.ids())
                .unwrap_or_default();
            let added = ids
                .iter()
                .filter(|id| !old_ids.contains(id))
                .map(i64::to_string)
                .collect::<Vec<_>>();
            if !added.is_empty() {
                let found: i64 = conn.query_row(
                    &format!(
                        "SELECT COUNT(*) FROM divisions WHERE deleted_at IS NULL AND id IN ({})",
                        added.join(",")
                    ),
                    [],
                    |row| row.get(0),
                )?;
                if found as usize != added.len() {
                    return Err(AppError::Validation("所选检测部门不存在或已删除".into()));
                }
            }
        }
        Ok(())
    }
}

fn parse_month(value: &str) -> Result<NaiveDate> {
    if value.len() == 7
        && value.as_bytes().iter().enumerate().all(|(index, byte)| {
            if index == 4 {
                *byte == b'-'
            } else {
                byte.is_ascii_digit()
            }
        })
    {
        if let Ok(date) = NaiveDate::parse_from_str(&format!("{value}-01"), "%Y-%m-%d") {
            if date.year() > 0 {
                return Ok(date);
            }
        }
    }
    Err(AppError::Validation("月份须使用 YYYY-MM 格式".into()))
}

#[derive(Serialize)]
struct Link {
    id: i64,
    revision: i64,
    expired: bool,
    config: LinkConfig,
}

impl Link {
    fn from_row((id, revision, value): (i64, i64, String)) -> Result<Self> {
        let config: LinkConfig = serde_json::from_str(&value)
            .map_err(|_| AppError::Internal("工作量 API 接入配置格式无效".into()))?;
        Ok(Self {
            id,
            revision,
            expired: config.is_expired(),
            config,
        })
    }
}

fn link_row(row: &Row) -> postgres_compat::Result<(i64, i64, String)> {
    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
}

fn config_text(config: &LinkConfig) -> Result<String> {
    serde_json::to_string(config).map_err(|_| AppError::Internal("工作量 API 配置编码失败".into()))
}

#[derive(Serialize)]
struct LinkList {
    endpoint: &'static str,
    links: Vec<Link>,
}

#[derive(Serialize)]
struct IssuedLink {
    link: Link,
    token: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Update {
    revision: i64,
    config: LinkConfig,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Revision {
    revision: i64,
}

fn require_admin(pool: &DbPool, headers: &HeaderMap) -> Result<AuthContext> {
    let actor = authz_service::authenticate(pool, headers)?;
    if !actor.user.is_admin {
        return Err(AppError::Forbidden(
            "仅系统管理员账号可管理工作量 API 接入".into(),
        ));
    }
    Ok(actor)
}

fn issue_token() -> String {
    format!("lfw_{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn digest(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

fn audit(
    conn: &Connection,
    actor: &AuthContext,
    action: &str,
    before: Option<&Link>,
    after: Option<&Link>,
) -> Result<()> {
    let link = after
        .or(before)
        .ok_or_else(|| AppError::Internal("工作量 API 接入审计缺少配置".into()))?;
    let before = before.map(|link| serde_json::json!(link.config));
    let after = after.map(|link| serde_json::json!(link.config));
    let label = match action {
        "create" => "生成工作量 API 接入",
        "rotate" => "轮换工作量 API 接入令牌",
        "delete" => "删除工作量 API 接入",
        _ => "更新工作量 API 接入范围或状态",
    };
    audit_repo::log_structured_actor_on_conn(
        conn,
        action,
        "workload_api_links",
        Some(link.id),
        actor.user.id,
        &actor.user.username,
        &format!("{label}：{}", link.config.name),
        "shared",
        "",
        before.as_ref(),
        after.as_ref(),
        "management",
    )
}

async fn list(
    State(state): State<IntegrationState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<LinkList>>> {
    require_admin(&state.pool, &headers)?;
    let conn = state.pool.get()?;
    let mut stmt =
        conn.prepare("SELECT id,revision,config FROM workload_api_links ORDER BY id DESC")?;
    let links = stmt
        .query_map([], link_row)?
        .map(|row| Link::from_row(row?))
        .collect::<Result<Vec<_>>>()?;
    Ok(Json(ApiResponse::ok(LinkList {
        endpoint: ENDPOINT,
        links,
    })))
}

async fn create(
    State(state): State<IntegrationState>,
    headers: HeaderMap,
    Json(mut config): Json<LinkConfig>,
) -> Result<Json<ApiResponse<IssuedLink>>> {
    let actor = require_admin(&state.pool, &headers)?;
    let mut conn = state.pool.get()?;
    let tx = conn.transaction()?;
    config.validate(&tx, None)?;
    let token = issue_token();
    let link = Link::from_row(tx.query_row(
        "INSERT INTO workload_api_links(token_digest,config) VALUES(?1,?2) RETURNING id,revision,config",
        postgres_compat::params![digest(&token), config_text(&config)?],
        link_row,
    )?)?;
    audit(&tx, &actor, "create", None, Some(&link))?;
    tx.commit()?;
    Ok(Json(ApiResponse::ok(IssuedLink { link, token })))
}

fn locked_link(conn: &Connection, id: i64, revision: i64) -> Result<Link> {
    let before = Link::from_row(
        conn.query_row(
            "SELECT id,revision,config FROM workload_api_links WHERE id=?1 FOR UPDATE",
            [id],
            link_row,
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound("工作量 API 接入不存在".into()))?,
    )?;
    if before.revision != revision {
        return Err(AppError::Conflict(
            "接入配置已变更，请刷新列表后重试".into(),
        ));
    }
    Ok(before)
}

async fn update(
    State(state): State<IntegrationState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(mut body): Json<Update>,
) -> Result<Json<ApiResponse<Link>>> {
    let actor = require_admin(&state.pool, &headers)?;
    let mut conn = state.pool.get()?;
    let tx = conn.transaction()?;
    let before = locked_link(&tx, id, body.revision)?;
    body.config.validate(&tx, Some(&before.config))?;
    let link = Link::from_row(tx.query_row(
        "UPDATE workload_api_links SET config=?1,revision=revision+1 WHERE id=?2 RETURNING id,revision,config",
        postgres_compat::params![config_text(&body.config)?, id],
        link_row,
    )?)?;
    audit(&tx, &actor, "update", Some(&before), Some(&link))?;
    tx.commit()?;
    Ok(Json(ApiResponse::ok(link)))
}

async fn rotate(
    State(state): State<IntegrationState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<Revision>,
) -> Result<Json<ApiResponse<IssuedLink>>> {
    let actor = require_admin(&state.pool, &headers)?;
    let mut conn = state.pool.get()?;
    let tx = conn.transaction()?;
    let before = locked_link(&tx, id, body.revision)?;
    let token = issue_token();
    let link = Link::from_row(tx.query_row(
        "UPDATE workload_api_links SET token_digest=?1,revision=revision+1 WHERE id=?2 RETURNING id,revision,config",
        postgres_compat::params![digest(&token), id],
        link_row,
    )?)?;
    audit(&tx, &actor, "rotate", Some(&before), Some(&link))?;
    tx.commit()?;
    Ok(Json(ApiResponse::ok(IssuedLink { link, token })))
}

async fn delete_link(
    State(state): State<IntegrationState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(query): Query<Revision>,
) -> Result<Json<ApiResponse<()>>> {
    let actor = require_admin(&state.pool, &headers)?;
    let mut conn = state.pool.get()?;
    let tx = conn.transaction()?;
    let before = locked_link(&tx, id, query.revision)?;
    tx.execute("DELETE FROM workload_api_links WHERE id=?1", [id])?;
    audit(&tx, &actor, "delete", Some(&before), None)?;
    tx.commit()?;
    if let Ok(mut snapshots) = state.snapshots.lock() {
        snapshots.remove(&id);
    }
    Ok(Json(ApiResponse::ok_msg("工作量 API 接入已删除")))
}

async fn manifest() -> Json<ApiResponse<serde_json::Value>> {
    Json(ApiResponse::ok(serde_json::json!({
        "appId": "labflow", "apiVersion": 1, "workloadRecordsPath": ENDPOINT
    })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PullQuery {
    month: String,
    snapshot: Option<String>,
    cursor: Option<usize>,
    limit: Option<usize>,
}

#[derive(Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Totals {
    detection_quantity: i64,
    detection_workload: f64,
    auxiliary_workload: f64,
}

struct Snapshot {
    key: Uuid,
    revision: i64,
    month: NaiveDate,
    until: Instant,
    rows: Vec<IntegrationWorkloadRecord>,
    totals: Totals,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PullPage {
    month: String,
    snapshot: String,
    items: Vec<IntegrationWorkloadRecord>,
    has_more: bool,
    next_cursor: Option<usize>,
    total_count: usize,
    totals: Totals,
}

impl Snapshot {
    fn page(&self, month: String, cursor: usize, limit: usize) -> PullResult<PullPage> {
        if cursor != 0 && cursor >= self.rows.len() {
            return Err(AppError::Validation("工作量分页游标无效".into()).into());
        }
        let end = (cursor + limit).min(self.rows.len());
        let has_more = end < self.rows.len();
        Ok(PullPage {
            month,
            snapshot: self.key.to_string(),
            items: self.rows[cursor..end].to_vec(),
            has_more,
            next_cursor: has_more.then_some(end),
            total_count: self.rows.len(),
            totals: self.totals,
        })
    }
}

#[derive(Debug)]
struct PullError(StatusCode, AppError);

type PullResult<T> = std::result::Result<T, PullError>;

impl PullError {
    fn invalid_token() -> Self {
        Self(
            StatusCode::UNAUTHORIZED,
            AppError::Forbidden("工作量 API 令牌无效、已到期或已停用".into()),
        )
    }

    fn busy() -> Self {
        Self(
            StatusCode::TOO_MANY_REQUESTS,
            AppError::Validation("工作量 API 正在处理其他请求，请稍后重试".into()),
        )
    }

    fn stale() -> Self {
        AppError::Conflict("分页快照已失效，请从本月第一页重新读取".into()).into()
    }
}

impl From<AppError> for PullError {
    fn from(error: AppError) -> Self {
        let status = match &error {
            AppError::Validation(_) => StatusCode::BAD_REQUEST,
            AppError::Forbidden(_) => StatusCode::FORBIDDEN,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            _ => StatusCode::SERVICE_UNAVAILABLE,
        };
        Self(status, error)
    }
}

impl IntoResponse for PullError {
    fn into_response(self) -> Response {
        let mut response = self.1.into_response();
        *response.status_mut() = self.0;
        if self.0 == StatusCode::UNAUTHORIZED {
            response
                .headers_mut()
                .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        }
        response
    }
}

fn token_digest(headers: &HeaderMap) -> PullResult<String> {
    let mut values = headers.get_all(header::AUTHORIZATION).iter();
    let token = values
        .next()
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or_else(PullError::invalid_token)?;
    let valid = token.strip_prefix("lfw_").is_some_and(|value| {
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    });
    if !valid || values.next().is_some() {
        return Err(PullError::invalid_token());
    }
    Ok(digest(token))
}

fn authorize(conn: &Connection, token_digest: &str, month: &str) -> PullResult<Link> {
    let row = conn
        .query_row(
            "SELECT id,revision,config FROM workload_api_links WHERE token_digest=?1",
            [token_digest],
            link_row,
        )
        .optional()
        .map_err(AppError::from)?
        .ok_or_else(PullError::invalid_token)?;
    let link = Link::from_row(row)?;
    if !link.config.enabled || link.expired {
        return Err(PullError::invalid_token());
    }
    if month < link.config.from_month.as_str()
        || link
            .config
            .through_month
            .as_deref()
            .is_some_and(|end| month > end)
    {
        return Err(AppError::Forbidden("所选月份不在此令牌的授权范围内".into()).into());
    }
    if link
        .config
        .departments
        .ids()
        .is_some_and(|ids| ids.is_empty() || ids.iter().any(|id| *id <= 0))
    {
        return Err(AppError::Forbidden("此令牌没有有效的检测部门范围".into()).into());
    }
    Ok(link)
}

fn live_snapshots(state: &IntegrationState) -> PullResult<MutexGuard<'_, HashMap<i64, Snapshot>>> {
    let mut snapshots = state
        .snapshots
        .lock()
        .map_err(|_| AppError::Internal("工作量 API 快照不可用".into()))?;
    snapshots.retain(|_, snapshot| snapshot.until > Instant::now());
    Ok(snapshots)
}

fn make_snapshot(conn: &Connection, month: NaiveDate, link: &Link) -> Result<Snapshot> {
    let end = month
        .checked_add_months(Months::new(1))
        .and_then(|date| date.pred_opt())
        .ok_or_else(|| AppError::Validation("所选月份超出范围".into()))?;
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY; SET LOCAL statement_timeout='20s'")?;
    let rows = integration_workload_records(
        &tx,
        month,
        end,
        link.config.kind.filter(),
        link.config.departments.ids(),
        MONTH_MAX_ROWS + 1,
    )?;
    tx.commit()?;
    if rows.len() > MONTH_MAX_ROWS {
        return Err(AppError::Validation(
            "本月工作量记录超过 50000 条，请按更小的部门范围配置接口".into(),
        ));
    }
    let invalid =
        || AppError::Validation("本月工作量包含无效数量或系数，不能作为完整月份同步".into());
    let mut totals = Totals::default();
    for row in &rows {
        if !row.is_valid() {
            return Err(invalid());
        }
        if row.is_auxiliary() {
            totals.auxiliary_workload += row.workload;
        } else {
            totals.detection_workload += row.workload;
            totals.detection_quantity = totals
                .detection_quantity
                .checked_add(row.detection_quantity)
                .ok_or_else(invalid)?;
        }
    }
    if !totals.auxiliary_workload.is_finite() || !totals.detection_workload.is_finite() {
        return Err(invalid());
    }
    Ok(Snapshot {
        key: Uuid::new_v4(),
        revision: link.revision,
        month,
        until: Instant::now() + SNAPSHOT_TTL,
        rows,
        totals,
    })
}

fn read_page(
    state: &IntegrationState,
    query: PullQuery,
    token_digest: &str,
) -> PullResult<PullPage> {
    let month = parse_month(&query.month)?;
    let limit = query.limit.unwrap_or(100);
    if !(1..=200).contains(&limit) || query.snapshot.is_some() != query.cursor.is_some() {
        return Err(
            AppError::Validation("分页大小须为 1–200；续读须同时提供快照和游标".into()).into(),
        );
    }
    let conn = state.pool.get().map_err(AppError::from)?;
    let link = authorize(&conn, token_digest, &query.month)?;
    if let Some(key) = query.snapshot {
        let key =
            Uuid::parse_str(&key).map_err(|_| AppError::Validation("分页快照标识无效".into()))?;
        let snapshots = live_snapshots(state)?;
        let snapshot = snapshots
            .get(&link.id)
            .filter(|snapshot| {
                snapshot.key == key && snapshot.revision == link.revision && snapshot.month == month
            })
            .ok_or_else(PullError::stale)?;
        return snapshot.page(query.month, query.cursor.unwrap_or(0), limit);
    }
    {
        let snapshots = live_snapshots(state)?;
        if snapshots.len() >= MAX_SNAPSHOTS && !snapshots.contains_key(&link.id) {
            return Err(PullError::busy());
        }
    }
    let snapshot = make_snapshot(&conn, month, &link)?;
    let mut snapshots = live_snapshots(state)?;
    if authorize(&conn, token_digest, &query.month)?.revision != link.revision {
        return Err(PullError::stale());
    }
    if snapshots.len() >= MAX_SNAPSHOTS && !snapshots.contains_key(&link.id) {
        return Err(PullError::busy());
    }
    let page = snapshot.page(query.month, 0, limit)?;
    snapshots.insert(link.id, snapshot);
    Ok(page)
}

async fn pull(
    State(state): State<IntegrationState>,
    headers: HeaderMap,
    Query(query): Query<PullQuery>,
) -> PullResult<Json<ApiResponse<PullPage>>> {
    let digest = token_digest(&headers)?;
    let permit = state
        .read_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| PullError::busy())?;
    let page = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        read_page(&state, query, &digest)
    })
    .await
    .map_err(|_| AppError::Internal("工作量 API 读取任务失败".into()))??;
    Ok(Json(ApiResponse::ok(page)))
}

#[cfg(test)]
#[path = "workload_api_tests.rs"]
mod tests;
