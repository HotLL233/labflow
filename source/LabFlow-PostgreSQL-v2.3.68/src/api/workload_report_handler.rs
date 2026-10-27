use super::export_data;
use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::ApiResponse;
use crate::service::authz_service::{self, AuthContext};
use axum::{
    extract::{Query, State},
    http::{header, HeaderMap},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use chrono::{Datelike, NaiveDate};
use postgres_compat::{Connection, Row};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Default, Deserialize)]
pub struct ReportQuery {
    pub start: Option<String>,
    pub end: Option<String>,
    pub view: Option<String>,
    pub subject_user_id: Option<i64>,
    pub division_id: Option<i64>,
    pub group_id: Option<i64>,
    pub project_id: Option<i64>,
    pub method_id: Option<i64>,
    pub instrument_id: Option<i64>,
    pub method_key: Option<String>,
    pub instrument_key: Option<String>,
    pub detection_division_ids: Option<String>,
    pub sending_division_ids: Option<String>,
    #[serde(rename = "type")]
    pub type_name: Option<String>,
    pub source: Option<String>,
    pub kind: Option<String>,
    pub auxiliary_category: Option<String>,
    pub auxiliary_key: Option<String>,
    pub compact_type_summary: Option<bool>,
    pub include_people: Option<bool>,
    pub group_by: Option<String>,
    pub include_pending_ownership: Option<bool>,
    pub include_zero_users: Option<bool>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[derive(Default, Serialize)]
pub struct Metrics {
    detection_quantity: i64,
    detection_workload: f64,
    auxiliary_quantity: i64,
    auxiliary_workload: f64,
    total_workload: f64,
    record_count: i64,
}

#[derive(Serialize)]
struct PersonRow {
    user_id: Option<i64>,
    user_name: String,
    #[serde(flatten)]
    metrics: Metrics,
}

#[derive(Serialize)]
struct MatrixRow {
    user_id: Option<i64>,
    user_name: String,
    #[serde(rename = "type")]
    type_name: String,
    #[serde(flatten)]
    metrics: Metrics,
}

#[derive(Serialize)]
struct BreakdownRow {
    key: String,
    name: String,
    #[serde(flatten)]
    metrics: Metrics,
}

#[derive(Serialize)]
struct TrendRow {
    period: String,
    #[serde(flatten)]
    metrics: Metrics,
}

#[derive(Serialize)]
struct DetailRow {
    id: i64,
    origin: String,
    kind: String,
    user_id: Option<i64>,
    user_name: String,
    recorded_at: String,
    department: String,
    lab: String,
    project: String,
    method: String,
    instrument: String,
    #[serde(rename = "type")]
    type_name: String,
    source: String,
    quantity: i64,
    coefficient: f64,
    workload: f64,
    source_record_id: Option<i64>,
    ownership_status: String,
    auxiliary_key: String,
    auxiliary_category: String,
}

#[derive(Serialize)]
struct Details {
    page: i64,
    page_size: i64,
    total: i64,
    items: Vec<DetailRow>,
}

#[derive(Serialize)]
struct ReportScope {
    view: String,
    can_view_scope: bool,
    subject_user_id: Option<i64>,
    allowed_division_ids: Option<Vec<i64>>,
}

#[derive(Serialize)]
struct Report {
    scope: ReportScope,
    summary: Metrics,
    people: Vec<PersonRow>,
    matrix: Vec<MatrixRow>,
    breakdowns: BTreeMap<String, Vec<BreakdownRow>>,
    trend: Vec<TrendRow>,
    details: Details,
}

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/stats/workload-report", get(report))
        .route("/api/stats/workload-report.xlsx", get(export_report))
        .with_state(pool)
}

pub(crate) fn scoped_subject(
    ctx: &AuthContext,
    requested: Option<i64>,
    mine: bool,
) -> Result<Option<i64>> {
    if requested.is_some_and(|id| id <= 0) {
        return Err(AppError::Validation("人员筛选值无效".into()));
    }
    if mine || !ctx.can_view_workload_scope() {
        if requested.is_some_and(|id| id != ctx.user.id) {
            return Err(AppError::Forbidden(
                "当前账号只能查看自己的工作量；查看所选人员需要授权范围统计权限".into(),
            ));
        }
        return Ok(Some(ctx.user.id));
    }
    Ok(requested)
}

fn scope(
    pool: &DbPool,
    headers: &HeaderMap,
    q: &ReportQuery,
    export: bool,
) -> Result<(AuthContext, ReportScope)> {
    let ctx = authz_service::authenticate(pool, headers)?;
    authz_service::require_permission(&ctx, "stats:portal:workload")?;
    if export {
        authz_service::require_permission(&ctx, "stats:workload:export")?;
    }
    let mine = match q.view.as_deref().unwrap_or("mine") {
        "mine" => true,
        "scope" if ctx.can_view_workload_scope() => false,
        "scope" => return Err(AppError::Forbidden("没有授权范围统计权限".into())),
        _ => return Err(AppError::Validation("统计视图无效".into())),
    };
    let scope = ReportScope {
        view: if mine { "mine" } else { "scope" }.into(),
        can_view_scope: ctx.can_view_workload_scope(),
        subject_user_id: scoped_subject(&ctx, q.subject_user_id, mine)?,
        allowed_division_ids: authz_service::work_allowed_division_ids(pool, &ctx)?,
    };
    Ok((ctx, scope))
}

fn dates(q: &ReportQuery) -> Result<(String, String)> {
    let now = chrono::Local::now().date_naive();
    let start = match q.start.as_deref() {
        Some(value) => NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .map_err(|_| AppError::Validation("开始日期无效".into()))?,
        None => now.with_day(1).unwrap(),
    };
    let end = match q.end.as_deref() {
        Some(value) => NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .map_err(|_| AppError::Validation("结束日期无效".into()))?,
        None => now,
    };
    if start > end {
        return Err(AppError::Validation("开始日期不能晚于结束日期".into()));
    }
    let next = end
        .succ_opt()
        .ok_or_else(|| AppError::Validation("结束日期超出范围".into()))?;
    Ok((format!("{start}T00:00:00"), format!("{next}T00:00:00")))
}

// Both the screen and workbook query this authorized, one-row-per-record set.
fn report_cte(
    conn: &Connection,
    q: &ReportQuery,
    scope: &ReportScope,
    viewer_user_id: i64,
) -> Result<(String, Vec<String>)> {
    let (start, end) = dates(q)?;
    let categories = crate::repo::settings_repo::workload_auxiliary_categories_on_conn(conn)?;
    let mut params = vec![start, end, serde_json::json!(categories).to_string()];
    let department = authz_service::work_record_authorization_division_sql("wr");
    let auxiliary_record = export_data::legacy_auxiliary_record_sql("wr");
    let work_scope = match scope.allowed_division_ids.as_deref() {
        None => String::new(),
        Some([]) => " AND FALSE".into(),
        Some(ids) => format!(
            " AND {department} IN ({})",
            ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
        ),
    };
    let work_subject = scope
        .subject_user_id
        .map(|id| format!(" AND wr.subject_user_id={id}"))
        .unwrap_or_default();
    let aux_subject = scope
        .subject_user_id
        .map(|id| format!(" AND awr.user_id={id}"))
        .unwrap_or_default();
    let pending = if q.include_pending_ownership.unwrap_or(false) {
        ""
    } else {
        " AND COALESCE(wr.ownership_status,'confirmed')<>'pending_confirmation'"
    };
    let aux_scope = export_data::auxiliary_scope_sql(
        "awr",
        scope.allowed_division_ids.as_deref(),
        Some(viewer_user_id),
    );
    let mut filters = Vec::new();
    if let Some(kind) = q.kind.as_deref() {
        if kind != "detection" && kind != "auxiliary" {
            return Err(AppError::Validation("工作类别筛选值无效".into()));
        }
        params.push(kind.into());
        filters.push(format!("kind=?{}", params.len()));
    }
    for (column, raw, label) in [
        (
            "detection_department_id",
            q.detection_division_ids.as_deref(),
            "检测部门",
        ),
        (
            "sending_department_id",
            q.sending_division_ids.as_deref(),
            "送样部门",
        ),
    ] {
        if let Some(ids) = crate::repo::record_repo::parse_dimension_ids(raw, label)? {
            if ids.is_empty() || ids.iter().any(|id| *id <= 0) {
                return Err(AppError::Validation(format!("{label}筛选值无效")));
            }
            let values = ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
            let predicate = format!("{column} IN ({values})");
            // Sending filters retain public auxiliary work, just like the existing personnel report.
            filters.push(if column == "sending_department_id" {
                format!("(kind='auxiliary' OR {predicate})")
            } else {
                predicate
            });
        }
    }
    for (column, value) in [
        ("department_id", q.division_id),
        ("lab_id", q.group_id),
        ("project_id", q.project_id),
        ("method_id", q.method_id),
        ("instrument_id", q.instrument_id),
    ] {
        if let Some(id) = value {
            if id <= 0 {
                return Err(AppError::Validation("分类筛选值无效".into()));
            }
            // Auxiliary work has no laboratory/project/method/instrument allocation.
            filters.push(if column == "department_id" {
                format!("{column}={id}")
            } else {
                format!("(kind='auxiliary' OR {column}={id})")
            });
        }
    }
    for (column, value) in [
        ("type_name", q.type_name.as_deref()),
        ("source", q.source.as_deref()),
        ("auxiliary_category", q.auxiliary_category.as_deref()),
        ("auxiliary_key", q.auxiliary_key.as_deref()),
    ] {
        if let Some(value) = value.filter(|value| !value.is_empty()) {
            params.push(value.into());
            filters.push(format!("{column}=?{}", params.len()));
        }
    }
    for (id, name, value) in [
        ("method_id", "method", q.method_key.as_deref()),
        ("instrument_id", "instrument", q.instrument_key.as_deref()),
    ] {
        if let Some(value) = value.filter(|value| !value.is_empty()) {
            params.push(value.into());
            filters.push(format!(
                "COALESCE(CAST({id} AS TEXT),'legacy:'||{name})=?{}",
                params.len()
            ));
        }
    }
    let filter = if filters.is_empty() {
        "TRUE".into()
    } else {
        filters.join(" AND ")
    };
    Ok((
        format!(
            r#"WITH auxiliary_categories AS (SELECT CAST(CAST(?3 AS TEXT) AS JSONB) AS value), raw_records AS (
        SELECT wr.id, CASE WHEN {auxiliary_record} THEN 'auxiliary' ELSE 'detection' END AS kind,
          wr.subject_user_id AS user_id, COALESCE(u.username, '历史未绑定：' || wr.user_name) AS user_name,
          CASE WHEN wr.subject_user_id IS NULL THEN wr.user_name ELSE '' END AS legacy_name,
          CAST(wr.recorded_at AS TIMESTAMPTZ) AS recorded_at,
          {department} AS department_id,
          wr.detection_division_id AS detection_department_id,
          {sending_department} AS sending_department_id,
          COALESCE(NULLIF(wr.detection_division_name_snapshot,''),d.name,'未绑定部门') AS department,
          CASE WHEN {auxiliary_record} THEN NULL::BIGINT ELSE wr.group_id END AS lab_id,
          CASE WHEN {auxiliary_record} THEN '' ELSE COALESCE(NULLIF(wr.lab_name_snapshot,''),g.name,'未绑定实验室') END AS lab,
          CASE WHEN {auxiliary_record} THEN NULL::BIGINT ELSE wr.project_id END AS project_id,
          CASE WHEN {auxiliary_record} THEN '' ELSE COALESCE(NULLIF(wr.project_name_snapshot,''),p.name,'未绑定项目') END AS project,
          wr.method_id, COALESCE(NULLIF(linked_aux.auxiliary_work_name_snapshot,''),NULLIF(wr.method_name_snapshot,''),m.name,'未绑定方法') AS method,
          CASE WHEN {auxiliary_record} THEN NULL::BIGINT ELSE wr.instrument_id_snapshot END AS instrument_id,
          CASE WHEN {auxiliary_record} THEN '' ELSE COALESCE(NULLIF(wr.instrument_code_snapshot,''),i.code,'未绑定仪器') END AS instrument,
          CASE WHEN {auxiliary_record} THEN '辅助工作'
            ELSE COALESCE(NULLIF(wr.detection_type_snapshot,''),(SELECT string_agg(DISTINCT mt.name, '、' ORDER BY mt.name) FROM method_type_links ml JOIN method_types mt ON mt.id=ml.method_type_id WHERE ml.method_id=wr.method_id),NULLIF(wr.instrument_type_snapshot,''),'未分类') END AS type_name,
          CASE WHEN {auxiliary_record} THEN 'auxiliary_work' ELSE COALESCE(NULLIF(wr.source_type,''),'analysis') END AS source,
          wr.quantity, COALESCE(wr.coefficient_snapshot,1.0)::DOUBLE PRECISION AS coefficient,
          (wr.quantity*COALESCE(wr.coefficient_snapshot,1.0))::DOUBLE PRECISION AS workload,
          wr.source_record_id, COALESCE(wr.ownership_status,'confirmed') AS ownership_status,
          CASE WHEN NOT {auxiliary_record} THEN ''
            WHEN linked_aux.id IS NOT NULL THEN COALESCE('item:'||CAST(linked_aux.auxiliary_work_item_id AS TEXT),'legacy:item:'||linked_aux.auxiliary_work_name_snapshot)
            ELSE COALESCE('method:'||CAST(wr.method_id AS TEXT),'legacy:method:'||COALESCE(NULLIF(wr.method_name_snapshot,''),'未绑定方法')) END AS auxiliary_key,
          CASE WHEN NOT {auxiliary_record} THEN '' ELSE COALESCE(NULLIF(wr.auxiliary_type_snapshot,''),NULLIF(linked_aux.auxiliary_type_snapshot,''),
            NULLIF((SELECT value FROM auxiliary_categories)->>CASE WHEN linked_aux.id IS NOT NULL THEN 'item:'||CAST(linked_aux.auxiliary_work_item_id AS TEXT) ELSE 'method:'||CAST(wr.method_id AS TEXT) END,''),'未分类') END AS auxiliary_category,
          'work_record' AS origin
        FROM work_records wr LEFT JOIN users u ON u.id=wr.subject_user_id LEFT JOIN projects p ON p.id=wr.project_id
          LEFT JOIN project_groups g ON g.id=wr.group_id LEFT JOIN divisions d ON d.id={department}
          LEFT JOIN methods m ON m.id=wr.method_id LEFT JOIN instruments i ON i.id=wr.instrument_id_snapshot
          LEFT JOIN auxiliary_work_records linked_aux ON wr.source_type='auxiliary_work' AND wr.source_record_id=linked_aux.id
        WHERE wr.deleted_at IS NULL AND CAST(wr.recorded_at AS TIMESTAMPTZ)>=CAST(CAST(?1 AS TEXT) AS TIMESTAMPTZ)
          AND CAST(wr.recorded_at AS TIMESTAMPTZ)<CAST(CAST(?2 AS TEXT) AS TIMESTAMPTZ){work_scope}{work_subject}{pending}
        UNION ALL
        SELECT awr.id, 'auxiliary', awr.user_id, COALESCE(u.username,'历史未绑定：' || awr.user_name_snapshot),
          CASE WHEN awr.user_id IS NULL THEN awr.user_name_snapshot ELSE '' END, awr.recorded_at,
          awr.detection_division_id, awr.detection_division_id, NULL::BIGINT, COALESCE(NULLIF(awr.detection_division_name_snapshot,''),d.name,'历史未归属部门'),
          NULL::BIGINT, '', NULL::BIGINT, '', NULL::BIGINT, awr.auxiliary_work_name_snapshot,
          NULL::BIGINT, '', '辅助工作', 'auxiliary_work', awr.quantity, awr.coefficient_snapshot,
          (awr.quantity*awr.coefficient_snapshot)::DOUBLE PRECISION, NULL::BIGINT, 'confirmed',
          COALESCE('item:'||CAST(awr.auxiliary_work_item_id AS TEXT),'legacy:item:'||awr.auxiliary_work_name_snapshot),
          COALESCE(NULLIF(awr.auxiliary_type_snapshot,''),NULLIF((SELECT value FROM auxiliary_categories)->>('item:'||CAST(awr.auxiliary_work_item_id AS TEXT)),''),'未分类'),
          'auxiliary_record'
        FROM auxiliary_work_records awr LEFT JOIN users u ON u.id=awr.user_id LEFT JOIN divisions d ON d.id=awr.detection_division_id
        WHERE awr.deleted_at IS NULL AND awr.recorded_at>=CAST(CAST(?1 AS TEXT) AS TIMESTAMPTZ) AND awr.recorded_at<CAST(CAST(?2 AS TEXT) AS TIMESTAMPTZ){aux_subject}{aux_scope}
      ), records AS (SELECT * FROM raw_records WHERE {filter}) "#,
            sending_department = authz_service::work_record_sending_division_sql("wr")
        ),
        params,
    ))
}

const METRICS: &str = "COALESCE(SUM(quantity) FILTER(WHERE kind='detection'),0)::BIGINT, COALESCE(SUM(workload) FILTER(WHERE kind='detection'),0)::DOUBLE PRECISION, COALESCE(SUM(quantity) FILTER(WHERE kind='auxiliary'),0)::BIGINT, COALESCE(SUM(workload) FILTER(WHERE kind='auxiliary'),0)::DOUBLE PRECISION, COALESCE(SUM(workload),0)::DOUBLE PRECISION, COUNT(*)::BIGINT";

fn metrics(row: &Row, offset: usize) -> postgres_compat::Result<Metrics> {
    Ok(Metrics {
        detection_quantity: row.get(offset)?,
        detection_workload: row.get(offset + 1)?,
        auxiliary_quantity: row.get(offset + 2)?,
        auxiliary_workload: row.get(offset + 3)?,
        total_workload: row.get(offset + 4)?,
        record_count: row.get(offset + 5)?,
    })
}

fn rows<T>(
    conn: &Connection,
    sql: &str,
    params: &[String],
    map: impl FnMut(&Row) -> postgres_compat::Result<T>,
) -> Result<Vec<T>> {
    let mut stmt = conn.prepare(sql)?;
    let data = stmt.query_map(postgres_compat::params_from_iter(params.iter()), map)?;
    Ok(data.collect::<std::result::Result<Vec<_>, _>>()?)
}

fn details(
    conn: &Connection,
    cte: &str,
    params: &[String],
    limit: i64,
    offset: i64,
) -> Result<Vec<DetailRow>> {
    rows(conn, &format!("{cte} SELECT id,kind,user_id,user_name,CAST(recorded_at AS TEXT),department,lab,project,method,instrument,type_name,source,quantity,coefficient,workload,source_record_id,ownership_status,auxiliary_key,auxiliary_category,origin FROM records ORDER BY recorded_at DESC,origin,id DESC LIMIT {limit} OFFSET {offset}"), params,
      |row| Ok(DetailRow { id: row.get(0)?, origin: row.get(19)?, kind: row.get(1)?, user_id: row.get(2)?, user_name: row.get(3)?, recorded_at: row.get(4)?, department: row.get(5)?, lab: row.get(6)?, project: row.get(7)?, method: row.get(8)?, instrument: row.get(9)?, type_name: row.get(10)?, source: row.get(11)?, quantity: row.get(12)?, coefficient: row.get(13)?, workload: row.get(14)?, source_record_id: row.get(15)?, ownership_status: row.get(16)?, auxiliary_key: row.get(17)?, auxiliary_category: row.get(18)? }))
}

fn query_report(
    conn: &Connection,
    q: &ReportQuery,
    scope: ReportScope,
    viewer: i64,
) -> Result<Report> {
    let (cte, params) = report_cte(conn, q, &scope, viewer)?;
    let summary = rows(
        conn,
        &format!("{cte} SELECT {METRICS} FROM records"),
        &params,
        |r| metrics(r, 0),
    )?
    .remove(0);
    let compact = q.compact_type_summary.unwrap_or(false);
    let include_people = !compact || q.include_people.unwrap_or(false);
    let mut people = if !include_people {
        Vec::new()
    } else {
        rows(conn, &format!("{cte} SELECT user_id,MAX(user_name),{METRICS} FROM records GROUP BY user_id,legacy_name ORDER BY 7 DESC,2"), &params, |r| Ok(PersonRow { user_id: r.get(0)?, user_name: r.get(1)?, metrics: metrics(r,2)? }))?
    };
    if include_people && q.include_zero_users.unwrap_or(false) {
        let mut user_scope = "u.is_active=1 AND u.deleted_at IS NULL".to_string();
        if let Some(id) = scope.subject_user_id {
            user_scope.push_str(&format!(" AND u.id={id}"));
        }
        if let Some(ids) = scope.allowed_division_ids.as_deref() {
            if ids.is_empty() {
                user_scope.push_str(" AND FALSE");
            } else {
                let ids = ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
                user_scope.push_str(&format!(" AND (u.division_id IN ({ids}) OR EXISTS(SELECT 1 FROM user_divisions ud WHERE ud.user_id=u.id AND ud.division_id IN ({ids})))"));
            }
        }
        if let Some(id) = q.division_id {
            user_scope.push_str(&format!(" AND (u.division_id={id} OR EXISTS(SELECT 1 FROM user_divisions ud WHERE ud.user_id=u.id AND ud.division_id={id}))"));
        }
        let zero_users = rows(
            conn,
            &format!("SELECT u.id,u.username FROM users u WHERE {user_scope} ORDER BY u.username"),
            &[],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
        )?;
        for (id, name) in zero_users {
            if !people.iter().any(|p| p.user_id == Some(id)) {
                people.push(PersonRow {
                    user_id: Some(id),
                    user_name: name,
                    metrics: Metrics::default(),
                });
            }
        }
    }
    let matrix = if compact {
        Vec::new()
    } else {
        rows(conn, &format!("{cte} SELECT user_id,MAX(user_name),type_name,{METRICS} FROM records GROUP BY user_id,legacy_name,type_name ORDER BY 2,3"), &params, |r| Ok(MatrixRow { user_id:r.get(0)?,user_name:r.get(1)?,type_name:r.get(2)?,metrics:metrics(r,3)? }))?
    };
    let mut breakdowns = BTreeMap::new();
    for (key, id, name) in [
        ("department", "department_id", "department"),
        ("lab", "lab_id", "lab"),
        ("project", "project_id", "project"),
        ("method", "method_id", "method"),
        ("instrument", "instrument_id", "instrument"),
        ("source", "source", "source"),
        ("type", "type_name", "type_name"),
    ] {
        let only_detection = if key == "source" || key == "type" {
            ""
        } else {
            " WHERE kind='detection'"
        };
        let grouping = if key == "source" || key == "type" {
            id.to_string()
        } else {
            format!("{id},CASE WHEN {id} IS NULL THEN {name} END")
        };
        let data=rows(conn,&format!("{cte} SELECT COALESCE(CAST({id} AS TEXT),'legacy:'||MAX({name})),MAX({name}),{METRICS} FROM records{only_detection} GROUP BY {grouping} ORDER BY 7 DESC,2"),&params,|r| Ok(BreakdownRow { key:r.get(0)?,name:r.get(1)?,metrics:metrics(r,2)? }))?;
        breakdowns.insert(key.to_string(), data);
    }
    for (key, id, name) in [
        (
            "auxiliary_category",
            "auxiliary_category",
            "auxiliary_category",
        ),
        ("auxiliary_method", "auxiliary_key", "method"),
    ] {
        let data = rows(conn, &format!("{cte} SELECT {id},MAX({name}),{METRICS} FROM records WHERE kind='auxiliary' GROUP BY {id} ORDER BY 7 DESC,2"), &params,
            |r| Ok(BreakdownRow { key:r.get(0)?, name:r.get(1)?, metrics:metrics(r,2)? }))?;
        breakdowns.insert(key.to_string(), data);
    }
    let period = match q.group_by.as_deref().unwrap_or("day") {
        "day" => "YYYY-MM-DD",
        "week" => "IYYY-\"W\"IW",
        "month" => "YYYY-MM",
        _ => return Err(AppError::Validation("趋势周期无效".into())),
    };
    let trend = if compact {
        Vec::new()
    } else {
        rows(conn,&format!("{cte} SELECT to_char(recorded_at,'{period}') AS period,{METRICS} FROM records GROUP BY period ORDER BY period"),&params,|r| Ok(TrendRow { period:r.get(0)?,metrics:metrics(r,1)? }))?
    };
    let page = q.page.unwrap_or(1).max(1);
    let page_size = q.page_size.unwrap_or(50).clamp(1, 200);
    let offset = (page - 1)
        .checked_mul(page_size)
        .ok_or_else(|| AppError::Validation("页码超出范围".into()))?;
    let items = details(conn, &cte, &params, page_size, offset)?;
    let total = summary.record_count;
    Ok(Report {
        scope,
        summary,
        people,
        matrix,
        breakdowns,
        trend,
        details: Details {
            page,
            page_size,
            total,
            items,
        },
    })
}

async fn report(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<ReportQuery>,
) -> Result<Json<ApiResponse<Report>>> {
    let (ctx, scope) = scope(&pool, &headers, &q, false)?;
    let conn = pool.get()?;
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")?;
    let report = query_report(&tx, &q, scope, ctx.user.id)?;
    Ok(Json(ApiResponse::ok(report)))
}

fn write_metrics(
    ws: &mut rust_xlsxwriter::Worksheet,
    row: u32,
    start: u16,
    m: &Metrics,
) -> Result<()> {
    for (i, value) in [
        m.detection_quantity as f64,
        m.detection_workload,
        m.auxiliary_quantity as f64,
        m.auxiliary_workload,
        m.total_workload,
        m.record_count as f64,
    ]
    .into_iter()
    .enumerate()
    {
        ws.write_number(row, start + i as u16, value)?;
    }
    Ok(())
}

fn headers(ws: &mut rust_xlsxwriter::Worksheet, leading: &[&str]) -> Result<()> {
    for (i, name) in leading
        .iter()
        .copied()
        .chain([
            "检测数量",
            "检测工作量",
            "辅助数量",
            "辅助工作量",
            "总工作量",
            "记录数",
        ])
        .enumerate()
    {
        ws.write_string(0, i as u16, name)?;
    }
    ws.set_column_width(0, 25)?;
    Ok(())
}

fn source_label(source: &str) -> &str {
    match source {
        "analysis" => "分析检测",
        "rd_sample" => "研发送样取样录入",
        "sample_info_sample" => "样品信息登记取样录入",
        "auxiliary_work" => "辅助工作",
        _ => source,
    }
}

async fn export_report(
    State(pool): State<DbPool>,
    request_headers: HeaderMap,
    Query(mut q): Query<ReportQuery>,
) -> Result<Response> {
    q.compact_type_summary = Some(false);
    let (ctx, scope) = scope(&pool, &request_headers, &q, true)?;
    let conn = pool.get()?;
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")?;
    let (cte, params) = report_cte(&tx, &q, &scope, ctx.user.id)?;
    let report = query_report(&tx, &q, scope, ctx.user.id)?;
    // Excel cannot represent more than 1,048,575 detail records plus its header.
    if report.details.total > 1_048_575 {
        return Err(AppError::Validation(
            "明细超出 Excel 行数上限，请缩小日期范围后导出".into(),
        ));
    }
    let mut wb = rust_xlsxwriter::Workbook::new();
    let ws = wb.add_worksheet();
    ws.set_name("工作量总览")?;
    headers(ws, &["范围"])?;
    ws.write_string(
        1,
        0,
        if report.scope.view == "mine" {
            "我的工作量"
        } else {
            "授权范围"
        },
    )?;
    write_metrics(ws, 1, 1, &report.summary)?;
    ws.write_string(3, 0, "工作量=数量×系数快照，辅助工作不分摊实验室/项目/仪器")?;
    let ws = wb.add_worksheet();
    ws.set_name("人员汇总")?;
    headers(ws, &["人员ID", "人员"])?;
    for (i, p) in report.people.iter().enumerate() {
        if let Some(id) = p.user_id {
            ws.write_number(i as u32 + 1, 0, id as f64)?;
        }
        ws.write_string(i as u32 + 1, 1, &p.user_name)?;
        write_metrics(ws, i as u32 + 1, 2, &p.metrics)?;
    }
    let ws = wb.add_worksheet();
    ws.set_name("人员类型矩阵")?;
    let types = report
        .matrix
        .iter()
        .map(|row| row.type_name.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if types.len() > 16_380 {
        return Err(AppError::Validation(
            "检测类型超过 Excel 列数上限，请筛选类型后导出".into(),
        ));
    }
    ws.write_string(0, 0, "人员")?;
    for (i, t) in types.iter().enumerate() {
        ws.write_string(0, i as u16 + 1, t)?;
    }
    ws.write_string(0, types.len() as u16 + 1, "行合计")?;
    for (i, p) in report.people.iter().enumerate() {
        let row = i as u32 + 1;
        ws.write_string(row, 0, &p.user_name)?;
        for (j, t) in types.iter().enumerate() {
            let score = report
                .matrix
                .iter()
                .filter(|m| {
                    m.user_id == p.user_id && m.user_name == p.user_name && &m.type_name == t
                })
                .map(|m| m.metrics.total_workload)
                .sum::<f64>();
            ws.write_number(row, j as u16 + 1, score)?;
        }
        ws.write_number(row, types.len() as u16 + 1, p.metrics.total_workload)?;
    }
    let total_row = report.people.len() as u32 + 1;
    ws.write_string(total_row, 0, "列合计")?;
    for (i, t) in types.iter().enumerate() {
        ws.write_number(
            total_row,
            i as u16 + 1,
            report
                .matrix
                .iter()
                .filter(|m| &m.type_name == t)
                .map(|m| m.metrics.total_workload)
                .sum::<f64>(),
        )?;
    }
    ws.write_number(
        total_row,
        types.len() as u16 + 1,
        report.summary.total_workload,
    )?;
    for (key, name) in [
        ("department", "部门"),
        ("lab", "实验室"),
        ("project", "项目"),
        ("method", "方法"),
        ("instrument", "仪器"),
        ("source", "来源"),
        ("type", "检测类型"),
        ("auxiliary_category", "辅助类型"),
        ("auxiliary_method", "辅助方法事项"),
    ] {
        let ws = wb.add_worksheet();
        ws.set_name(name)?;
        headers(ws, &[name])?;
        ws.write_string(0, 7, "分类标识")?;
        for (i, p) in report.breakdowns[key].iter().enumerate() {
            ws.write_string(
                i as u32 + 1,
                0,
                if key == "source" {
                    source_label(&p.name)
                } else {
                    &p.name
                },
            )?;
            write_metrics(ws, i as u32 + 1, 1, &p.metrics)?;
            ws.write_string(i as u32 + 1, 7, &p.key)?;
        }
    }
    let ws = wb.add_worksheet();
    ws.set_name("趋势")?;
    headers(ws, &["周期"])?;
    for (i, p) in report.trend.iter().enumerate() {
        ws.write_string(i as u32 + 1, 0, &p.period)?;
        write_metrics(ws, i as u32 + 1, 1, &p.metrics)?;
    }
    let ws = wb.add_worksheet();
    ws.set_name("当前筛选全部明细")?;
    for (i, name) in [
        "记录ID",
        "类别",
        "人员ID",
        "人员",
        "日期",
        "部门",
        "实验室",
        "项目",
        "方法",
        "仪器",
        "类型",
        "来源",
        "数量",
        "系数快照",
        "工作量",
        "来源记录ID",
        "归属状态",
        "辅助分类标识",
        "辅助子类型",
        "维护入口",
    ]
    .iter()
    .enumerate()
    {
        ws.write_string(0, i as u16, *name)?;
    }
    let mut offset = 0;
    while offset < report.details.total {
        let batch = details(&tx, &cte, &params, 2000, offset)?;
        for (i, d) in batch.iter().enumerate() {
            let r = offset as u32 + i as u32 + 1;
            ws.write_number(r, 0, d.id as f64)?;
            ws.write_string(
                r,
                1,
                if d.kind == "auxiliary" {
                    "辅助工作"
                } else {
                    "检测工作"
                },
            )?;
            ws.write_string(r, 17, &d.auxiliary_key)?;
            ws.write_string(r, 18, &d.auxiliary_category)?;
            ws.write_string(
                r,
                19,
                if d.origin == "auxiliary_record" {
                    "辅助工作入口"
                } else {
                    "原工作量入口"
                },
            )?;
            if let Some(id) = d.user_id {
                ws.write_number(r, 2, id as f64)?;
            }
            for (j, value) in [
                &d.user_name,
                &d.recorded_at,
                &d.department,
                &d.lab,
                &d.project,
                &d.method,
                &d.instrument,
                &d.type_name,
            ]
            .iter()
            .enumerate()
            {
                ws.write_string(r, j as u16 + 3, *value)?;
            }
            ws.write_string(r, 11, source_label(&d.source))?;
            ws.write_number(r, 12, d.quantity as f64)?;
            ws.write_number(r, 13, d.coefficient)?;
            ws.write_number(r, 14, d.workload)?;
            if let Some(id) = d.source_record_id {
                ws.write_number(r, 15, id as f64)?;
            }
            ws.write_string(
                r,
                16,
                if d.ownership_status == "pending_confirmation" {
                    "待确认"
                } else {
                    "已确认"
                },
            )?;
        }
        if batch.is_empty() {
            break;
        }
        offset += batch.len() as i64;
    }
    let bytes = wb.save_to_buffer()?;
    Ok((
        [
            (
                header::CONTENT_TYPE,
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            ),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=LabFlow-workload-report.xlsx",
            ),
        ],
        bytes,
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::user_repo;
    use calamine::Reader;

    fn fixture() -> (DbPool, i64, i64, i64, i64, i64) {
        let pool = crate::db::init_pool("postgres-test");
        let conn = pool.get().unwrap();
        crate::db::test_migrations::run(&conn).unwrap();
        conn.execute("INSERT INTO divisions(name) VALUES('report_A')", [])
            .unwrap();
        let a = conn.last_insert_rowid();
        conn.execute("INSERT INTO divisions(name) VALUES('report_B')", [])
            .unwrap();
        let b = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO users(username,password,division_id) VALUES('report_alice','hash',?1)",
            [a],
        )
        .unwrap();
        let alice = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO users(username,password,division_id) VALUES('report_bob','hash',?1)",
            [b],
        )
        .unwrap();
        let bob = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO users(username,password,division_id) VALUES('report_zero','hash',?1)",
            [a],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO project_groups(name,division_id) VALUES('report_lab',?1)",
            [a],
        )
        .unwrap();
        let lab = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO projects(name,coefficient,group_id) VALUES('report_project',99,?1)",
            [lab],
        )
        .unwrap();
        let project = conn.last_insert_rowid();
        for (owner, division, qty, at, status, source) in [
            (
                alice,
                a,
                3,
                "2091-01-31T23:59:59.500",
                "confirmed",
                "rd_sample",
            ),
            (bob, b, 7, "2091-01-31T12:00:00", "confirmed", "analysis"),
            (
                alice,
                a,
                11,
                "2091-01-31T13:00:00",
                "pending_confirmation",
                "analysis",
            ),
            (alice, a, 13, "2091-02-01T00:00:00", "confirmed", "analysis"),
        ] {
            conn.execute("INSERT INTO work_records(project_id,group_id,user_name,subject_user_id,quantity,recorded_at,coefficient_snapshot,multiplier,detection_division_id,ownership_status,source_type,business_no) VALUES(?1,?2,'stale-name',?3,?4,?5,2,99,?6,?7,?8,?9)",postgres_compat::params![project,lab,owner,qty,at,division,status,source,format!("test_{owner}_{qty}")]).unwrap();
        }
        for (owner, division, qty) in [
            (alice, Some(a), 2),
            (bob, Some(b), 5),
            (alice, None, 1),
            (bob, None, 17),
        ] {
            conn.execute("INSERT INTO auxiliary_work_records(user_id,user_name_snapshot,auxiliary_work_name_snapshot,quantity,coefficient_snapshot,recorded_at,detection_division_id) VALUES(?1,'stale-name','report_aux',?2,3,'2091-01-31T12:00:00',?3)",postgres_compat::params![owner,qty,division]).unwrap();
        }
        // A real linked compatibility row proves duplication; partial RD entries remain separate.
        conn.execute("INSERT INTO auxiliary_work_records(user_id,user_name_snapshot,auxiliary_work_name_snapshot,quantity,coefficient_snapshot,recorded_at,detection_division_id) VALUES(?1,'stale-name','linked',4,3,'2091-01-31T14:00:00',?2)",postgres_compat::params![alice,a]).unwrap();
        let linked = conn.last_insert_rowid();
        conn.execute("INSERT INTO work_records(project_id,user_name,subject_user_id,quantity,recorded_at,coefficient_snapshot,detection_division_id,source_type,source_record_id,instrument_type_snapshot) VALUES(?1,'stale-name',?2,4,'2091-01-31T14:00:00',3,?3,'auxiliary_work',?4,'辅助工作')",postgres_compat::params![project,alice,a,linked]).unwrap();
        drop(conn);
        (pool, a, b, alice, bob, lab)
    }

    fn query() -> ReportQuery {
        ReportQuery {
            start: Some("2091-01-31".into()),
            end: Some("2091-01-31".into()),
            view: Some("scope".into()),
            ..Default::default()
        }
    }
    fn report_scope(ids: Option<Vec<i64>>, subject: Option<i64>) -> ReportScope {
        ReportScope {
            view: "scope".into(),
            can_view_scope: true,
            subject_user_id: subject,
            allowed_division_ids: ids,
        }
    }

    #[test]
    fn independent_auxiliary_origin_zero_and_void_status_are_consistent() {
        let (pool, a, _, alice, _, _) = fixture();
        let conn = pool.get().unwrap();
        conn.execute("INSERT INTO work_records(id,project_id,user_name,subject_user_id,quantity,coefficient_snapshot,recorded_at,detection_division_id,instrument_type_snapshot,business_no) SELECT 900000,project_id,'old_name',?1,1,0,'2091-01-31T12:00:00',?2,'辅助工作','origin68_zero' FROM work_records WHERE source_type='rd_sample'",postgres_compat::params![alice,a]).unwrap();
        conn.execute("INSERT INTO auxiliary_work_records(id,user_id,user_name_snapshot,auxiliary_work_name_snapshot,quantity,coefficient_snapshot,recorded_at,detection_division_id) VALUES(900000,?1,'old_name','zero',2,0,'2091-01-31T12:00:00',?2)",postgres_compat::params![alice,a]).unwrap();
        let result = query_report(
            &conn,
            &query(),
            report_scope(Some(vec![a]), Some(alice)),
            alice,
        )
        .unwrap();
        let collision = result
            .details
            .items
            .iter()
            .filter(|row| row.id == 900000)
            .collect::<Vec<_>>();
        assert_eq!(collision.len(), 2);
        assert_ne!(collision[0].origin, collision[1].origin);
        assert!(collision
            .iter()
            .all(|row| row.workload == 0.0 && row.kind == "auxiliary"));
        assert_eq!(result.summary.total_workload, 27.0);
        conn.execute("UPDATE auxiliary_work_records SET status='作废' WHERE user_id=?1 AND auxiliary_work_name_snapshot='report_aux' AND detection_division_id=?2",postgres_compat::params![alice,a]).unwrap();
        let void = query_report(
            &conn,
            &query(),
            report_scope(Some(vec![a]), Some(alice)),
            alice,
        )
        .unwrap();
        assert_eq!(void.summary.total_workload, 21.0);
        conn.execute("UPDATE auxiliary_work_records SET status='历史状态' WHERE user_id=?1 AND auxiliary_work_name_snapshot='report_aux' AND detection_division_id=?2",postgres_compat::params![alice,a]).unwrap();
        let retained = query_report(
            &conn,
            &query(),
            report_scope(Some(vec![a]), Some(alice)),
            alice,
        )
        .unwrap();
        assert_eq!(retained.summary.total_workload, 27.0);
        let exported = export_data::query_person_type_workload_data(
            &conn,
            "2091-01-31",
            "2091-01-31",
            Some(alice),
            Some(&[a]),
        )
        .unwrap();
        assert!(exported.iter().all(|row| row.user_id == Some(alice)));
    }

    #[test]
    fn report_isolated_departments_person_id_snapshots_and_month_boundary() {
        let (pool, a, _, alice, bob, lab) = fixture();
        let conn = pool.get().unwrap();
        let mut q = query();
        q.page_size = Some(1);
        q.include_zero_users = Some(true);
        let r = query_report(&conn, &q, report_scope(Some(vec![a]), None), alice).unwrap();
        assert_eq!(r.summary.detection_quantity, 3);
        assert_eq!(r.summary.detection_workload, 6.0);
        assert_eq!(r.summary.auxiliary_quantity, 7);
        assert_eq!(r.summary.auxiliary_workload, 21.0);
        assert_eq!(r.summary.total_workload, 27.0);
        assert_eq!(r.details.total, 4);
        assert_eq!(r.details.items.len(), 1);
        assert!(r
            .people
            .iter()
            .any(|p| p.user_name == "report_zero" && p.metrics.record_count == 0));
        assert!(!r.people.iter().any(|p| p.user_id == Some(bob)));
        assert_eq!(
            r.breakdowns["lab"]
                .iter()
                .map(|r| r.metrics.total_workload)
                .sum::<f64>(),
            6.0
        );
        assert_eq!(
            r.matrix
                .iter()
                .map(|r| r.metrics.total_workload)
                .sum::<f64>(),
            27.0
        );
        let empty = query_report(&conn, &q, report_scope(Some(vec![]), None), alice).unwrap();
        assert_eq!(empty.summary.total_workload, 3.0);
        assert_eq!(empty.summary.detection_quantity, 0);
        q.include_pending_ownership = Some(true);
        let pending =
            query_report(&conn, &q, report_scope(Some(vec![a]), Some(alice)), alice).unwrap();
        assert_eq!(pending.summary.detection_quantity, 14);
        q.group_id = Some(lab + 999);
        q.include_pending_ownership = Some(false);
        let other_lab =
            query_report(&conn, &q, report_scope(Some(vec![a]), Some(alice)), alice).unwrap();
        assert_eq!(other_lab.summary.detection_workload, 0.0);
        assert_eq!(other_lab.summary.auxiliary_workload, 21.0);
        let global = query_report(&conn, &query(), report_scope(None, None), alice).unwrap();
        assert_eq!(global.summary.detection_quantity, 10);
        assert_eq!(global.summary.total_workload, 107.0);
        let mut person = user_repo::find_by_id(&pool, alice).unwrap().unwrap();
        person.permissions.clear();
        let ctx = AuthContext {
            role_names: vec![],
            user: person,
        };
        assert!(scoped_subject(&ctx, Some(bob), false).is_err());
        assert_eq!(scoped_subject(&ctx, None, false).unwrap(), Some(alice));
        conn.execute("UPDATE work_records SET deleted_at='2091-02-01' WHERE subject_user_id=?1 AND source_type='rd_sample'",[alice]).unwrap();
        let deleted = query_report(
            &conn,
            &query(),
            report_scope(Some(vec![a]), Some(alice)),
            alice,
        )
        .unwrap();
        assert_eq!(deleted.summary.detection_workload, 0.0);
        conn.execute("UPDATE work_records SET deleted_at=NULL WHERE subject_user_id=?1 AND source_type='rd_sample'",[alice]).unwrap();
        assert_eq!(
            query_report(
                &conn,
                &query(),
                report_scope(Some(vec![a]), Some(alice)),
                alice
            )
            .unwrap()
            .summary
            .total_workload,
            27.0
        );
        conn.execute("INSERT INTO work_records(project_id,user_name,quantity,recorded_at,coefficient_snapshot,detection_division_id,business_no) SELECT project_id,'report_alice',9,'2091-01-31T15:00:00',2,detection_division_id,'history_test' FROM work_records WHERE subject_user_id=?1 AND source_type='rd_sample'",[alice]).unwrap();
        let own = query_report(
            &conn,
            &query(),
            report_scope(Some(vec![a]), Some(alice)),
            alice,
        )
        .unwrap();
        assert_eq!(own.summary.total_workload, 27.0);
        let manager =
            query_report(&conn, &query(), report_scope(Some(vec![a]), None), alice).unwrap();
        assert!(manager
            .people
            .iter()
            .any(|p| p.user_id.is_none() && p.user_name == "历史未绑定：report_alice"));
        conn.execute("UPDATE work_records SET source_record_id=88 WHERE subject_user_id=?1 AND source_type='rd_sample'",[alice]).unwrap();
        conn.execute("INSERT INTO work_records(project_id,user_name,subject_user_id,quantity,recorded_at,coefficient_snapshot,detection_division_id,source_type,source_record_id,business_no) SELECT project_id,user_name,subject_user_id,1,recorded_at,coefficient_snapshot,detection_division_id,source_type,source_record_id,'partial_test' FROM work_records WHERE subject_user_id=?1 AND source_type='rd_sample'",[alice]).unwrap();
        let partial = query_report(
            &conn,
            &query(),
            report_scope(Some(vec![a]), Some(alice)),
            alice,
        )
        .unwrap();
        assert_eq!(partial.summary.detection_quantity, 4);
    }

    #[test]
    fn type_drilldown_preserves_ids_snapshots_permissions_and_full_totals() {
        let (pool, a, b, alice, _, _) = fixture();
        let conn = pool.get().unwrap();
        let mut method_ids = Vec::new();
        for _ in 0..2 {
            conn.execute("INSERT INTO methods(name) VALUES('same_method')", [])
                .unwrap();
            method_ids.push(conn.last_insert_rowid());
        }
        let mut instrument_ids = Vec::new();
        for code in ["report_LC_A", "report_LC_B"] {
            conn.execute(
                "INSERT INTO instruments(code,name,instrument_type) VALUES(?1,?1,'液相')",
                [code],
            )
            .unwrap();
            instrument_ids.push(conn.last_insert_rowid());
        }
        conn.execute("UPDATE work_records SET method_id=?1,method_name_snapshot='same_method',instrument_id_snapshot=?2,detection_type_snapshot='液相' WHERE source_type='rd_sample'", postgres_compat::params![method_ids[0],instrument_ids[0]]).unwrap();
        for (method, instrument, quantity) in [
            (Some(method_ids[0]), Some(instrument_ids[1]), 4),
            (Some(method_ids[1]), Some(instrument_ids[0]), 5),
            (None, None, 6),
        ] {
            conn.execute("INSERT INTO work_records(project_id,user_name,subject_user_id,quantity,coefficient_snapshot,recorded_at,detection_division_id,method_id,method_name_snapshot,instrument_id_snapshot,instrument_code_snapshot,detection_type_snapshot,business_no) SELECT project_id,'old_name',?1,?2,2,'2091-01-31T15:00:00',?3,?4,CASE WHEN ?4 IS NULL THEN '历史方法' ELSE 'same_method' END,?5,CASE WHEN ?5 IS NULL THEN '历史仪器' ELSE '' END,'液相',?6 FROM work_records WHERE source_type='rd_sample'", postgres_compat::params![alice,quantity,a,method,instrument,format!("type_{}",uuid::Uuid::new_v4())]).unwrap();
        }
        conn.execute(
            "UPDATE work_records SET detection_type_snapshot='液相' WHERE detection_division_id=?1",
            [b],
        )
        .unwrap();
        let mut q = query();
        q.kind = Some("detection".into());
        q.type_name = Some("液相".into());
        q.page_size = Some(1);
        q.compact_type_summary = Some(true);
        let r = query_report(&conn, &q, report_scope(Some(vec![a]), None), alice).unwrap();
        assert_eq!(r.summary.detection_quantity, 18);
        assert_eq!(r.summary.total_workload, 36.0);
        assert_eq!(r.summary.auxiliary_workload, 0.0);
        assert_eq!(r.details.total, 4);
        assert_eq!(r.details.items.len(), 1);
        assert!(r.people.is_empty() && r.matrix.is_empty() && r.trend.is_empty());
        q.include_people = Some(true);
        let candidates = query_report(&conn, &q, report_scope(Some(vec![a]), None), alice).unwrap();
        assert_eq!(candidates.people.len(), 1);
        assert_eq!(candidates.people[0].user_id, Some(alice));
        assert!(candidates.matrix.is_empty() && candidates.trend.is_empty());
        q.include_people = Some(false);
        assert_eq!(r.breakdowns["method"].len(), 3);
        assert_eq!(
            r.breakdowns["method"]
                .iter()
                .filter(|r| r.name == "same_method")
                .count(),
            2
        );
        q.method_key = Some(method_ids[0].to_string());
        let method = query_report(&conn, &q, report_scope(Some(vec![a]), None), alice).unwrap();
        assert_eq!(method.summary.detection_quantity, 7);
        assert_eq!(method.breakdowns["instrument"].len(), 2);
        q.instrument_key = Some(instrument_ids[0].to_string());
        let instrument = query_report(&conn, &q, report_scope(Some(vec![a]), None), alice).unwrap();
        assert_eq!(instrument.summary.detection_quantity, 3);
        q.method_key = Some("legacy:历史方法".into());
        q.instrument_key = Some("legacy:历史仪器".into());
        let legacy = query_report(&conn, &q, report_scope(Some(vec![a]), None), alice).unwrap();
        assert_eq!(legacy.summary.detection_quantity, 6);
        q.method_key = None;
        q.instrument_key = None;
        q.detection_division_ids = Some(format!("{a},{b}"));
        let authorized = query_report(&conn, &q, report_scope(Some(vec![a]), None), alice).unwrap();
        assert_eq!(authorized.summary.detection_quantity, 18);
        q.detection_division_ids = Some(b.to_string());
        let forbidden = query_report(&conn, &q, report_scope(Some(vec![a]), None), alice).unwrap();
        assert_eq!(forbidden.summary.record_count, 0);
        for invalid in ["0", "-1", "1,bad", ",,,"] {
            q.detection_division_ids = Some(invalid.into());
            assert!(query_report(&conn, &q, report_scope(None, None), alice).is_err());
        }
        q.detection_division_ids = None;
        conn.execute(
            "INSERT INTO method_types(name) VALUES('辅助工作') ON CONFLICT(name) DO NOTHING",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO method_type_links(method_id,method_type_id) SELECT ?1,id FROM method_types WHERE name='辅助工作' ON CONFLICT DO NOTHING", [method_ids[0]]).unwrap();
        let unchanged = query_report(&conn, &q, report_scope(Some(vec![a]), None), alice).unwrap();
        assert_eq!(unchanged.summary.detection_quantity, 18);
        let people = export_data::query_person_type_workload_data(
            &conn,
            "2091-01-31",
            "2091-01-31",
            Some(alice),
            Some(&[a]),
        )
        .unwrap();
        assert_eq!(
            people
                .iter()
                .filter(|r| r.method_type == "液相")
                .map(|r| r.quantity)
                .sum::<i64>(),
            18
        );
        let methods = export_data::query_sheet6_data(
            &conn,
            "2091-01-31",
            "2091-01-31",
            Some(alice),
            Some(&[a]),
        )
        .unwrap();
        assert_eq!(
            methods
                .iter()
                .filter(|r| r.method_type == "液相")
                .map(|r| r.quantity)
                .sum::<i64>(),
            18
        );
    }

    #[test]
    fn auxiliary_drilldown_distinguishes_ids_legacy_names_and_explicit_links() {
        let (pool, a, _, alice, _, lab) = fixture();
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO method_types(name) VALUES('辅助工作') ON CONFLICT(name) DO NOTHING",
            [],
        )
        .unwrap();
        let mut methods = Vec::new();
        let mut items = Vec::new();
        for i in 0..2 {
            conn.execute("INSERT INTO methods(name) VALUES('same_auxiliary')", [])
                .unwrap();
            let method = conn.last_insert_rowid();
            methods.push(method);
            conn.execute("INSERT INTO method_type_links(method_id,method_type_id) SELECT ?1,id FROM method_types WHERE name='辅助工作'", [method]).unwrap();
            conn.execute("INSERT INTO work_records(project_id,user_name,subject_user_id,quantity,coefficient_snapshot,recorded_at,detection_division_id,method_id,method_name_snapshot,auxiliary_type_snapshot,business_no) SELECT project_id,'old_name',?1,?2,2,'2091-01-31T15:00:00',?3,?4,'same_auxiliary',?5,?6 FROM work_records WHERE source_type='rd_sample'", postgres_compat::params![alice,i+2,a,method,if i==0 {"前处理"} else {""},format!("auxiliary_{}",uuid::Uuid::new_v4())]).unwrap();
            conn.execute(
                "INSERT INTO auxiliary_work_items(name,group_id) VALUES('same_auxiliary',?1)",
                postgres_compat::params![if i == 0 { Some(lab) } else { None }],
            )
            .unwrap();
            let item = conn.last_insert_rowid();
            items.push(item);
            conn.execute("INSERT INTO auxiliary_work_records(user_id,user_name_snapshot,auxiliary_work_name_snapshot,auxiliary_work_item_id,quantity,coefficient_snapshot,recorded_at,detection_division_id,auxiliary_type_snapshot) VALUES(?1,'old_name','same_auxiliary',?2,?3,2,'2091-01-31T15:00:00',?4,?5)", postgres_compat::params![alice,item,5+i*2,a,if i==0 {"前处理"} else {""}]).unwrap();
        }
        let categories = serde_json::json!({format!("method:{}",methods[0]):"改后分类",format!("method:{}",methods[1]):"维护",format!("item:{}",items[1]):"维护"});
        conn.execute("INSERT INTO system_settings(key,value) VALUES('workload_auxiliary_categories',?1) ON CONFLICT(key) DO UPDATE SET value=EXCLUDED.value", [categories.to_string()]).unwrap();
        conn.execute("UPDATE auxiliary_work_records SET auxiliary_work_item_id=?1,auxiliary_type_snapshot='维护' WHERE auxiliary_work_name_snapshot='linked'", [items[0]]).unwrap();
        let mut q = query();
        q.kind = Some("auxiliary".into());
        q.auxiliary_category = Some("前处理".into());
        q.page_size = Some(1);
        let r = query_report(&conn, &q, report_scope(Some(vec![a]), Some(alice)), alice).unwrap();
        assert_eq!(r.summary.auxiliary_quantity, 7);
        assert_eq!(r.summary.total_workload, 14.0);
        assert_eq!(r.details.total, 2);
        assert_eq!(r.breakdowns["auxiliary_method"].len(), 2);
        assert_eq!(r.breakdowns["method"].len(), 0);
        q.auxiliary_key = Some(format!("method:{}", methods[0]));
        let method =
            query_report(&conn, &q, report_scope(Some(vec![a]), Some(alice)), alice).unwrap();
        assert_eq!(method.summary.auxiliary_quantity, 2);
        assert_eq!(method.details.items[0].auxiliary_category, "前处理");
        q.auxiliary_key = Some(format!("item:{}", items[0]));
        let item =
            query_report(&conn, &q, report_scope(Some(vec![a]), Some(alice)), alice).unwrap();
        assert_eq!(item.summary.auxiliary_quantity, 5);
        q.auxiliary_category = Some("维护".into());
        q.auxiliary_key = None;
        let maintenance =
            query_report(&conn, &q, report_scope(Some(vec![a]), Some(alice)), alice).unwrap();
        assert_eq!(maintenance.summary.auxiliary_quantity, 14);
        assert_eq!(maintenance.breakdowns["auxiliary_method"].len(), 3);
        q.auxiliary_key = Some(format!("item:{}", items[0]));
        let linked =
            query_report(&conn, &q, report_scope(Some(vec![a]), Some(alice)), alice).unwrap();
        assert_eq!(linked.summary.auxiliary_quantity, 4);
        assert_eq!(linked.details.total, 1);
        assert_eq!(linked.details.items[0].method, "linked");
        q.auxiliary_category = None;
        q.auxiliary_key = Some("legacy:item:report_aux".into());
        let history =
            query_report(&conn, &q, report_scope(Some(vec![a]), Some(alice)), alice).unwrap();
        assert_eq!(history.summary.auxiliary_quantity, 3);
        assert_eq!(history.breakdowns["auxiliary_category"][0].name, "未分类");
        conn.execute(
            "UPDATE system_settings SET value='not-json' WHERE key='workload_auxiliary_categories'",
            [],
        )
        .unwrap();
        q.auxiliary_key = Some(format!("method:{}", methods[1]));
        let invalid_config =
            query_report(&conn, &q, report_scope(Some(vec![a]), Some(alice)), alice).unwrap();
        assert_eq!(
            invalid_config.breakdowns["auxiliary_category"][0].name,
            "未分类"
        );
    }

    #[tokio::test]
    async fn report_excel_matches_page_and_contains_all_filtered_rows() {
        let (pool, _, _, alice, bob, _) = fixture();
        let admin = user_repo::find_by_username(&pool, "admin")
            .unwrap()
            .unwrap();
        let token =
            crate::service::auth_service::create_login_response(&pool, admin, false, None, None)
                .unwrap()
                .token;
        let mut h = HeaderMap::new();
        h.insert("authorization", format!("Bearer {token}").parse().unwrap());
        let mut q = query();
        q.subject_user_id = Some(alice);
        q.page_size = Some(1);
        let page = report(State(pool.clone()), h.clone(), Query(q))
            .await
            .unwrap()
            .0
            .data
            .unwrap();
        assert_eq!(page.summary.total_workload, 27.0);
        let mut q = query();
        q.subject_user_id = Some(alice);
        q.page_size = Some(1);
        let response = export_report(State(pool.clone()), h.clone(), Query(q))
            .await
            .unwrap();
        let bytes = axum::body::to_bytes(response.into_body(), 32 * 1024 * 1024)
            .await
            .unwrap();
        let mut wb: calamine::Xlsx<_> =
            calamine::open_workbook_from_rs(std::io::Cursor::new(bytes.to_vec())).unwrap();
        let total = wb.worksheet_range("工作量总览").unwrap();
        assert_eq!(
            total.get_value((1, 5)).unwrap().to_string(),
            page.summary.total_workload.to_string()
        );
        let rows = wb.worksheet_range("当前筛选全部明细").unwrap();
        assert_eq!(rows.height(), page.details.total as usize + 1);
        let matrix = wb.worksheet_range("人员类型矩阵").unwrap();
        assert!(matrix
            .rows()
            .flat_map(|r| r.iter())
            .any(|v| v.to_string() == "列合计"));
        assert!(wb.worksheet_range("辅助类型").is_ok());
        assert!(wb.worksheet_range("辅助方法事项").is_ok());
        let mut q = query();
        q.subject_user_id = Some(alice);
        q.kind = Some("auxiliary".into());
        q.auxiliary_key = Some("legacy:item:report_aux".into());
        q.compact_type_summary = Some(true);
        q.page_size = Some(1);
        let filtered = report(State(pool.clone()), h.clone(), Query(q))
            .await
            .unwrap()
            .0
            .data
            .unwrap();
        assert_eq!(filtered.summary.total_workload, 9.0);
        let mut q = query();
        q.subject_user_id = Some(alice);
        q.kind = Some("auxiliary".into());
        q.auxiliary_key = Some("legacy:item:report_aux".into());
        q.compact_type_summary = Some(true);
        q.page_size = Some(1);
        let response = export_report(State(pool.clone()), h, Query(q))
            .await
            .unwrap();
        let bytes = axum::body::to_bytes(response.into_body(), 32 * 1024 * 1024)
            .await
            .unwrap();
        let mut wb: calamine::Xlsx<_> =
            calamine::open_workbook_from_rs(std::io::Cursor::new(bytes.to_vec())).unwrap();
        let total = wb.worksheet_range("工作量总览").unwrap();
        assert_eq!(total.get_value((1, 5)).unwrap().to_string(), "9");
        let rows = wb.worksheet_range("当前筛选全部明细").unwrap();
        assert_eq!(rows.height(), 3);
        assert_eq!(
            rows.get_value((1, 17)).unwrap().to_string(),
            "legacy:item:report_aux"
        );
        assert_eq!(rows.get_value((1, 18)).unwrap().to_string(), "未分类");
        assert_eq!(
            wb.worksheet_range("辅助方法事项")
                .unwrap()
                .get_value((1, 7))
                .unwrap()
                .to_string(),
            "legacy:item:report_aux"
        );
        assert!(wb.worksheet_range("人员汇总").unwrap().height() > 1);

        let conn = pool.get().unwrap();
        conn.execute("INSERT INTO roles(name) VALUES('report_portal_only')", [])
            .unwrap();
        let role = conn.last_insert_rowid();
        conn.execute("INSERT INTO role_permissions(role_id,permission_key) VALUES(?1,'stats:portal:workload')", [role]).unwrap();
        conn.execute(
            "INSERT INTO user_roles(user_id,role_id) VALUES(?1,?2)",
            postgres_compat::params![alice, role],
        )
        .unwrap();
        drop(conn);
        let ordinary = user_repo::find_by_id(&pool, alice).unwrap().unwrap();
        let token =
            crate::service::auth_service::create_login_response(&pool, ordinary, false, None, None)
                .unwrap()
                .token;
        let mut h = HeaderMap::new();
        h.insert("authorization", format!("Bearer {token}").parse().unwrap());
        let mut q = query();
        q.view = Some("mine".into());
        q.kind = Some("auxiliary".into());
        let own = report(State(pool.clone()), h.clone(), Query(q))
            .await
            .unwrap()
            .0
            .data
            .unwrap();
        assert_eq!(own.summary.auxiliary_quantity, 1);
        assert!(own.details.items.iter().all(|r| r.user_id == Some(alice)));
        let mut q = query();
        q.view = Some("mine".into());
        q.subject_user_id = Some(bob);
        assert!(report(State(pool.clone()), h.clone(), Query(q))
            .await
            .is_err());
        assert!(report(State(pool.clone()), h.clone(), Query(query()))
            .await
            .is_err());
        let mut q = query();
        q.view = Some("mine".into());
        assert!(export_report(State(pool), h, Query(q)).await.is_err());
    }
}
