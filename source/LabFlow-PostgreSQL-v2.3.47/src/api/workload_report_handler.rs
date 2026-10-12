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
    #[serde(rename = "type")]
    pub type_name: Option<String>,
    pub source: Option<String>,
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
    q: &ReportQuery,
    scope: &ReportScope,
    viewer_user_id: i64,
) -> Result<(String, Vec<String>)> {
    let (start, end) = dates(q)?;
    let mut params = vec![start, end];
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
    ] {
        if let Some(value) = value.filter(|value| !value.is_empty()) {
            params.push(value.into());
            filters.push(format!("{column}=?{}", params.len()));
        }
    }
    let filter = if filters.is_empty() {
        "TRUE".into()
    } else {
        filters.join(" AND ")
    };
    Ok((
        format!(
            r#"WITH raw_records AS (
        SELECT wr.id, CASE WHEN {auxiliary_record} THEN 'auxiliary' ELSE 'detection' END AS kind,
          wr.subject_user_id AS user_id, COALESCE(u.username, '历史未绑定：' || wr.user_name) AS user_name,
          CASE WHEN wr.subject_user_id IS NULL THEN wr.user_name ELSE '' END AS legacy_name,
          CAST(wr.recorded_at AS TIMESTAMPTZ) AS recorded_at,
          {department} AS department_id,
          COALESCE(NULLIF(wr.detection_division_name_snapshot,''),d.name,'未绑定部门') AS department,
          CASE WHEN {auxiliary_record} THEN NULL::BIGINT ELSE wr.group_id END AS lab_id,
          CASE WHEN {auxiliary_record} THEN '' ELSE COALESCE(NULLIF(wr.lab_name_snapshot,''),g.name,'未绑定实验室') END AS lab,
          CASE WHEN {auxiliary_record} THEN NULL::BIGINT ELSE wr.project_id END AS project_id,
          CASE WHEN {auxiliary_record} THEN '' ELSE COALESCE(NULLIF(wr.project_name_snapshot,''),p.name,'未绑定项目') END AS project,
          wr.method_id, COALESCE(NULLIF(wr.method_name_snapshot,''),m.name,'未绑定方法') AS method,
          CASE WHEN {auxiliary_record} THEN NULL::BIGINT ELSE wr.instrument_id_snapshot END AS instrument_id,
          CASE WHEN {auxiliary_record} THEN '' ELSE COALESCE(NULLIF(wr.instrument_code_snapshot,''),i.code,'未绑定仪器') END AS instrument,
          CASE WHEN {auxiliary_record} THEN '辅助工作'
            ELSE COALESCE((SELECT string_agg(DISTINCT mt.name, '、' ORDER BY mt.name) FROM method_type_links ml JOIN method_types mt ON mt.id=ml.method_type_id WHERE ml.method_id=wr.method_id),NULLIF(wr.instrument_type_snapshot,''),'未分类') END AS type_name,
          CASE WHEN {auxiliary_record} THEN 'auxiliary_work' ELSE COALESCE(NULLIF(wr.source_type,''),'analysis') END AS source,
          wr.quantity, COALESCE(wr.coefficient_snapshot,1.0)::DOUBLE PRECISION AS coefficient,
          (wr.quantity*COALESCE(wr.coefficient_snapshot,1.0))::DOUBLE PRECISION AS workload,
          wr.source_record_id, COALESCE(wr.ownership_status,'confirmed') AS ownership_status
        FROM work_records wr LEFT JOIN users u ON u.id=wr.subject_user_id LEFT JOIN projects p ON p.id=wr.project_id
          LEFT JOIN project_groups g ON g.id=wr.group_id LEFT JOIN divisions d ON d.id={department}
          LEFT JOIN methods m ON m.id=wr.method_id LEFT JOIN instruments i ON i.id=wr.instrument_id_snapshot
        WHERE wr.deleted_at IS NULL AND CAST(wr.recorded_at AS TIMESTAMPTZ)>=CAST(CAST(?1 AS TEXT) AS TIMESTAMPTZ)
          AND CAST(wr.recorded_at AS TIMESTAMPTZ)<CAST(CAST(?2 AS TEXT) AS TIMESTAMPTZ){work_scope}{work_subject}{pending}
        UNION ALL
        SELECT awr.id, 'auxiliary', awr.user_id, COALESCE(u.username,'历史未绑定：' || awr.user_name_snapshot),
          CASE WHEN awr.user_id IS NULL THEN awr.user_name_snapshot ELSE '' END, awr.recorded_at,
          awr.detection_division_id, COALESCE(NULLIF(awr.detection_division_name_snapshot,''),d.name,'历史未归属部门'),
          NULL::BIGINT, '', NULL::BIGINT, '', NULL::BIGINT, awr.auxiliary_work_name_snapshot,
          NULL::BIGINT, '', '辅助工作', 'auxiliary_work', awr.quantity, awr.coefficient_snapshot,
          (awr.quantity*awr.coefficient_snapshot)::DOUBLE PRECISION, NULL::BIGINT, 'confirmed'
        FROM auxiliary_work_records awr LEFT JOIN users u ON u.id=awr.user_id LEFT JOIN divisions d ON d.id=awr.detection_division_id
        WHERE awr.deleted_at IS NULL AND awr.recorded_at>=CAST(CAST(?1 AS TEXT) AS TIMESTAMPTZ) AND awr.recorded_at<CAST(CAST(?2 AS TEXT) AS TIMESTAMPTZ){aux_subject}{aux_scope}
      ), records AS (SELECT * FROM raw_records WHERE {filter}) "#
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
    rows(conn, &format!("{cte} SELECT id,kind,user_id,user_name,CAST(recorded_at AS TEXT),department,lab,project,method,instrument,type_name,source,quantity,coefficient,workload,source_record_id,ownership_status FROM records ORDER BY recorded_at DESC,kind,id DESC LIMIT {limit} OFFSET {offset}"), params,
      |row| Ok(DetailRow { id: row.get(0)?, kind: row.get(1)?, user_id: row.get(2)?, user_name: row.get(3)?, recorded_at: row.get(4)?, department: row.get(5)?, lab: row.get(6)?, project: row.get(7)?, method: row.get(8)?, instrument: row.get(9)?, type_name: row.get(10)?, source: row.get(11)?, quantity: row.get(12)?, coefficient: row.get(13)?, workload: row.get(14)?, source_record_id: row.get(15)?, ownership_status: row.get(16)? }))
}

fn query_report(
    conn: &Connection,
    q: &ReportQuery,
    scope: ReportScope,
    viewer: i64,
) -> Result<Report> {
    let (cte, params) = report_cte(q, &scope, viewer)?;
    let summary = rows(
        conn,
        &format!("{cte} SELECT {METRICS} FROM records"),
        &params,
        |r| metrics(r, 0),
    )?
    .remove(0);
    let mut people = rows(conn, &format!("{cte} SELECT user_id,MAX(user_name),{METRICS} FROM records GROUP BY user_id,legacy_name ORDER BY 7 DESC,2"), &params, |r| Ok(PersonRow { user_id: r.get(0)?, user_name: r.get(1)?, metrics: metrics(r,2)? }))?;
    if q.include_zero_users.unwrap_or(false) {
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
    let matrix = rows(conn, &format!("{cte} SELECT user_id,MAX(user_name),type_name,{METRICS} FROM records GROUP BY user_id,legacy_name,type_name ORDER BY 2,3"), &params, |r| Ok(MatrixRow { user_id:r.get(0)?,user_name:r.get(1)?,type_name:r.get(2)?,metrics:metrics(r,3)? }))?;
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
    let period = match q.group_by.as_deref().unwrap_or("day") {
        "day" => "YYYY-MM-DD",
        "week" => "IYYY-\"W\"IW",
        "month" => "YYYY-MM",
        _ => return Err(AppError::Validation("趋势周期无效".into())),
    };
    let trend=rows(conn,&format!("{cte} SELECT to_char(recorded_at,'{period}') AS period,{METRICS} FROM records GROUP BY period ORDER BY period"),&params,|r| Ok(TrendRow { period:r.get(0)?,metrics:metrics(r,1)? }))?;
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
    Query(q): Query<ReportQuery>,
) -> Result<Response> {
    let (ctx, scope) = scope(&pool, &request_headers, &q, true)?;
    let conn = pool.get()?;
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")?;
    let (cte, params) = report_cte(&q, &scope, ctx.user.id)?;
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
    ] {
        let ws = wb.add_worksheet();
        ws.set_name(name)?;
        headers(ws, &[name])?;
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

    #[tokio::test]
    async fn report_excel_matches_page_and_contains_all_filtered_rows() {
        let (pool, _, _, alice, _, _) = fixture();
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
        let response = export_report(State(pool), h, Query(q)).await.unwrap();
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
    }
}
