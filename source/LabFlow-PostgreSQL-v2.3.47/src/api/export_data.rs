use crate::error::{AppError, Result};
use crate::service::authz_service;
/// 导出数据查询层 - v0.3.21 版本
/// 支持 10 个 Sheet 的数据查询
/// v0.3.21 关键修复：汇总表使用 group_concat 子查询获取实验室名（拼接显示），GROUP BY 不含实验室维度
///   这样每条 (项目, 方法) 只有一行，数量不会翻倍（修复 v0.3.19/0.3.20 的 JOIN 展开问题）
use postgres_compat::Connection;

// ========== 通用数据结构 ==========

/// 扁平行数据（Sheet 1 使用）
pub type FlatRow = (
    String,
    String,
    String,
    String,
    f64,
    i64,
    String,
    f64,
    Option<String>,
);
// (实验室, 项目代号, 仪器, 方法, 单价倍率, 数量, 实际检测类型, 系数, 高项)

/// 仪器汇总行（Sheet 2）
#[derive(Debug, Clone, serde::Serialize)]
pub struct InstrumentDailyRow {
    pub date: String,
    pub instrument: String,
    pub lab: String,
    pub project: String,
    pub method: String,
    pub multiplier: f64,
    pub quantity: i64,
    pub high_item: Option<String>,
}

/// 项目汇总行（Sheet 3）
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProjectSummaryRow {
    pub project: String,
    pub lab: String,
    pub instrument: String,
    pub method: String,
    pub multiplier: f64,
    pub quantity: i64,
    pub unit_price: f64,
    pub high_item: Option<String>,
}

/// 实验室汇总行（Sheet 4）
#[derive(Debug, Clone, serde::Serialize)]
pub struct LabSummaryRow {
    pub lab: String,
    pub project: String,
    pub instrument: String,
    pub method: String,
    pub multiplier: f64,
    pub quantity: i64,
    pub unit_price: f64,
    pub high_item: Option<String>,
}

/// 人员原始记录行（Sheet 5）
#[derive(Debug, Clone, serde::Serialize)]
pub struct PersonRecordRow {
    pub recorded_at: String,
    pub lab: String,
    pub project: String,
    pub method: String,
    pub method_type: String,
    pub multiplier: f64,
    pub quantity: i64,
    pub user_name: String,
    pub high_item: Option<String>,
}

/// 分析检测人员原始记录行（Sheet 5）。
/// 与研发送样共用基础字段，但部门维度只属于分析检测导出。
#[derive(Debug, Clone, serde::Serialize)]
pub struct AnalysisPersonRecordRow {
    pub recorded_at: String,
    pub detection_department: String,
    pub sending_department: String,
    pub lab: String,
    pub project: String,
    pub method: String,
    pub method_type: String,
    pub multiplier: f64,
    pub quantity: i64,
    pub user_name: String,
    pub high_item: Option<String>,
}

/// 人员汇总行（Sheet 6）
#[derive(Debug, Clone, serde::Serialize)]
pub struct PersonSummaryRow {
    pub user_name: String,
    pub project: String,
    pub instrument: String,
    pub method_type: String, // 数据库中的实际检测类型
    pub method: String,
    pub coefficient: f64,
    pub multiplier: f64,
    pub quantity: i64,
    pub workload: f64,
}

/// 按检测类型逐行展示的人员工作量汇总行。
/// 工作量只由数量和系数决定；金额倍率不参与此结构。
#[derive(Debug, Clone, serde::Serialize)]
pub struct PersonTypeWorkloadRow {
    pub user_name: String,
    pub method_type: String,
    pub coefficient: f64,
    pub quantity: i64,
    pub workload: f64,
}

/// 辅助工作明细行（单独导出 Sheet）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct AuxiliaryWorkRow {
    pub user_name: String,
    pub project: String,
    pub auxiliary_method: String,
    pub coefficient: f64,
    pub quantity: i64,
    pub workload: f64,
}

/// 实验室总表行（Sheet 7）
#[derive(Debug, Clone, serde::Serialize)]
pub struct LabTotalRow {
    pub lab: String,
    pub project: String,
    pub method_type: String,
    pub multiplier: f64,
    pub unit_price: f64, // 方法单价（原 amount）
    pub quantity: i64,
}

/// 项目总表行（Sheet 8）
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProjectTotalRow {
    pub project: String,
    pub method_type: String,
    pub multiplier: f64,
    pub unit_price: f64, // 方法单价（原 amount）
    pub quantity: i64,
}

/// 仪器汇总表行（Sheet 9）
#[derive(Debug, Clone, serde::Serialize)]
pub struct InstrumentSummaryRow {
    pub instrument: String,
    pub quantity: i64,
    pub instrument_type: String, // lc/gc/icp等
    pub multiplier: f64,
}

/// 理化汇总表行（Sheet 10）
#[derive(Debug, Clone, serde::Serialize)]
pub struct PhysChemRow {
    pub method: String,
    pub multiplier: f64,
    pub quantity: i64,
}

// ========== 辅助函数 ==========

/// 从项目名称提取代号（取 - 前部分）
pub fn extract_code(name: &str) -> &str {
    name.split('-').next().unwrap_or(name)
}

pub fn month_bounds(ref_date: &str) -> (String, String) {
    let parts: Vec<&str> = ref_date.split('-').collect();
    if parts.len() < 2 {
        return (ref_date.to_string(), ref_date.to_string());
    }
    let year: i32 = parts[0].parse().unwrap_or(2026);
    let month: u32 = parts[1].parse().unwrap_or(1);
    let start = format!("{}-{:02}-01", year, month);
    let end = if month == 12 {
        format!("{}-01-01", year + 1)
    } else {
        format!("{}-{:02}-01", year, month + 1)
    };
    (start, end)
}

pub fn configure_dimension_filters(
    conn: &Connection,
    group_id: Option<i64>,
    detection_division_ids: Option<&str>,
    sending_division_ids: Option<&str>,
    include_pending_ownership: bool,
    allowed_sending_division_ids: Option<&[i64]>,
) -> Result<()> {
    fn normalize(raw: Option<&str>, label: &str) -> Result<String> {
        let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
            return Ok(String::new());
        };
        let mut ids = raw
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| {
                value
                    .parse::<i64>()
                    .map_err(|_| AppError::Validation(format!("{label}筛选值无效")))
            })
            .collect::<Result<Vec<_>>>()?;
        ids.sort_unstable();
        ids.dedup();
        if ids.is_empty() {
            return Ok(String::new());
        }
        Ok(ids.iter().map(i64::to_string).collect::<Vec<_>>().join(","))
    }

    conn.execute(
        "SELECT set_config('workload.export_group_id',?1,true)",
        [group_id.map(|id| id.to_string()).unwrap_or_default()],
    )?;
    let detection = normalize(detection_division_ids, "检测部门")?;
    let sending = normalize(sending_division_ids, "送样部门")?;
    // The role scope is based on the detector/execution department. Sending
    // department is an independent filter: the final query still applies the
    // role scope before this filter, so an arbitrary sending department never
    // widens the visible data set.
    let _ = allowed_sending_division_ids;
    conn.execute(
        "SELECT set_config('workload.export_detection_division_ids',?1,true)",
        [&detection],
    )?;
    conn.execute(
        "SELECT set_config('workload.export_sending_division_ids',?1,true)",
        [&sending],
    )?;
    conn.execute(
        "SELECT set_config('workload.export_include_pending_ownership',?1,true)",
        [include_pending_ownership.to_string()],
    )?;
    Ok(())
}

pub(crate) fn configure_text_filters(
    conn: &Connection,
    lab_name: Option<&str>,
    project_name: Option<&str>,
    instrument_code: Option<&str>,
    method_name: Option<&str>,
) -> Result<()> {
    for (key, value) in [
        ("workload.export_lab_name", lab_name),
        ("workload.export_project_name", project_name),
        ("workload.export_instrument_code", instrument_code),
        ("workload.export_method_name", method_name),
    ] {
        conn.execute(
            "SELECT set_config(?1,?2,true)",
            [key, value.unwrap_or_default()],
        )?;
    }
    Ok(())
}

pub(crate) fn text_filter_predicate_sql(alias: &str) -> String {
    let lab = "NULLIF(current_setting('workload.export_lab_name',true),'')";
    let project = "NULLIF(current_setting('workload.export_project_name',true),'')";
    let instrument = "NULLIF(current_setting('workload.export_instrument_code',true),'')";
    let method = "NULLIF(current_setting('workload.export_method_name',true),'')";
    let auxiliary = legacy_auxiliary_record_sql(alias);
    format!(
        " AND ({auxiliary} OR (
            ({lab} IS NULL OR {alias}.lab_name_snapshot={lab}
                OR EXISTS(SELECT 1 FROM project_groups text_lab WHERE text_lab.id={alias}.group_id AND text_lab.name={lab}))
            AND ({project} IS NULL OR {alias}.project_name_snapshot={project}
                OR split_part({alias}.project_name_snapshot,'-',1)={project}
                OR EXISTS(SELECT 1 FROM projects text_project WHERE text_project.id={alias}.project_id
                    AND (text_project.name={project} OR split_part(text_project.name,'-',1)={project})))
            AND ({instrument} IS NULL OR COALESCE(NULLIF({alias}.instrument_code_snapshot,''),'未绑定')={instrument})
            AND ({method} IS NULL OR {alias}.method_name_snapshot={method}
                OR EXISTS(SELECT 1 FROM methods text_method WHERE text_method.id={alias}.method_id
                    AND (text_method.name={method} OR text_method.full_name={method}
                         OR text_method.name || CASE WHEN COALESCE({alias}.instrument_code_snapshot,'')=''
                             THEN '' ELSE ' [' || {alias}.instrument_code_snapshot || ']' END={method})))
        ))"
    )
}

/// The viewer is distinct from the optional personnel filter: managers may still
/// read their own unassigned historical auxiliary records.
pub(crate) fn configure_auxiliary_viewer(conn: &Connection, viewer_user_id: i64) -> Result<()> {
    conn.execute(
        "SELECT set_config('workload.export_viewer_user_id',?1,true)",
        [viewer_user_id.to_string()],
    )?;
    Ok(())
}

/// `None` is reserved for unrestricted administrator scope. An empty scope must
/// not expose other people's history. Explicit department snapshots never use
/// current user membership as a fallback.
pub(crate) fn auxiliary_scope_sql(
    alias: &str,
    allowed_division_ids: Option<&[i64]>,
    viewer_user_id: Option<i64>,
) -> String {
    let authorization = match allowed_division_ids {
        None => String::new(),
        Some(ids) => {
            let department = if ids.is_empty() {
                "FALSE".to_string()
            } else {
                format!(
                    "{alias}.detection_division_id IN ({})",
                    ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
                )
            };
            let viewer = viewer_user_id.map(|id| id.to_string()).unwrap_or_else(|| {
                "NULLIF(current_setting('workload.export_viewer_user_id',true),'')::BIGINT"
                    .to_string()
            });
            format!(" AND ({department} OR ({alias}.detection_division_id IS NULL AND {alias}.user_id={viewer}))")
        }
    };
    // Only a recorded source link proves duplication. Similar names, dates and
    // quantities can represent distinct work and must never be deduplicated.
    format!("{authorization}
        AND (NULLIF(current_setting('workload.export_detection_division_ids',true),'') IS NULL
             OR {alias}.detection_division_id = ANY(string_to_array(current_setting('workload.export_detection_division_ids',true), ',')::BIGINT[]))
        AND NOT EXISTS (SELECT 1 FROM work_records auxiliary_source WHERE auxiliary_source.source_type='auxiliary_work' AND auxiliary_source.source_record_id={alias}.id AND auxiliary_source.deleted_at IS NULL)")
}

pub(crate) fn legacy_auxiliary_record_sql(alias: &str) -> String {
    format!(
        "COALESCE(({alias}.instrument_type_snapshot='辅助工作' OR {alias}.source_type='auxiliary_work'
          OR EXISTS (SELECT 1 FROM method_type_links auxiliary_link
                     JOIN method_types auxiliary_type ON auxiliary_type.id=auxiliary_link.method_type_id
                     WHERE auxiliary_link.method_id={alias}.method_id AND auxiliary_type.name='辅助工作')),FALSE)"
    )
}

fn detection_scope_sql(allowed_division_ids: Option<&[i64]>) -> String {
    format!(
        "{} AND NOT {}",
        division_scope_sql(allowed_division_ids),
        legacy_auxiliary_record_sql("wr")
    )
}

fn division_scope_sql(allowed_division_ids: Option<&[i64]>) -> String {
    let sending_scope = match allowed_division_ids {
        None => String::new(),
        Some([]) => " AND 1=0".to_string(),
        Some(ids) => {
            let values = ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
            format!(
                " AND {} IN ({values})",
                authz_service::work_record_authorization_division_sql("wr")
            )
        }
    };
    let text_filters = text_filter_predicate_sql("wr");
    let auxiliary_record = legacy_auxiliary_record_sql("wr");
    format!(
        "{sending_scope}
         AND (NULLIF(current_setting('workload.export_group_id',true),'') IS NULL
              OR {auxiliary_record}
              OR wr.group_id = NULLIF(current_setting('workload.export_group_id',true),'')::BIGINT)
         AND (COALESCE(current_setting('workload.export_include_pending_ownership',true),'false')='true'
              OR COALESCE(wr.ownership_status,'confirmed')<>'pending_confirmation')
         AND (NULLIF(current_setting('workload.export_detection_division_ids',true),'') IS NULL
              OR wr.detection_division_id = ANY(string_to_array(current_setting('workload.export_detection_division_ids',true), ',')::BIGINT[]))
         AND (NULLIF(current_setting('workload.export_sending_division_ids',true),'') IS NULL
              OR {auxiliary_record}
              OR COALESCE(wr.sending_division_id,wr.execution_division_id,wr.division_id,(SELECT division_id FROM project_groups WHERE id=wr.group_id)) = ANY(string_to_array(current_setting('workload.export_sending_division_ids',true), ',')::BIGINT[])){text_filters}"
    )
}

/// Date pickers end at the final second; include its fractional timestamps too.
pub(crate) fn end_of_day_bound(end: &str) -> String {
    let end = end.trim();
    if !end.contains('T') && !end.contains(' ') {
        format!("{end}T23:59:59.999999")
    } else if end.ends_with("T23:59:59") || end.ends_with(" 23:59:59") {
        format!("{end}.999999")
    } else {
        end.to_owned()
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FilterDivision {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExportFilterOptions {
    pub detection_departments: Vec<FilterDivision>,
    pub sending_departments: Vec<FilterDivision>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DepartmentPairSummaryRow {
    pub detection_department: String,
    pub sending_department: String,
    pub total_quantity: i64,
    pub record_count: i64,
    pub coefficient_score: f64,
    pub lab_count: i64,
}

pub fn query_department_pair_summary_data(
    conn: &Connection,
    start: &str,
    end: &str,
    subject_user_id: Option<i64>,
    allowed_sending_division_ids: Option<&[i64]>,
) -> Result<Vec<DepartmentPairSummaryRow>> {
    let end_closed = end_of_day_bound(end);
    let division_scope = detection_scope_sql(allowed_sending_division_ids);
    let sql = format!(
        "SELECT
            COALESCE(NULLIF(wr.detection_division_name_snapshot,''), detection.name, '未配置') AS detection_department,
            COALESCE(NULLIF(wr.sending_division_name_snapshot,''), sending.name, '未配置') AS sending_department,
            COALESCE(SUM(wr.quantity),0)::BIGINT,
            COUNT(wr.id),
            COALESCE(SUM(wr.quantity * wr.coefficient_snapshot),0.0)::DOUBLE PRECISION,
            COUNT(DISTINCT COALESCE(wr.sending_group_id,wr.group_id))
         FROM work_records wr
         LEFT JOIN divisions detection ON detection.id=wr.detection_division_id
         LEFT JOIN divisions sending ON sending.id=COALESCE(wr.sending_division_id,wr.execution_division_id,wr.division_id)
         WHERE wr.deleted_at IS NULL AND wr.recorded_at>=?1 AND wr.recorded_at<=?2
           AND (?3 IS NULL OR wr.subject_user_id=?3){division_scope}
         GROUP BY detection.id,detection.name,wr.detection_division_name_snapshot,
                  sending.id,sending.name,wr.sending_division_name_snapshot
         ORDER BY detection_department,sending_department"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        postgres_compat::params![start, end_closed, subject_user_id],
        |row| {
            Ok(DepartmentPairSummaryRow {
                detection_department: row.get(0)?,
                sending_department: row.get(1)?,
                total_quantity: row.get(2)?,
                record_count: row.get(3)?,
                coefficient_score: row.get::<_, f64>(4).unwrap_or(0.0),
                lab_count: row.get(5)?,
            })
        },
    )?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

pub fn query_filter_options(
    conn: &Connection,
    start: &str,
    end: &str,
    detection_division_ids: Option<&str>,
    include_pending_ownership: bool,
    subject_user_id: Option<i64>,
    allowed_sending_division_ids: Option<&[i64]>,
) -> Result<ExportFilterOptions> {
    configure_dimension_filters(
        conn,
        None,
        detection_division_ids,
        None,
        include_pending_ownership,
        allowed_sending_division_ids,
    )?;
    let end_closed = end_of_day_bound(end);
    let mut base = String::from(
        " FROM work_records wr
           LEFT JOIN divisions detection ON detection.id=wr.detection_division_id
           LEFT JOIN divisions sending ON sending.id=COALESCE(wr.sending_division_id,wr.execution_division_id,wr.division_id)
          WHERE wr.deleted_at IS NULL AND wr.recorded_at>=?1 AND wr.recorded_at<=?2",
    );
    if let Some(user_id) = subject_user_id {
        base.push_str(&format!(" AND wr.subject_user_id={user_id}"));
    }
    base.push_str(&division_scope_sql(allowed_sending_division_ids));

    let mut detection_stmt = conn.prepare(&format!(
        "SELECT DISTINCT detection.id,COALESCE(detection.name,'未配置检测部门') {base}
          AND wr.detection_division_id IS NOT NULL ORDER BY 2"
    ))?;
    let detection_departments = detection_stmt
        .query_map(postgres_compat::params![start, end_closed], |row| {
            Ok(FilterDivision {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut sending_stmt = conn.prepare(&format!(
        "SELECT DISTINCT sending.id,COALESCE(sending.name,'未配置送样部门') {base}
          AND COALESCE(wr.sending_division_id,wr.execution_division_id,wr.division_id) IS NOT NULL ORDER BY 2"
    ))?;
    let sending_departments = sending_stmt
        .query_map(postgres_compat::params![start, end_closed], |row| {
            Ok(FilterDivision {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(ExportFilterOptions {
        detection_departments,
        sending_departments,
    })
}

// ========== Sheet 1: 各实验室项目方法对应表 ==========

pub fn query_sheet1_data(
    conn: &Connection,
    start: &str,
    end: &str,
    group_id: Option<i64>,
    subject_user_id: Option<i64>,
    allowed_division_ids: Option<&[i64]>,
) -> Result<Vec<FlatRow>> {
    let end_closed = end_of_day_bound(end);

    // v0.3.25 修复：使用 wr.group_id 对应的 project_groups.name 显示单个实验室
    let mut sql = String::from(
        "SELECT COALESCE(pg.name, '未知') as lab_name,
                p.name, COALESCE(m.full_name, m.name), m.name, m.coefficient,
                COALESCE(wr.multiplier, m.multiplier, 1.0),
                SUM(wr.quantity)::BIGINT,
                COALESCE(wr.high_item, p.high_item),
                COALESCE(NULLIF(wr.instrument_code_snapshot,''), '未绑定'),
                COALESCE((SELECT mt.name
                          FROM method_type_links mtl
                          JOIN method_types mt ON mt.id=mtl.method_type_id
                          WHERE mtl.method_id=wr.method_id AND mt.deleted_at IS NULL
                          ORDER BY mt.sort_order, mt.id
                          LIMIT 1), '未分类')
         FROM work_records wr
         LEFT JOIN project_groups pg ON pg.id = wr.group_id
         JOIN projects p ON wr.project_id = p.id
         LEFT JOIN methods m ON wr.method_id = m.id
         WHERE wr.deleted_at IS NULL
           AND wr.recorded_at >= ?1
           AND wr.recorded_at <= ?2
           AND (?3 IS NULL OR wr.subject_user_id = ?3)",
    );

    let params: Vec<Box<dyn postgres_compat::types::ToSql>> = vec![
        Box::new(start.to_string()),
        Box::new(end_closed),
        Box::new(subject_user_id),
    ];

    sql.push_str(&detection_scope_sql(allowed_division_ids));
    if let Some(gid) = group_id {
        sql.push_str(&format!(" AND wr.group_id = {}", gid));
    }

    sql.push_str(" GROUP BY pg.id, pg.name, p.id, m.id, wr.method_id, wr.multiplier, wr.high_item, wr.instrument_code_snapshot ORDER BY lab_name, p.name, m.name");

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        postgres_compat::params_from_iter(params.iter().map(|p| p.as_ref())),
        |row| {
            let lab: String = row.get(0)?;
            let project: String = row.get(1)?;
            let _full_name: String = row.get(2).unwrap_or_default();
            let method: String = row.get(3).unwrap_or_default();
            let coefficient: f64 = row.get(4).unwrap_or(1.0);
            let multiplier: f64 = row.get(5).unwrap_or(1.0);
            let quantity: i64 = row.get(6)?;

            let project_code = extract_code(&project).to_string();
            let high_item: Option<String> = row.get(7).unwrap_or(None);
            let instrument: String = row.get(8).unwrap_or_else(|_| "未绑定".into());
            let method_type: String = row.get(9).unwrap_or_else(|_| "未分类".into());
            Ok((
                lab,
                project_code,
                instrument,
                method,
                multiplier,
                quantity,
                method_type,
                coefficient,
                high_item,
            ))
        },
    )?;

    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

// ========= Sheet 2: 仪器-汇总 =========

pub fn query_sheet2_data(
    conn: &Connection,
    start: &str,
    end: &str,
    subject_user_id: Option<i64>,
    allowed_division_ids: Option<&[i64]>,
) -> Result<Vec<InstrumentDailyRow>> {
    let end_closed = end_of_day_bound(end);

    // v0.3.25 修复：使用 wr.group_id 对应的 project_groups.name 显示单个实验室
    let division_scope = detection_scope_sql(allowed_division_ids);
    let sql = format!("SELECT date(wr.recorded_at) AS record_date,
                COALESCE(m.full_name, m.name),
                COALESCE(pg.name, '未知') AS lab_name,
                p.name AS project_name,
                m.name AS method_name,
                COALESCE(wr.multiplier, m.multiplier, 1.0),
                SUM(wr.quantity)::BIGINT AS total_qty,
                COALESCE(wr.high_item, p.high_item),
                COALESCE(NULLIF(wr.instrument_code_snapshot,''), '未绑定')
         FROM work_records wr
         LEFT JOIN project_groups pg ON pg.id = wr.group_id
         JOIN projects p ON wr.project_id = p.id
         LEFT JOIN methods m ON wr.method_id = m.id
         WHERE wr.deleted_at IS NULL
           AND wr.recorded_at >= ?1
           AND wr.recorded_at <= ?2
           AND (?3 IS NULL OR wr.subject_user_id = ?3){division_scope}
         GROUP BY LEFT(wr.recorded_at, 10), pg.id, pg.name, m.id, p.id, wr.multiplier, wr.high_item, wr.instrument_code_snapshot
         ORDER BY record_date, m.full_name, lab_name, p.name");

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        postgres_compat::params![start, end_closed, subject_user_id],
        |row| {
            let date: String = row.get(0)?;
            let instrument: String = row.get(8).unwrap_or_else(|_| "未绑定".into());

            Ok(InstrumentDailyRow {
                date,
                instrument,
                lab: row.get(2)?,
                project: row.get(3)?,
                method: row.get(4).unwrap_or_default(),
                multiplier: row.get::<_, f64>(5).unwrap_or(1.0),
                quantity: row.get(6)?,
                high_item: row.get(7).unwrap_or(None),
            })
        },
    )?;

    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

// ========= Sheet 3: 项目-汇总 =========

pub fn query_sheet3_data(
    conn: &Connection,
    start: &str,
    end: &str,
    subject_user_id: Option<i64>,
    allowed_division_ids: Option<&[i64]>,
) -> Result<Vec<ProjectSummaryRow>> {
    let end_closed = end_of_day_bound(end);

    // v0.3.25 修复：使用 wr.group_id 对应的 project_groups.name 显示单个实验室
    let division_scope = detection_scope_sql(allowed_division_ids);
    let sql = format!("SELECT p.name AS project_name,
                COALESCE(pg.name, '未知') AS lab_name,
                COALESCE(m.full_name, m.name),
                m.name AS method_name,
                COALESCE(wr.multiplier, m.multiplier, 1.0),
                m.amount,
                SUM(wr.quantity)::BIGINT AS total_qty,
                COALESCE(wr.high_item, p.high_item),
                COALESCE(NULLIF(wr.instrument_code_snapshot,''), '未绑定')
         FROM work_records wr
         LEFT JOIN project_groups pg ON pg.id = wr.group_id
         JOIN projects p ON wr.project_id = p.id
         LEFT JOIN methods m ON wr.method_id = m.id
         WHERE wr.deleted_at IS NULL
           AND wr.recorded_at >= ?1
           AND wr.recorded_at <= ?2
           AND (?3 IS NULL OR wr.subject_user_id = ?3){division_scope}
         GROUP BY p.id, pg.id, pg.name, m.id, wr.multiplier, wr.high_item, wr.instrument_code_snapshot
         ORDER BY p.name, lab_name, m.name");

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        postgres_compat::params![start, end_closed, subject_user_id],
        |row| {
            let instrument: String = row.get(8).unwrap_or_else(|_| "未绑定".into());

            Ok(ProjectSummaryRow {
                project: row.get(0)?,
                lab: row.get(1)?,
                instrument,
                method: row.get(3).unwrap_or_default(),
                multiplier: row.get::<_, f64>(4).unwrap_or(1.0),
                quantity: row.get(6)?,
                unit_price: row.get::<_, f64>(5).unwrap_or(0.0),
                high_item: row.get(7).unwrap_or(None),
            })
        },
    )?;

    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

// ========= Sheet 4: 实验室-汇总 =========

pub fn query_sheet4_data(
    conn: &Connection,
    start: &str,
    end: &str,
    subject_user_id: Option<i64>,
    allowed_division_ids: Option<&[i64]>,
) -> Result<Vec<LabSummaryRow>> {
    let end_closed = end_of_day_bound(end);

    // v0.3.25 修复：使用 wr.group_id 对应的 project_groups.name 显示单个实验室
    let division_scope = detection_scope_sql(allowed_division_ids);
    let sql = format!("SELECT COALESCE(pg.name, '未知') AS lab_name,
                p.name AS project_name,
                COALESCE(m.full_name, m.name),
                m.name AS method_name,
                COALESCE(wr.multiplier, m.multiplier, 1.0),
                m.amount,
                SUM(wr.quantity)::BIGINT AS total_qty,
                COALESCE(wr.high_item, p.high_item),
                COALESCE(NULLIF(wr.instrument_code_snapshot,''), '未绑定')
         FROM work_records wr
         LEFT JOIN project_groups pg ON pg.id = wr.group_id
         JOIN projects p ON wr.project_id = p.id
         LEFT JOIN methods m ON wr.method_id = m.id
         WHERE wr.deleted_at IS NULL
           AND wr.recorded_at >= ?1
           AND wr.recorded_at <= ?2
           AND (?3 IS NULL OR wr.subject_user_id = ?3){division_scope}
         GROUP BY p.id, pg.id, pg.name, m.id, wr.multiplier, wr.high_item, wr.instrument_code_snapshot
         ORDER BY lab_name, p.name, m.name");

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        postgres_compat::params![start, end_closed, subject_user_id],
        |row| {
            let instrument: String = row.get(8).unwrap_or_else(|_| "未绑定".into());

            Ok(LabSummaryRow {
                lab: row.get(0)?,
                project: row.get(1)?,
                instrument,
                method: row.get(3).unwrap_or_default(),
                multiplier: row.get::<_, f64>(4).unwrap_or(1.0),
                quantity: row.get(6)?,
                unit_price: row.get::<_, f64>(5).unwrap_or(0.0),
                high_item: row.get(7).unwrap_or(None),
            })
        },
    )?;

    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

// ========= Sheet 5: 人员-汇总（原始记录） ==========

pub fn query_sheet5_data(
    conn: &Connection,
    start: &str,
    end: &str,
    subject_user_id: Option<i64>,
    allowed_division_ids: Option<&[i64]>,
) -> Result<Vec<AnalysisPersonRecordRow>> {
    let end_closed = end_of_day_bound(end);

    // v0.3.25 修复：使用 wr.group_id 对应的 project_groups.name 显示单个实验室
    let division_scope = division_scope_sql(allowed_division_ids);
    let sql = format!("SELECT wr.recorded_at,
                COALESCE(NULLIF(wr.detection_division_name_snapshot,''), (SELECT d.name FROM divisions d WHERE d.id=wr.detection_division_id), '未配置') AS detection_department,
                COALESCE(NULLIF(wr.sending_division_name_snapshot,''), (SELECT d.name FROM divisions d WHERE d.id=COALESCE(wr.sending_division_id,wr.execution_division_id,wr.division_id)), '未配置') AS sending_department,
                COALESCE(pg.name, '未知') AS lab_name,
                p.name AS project_name,
                m.name AS method_name,
                COALESCE((SELECT string_agg(DISTINCT mt2.name, ',') FROM method_type_links mtl2 JOIN method_types mt2 ON mt2.id=mtl2.method_type_id WHERE mtl2.method_id=wr.method_id), '其他') AS method_types,
                wr.quantity,
                wr.user_name,
                COALESCE(wr.high_item, p.high_item)
         FROM work_records wr
         LEFT JOIN project_groups pg ON pg.id = wr.group_id
         JOIN projects p ON wr.project_id = p.id
         LEFT JOIN methods m ON wr.method_id = m.id
         WHERE wr.deleted_at IS NULL
           AND wr.recorded_at >= ?1
           AND wr.recorded_at <= ?2
           AND (?3 IS NULL OR wr.subject_user_id = ?3){division_scope}
         ORDER BY wr.recorded_at DESC");

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        postgres_compat::params![start, end_closed, subject_user_id],
        |row| {
            Ok(AnalysisPersonRecordRow {
                recorded_at: row.get(0)?,
                detection_department: row.get(1)?,
                sending_department: row.get(2)?,
                lab: row.get(3)?,
                project: row.get(4)?,
                method: row.get(5).unwrap_or_default(),
                method_type: row.get(6)?,
                multiplier: 1.0,
                quantity: row.get(7)?,
                user_name: row.get(8)?,
                high_item: row.get(9).unwrap_or(None),
            })
        },
    )?;

    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

// ========== Sheet 6: 人员汇总表 ==========

pub fn query_sheet6_data(
    conn: &Connection,
    start: &str,
    end: &str,
    subject_user_id: Option<i64>,
    allowed_division_ids: Option<&[i64]>,
) -> Result<Vec<PersonSummaryRow>> {
    let end_closed = end_of_day_bound(end);
    let division_scope = division_scope_sql(allowed_division_ids);
    let auxiliary_record = legacy_auxiliary_record_sql("wr");

    let sql = format!(
        "SELECT COALESCE((SELECT username FROM users WHERE id=wr.subject_user_id), '历史未绑定：' || MAX(wr.user_name)),
                string_agg(DISTINCT COALESCE(NULLIF(wr.instrument_code_snapshot,''), i.code, '未绑定仪器'), ', ' ORDER BY COALESCE(NULLIF(wr.instrument_code_snapshot,''), i.code, '未绑定仪器')) AS instrument_name,
                CASE WHEN {auxiliary_record} THEN '辅助工作' ELSE COALESCE(mt.name, '未分类') END AS method_type,
                COALESCE(NULLIF(wr.method_name_snapshot,''), NULLIF(m.full_name,''), m.name, '未知方法') AS method_name,
                COALESCE(wr.coefficient_snapshot, m.coefficient, 1.0) AS coefficient,
                SUM(wr.quantity)::BIGINT AS total_qty,
                SUM(wr.quantity * COALESCE(wr.coefficient_snapshot, m.coefficient, 1.0))::DOUBLE PRECISION AS total_workload
         FROM work_records wr
         LEFT JOIN project_groups pg ON pg.id = wr.group_id
         LEFT JOIN projects p ON wr.project_id = p.id
         LEFT JOIN methods m ON wr.method_id = m.id
         LEFT JOIN instruments i ON i.id = COALESCE(wr.instrument_id_snapshot, m.instrument_id)
         LEFT JOIN method_type_links mtl ON m.id = mtl.method_id
         LEFT JOIN method_types mt ON mtl.method_type_id = mt.id
         WHERE wr.deleted_at IS NULL
           AND wr.recorded_at >= ?1
           AND wr.recorded_at <= ?2
           AND (?3 IS NULL OR wr.subject_user_id = ?3){division_scope}
         GROUP BY wr.subject_user_id, CASE WHEN wr.subject_user_id IS NULL THEN wr.user_name END,
                  CASE WHEN {auxiliary_record} THEN '辅助工作' ELSE COALESCE(mt.name, '未分类') END,
                  COALESCE(NULLIF(wr.method_name_snapshot,''), NULLIF(m.full_name,''), m.name, '未知方法'),
                  COALESCE(wr.coefficient_snapshot, m.coefficient, 1.0),
                  mt.name
         ORDER BY 1, method_type, method_name, coefficient"
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        postgres_compat::params![start, end_closed, subject_user_id],
        |row| {
            Ok(PersonSummaryRow {
                user_name: row.get(0)?,
                project: String::new(),
                instrument: row.get(1)?,
                method_type: row.get(2)?,
                method: row.get(3)?,
                coefficient: row.get::<_, f64>(4).unwrap_or(1.0),
                multiplier: 1.0,
                quantity: row.get(5)?,
                workload: row.get::<_, f64>(6).unwrap_or(0.0),
            })
        },
    )?;

    let mut result = rows.collect::<std::result::Result<Vec<_>, _>>()?;
    let auxiliary_scope = auxiliary_scope_sql("awr", allowed_division_ids, None);
    let auxiliary_sql = format!("SELECT COALESCE((SELECT username FROM users WHERE id=awr.user_id), '历史未绑定：' || MAX(awr.user_name_snapshot)), awr.auxiliary_work_name_snapshot, awr.coefficient_snapshot, SUM(awr.quantity)::BIGINT, SUM(awr.quantity * awr.coefficient_snapshot)::DOUBLE PRECISION FROM auxiliary_work_records awr WHERE awr.deleted_at IS NULL AND CAST(CAST(awr.recorded_at AS TEXT) AS TIMESTAMPTZ) >= CAST(CAST(?1 AS TEXT) AS TIMESTAMPTZ) AND CAST(CAST(awr.recorded_at AS TEXT) AS TIMESTAMPTZ) <= CAST(CAST(?2 AS TEXT) AS TIMESTAMPTZ) AND (CAST(?3 AS BIGINT) IS NULL OR awr.user_id = CAST(?3 AS BIGINT)){auxiliary_scope} GROUP BY awr.user_id, CASE WHEN awr.user_id IS NULL THEN awr.user_name_snapshot END, awr.auxiliary_work_name_snapshot, awr.coefficient_snapshot ORDER BY 1, awr.auxiliary_work_name_snapshot");
    let mut auxiliary = conn.prepare(&auxiliary_sql)?;
    let auxiliary_rows = auxiliary.query_map(
        postgres_compat::params![start, end_closed, subject_user_id],
        |row| {
            Ok(PersonSummaryRow {
                user_name: row.get(0)?,
                project: String::new(),
                instrument: String::new(),
                method_type: "辅助工作".into(),
                method: row.get(1)?,
                coefficient: row.get::<_, f64>(2).unwrap_or(1.0),
                multiplier: 1.0,
                quantity: row.get(3)?,
                workload: row.get::<_, f64>(4).unwrap_or(0.0),
            })
        },
    )?;
    result.extend(auxiliary_rows.collect::<std::result::Result<Vec<_>, _>>()?);
    result.sort_by(|a, b| {
        a.user_name
            .cmp(&b.user_name)
            .then(a.method_type.cmp(&b.method_type))
            .then(a.method.cmp(&b.method))
    });
    Ok(result)
}

/// 查询人员工作量汇总（检测类型逐行）。
/// 系数优先取记录快照，保证历史工作量不受后续主数据修改影响。
pub fn query_person_type_workload_data(
    conn: &Connection,
    start: &str,
    end: &str,
    subject_user_id: Option<i64>,
    allowed_division_ids: Option<&[i64]>,
) -> Result<Vec<PersonTypeWorkloadRow>> {
    let end_closed = end_of_day_bound(end);
    let division_scope = division_scope_sql(allowed_division_ids);
    let auxiliary_record = legacy_auxiliary_record_sql("wr");
    let sql = format!(
        "SELECT COALESCE((SELECT username FROM users WHERE id=wr.subject_user_id), '历史未绑定：' || MAX(wr.user_name)),
                CASE WHEN {auxiliary_record} THEN '辅助工作' ELSE COALESCE(mt.name, '未分类') END AS method_type,
                COALESCE(wr.coefficient_snapshot, m.coefficient, 1.0) AS coefficient,
                SUM(wr.quantity)::BIGINT AS total_qty,
                SUM(wr.quantity * COALESCE(wr.coefficient_snapshot, m.coefficient, 1.0))::DOUBLE PRECISION AS total_workload
         FROM work_records wr
         LEFT JOIN methods m ON m.id = wr.method_id
         LEFT JOIN method_type_links mtl ON m.id = mtl.method_id
         LEFT JOIN method_types mt ON mtl.method_type_id = mt.id
         WHERE wr.deleted_at IS NULL
           AND wr.recorded_at >= ?1
           AND wr.recorded_at <= ?2
           AND (?3 IS NULL OR wr.subject_user_id = ?3){division_scope}
         GROUP BY wr.subject_user_id, CASE WHEN wr.subject_user_id IS NULL THEN wr.user_name END,
                  CASE WHEN {auxiliary_record} THEN '辅助工作' ELSE COALESCE(mt.name, '未分类') END,
                  COALESCE(wr.coefficient_snapshot, m.coefficient, 1.0)
         ORDER BY 1, method_type, coefficient"
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        postgres_compat::params![start, end_closed, subject_user_id],
        |row| {
            Ok(PersonTypeWorkloadRow {
                user_name: row.get(0)?,
                method_type: row.get(1)?,
                coefficient: row.get::<_, f64>(2).unwrap_or(1.0),
                quantity: row.get(3)?,
                workload: row.get::<_, f64>(4).unwrap_or(0.0),
            })
        },
    )?;

    let mut result = rows.collect::<std::result::Result<Vec<_>, _>>()?;
    let auxiliary_scope = auxiliary_scope_sql("awr", allowed_division_ids, None);
    let auxiliary_sql = format!("SELECT COALESCE((SELECT username FROM users WHERE id=awr.user_id), '历史未绑定：' || MAX(awr.user_name_snapshot)), awr.coefficient_snapshot, SUM(awr.quantity)::BIGINT, SUM(awr.quantity * awr.coefficient_snapshot)::DOUBLE PRECISION FROM auxiliary_work_records awr WHERE awr.deleted_at IS NULL AND CAST(CAST(awr.recorded_at AS TEXT) AS TIMESTAMPTZ) >= CAST(CAST(?1 AS TEXT) AS TIMESTAMPTZ) AND CAST(CAST(awr.recorded_at AS TEXT) AS TIMESTAMPTZ) <= CAST(CAST(?2 AS TEXT) AS TIMESTAMPTZ) AND (CAST(?3 AS BIGINT) IS NULL OR awr.user_id = CAST(?3 AS BIGINT)){auxiliary_scope} GROUP BY awr.user_id, CASE WHEN awr.user_id IS NULL THEN awr.user_name_snapshot END, awr.coefficient_snapshot ORDER BY 1");
    let mut auxiliary = conn.prepare(&auxiliary_sql)?;
    let auxiliary_rows = auxiliary.query_map(
        postgres_compat::params![start, end_closed, subject_user_id],
        |row| {
            Ok(PersonTypeWorkloadRow {
                user_name: row.get(0)?,
                method_type: "辅助工作".into(),
                coefficient: row.get::<_, f64>(1).unwrap_or(1.0),
                quantity: row.get(2)?,
                workload: row.get::<_, f64>(3).unwrap_or(0.0),
            })
        },
    )?;
    result.extend(auxiliary_rows.collect::<std::result::Result<Vec<_>, _>>()?);
    result.sort_by(|a, b| {
        a.user_name
            .cmp(&b.user_name)
            .then(a.method_type.cmp(&b.method_type))
    });
    Ok(result)
}

/// 查询辅助工作明细。辅助工作记录优先使用写入时保存的系数快照，
/// 仅对历史缺失快照的记录回退到方法当前系数，再回退到 1.0。
pub fn query_auxiliary_work_details(
    conn: &Connection,
    start: &str,
    end: &str,
    subject_user_id: Option<i64>,
    allowed_division_ids: Option<&[i64]>,
) -> Result<Vec<AuxiliaryWorkRow>> {
    let end_closed = end_of_day_bound(end);
    let auxiliary_scope = auxiliary_scope_sql("awr", allowed_division_ids, None);
    let sql = format!(
        "SELECT COALESCE((SELECT username FROM users WHERE id=awr.user_id), '历史未绑定：' || MAX(awr.user_name_snapshot)),
                '' AS project_name,
                awr.auxiliary_work_name_snapshot,
                awr.coefficient_snapshot,
                SUM(awr.quantity)::BIGINT,
                SUM(awr.quantity * awr.coefficient_snapshot)::DOUBLE PRECISION
         FROM auxiliary_work_records awr
         WHERE awr.deleted_at IS NULL
           AND CAST(CAST(awr.recorded_at AS TEXT) AS TIMESTAMPTZ) >= CAST(CAST(?1 AS TEXT) AS TIMESTAMPTZ)
           AND CAST(CAST(awr.recorded_at AS TEXT) AS TIMESTAMPTZ) <= CAST(CAST(?2 AS TEXT) AS TIMESTAMPTZ)
           AND (CAST(?3 AS BIGINT) IS NULL OR awr.user_id = CAST(?3 AS BIGINT)){auxiliary_scope}
         GROUP BY awr.user_id, CASE WHEN awr.user_id IS NULL THEN awr.user_name_snapshot END, awr.auxiliary_work_name_snapshot, awr.coefficient_snapshot
         ORDER BY 1, 2, 3"
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        postgres_compat::params![start, end_closed, subject_user_id],
        |row| {
            Ok(AuxiliaryWorkRow {
                user_name: row.get(0)?,
                project: row.get(1)?,
                auxiliary_method: row.get(2)?,
                coefficient: row.get::<_, f64>(3).unwrap_or(1.0),
                quantity: row.get(4)?,
                workload: row.get::<_, f64>(5).unwrap_or(0.0),
            })
        },
    )?;

    let mut result = rows.collect::<std::result::Result<Vec<_>, _>>()?;
    let division_scope = division_scope_sql(allowed_division_ids);
    let auxiliary_record = legacy_auxiliary_record_sql("wr");
    let legacy_sql = format!(
        "SELECT COALESCE((SELECT username FROM users WHERE id=wr.subject_user_id), '历史未绑定：' || MAX(wr.user_name)), '' AS project_name,
                COALESCE(NULLIF(wr.method_name_snapshot,''), '辅助工作'),
                COALESCE(wr.coefficient_snapshot, 1.0), SUM(wr.quantity)::BIGINT,
                SUM(wr.quantity * COALESCE(wr.coefficient_snapshot, 1.0))::DOUBLE PRECISION
         FROM work_records wr
         WHERE wr.deleted_at IS NULL AND {auxiliary_record}
           AND CAST(CAST(wr.recorded_at AS TEXT) AS TIMESTAMPTZ) >= CAST(CAST(?1 AS TEXT) AS TIMESTAMPTZ)
           AND CAST(CAST(wr.recorded_at AS TEXT) AS TIMESTAMPTZ) <= CAST(CAST(?2 AS TEXT) AS TIMESTAMPTZ)
           AND (CAST(?3 AS BIGINT) IS NULL OR wr.subject_user_id = CAST(?3 AS BIGINT)){division_scope}
         GROUP BY wr.subject_user_id, CASE WHEN wr.subject_user_id IS NULL THEN wr.user_name END, COALESCE(NULLIF(wr.method_name_snapshot,''), '辅助工作'), wr.coefficient_snapshot
         ORDER BY 1, 3"
    );
    let mut legacy = conn.prepare(&legacy_sql)?;
    let legacy_rows = legacy.query_map(
        postgres_compat::params![start, end_closed, subject_user_id],
        |row| {
            Ok(AuxiliaryWorkRow {
                user_name: row.get(0)?,
                project: row.get(1)?,
                auxiliary_method: row.get(2)?,
                coefficient: row.get::<_, f64>(3).unwrap_or(1.0),
                quantity: row.get(4)?,
                workload: row.get::<_, f64>(5).unwrap_or(0.0),
            })
        },
    )?;
    result.extend(legacy_rows.collect::<std::result::Result<Vec<_>, _>>()?);
    result.sort_by(|a, b| {
        a.user_name
            .cmp(&b.user_name)
            .then(a.auxiliary_method.cmp(&b.auxiliary_method))
    });
    Ok(result)
}

// ========= Sheet 7: 实验室总表 ==========

pub fn query_sheet7_data(
    conn: &Connection,
    start: &str,
    end: &str,
    subject_user_id: Option<i64>,
    allowed_division_ids: Option<&[i64]>,
) -> Result<Vec<LabTotalRow>> {
    let end_closed = end_of_day_bound(end);

    // v0.3.25 修复：使用 wr.group_id 对应的 project_groups.name 显示单个实验室
    let division_scope = detection_scope_sql(allowed_division_ids);
    let sql = format!(
        "SELECT COALESCE(pg.name, '未知') AS lab_name,
                p.name AS project_name,
                COALESCE(mt.name, '未分类') AS method_type,
                COALESCE(wr.multiplier, m.multiplier, 1.0),
                m.amount,
                SUM(wr.quantity)::BIGINT AS total_qty
         FROM work_records wr
         LEFT JOIN project_groups pg ON pg.id = wr.group_id
         JOIN projects p ON wr.project_id = p.id
         LEFT JOIN methods m ON wr.method_id = m.id
         LEFT JOIN method_type_links mtl ON m.id = mtl.method_id
         LEFT JOIN method_types mt ON mtl.method_type_id = mt.id
         WHERE wr.deleted_at IS NULL
           AND wr.recorded_at >= ?1
           AND wr.recorded_at <= ?2
           AND (?3 IS NULL OR wr.subject_user_id = ?3){division_scope}
         GROUP BY pg.id, pg.name, p.id, mt.name, m.amount, m.multiplier, wr.multiplier
         ORDER BY lab_name, p.name, mt.name"
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        postgres_compat::params![start, end_closed, subject_user_id],
        |row| {
            Ok(LabTotalRow {
                lab: row.get(0)?,
                project: row.get(1)?,
                method_type: row.get(2)?,
                multiplier: row.get::<_, f64>(3).unwrap_or(1.0),
                unit_price: row.get::<_, f64>(4).unwrap_or(0.0),
                quantity: row.get(5)?,
            })
        },
    )?;

    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

// ========== Sheet 8: 项目总表 ==========

pub fn query_sheet8_data(
    conn: &Connection,
    start: &str,
    end: &str,
    subject_user_id: Option<i64>,
    allowed_division_ids: Option<&[i64]>,
) -> Result<Vec<ProjectTotalRow>> {
    let end_closed = end_of_day_bound(end);

    let division_scope = detection_scope_sql(allowed_division_ids);
    let sql = format!(
        "SELECT p.name AS project_name,
                COALESCE(mt.name, '未分类') AS method_type,
                COALESCE(wr.multiplier, m.multiplier, 1.0),
                m.amount,
                SUM(wr.quantity)::BIGINT AS total_qty
         FROM work_records wr
         LEFT JOIN project_groups pg ON pg.id = wr.group_id
         JOIN projects p ON wr.project_id = p.id
         LEFT JOIN methods m ON wr.method_id = m.id
         LEFT JOIN method_type_links mtl ON m.id = mtl.method_id
         LEFT JOIN method_types mt ON mtl.method_type_id = mt.id
         WHERE wr.deleted_at IS NULL
           AND wr.recorded_at >= ?1
           AND wr.recorded_at <= ?2
           AND (?3 IS NULL OR wr.subject_user_id = ?3){division_scope}
         GROUP BY p.id, mt.name, m.amount, m.multiplier, wr.multiplier
         ORDER BY p.name, mt.name"
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        postgres_compat::params![start, end_closed, subject_user_id],
        |row| {
            Ok(ProjectTotalRow {
                project: row.get(0)?,
                method_type: row.get(1)?,
                multiplier: row.get::<_, f64>(2).unwrap_or(1.0),
                unit_price: row.get::<_, f64>(3).unwrap_or(0.0),
                quantity: row.get(4)?,
            })
        },
    )?;

    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

// ========== Sheet 9: 仪器汇总表 ==========

pub fn query_sheet9_data(
    conn: &Connection,
    start: &str,
    end: &str,
    subject_user_id: Option<i64>,
    allowed_division_ids: Option<&[i64]>,
) -> Result<Vec<InstrumentSummaryRow>> {
    let end_closed = end_of_day_bound(end);

    let division_scope = detection_scope_sql(allowed_division_ids);
    let sql = format!(
        "SELECT COALESCE(NULLIF(wr.instrument_code_snapshot,''), '未绑定'),
                SUM(wr.quantity)::BIGINT AS total_qty,
                COALESCE(NULLIF(wr.instrument_type_snapshot,''), '其他')
         FROM work_records wr
         LEFT JOIN project_groups pg ON pg.id = wr.group_id
         JOIN projects p ON wr.project_id = p.id
         LEFT JOIN methods m ON wr.method_id = m.id
         WHERE wr.deleted_at IS NULL
           AND wr.recorded_at >= ?1
           AND wr.recorded_at <= ?2
           AND (?3 IS NULL OR wr.subject_user_id = ?3){division_scope}
         GROUP BY COALESCE(CAST(wr.instrument_id_snapshot AS TEXT), 'legacy:' || COALESCE(NULLIF(wr.instrument_code_snapshot,''), '未绑定')),
                  COALESCE(NULLIF(wr.instrument_code_snapshot,''), '未绑定'),
                  COALESCE(NULLIF(wr.instrument_type_snapshot,''), '其他')
         ORDER BY total_qty DESC");

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        postgres_compat::params![start, end_closed, subject_user_id],
        |row| {
            Ok(InstrumentSummaryRow {
                instrument: row.get(0)?,
                quantity: row.get(1)?,
                instrument_type: row.get(2)?,
                multiplier: 1.0,
            })
        },
    )?;

    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

// ========== Sheet 10: 理化汇总表 ==========

pub fn query_sheet10_data(
    conn: &Connection,
    start: &str,
    end: &str,
    subject_user_id: Option<i64>,
    allowed_division_ids: Option<&[i64]>,
) -> Result<Vec<PhysChemRow>> {
    let end_closed = end_of_day_bound(end);

    let division_scope = detection_scope_sql(allowed_division_ids);
    let sql = format!(
        "SELECT m.name || CASE WHEN COALESCE(wr.instrument_code_snapshot,'')='' THEN '' ELSE ' [' || wr.instrument_code_snapshot || ']' END AS method_name,
                SUM(wr.quantity)::BIGINT AS total_qty
         FROM work_records wr
         LEFT JOIN project_groups pg ON pg.id = wr.group_id
         JOIN projects p ON wr.project_id = p.id
         LEFT JOIN methods m ON wr.method_id = m.id
         LEFT JOIN method_type_links mtl ON m.id = mtl.method_id
         LEFT JOIN method_types mt ON mtl.method_type_id = mt.id
         WHERE wr.deleted_at IS NULL
           AND wr.recorded_at >= ?1
           AND wr.recorded_at <= ?2
           AND (?3 IS NULL OR wr.subject_user_id = ?3){division_scope}
           AND mt.name = '理化'
         GROUP BY wr.method_id, wr.instrument_id_snapshot, m.name, wr.instrument_code_snapshot
         ORDER BY total_qty DESC");

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        postgres_compat::params![start, end_closed, subject_user_id],
        |row| {
            Ok(PhysChemRow {
                method: row.get(0).unwrap_or_default(),
                quantity: row.get(1)?,
                multiplier: 1.0,
            })
        },
    )?;

    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

/// 类型汇总行（Sheet 11）
#[derive(Debug, Clone, serde::Serialize)]
pub struct TypeSummaryRow {
    pub method_type: String,
    pub quantity: i64,
    pub unit_price: f64,
    pub multiplier: f64,
}

// ========== Sheet 11: 类型汇总表 ==========

pub fn query_sheet11_data(
    conn: &Connection,
    start: &str,
    end: &str,
    subject_user_id: Option<i64>,
) -> Result<Vec<TypeSummaryRow>> {
    let end_closed = end_of_day_bound(end);
    let auxiliary_record = legacy_auxiliary_record_sql("wr");

    let sql = format!(
        "SELECT COALESCE(mt.name, '其他') AS method_type,
                SUM(wr.quantity)::BIGINT AS total_qty,
                m.amount
         FROM work_records wr
         LEFT JOIN project_groups pg ON pg.id = wr.group_id
         JOIN projects p ON wr.project_id = p.id
         LEFT JOIN methods m ON wr.method_id = m.id
         LEFT JOIN method_type_links mtl ON m.id = mtl.method_id
         LEFT JOIN method_types mt ON mtl.method_type_id = mt.id
         WHERE wr.deleted_at IS NULL
           AND wr.recorded_at >= ?1
           AND wr.recorded_at <= ?2
           AND (?3 IS NULL OR wr.subject_user_id = ?3) AND NOT {auxiliary_record}
         GROUP BY mt.name, m.amount
         ORDER BY method_type, m.amount"
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        postgres_compat::params![start, end_closed, subject_user_id],
        |row| {
            Ok(TypeSummaryRow {
                method_type: row.get(0)?,
                quantity: row.get(1)?,
                unit_price: row.get::<_, f64>(2).unwrap_or(0.0),
                multiplier: 1.0,
            })
        },
    )?;

    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

#[cfg(test)]
mod identity_tests {
    use super::*;

    #[test]
    fn export_date_bound_includes_final_microsecond_and_preserves_precise_times() {
        let conn = postgres_compat::Connection::open_test_database().unwrap();
        for end in ["2091-01-31", "2091-01-31T23:59:59"] {
            let bound = end_of_day_bound(end);
            let count: i64 = conn.query_row("WITH records(recorded_at) AS (VALUES ('2091-01-31T23:59:59.500'),('2091-01-31T23:59:59.999999'),('2091-02-01T00:00:00')) SELECT COUNT(*) FROM records WHERE recorded_at<=?1",[bound],|row|row.get(0)).unwrap();
            assert_eq!(count, 2);
        }
        assert_eq!(
            end_of_day_bound("2091-01-31T12:15:00"),
            "2091-01-31T12:15:00"
        );
    }

    #[test]
    fn professional_text_filters_match_project_codes_and_method_labels_without_allocating_auxiliary(
    ) {
        let pool = crate::db::init_pool("postgres-test");
        let conn = pool.get().expect("connection");
        crate::db::test_migrations::run(&conn).expect("migrations");
        let tx = conn.unchecked_transaction().expect("transaction");
        tx.execute_batch("CREATE TEMP TABLE users ON COMMIT DROP AS SELECT * FROM users WITH NO DATA;
            CREATE TEMP TABLE work_records ON COMMIT DROP AS SELECT * FROM work_records WITH NO DATA;
            CREATE TEMP TABLE auxiliary_work_records ON COMMIT DROP AS SELECT * FROM auxiliary_work_records WITH NO DATA;
            CREATE TEMP TABLE projects ON COMMIT DROP AS SELECT * FROM projects WITH NO DATA;
            CREATE TEMP TABLE project_groups ON COMMIT DROP AS SELECT * FROM project_groups WITH NO DATA;
            CREATE TEMP TABLE methods ON COMMIT DROP AS SELECT * FROM methods WITH NO DATA;
            CREATE TEMP TABLE method_types ON COMMIT DROP AS SELECT * FROM method_types WITH NO DATA;
            CREATE TEMP TABLE method_type_links ON COMMIT DROP AS SELECT * FROM method_type_links WITH NO DATA;
            ALTER TABLE projects ADD PRIMARY KEY(id);
            ALTER TABLE project_groups ADD PRIMARY KEY(id);
            ALTER TABLE methods ADD PRIMARY KEY(id);
            INSERT INTO users(id,username) VALUES(93001,'viewer');
            INSERT INTO projects(id,name) VALUES(93011,'P1-description'),(93012,'P2-description');
            INSERT INTO project_groups(id,name) VALUES(93021,'L1'),(93022,'L2');
            INSERT INTO methods(id,name,full_name,coefficient,amount,multiplier) VALUES(93031,'M1','M1 full',3,10,2),(93032,'M2','M2 full',3,10,2);
            INSERT INTO method_types(id,name) VALUES(93051,'理化');
            INSERT INTO method_type_links(method_id,method_type_id) VALUES(93031,93051),(93032,93051);
            INSERT INTO work_records(id,subject_user_id,user_name,quantity,coefficient_snapshot,recorded_at,detection_division_id,project_id,group_id,method_id,instrument_code_snapshot,instrument_type_snapshot,lab_name_snapshot,project_name_snapshot,method_name_snapshot)
              VALUES(1,93001,'viewer',2,3,'2035-01-02T12:00:00',93041,93011,93021,93031,'I1','','old_L1','old_P1','old_M1'),
                    (2,93001,'viewer',4,3,'2035-01-02T12:00:00',93041,93012,93022,93032,'I2','','L2','P2-description','M2'),
                    (3,93001,'viewer',5,3,'2035-01-02T12:00:00',93041,93012,93022,93032,'I2','辅助工作','L2','P2-description','M2');
            INSERT INTO auxiliary_work_records(id,user_id,user_name_snapshot,auxiliary_work_name_snapshot,coefficient_snapshot,quantity,recorded_at)
              VALUES(1,93001,'viewer','independent_helper',3,7,'2035-01-02T12:00:00');").expect("fixtures");
        configure_dimension_filters(&tx, None, None, None, false, None).unwrap();
        configure_auxiliary_viewer(&tx, 93001).unwrap();
        configure_text_filters(&tx, Some("L1"), Some("P1"), Some("I1"), Some("M1 [I1]")).unwrap();
        let scoped =
            query_sheet6_data(&tx, "2035-01-01", "2035-01-03", None, Some(&[93041])).unwrap();
        assert_eq!(scoped.iter().map(|row| row.quantity).sum::<i64>(), 14);
        let types =
            query_person_type_workload_data(&tx, "2035-01-01", "2035-01-03", None, Some(&[93041]))
                .unwrap();
        assert_eq!(types.iter().map(|row| row.workload).sum::<f64>(), 42.0);
        let auxiliary =
            query_auxiliary_work_details(&tx, "2035-01-01", "2035-01-03", None, Some(&[93041]))
                .unwrap();
        assert_eq!(auxiliary.iter().map(|row| row.quantity).sum::<i64>(), 12);
        configure_text_filters(&tx, Some("L1' OR TRUE"), None, None, None).unwrap();
        let no_detection =
            query_sheet6_data(&tx, "2035-01-01", "2035-01-03", None, Some(&[93041])).unwrap();
        assert_eq!(no_detection.iter().map(|row| row.quantity).sum::<i64>(), 12);
        configure_text_filters(&tx, None, None, None, None).unwrap();
        let reset =
            query_sheet6_data(&tx, "2035-01-01", "2035-01-03", None, Some(&[93041])).unwrap();
        assert_eq!(reset.iter().map(|row| row.quantity).sum::<i64>(), 18);
        assert_eq!(
            query_sheet1_data(&tx, "2035-01-01", "2035-01-03", None, None, Some(&[93041]))
                .unwrap()
                .iter()
                .map(|row| row.5)
                .sum::<i64>(),
            6
        );
        assert_eq!(
            query_sheet2_data(&tx, "2035-01-01", "2035-01-03", None, Some(&[93041]))
                .unwrap()
                .iter()
                .map(|row| row.quantity)
                .sum::<i64>(),
            6
        );
        let project =
            query_sheet3_data(&tx, "2035-01-01", "2035-01-03", None, Some(&[93041])).unwrap();
        assert_eq!(project.iter().map(|row| row.quantity).sum::<i64>(), 6);
        assert_eq!(
            project
                .iter()
                .map(|row| row.quantity as f64 * row.unit_price * row.multiplier)
                .sum::<f64>(),
            120.0
        );
        assert_eq!(
            query_sheet4_data(&tx, "2035-01-01", "2035-01-03", None, Some(&[93041]))
                .unwrap()
                .iter()
                .map(|row| row.quantity)
                .sum::<i64>(),
            6
        );
        let lab = query_sheet7_data(&tx, "2035-01-01", "2035-01-03", None, Some(&[93041])).unwrap();
        assert_eq!(lab.iter().map(|row| row.quantity).sum::<i64>(), 6);
        assert_eq!(
            lab.iter()
                .map(|row| row.quantity as f64 * row.unit_price * row.multiplier)
                .sum::<f64>(),
            120.0
        );
        assert_eq!(
            query_sheet8_data(&tx, "2035-01-01", "2035-01-03", None, Some(&[93041]))
                .unwrap()
                .iter()
                .map(|row| row.quantity)
                .sum::<i64>(),
            6
        );
        assert_eq!(
            query_sheet9_data(&tx, "2035-01-01", "2035-01-03", None, Some(&[93041]))
                .unwrap()
                .iter()
                .map(|row| row.quantity)
                .sum::<i64>(),
            6
        );
        assert_eq!(
            query_sheet10_data(&tx, "2035-01-01", "2035-01-03", None, Some(&[93041]))
                .unwrap()
                .iter()
                .map(|row| row.quantity)
                .sum::<i64>(),
            6
        );
        assert_eq!(
            query_sheet11_data(&tx, "2035-01-01", "2035-01-03", None)
                .unwrap()
                .iter()
                .map(|row| row.quantity)
                .sum::<i64>(),
            6
        );
        assert_eq!(
            query_department_pair_summary_data(
                &tx,
                "2035-01-01",
                "2035-01-03",
                None,
                Some(&[93041])
            )
            .unwrap()
            .iter()
            .map(|row| row.total_quantity)
            .sum::<i64>(),
            6
        );
        assert_eq!(
            query_sheet5_data(&tx, "2035-01-01", "2035-01-03", None, Some(&[93041]))
                .unwrap()
                .iter()
                .map(|row| row.quantity)
                .sum::<i64>(),
            11
        );
        configure_dimension_filters(&tx, Some(93021), None, None, false, None).unwrap();
        let selected_lab =
            query_sheet6_data(&tx, "2035-01-01", "2035-01-03", None, Some(&[93041])).unwrap();
        assert_eq!(selected_lab.iter().map(|row| row.quantity).sum::<i64>(), 14);
        configure_dimension_filters(&tx, Some(93021), None, Some("93999"), false, None).unwrap();
        let selected_sending =
            query_person_type_workload_data(&tx, "2035-01-01", "2035-01-03", None, Some(&[93041]))
                .unwrap();
        assert_eq!(
            selected_sending.iter().map(|row| row.quantity).sum::<i64>(),
            12
        );
        let auxiliary =
            query_auxiliary_work_details(&tx, "2035-01-01", "2035-01-03", None, Some(&[93041]))
                .unwrap();
        assert_eq!(auxiliary.iter().map(|row| row.quantity).sum::<i64>(), 12);
        configure_dimension_filters(&tx, None, Some("93041"), None, false, None).unwrap();
        let selected_department =
            query_sheet6_data(&tx, "2035-01-01", "2035-01-03", None, Some(&[93041])).unwrap();
        assert_eq!(
            selected_department
                .iter()
                .map(|row| row.quantity)
                .sum::<i64>(),
            11
        );
        let auxiliary =
            query_auxiliary_work_details(&tx, "2035-01-01", "2035-01-03", None, Some(&[93041]))
                .unwrap();
        assert_eq!(auxiliary.iter().map(|row| row.quantity).sum::<i64>(), 5);
    }

    #[test]
    fn auxiliary_exports_enforce_department_snapshots_and_exact_source_links() {
        let pool = crate::db::init_pool("postgres-test");
        let conn = pool.get().expect("connection");
        crate::db::test_migrations::run(&conn).expect("migrations");
        let tx = conn.unchecked_transaction().expect("transaction");
        tx.execute_batch("CREATE TEMP TABLE users ON COMMIT DROP AS SELECT * FROM users WITH NO DATA;
            CREATE TEMP TABLE work_records ON COMMIT DROP AS SELECT * FROM work_records WITH NO DATA;
            CREATE TEMP TABLE auxiliary_work_records ON COMMIT DROP AS SELECT * FROM auxiliary_work_records WITH NO DATA;
            CREATE TEMP TABLE methods ON COMMIT DROP AS SELECT * FROM methods WITH NO DATA;
            CREATE TEMP TABLE method_types ON COMMIT DROP AS SELECT * FROM method_types WITH NO DATA;
            CREATE TEMP TABLE method_type_links ON COMMIT DROP AS SELECT * FROM method_type_links WITH NO DATA;
            INSERT INTO methods(id,name,coefficient) VALUES(92301,'helper_method',99);
            INSERT INTO method_types(id,name) VALUES(92302,'辅助工作');
            INSERT INTO method_type_links(method_id,method_type_id) VALUES(92301,92302);
            INSERT INTO users(id,username) VALUES(92001,'viewer'),(92002,'same_department'),(92003,'other_department');
            INSERT INTO auxiliary_work_records(id,user_id,user_name_snapshot,auxiliary_work_name_snapshot,coefficient_snapshot,quantity,recorded_at,detection_division_id)
              VALUES(1,92001,'viewer','own_history',2,3,'2035-01-02T12:00:00',NULL),
                    (2,92002,'same_department','same_department',2,2,'2035-01-02T12:00:00',92101),
                    (3,92001,'viewer','own_department',2,5,'2035-01-02T12:00:00',92101),
                    (4,92001,'viewer','other_department',2,7,'2035-01-02T12:00:00',92102),
                    (5,92003,'other_department','other_history',2,11,'2035-01-02T12:00:00',NULL),
                    (6,NULL,'unbound','unbound_history',2,13,'2035-01-02T12:00:00',NULL),
                    (7,92001,'viewer','linked_legacy',2,31,'2035-01-02T12:00:00',92101),
                    (8,92001,'viewer','deleted_history',2,14,'2035-01-02T12:00:00',NULL);
            UPDATE auxiliary_work_records SET deleted_at=CURRENT_TIMESTAMP WHERE id=8;
            INSERT INTO work_records(id,subject_user_id,user_name,quantity,coefficient_snapshot,recorded_at,instrument_type_snapshot,detection_division_id,source_type,source_record_id)
              VALUES(1,92002,'same_department',17,2,'2035-01-02T12:00:00','辅助工作',92101,NULL,NULL),
                    (2,92003,'other_department',19,2,'2035-01-02T12:00:00','辅助工作',92102,NULL,NULL),
                    (3,NULL,'unbound',23,2,'2035-01-02T12:00:00','辅助工作',NULL,NULL,NULL),
                    (4,92001,'viewer',31,2,'2035-01-02T12:00:00','',92101,'auxiliary_work',7);
            INSERT INTO work_records(id,subject_user_id,user_name,quantity,coefficient_snapshot,recorded_at,instrument_type_snapshot,detection_division_id,method_id)
              VALUES(5,92002,'same_department',37,2,'2035-01-02T12:00:00','',92101,92301);").expect("fixtures");
        configure_dimension_filters(&tx, None, None, None, false, None).expect("scope");
        configure_auxiliary_viewer(&tx, 92001).expect("viewer");

        let assert_totals = |allowed: Option<&[i64]>, expected: i64| {
            let methods =
                query_sheet6_data(&tx, "2035-01-01", "2035-01-03", None, allowed).unwrap();
            let types =
                query_person_type_workload_data(&tx, "2035-01-01", "2035-01-03", None, allowed)
                    .unwrap();
            let details =
                query_auxiliary_work_details(&tx, "2035-01-01", "2035-01-03", None, allowed)
                    .unwrap();
            assert_eq!(
                methods.iter().map(|row| row.quantity).sum::<i64>(),
                expected
            );
            assert_eq!(types.iter().map(|row| row.quantity).sum::<i64>(), expected);
            assert_eq!(
                details.iter().map(|row| row.quantity).sum::<i64>(),
                expected
            );
            assert!(methods.iter().all(|row| row.method_type == "辅助工作"));
            assert!(types.iter().all(|row| row.method_type == "辅助工作"));
            assert_eq!(
                methods.iter().map(|row| row.workload).sum::<f64>(),
                (expected * 2) as f64
            );
            assert_eq!(
                types.iter().map(|row| row.workload).sum::<f64>(),
                (expected * 2) as f64
            );
            assert_eq!(
                details.iter().map(|row| row.workload).sum::<f64>(),
                (expected * 2) as f64
            );
        };
        assert_totals(Some(&[92101]), 95);
        assert_totals(Some(&[]), 3);
        assert_totals(None, 168);
        configure_dimension_filters(&tx, None, Some("92101"), None, false, None).unwrap();
        assert_totals(Some(&[92101]), 92);
        assert_totals(Some(&[]), 0);
        assert_totals(None, 92);
        configure_dimension_filters(&tx, None, None, None, false, None).unwrap();
        let own = query_auxiliary_work_details(
            &tx,
            "2035-01-01",
            "2035-01-03",
            Some(92001),
            Some(&[92101]),
        )
        .unwrap();
        assert_eq!(own.iter().map(|row| row.quantity).sum::<i64>(), 39);

        // Deleting/restoring the linked compatibility row must keep one copy.
        tx.execute_batch("UPDATE work_records SET deleted_at=CURRENT_TIMESTAMP WHERE id=4")
            .unwrap();
        assert_totals(Some(&[92101]), 95);
        tx.execute_batch("UPDATE work_records SET deleted_at=NULL WHERE id=4")
            .unwrap();
        assert_totals(Some(&[92101]), 95);
    }

    #[test]
    fn personnel_exports_use_ids_and_keep_unbound_history_global_only() {
        let pool = crate::db::init_pool("postgres-test");
        let conn = pool.get().expect("connection");
        crate::db::test_migrations::run(&conn).expect("migrations");
        let tx = conn.unchecked_transaction().expect("transaction");
        tx.execute_batch("CREATE TEMP TABLE users ON COMMIT DROP AS SELECT * FROM users WITH NO DATA;
            CREATE TEMP TABLE work_records ON COMMIT DROP AS SELECT * FROM work_records WITH NO DATA;
            CREATE TEMP TABLE auxiliary_work_records ON COMMIT DROP AS SELECT * FROM auxiliary_work_records WITH NO DATA;
            INSERT INTO users(id,username) VALUES(91001,'renamed'),(91002,'original');
            INSERT INTO work_records(subject_user_id,user_name,quantity,coefficient_snapshot,recorded_at,instrument_type_snapshot)
              VALUES(91001,'original',2,3,'2035-01-02T12:00:00','辅助工作'),
                    (91001,'renamed',3,3,'2035-01-02T12:00:00','辅助工作'),
                    (NULL,'original',7,3,'2035-01-02T12:00:00','辅助工作');
            INSERT INTO auxiliary_work_records(user_id,user_name_snapshot,auxiliary_work_name_snapshot,coefficient_snapshot,quantity,recorded_at)
              VALUES(91001,'original','helper',3,2,'2035-01-02T12:00:00'),
                    (91001,'renamed','helper',3,3,'2035-01-02T12:00:00'),
                    (NULL,'original','helper',3,7,'2035-01-02T12:00:00');").expect("fixtures");
        configure_dimension_filters(&tx, None, None, None, false, None).expect("scope");
        assert!(
            query_sheet6_data(&tx, "2035-01-01", "2035-01-03", Some(91002), None)
                .unwrap()
                .is_empty()
        );
        assert!(query_person_type_workload_data(
            &tx,
            "2035-01-01",
            "2035-01-03",
            Some(91002),
            None
        )
        .unwrap()
        .is_empty());
        assert!(
            query_auxiliary_work_details(&tx, "2035-01-01", "2035-01-03", Some(91002), None)
                .unwrap()
                .is_empty()
        );
        let own = query_sheet6_data(&tx, "2035-01-01", "2035-01-03", Some(91001), None).unwrap();
        assert_eq!(own.len(), 2);
        assert!(own
            .iter()
            .all(|row| row.user_name == "renamed" && row.quantity == 5));
        tx.execute_batch("CREATE TEMP TABLE projects (LIKE projects INCLUDING ALL) ON COMMIT DROP;
            INSERT INTO projects(id,name,group_id) VALUES(91010,'test',91011);
            UPDATE work_records SET project_id=91010,group_id=CASE WHEN quantity=2 THEN 91011 ELSE 91012 END,
                instrument_type_snapshot=CASE WHEN quantity=2 THEN '' ELSE instrument_type_snapshot END;").unwrap();
        let lab =
            query_sheet1_data(&tx, "2035-01-01", "2035-01-03", Some(91011), None, None).unwrap();
        assert_eq!(lab.iter().map(|row| row.5).sum::<i64>(), 2);
        configure_dimension_filters(&tx, Some(91011), None, None, false, None).unwrap();
        let scoped =
            query_person_type_workload_data(&tx, "2035-01-01", "2035-01-03", Some(91001), None)
                .unwrap();
        assert_eq!(scoped.iter().map(|row| row.quantity).sum::<i64>(), 10); // 2 lab samples + 3 legacy and 5 independent auxiliary work.
        configure_dimension_filters(&tx, None, None, None, false, None).unwrap();
        let all =
            query_person_type_workload_data(&tx, "2035-01-01", "2035-01-03", None, None).unwrap();
        assert_eq!(all.iter().map(|row| row.quantity).sum::<i64>(), 24);
        assert_eq!(
            all.iter()
                .filter(|row| row.user_name == "历史未绑定：original")
                .count(),
            2
        );
    }
}
