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
    format!(
        "{sending_scope}
         AND (COALESCE(current_setting('workload.export_include_pending_ownership',true),'false')='true'
              OR COALESCE(wr.ownership_status,'confirmed')<>'pending_confirmation')
         AND (NULLIF(current_setting('workload.export_detection_division_ids',true),'') IS NULL
              OR wr.detection_division_id = ANY(string_to_array(current_setting('workload.export_detection_division_ids',true), ',')::BIGINT[]))
         AND (NULLIF(current_setting('workload.export_sending_division_ids',true),'') IS NULL
              OR COALESCE(wr.sending_division_id,wr.execution_division_id,wr.division_id,(SELECT division_id FROM project_groups WHERE id=wr.group_id)) = ANY(string_to_array(current_setting('workload.export_sending_division_ids',true), ',')::BIGINT[]))"
    )
}

/// Converts a date-only upper bound to the end of that day without modifying
/// an upper bound that already contains a time component.
pub(crate) fn end_of_day_bound(end: &str) -> String {
    let end = end.trim();
    if end.contains('T') || end.contains(' ') {
        end.to_owned()
    } else {
        format!("{end}T23:59:59")
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
    let division_scope = division_scope_sql(allowed_sending_division_ids);
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

    sql.push_str(&division_scope_sql(allowed_division_ids));
    if let Some(gid) = group_id {
        sql.push_str(&format!(" AND EXISTS (SELECT 1 FROM project_lab_links pll_f WHERE pll_f.project_id = p.id AND pll_f.group_id = {})", gid));
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
    let division_scope = division_scope_sql(allowed_division_ids);
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
    let division_scope = division_scope_sql(allowed_division_ids);
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
    let division_scope = division_scope_sql(allowed_division_ids);
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

    let sql = format!(
        "SELECT wr.user_name,
                string_agg(DISTINCT COALESCE(NULLIF(wr.instrument_code_snapshot,''), i.code, '未绑定仪器'), ', ' ORDER BY COALESCE(NULLIF(wr.instrument_code_snapshot,''), i.code, '未绑定仪器')) AS instrument_name,
                CASE WHEN COALESCE(mt.name, '未分类')='辅助工作' THEN '辅助工作' ELSE COALESCE(mt.name, '未分类') END AS method_type,
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
         GROUP BY wr.user_name,
                  CASE WHEN COALESCE(mt.name, '未分类')='辅助工作' THEN '辅助工作' ELSE COALESCE(mt.name, '未分类') END,
                  COALESCE(NULLIF(wr.method_name_snapshot,''), NULLIF(m.full_name,''), m.name, '未知方法'),
                  COALESCE(wr.coefficient_snapshot, m.coefficient, 1.0),
                  mt.name
         ORDER BY wr.user_name, method_type, method_name, coefficient"
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
    let mut auxiliary = conn.prepare("SELECT user_name_snapshot, auxiliary_work_name_snapshot, coefficient_snapshot, SUM(quantity)::BIGINT, SUM(quantity * coefficient_snapshot)::DOUBLE PRECISION FROM auxiliary_work_records WHERE deleted_at IS NULL AND CAST(CAST(recorded_at AS TEXT) AS TIMESTAMPTZ) >= CAST(CAST(?1 AS TEXT) AS TIMESTAMPTZ) AND CAST(CAST(recorded_at AS TEXT) AS TIMESTAMPTZ) <= CAST(CAST(?2 AS TEXT) AS TIMESTAMPTZ) AND (CAST(CAST(?3 AS TEXT) AS BIGINT) IS NULL OR user_id = CAST(CAST(?3 AS TEXT) AS BIGINT) OR user_name_snapshot = (SELECT username FROM users WHERE id = CAST(CAST(?3 AS TEXT) AS BIGINT) LIMIT 1)) GROUP BY user_name_snapshot, auxiliary_work_name_snapshot, coefficient_snapshot ORDER BY user_name_snapshot, auxiliary_work_name_snapshot")?;
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
    let sql = format!(
        "SELECT wr.user_name,
                COALESCE(mt.name, '未分类') AS method_type,
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
         GROUP BY wr.user_name, COALESCE(mt.name, '未分类'),
                  COALESCE(wr.coefficient_snapshot, m.coefficient, 1.0)
         ORDER BY wr.user_name, method_type, coefficient"
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
    let mut auxiliary = conn.prepare("SELECT user_name_snapshot, coefficient_snapshot, SUM(quantity)::BIGINT, SUM(quantity * coefficient_snapshot)::DOUBLE PRECISION FROM auxiliary_work_records WHERE deleted_at IS NULL AND CAST(CAST(recorded_at AS TEXT) AS TIMESTAMPTZ) >= CAST(CAST(?1 AS TEXT) AS TIMESTAMPTZ) AND CAST(CAST(recorded_at AS TEXT) AS TIMESTAMPTZ) <= CAST(CAST(?2 AS TEXT) AS TIMESTAMPTZ) AND (CAST(CAST(?3 AS TEXT) AS BIGINT) IS NULL OR user_id = CAST(CAST(?3 AS TEXT) AS BIGINT) OR user_name_snapshot = (SELECT username FROM users WHERE id = CAST(CAST(?3 AS TEXT) AS BIGINT) LIMIT 1)) GROUP BY user_name_snapshot, coefficient_snapshot ORDER BY user_name_snapshot")?;
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
    _allowed_division_ids: Option<&[i64]>,
) -> Result<Vec<AuxiliaryWorkRow>> {
    let end_closed = end_of_day_bound(end);
    let sql = format!(
        "SELECT awr.user_name_snapshot,
                '' AS project_name,
                awr.auxiliary_work_name_snapshot,
                awr.coefficient_snapshot,
                SUM(awr.quantity)::BIGINT,
                SUM(awr.quantity * awr.coefficient_snapshot)::DOUBLE PRECISION
         FROM auxiliary_work_records awr
         WHERE awr.deleted_at IS NULL
           AND CAST(CAST(awr.recorded_at AS TEXT) AS TIMESTAMPTZ) >= CAST(CAST(?1 AS TEXT) AS TIMESTAMPTZ)
           AND CAST(CAST(awr.recorded_at AS TEXT) AS TIMESTAMPTZ) <= CAST(CAST(?2 AS TEXT) AS TIMESTAMPTZ)
           AND (CAST(CAST(?3 AS TEXT) AS BIGINT) IS NULL OR awr.user_id = CAST(CAST(?3 AS TEXT) AS BIGINT))
         GROUP BY awr.user_name_snapshot, awr.auxiliary_work_name_snapshot, awr.coefficient_snapshot
         ORDER BY awr.user_name_snapshot, 2, 3"
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
    let mut legacy = conn.prepare(
        "SELECT user_name, '' AS project_name,
                COALESCE(NULLIF(method_name_snapshot,''), '辅助工作'),
                COALESCE(coefficient_snapshot, 1.0), SUM(quantity)::BIGINT,
                SUM(quantity * COALESCE(coefficient_snapshot, 1.0))::DOUBLE PRECISION
         FROM work_records
         WHERE deleted_at IS NULL AND instrument_type_snapshot = '辅助工作'
           AND CAST(CAST(recorded_at AS TEXT) AS TIMESTAMPTZ) >= CAST(CAST(?1 AS TEXT) AS TIMESTAMPTZ)
           AND CAST(CAST(recorded_at AS TEXT) AS TIMESTAMPTZ) <= CAST(CAST(?2 AS TEXT) AS TIMESTAMPTZ)
           AND (CAST(CAST(?3 AS TEXT) AS BIGINT) IS NULL OR subject_user_id = CAST(CAST(?3 AS TEXT) AS BIGINT)
                OR user_name = (SELECT username FROM users WHERE id = CAST(CAST(?3 AS TEXT) AS BIGINT) LIMIT 1))
         GROUP BY user_name, COALESCE(NULLIF(method_name_snapshot,''), '辅助工作'), coefficient_snapshot
         ORDER BY user_name, 3",
    )?;
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
    let division_scope = division_scope_sql(allowed_division_ids);
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

    let division_scope = division_scope_sql(allowed_division_ids);
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

    let division_scope = division_scope_sql(allowed_division_ids);
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

    let division_scope = division_scope_sql(allowed_division_ids);
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

    let sql = "SELECT COALESCE(mt.name, '其他') AS method_type,
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
           AND (?3 IS NULL OR wr.subject_user_id = ?3)
         GROUP BY mt.name, m.amount
         ORDER BY method_type, m.amount";

    let mut stmt = conn.prepare(sql)?;
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
