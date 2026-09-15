use crate::error::Result;
/// 导出数据查询层 - v0.3.21 版本
/// 支持 10 个 Sheet 的数据查询
/// v0.3.21 关键修复：汇总表使用 group_concat 子查询获取实验室名（拼接显示），GROUP BY 不含实验室维度
///   这样每条 (项目, 方法) 只有一行，数量不会翻倍（修复 v0.3.19/0.3.20 的 JOIN 展开问题）
use postgres_compat::Connection;

// ========== 通用数据结构 ==========

// 复用 export_data 的 Sheet 行结构类型，确保与 export_write::write_sheetN 期望的类型一致
// （rd 模块与分析检测共用同一套导出结构，仅底层查询表不同）
pub use super::export_data::{
    FlatRow, InstrumentDailyRow, InstrumentSummaryRow, LabSummaryRow, LabTotalRow, PersonRecordRow,
    PersonSummaryRow, PhysChemRow, ProjectSummaryRow, ProjectTotalRow, TypeSummaryRow,
};

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

// Department scopes are calculated from role configuration. The IDs originate
// from the database, but are still rendered as integers only to keep all export
// sheets on the same parameter layout across PostgreSQL and compatibility tests.
fn division_scope_clause(allowed_division_ids: Option<&[i64]>) -> String {
    let execution_scope = match allowed_division_ids {
        None => String::new(),
        Some([]) => " AND 1=0".to_string(),
        Some(ids) => format!(
            " AND COALESCE(wr.execution_division_id,wr.division_id,(SELECT division_id FROM project_groups WHERE id=wr.group_id)) IN ({})",
            ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
        ),
    };
    format!(
        "{execution_scope}
         AND (COALESCE(current_setting('workload.export_include_pending_ownership',true),'false')='true'
              OR COALESCE(wr.ownership_status,'confirmed')<>'pending_confirmation')
         AND (NULLIF(current_setting('workload.export_division_id',true),'') IS NULL
              OR CAST(current_setting('workload.export_division_id',true) AS BIGINT) = 0
              OR CASE COALESCE(current_setting('workload.export_ownership_basis',true),'execution')
                   WHEN 'project' THEN wr.project_division_id
                   WHEN 'submitted' THEN wr.submitted_division_id
                   ELSE wr.execution_division_id
                 END = CAST(current_setting('workload.export_division_id',true) AS BIGINT))"
    )
}

fn apply_division_scope(sql: &str, allowed_division_ids: Option<&[i64]>) -> String {
    sql.replacen(
        "/* rd_division_scope */",
        &division_scope_clause(allowed_division_ids),
        1,
    )
}

// ========== Sheet 1: 各实验室项目方法对应表 ==========

pub fn query_sheet1_data(
    conn: &Connection,
    start: &str,
    end: &str,
    group_id: Option<i64>,
    subject_user_id: Option<i64>,
    scope_group_id: Option<i64>,
    allowed_division_ids: Option<&[i64]>,
) -> Result<Vec<FlatRow>> {
    let end_closed = super::export_data::end_of_day_bound(end);

    // v0.3.25 修复：使用 wr.group_id 对应的 project_groups.name 显示单个实验室
    let mut sql = String::from(
        "SELECT COALESCE(pg.name, '未知') as lab_name,
                p.name, COALESCE(m.full_name, m.name), m.name, m.coefficient,
                SUM(wr.quantity)::BIGINT,
                COALESCE(NULLIF(wr.instrument_code_snapshot,''), '未绑定'),
                COALESCE((SELECT mt.name
                          FROM method_type_links mtl
                          JOIN method_types mt ON mt.id=mtl.method_type_id
                          WHERE mtl.method_id=wr.method_id AND mt.deleted_at IS NULL
                          ORDER BY mt.sort_order, mt.id
                          LIMIT 1), '未分类')
         FROM rd_work_records wr
         LEFT JOIN project_groups pg ON pg.id = wr.group_id
         JOIN projects p ON wr.project_id = p.id
         LEFT JOIN methods m ON wr.method_id = m.id
          WHERE wr.deleted_at IS NULL AND wr.status NOT IN ('已退回','已退回已确认','退回待修改','已作废')
           AND wr.recorded_at >= ?1
           AND wr.recorded_at <= ?2
           AND (?3 IS NULL OR wr.subject_user_id = ?3)
           AND (?4 IS NULL OR wr.group_id = ?4)",
    );

    let params: Vec<Box<dyn postgres_compat::types::ToSql>> = vec![
        Box::new(start.to_string()),
        Box::new(end_closed),
        Box::new(subject_user_id),
        Box::new(scope_group_id),
    ];

    if let Some(gid) = group_id {
        sql.push_str(&format!(" AND wr.group_id = {}", gid));
    }
    sql.push_str(&division_scope_clause(allowed_division_ids));

    sql.push_str(" GROUP BY pg.id, pg.name, p.id, m.id, wr.method_id, wr.instrument_code_snapshot ORDER BY lab_name, p.name, m.name");

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        postgres_compat::params_from_iter(params.iter().map(|p| p.as_ref())),
        |row| {
            let lab: String = row.get(0)?;
            let project: String = row.get(1)?;
            let _full_name: String = row.get(2).unwrap_or_default();
            let method: String = row.get(3).unwrap_or_default();
            let coefficient: f64 = row.get(4).unwrap_or(1.0);
            let quantity: i64 = row.get(5)?;

            let project_code = extract_code(&project).to_string();
            let instrument: String = row.get(6).unwrap_or_else(|_| "未绑定".into());
            let method_type: String = row.get(7).unwrap_or_else(|_| "未分类".into());

            Ok((
                lab,
                project_code,
                instrument,
                method,
                1.0,
                quantity,
                method_type,
                coefficient,
                None,
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
    scope_group_id: Option<i64>,
    allowed_division_ids: Option<&[i64]>,
) -> Result<Vec<InstrumentDailyRow>> {
    let end_closed = super::export_data::end_of_day_bound(end);

    // v0.3.25 修复：使用 wr.group_id 对应的 project_groups.name 显示单个实验室
    let sql = apply_division_scope("SELECT date(wr.recorded_at) AS record_date,
                COALESCE(m.full_name, m.name),
                COALESCE(pg.name, '未知') AS lab_name,
                p.name AS project_name,
                m.name AS method_name,
                SUM(wr.quantity)::BIGINT AS total_qty,
                COALESCE(NULLIF(wr.instrument_code_snapshot,''), '未绑定')
         FROM rd_work_records wr
         LEFT JOIN project_groups pg ON pg.id = wr.group_id
         JOIN projects p ON wr.project_id = p.id
         LEFT JOIN methods m ON wr.method_id = m.id
          WHERE wr.deleted_at IS NULL AND wr.status NOT IN ('已退回','已退回已确认','退回待修改','已作废')
           AND wr.recorded_at >= ?1
           AND wr.recorded_at <= ?2
           AND (?3 IS NULL OR wr.subject_user_id = ?3)
           AND (?4 IS NULL OR wr.group_id = ?4)
           /* rd_division_scope */
         GROUP BY LEFT(wr.recorded_at, 10), pg.id, pg.name, m.id, p.id, wr.instrument_code_snapshot
         ORDER BY record_date, m.full_name, lab_name, p.name", allowed_division_ids);

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        postgres_compat::params![start, end_closed, subject_user_id, scope_group_id],
        |row| {
            let date: String = row.get(0)?;
            let instrument: String = row.get(6).unwrap_or_else(|_| "未绑定".into());

            Ok(InstrumentDailyRow {
                date,
                instrument,
                lab: row.get(2)?,
                project: row.get(3)?,
                method: row.get(4).unwrap_or_default(),
                multiplier: 1.0,
                quantity: row.get(5)?,
                high_item: None,
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
    scope_group_id: Option<i64>,
    allowed_division_ids: Option<&[i64]>,
) -> Result<Vec<ProjectSummaryRow>> {
    let end_closed = super::export_data::end_of_day_bound(end);

    // v0.3.25 修复：使用 wr.group_id 对应的 project_groups.name 显示单个实验室
    let sql = apply_division_scope("SELECT p.name AS project_name,
                COALESCE(pg.name, '未知') AS lab_name,
                COALESCE(m.full_name, m.name),
                m.name AS method_name,
                m.amount,
                COALESCE(m.multiplier, 1.0),
                SUM(wr.quantity)::BIGINT AS total_qty,
                COALESCE(NULLIF(wr.instrument_code_snapshot,''), '未绑定')
         FROM rd_work_records wr
         LEFT JOIN project_groups pg ON pg.id = wr.group_id
         JOIN projects p ON wr.project_id = p.id
         LEFT JOIN methods m ON wr.method_id = m.id
          WHERE wr.deleted_at IS NULL AND wr.status NOT IN ('已退回','已退回已确认','退回待修改','已作废')
           AND wr.recorded_at >= ?1
           AND wr.recorded_at <= ?2
           AND (?3 IS NULL OR wr.subject_user_id = ?3)
           AND (?4 IS NULL OR wr.group_id = ?4)
           /* rd_division_scope */
         GROUP BY p.id, pg.id, pg.name, m.id, wr.instrument_code_snapshot
         ORDER BY p.name, lab_name, m.name", allowed_division_ids);

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        postgres_compat::params![start, end_closed, subject_user_id, scope_group_id],
        |row| {
            let instrument: String = row.get(7).unwrap_or_else(|_| "未绑定".into());

            Ok(ProjectSummaryRow {
                project: row.get(0)?,
                lab: row.get(1)?,
                instrument,
                method: row.get(3).unwrap_or_default(),
                multiplier: row.get::<_, f64>(5).unwrap_or(1.0),
                high_item: None,
                quantity: row.get(6)?,
                unit_price: row.get::<_, f64>(4).unwrap_or(0.0),
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
    scope_group_id: Option<i64>,
    allowed_division_ids: Option<&[i64]>,
) -> Result<Vec<LabSummaryRow>> {
    let end_closed = super::export_data::end_of_day_bound(end);

    // v0.3.25 修复：使用 wr.group_id 对应的 project_groups.name 显示单个实验室
    let sql = apply_division_scope("SELECT COALESCE(pg.name, '未知') AS lab_name,
                p.name AS project_name,
                COALESCE(m.full_name, m.name),
                m.name AS method_name,
                m.amount,
                COALESCE(m.multiplier, 1.0),
                SUM(wr.quantity)::BIGINT AS total_qty,
                COALESCE(NULLIF(wr.instrument_code_snapshot,''), '未绑定')
         FROM rd_work_records wr
         LEFT JOIN project_groups pg ON pg.id = wr.group_id
         JOIN projects p ON wr.project_id = p.id
         LEFT JOIN methods m ON wr.method_id = m.id
          WHERE wr.deleted_at IS NULL AND wr.status NOT IN ('已退回','已退回已确认','退回待修改','已作废')
           AND wr.recorded_at >= ?1
           AND wr.recorded_at <= ?2
           AND (?3 IS NULL OR wr.subject_user_id = ?3)
           AND (?4 IS NULL OR wr.group_id = ?4)
           /* rd_division_scope */
         GROUP BY p.id, pg.id, pg.name, m.id, wr.instrument_code_snapshot
         ORDER BY lab_name, p.name, m.name", allowed_division_ids);

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        postgres_compat::params![start, end_closed, subject_user_id, scope_group_id],
        |row| {
            let instrument: String = row.get(7).unwrap_or_else(|_| "未绑定".into());

            Ok(LabSummaryRow {
                lab: row.get(0)?,
                project: row.get(1)?,
                instrument,
                method: row.get(3).unwrap_or_default(),
                high_item: None,
                multiplier: row.get::<_, f64>(5).unwrap_or(1.0),
                quantity: row.get(6)?,
                unit_price: row.get::<_, f64>(4).unwrap_or(0.0),
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
    scope_group_id: Option<i64>,
    allowed_division_ids: Option<&[i64]>,
) -> Result<Vec<PersonRecordRow>> {
    let end_closed = super::export_data::end_of_day_bound(end);

    // v2.2.8 修复：统计所有送样记录，显示送样人（不是检测人员）
    let sql = apply_division_scope("SELECT wr.recorded_at,
                COALESCE(pg.name, '未知') AS lab_name,
                p.name AS project_name,
                m.name AS method_name,
                COALESCE((SELECT string_agg(DISTINCT mt2.name, ',') FROM method_type_links mtl2 JOIN method_types mt2 ON mt2.id=mtl2.method_type_id WHERE mtl2.method_id=wr.method_id), '其他') AS method_types,
                wr.quantity,
                wr.user_name
         FROM rd_work_records wr
         LEFT JOIN project_groups pg ON pg.id = wr.group_id
         JOIN projects p ON wr.project_id = p.id
         LEFT JOIN methods m ON wr.method_id = m.id
          WHERE wr.deleted_at IS NULL AND wr.status NOT IN ('已退回','已退回已确认','退回待修改','已作废')
           AND wr.recorded_at >= ?1
           AND wr.recorded_at <= ?2
           AND (?3 IS NULL OR wr.subject_user_id = ?3)
           AND (?4 IS NULL OR wr.group_id = ?4)
           /* rd_division_scope */
         ORDER BY wr.recorded_at DESC", allowed_division_ids);

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        postgres_compat::params![start, end_closed, subject_user_id, scope_group_id],
        |row| {
            Ok(PersonRecordRow {
                recorded_at: row.get(0)?,
                lab: row.get(1)?,
                project: row.get(2)?,
                method: row.get(3).unwrap_or_default(),
                high_item: None,
                method_type: row.get(4)?,
                multiplier: 1.0,
                quantity: row.get(5)?,
                user_name: row.get(6)?,
            })
        },
    )?;

    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

// ========== Sheet 6: 送样人汇总表（统计送样量，不含工作量系数） ==========

pub fn query_sheet6_data(
    conn: &Connection,
    start: &str,
    end: &str,
    subject_user_id: Option<i64>,
    scope_group_id: Option<i64>,
    allowed_division_ids: Option<&[i64]>,
) -> Result<Vec<PersonSummaryRow>> {
    let end_closed = super::export_data::end_of_day_bound(end);

    // v2.2.8 修复：统计送样人的送样量
    let sql = apply_division_scope("SELECT wr.user_name,
                COALESCE(mt.name, '未分类') AS method_type,
                CAST(1.0 AS DOUBLE PRECISION) AS coefficient_placeholder,
                SUM(wr.quantity)::BIGINT AS total_qty
         FROM rd_work_records wr
         LEFT JOIN project_groups pg ON pg.id = wr.group_id
         JOIN projects p ON wr.project_id = p.id
         LEFT JOIN methods m ON wr.method_id = m.id
         LEFT JOIN method_type_links mtl ON m.id = mtl.method_id
         LEFT JOIN method_types mt ON mtl.method_type_id = mt.id
          WHERE wr.deleted_at IS NULL AND wr.status NOT IN ('已退回','已退回已确认','退回待修改','已作废')
           AND wr.recorded_at >= ?1
           AND wr.recorded_at <= ?2
           AND (?3 IS NULL OR wr.subject_user_id = ?3)
           AND (?4 IS NULL OR wr.group_id = ?4)
           /* rd_division_scope */
         GROUP BY wr.user_name, mt.name
         ORDER BY wr.user_name, mt.name", allowed_division_ids);

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        postgres_compat::params![start, end_closed, subject_user_id, scope_group_id],
        |row| {
            Ok(PersonSummaryRow {
                user_name: row.get(0)?,
                project: String::new(),
                instrument: String::new(),
                method_type: row.get(1)?,
                method: String::new(),
                coefficient: row.get::<_, f64>(2).unwrap_or(1.0),
                multiplier: 1.0,
                quantity: row.get(3)?,
                workload: row.get::<_, i64>(3).unwrap_or(0) as f64
                    * row.get::<_, f64>(2).unwrap_or(1.0),
            })
        },
    )?;

    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

// ========= Sheet 7: 实验室总表 ==========

pub fn query_sheet7_data(
    conn: &Connection,
    start: &str,
    end: &str,
    subject_user_id: Option<i64>,
    scope_group_id: Option<i64>,
    allowed_division_ids: Option<&[i64]>,
) -> Result<Vec<LabTotalRow>> {
    let end_closed = super::export_data::end_of_day_bound(end);

    // v0.3.25 修复：使用 wr.group_id 对应的 project_groups.name 显示单个实验室
    let sql = apply_division_scope("SELECT COALESCE(pg.name, '未知') AS lab_name,
                p.name AS project_name,
                COALESCE(mt.name, '未分类') AS method_type,
                m.amount,
                COALESCE(m.multiplier, 1.0),
                SUM(wr.quantity)::BIGINT AS total_qty
         FROM rd_work_records wr
         LEFT JOIN project_groups pg ON pg.id = wr.group_id
         JOIN projects p ON wr.project_id = p.id
         LEFT JOIN methods m ON wr.method_id = m.id
         LEFT JOIN method_type_links mtl ON m.id = mtl.method_id
         LEFT JOIN method_types mt ON mtl.method_type_id = mt.id
          WHERE wr.deleted_at IS NULL AND wr.status NOT IN ('已退回','已退回已确认','退回待修改','已作废')
           AND wr.recorded_at >= ?1
           AND wr.recorded_at <= ?2
           AND (?3 IS NULL OR wr.subject_user_id = ?3)
           AND (?4 IS NULL OR wr.group_id = ?4)
           /* rd_division_scope */
         GROUP BY pg.id, pg.name, p.id, mt.name, m.amount, m.multiplier
         ORDER BY lab_name, p.name, mt.name", allowed_division_ids);

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        postgres_compat::params![start, end_closed, subject_user_id, scope_group_id],
        |row| {
            Ok(LabTotalRow {
                lab: row.get(0)?,
                project: row.get(1)?,
                method_type: row.get(2)?,
                multiplier: row.get::<_, f64>(4).unwrap_or(1.0),
                unit_price: row.get::<_, f64>(3).unwrap_or(0.0),
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
    scope_group_id: Option<i64>,
    allowed_division_ids: Option<&[i64]>,
) -> Result<Vec<ProjectTotalRow>> {
    let end_closed = super::export_data::end_of_day_bound(end);

    let sql = apply_division_scope("SELECT p.name AS project_name,
                COALESCE(mt.name, '未分类') AS method_type,
                m.amount,
                COALESCE(m.multiplier, 1.0),
                SUM(wr.quantity)::BIGINT AS total_qty
         FROM rd_work_records wr
         LEFT JOIN project_groups pg ON pg.id = wr.group_id
         JOIN projects p ON wr.project_id = p.id
         LEFT JOIN methods m ON wr.method_id = m.id
         LEFT JOIN method_type_links mtl ON m.id = mtl.method_id
         LEFT JOIN method_types mt ON mtl.method_type_id = mt.id
          WHERE wr.deleted_at IS NULL AND wr.status NOT IN ('已退回','已退回已确认','退回待修改','已作废')
           AND wr.recorded_at >= ?1
           AND wr.recorded_at <= ?2
           AND (?3 IS NULL OR wr.subject_user_id = ?3)
           AND (?4 IS NULL OR wr.group_id = ?4)
           /* rd_division_scope */
         GROUP BY p.id, mt.name, m.amount, m.multiplier
         ORDER BY p.name, mt.name", allowed_division_ids);

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        postgres_compat::params![start, end_closed, subject_user_id, scope_group_id],
        |row| {
            Ok(ProjectTotalRow {
                project: row.get(0)?,
                method_type: row.get(1)?,
                multiplier: row.get::<_, f64>(3).unwrap_or(1.0),
                unit_price: row.get::<_, f64>(2).unwrap_or(0.0),
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
    scope_group_id: Option<i64>,
    allowed_division_ids: Option<&[i64]>,
) -> Result<Vec<InstrumentSummaryRow>> {
    let end_closed = super::export_data::end_of_day_bound(end);

    let sql = apply_division_scope(
        "SELECT COALESCE(NULLIF(wr.instrument_code_snapshot,''), '未绑定'),
                SUM(wr.quantity)::BIGINT AS total_qty,
                COALESCE(NULLIF(wr.instrument_type_snapshot,''), '其他')
         FROM rd_work_records wr
         LEFT JOIN project_groups pg ON pg.id = wr.group_id
         JOIN projects p ON wr.project_id = p.id
         LEFT JOIN methods m ON wr.method_id = m.id
          WHERE wr.deleted_at IS NULL AND wr.status NOT IN ('已退回','已退回已确认','退回待修改','已作废')
           AND wr.recorded_at >= ?1
           AND wr.recorded_at <= ?2
           AND (?3 IS NULL OR wr.subject_user_id = ?3)
           AND (?4 IS NULL OR wr.group_id = ?4)
           /* rd_division_scope */
         GROUP BY COALESCE(CAST(wr.instrument_id_snapshot AS TEXT), 'legacy:' || COALESCE(NULLIF(wr.instrument_code_snapshot,''), '未绑定')),
                  COALESCE(NULLIF(wr.instrument_code_snapshot,''), '未绑定'),
                  COALESCE(NULLIF(wr.instrument_type_snapshot,''), '其他')
         ORDER BY total_qty DESC",
        allowed_division_ids,
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        postgres_compat::params![start, end_closed, subject_user_id, scope_group_id],
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
    scope_group_id: Option<i64>,
    allowed_division_ids: Option<&[i64]>,
) -> Result<Vec<PhysChemRow>> {
    let end_closed = super::export_data::end_of_day_bound(end);

    let sql = apply_division_scope(
        "SELECT m.name || CASE WHEN COALESCE(wr.instrument_code_snapshot,'')='' THEN '' ELSE ' [' || wr.instrument_code_snapshot || ']' END AS method_name,
                SUM(wr.quantity)::BIGINT AS total_qty
         FROM rd_work_records wr
         LEFT JOIN project_groups pg ON pg.id = wr.group_id
         JOIN projects p ON wr.project_id = p.id
         LEFT JOIN methods m ON wr.method_id = m.id
         LEFT JOIN method_type_links mtl ON m.id = mtl.method_id
         LEFT JOIN method_types mt ON mtl.method_type_id = mt.id
          WHERE wr.deleted_at IS NULL AND wr.status NOT IN ('已退回','已退回已确认','退回待修改','已作废')
           AND wr.recorded_at >= ?1
           AND wr.recorded_at <= ?2
           AND (?3 IS NULL OR wr.subject_user_id = ?3)
           AND (?4 IS NULL OR wr.group_id = ?4)
           /* rd_division_scope */
           AND mt.name = '理化'
         GROUP BY wr.method_id, wr.instrument_id_snapshot, m.name, wr.instrument_code_snapshot
         ORDER BY total_qty DESC",
        allowed_division_ids,
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        postgres_compat::params![start, end_closed, subject_user_id, scope_group_id],
        |row| {
            Ok(PhysChemRow {
                method: row.get(0).unwrap_or_default(),
                multiplier: 1.0,
                quantity: row.get(1)?,
            })
        },
    )?;

    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

// ========== Sheet 11: 类型汇总表 ==========

pub fn query_sheet11_data(
    conn: &Connection,
    start: &str,
    end: &str,
    subject_user_id: Option<i64>,
    scope_group_id: Option<i64>,
    allowed_division_ids: Option<&[i64]>,
) -> Result<Vec<TypeSummaryRow>> {
    let end_closed = super::export_data::end_of_day_bound(end);

    let sql = apply_division_scope("SELECT COALESCE(mt.name, '其他') AS method_type,
                SUM(wr.quantity)::BIGINT AS total_qty,
                m.amount
         FROM rd_work_records wr
         LEFT JOIN project_groups pg ON pg.id = wr.group_id
         JOIN projects p ON wr.project_id = p.id
         LEFT JOIN methods m ON wr.method_id = m.id
         LEFT JOIN method_type_links mtl ON m.id = mtl.method_id
         LEFT JOIN method_types mt ON mtl.method_type_id = mt.id
          WHERE wr.deleted_at IS NULL AND wr.status NOT IN ('已退回','已退回已确认','退回待修改','已作废')
           AND wr.recorded_at >= ?1
           AND wr.recorded_at <= ?2
           AND (?3 IS NULL OR wr.subject_user_id = ?3)
           AND (?4 IS NULL OR wr.group_id = ?4)
           /* rd_division_scope */
         GROUP BY mt.name, m.amount
         ORDER BY method_type, m.amount", allowed_division_ids);

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        postgres_compat::params![start, end_closed, subject_user_id, scope_group_id],
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
mod tests {
    use super::*;

    #[test]
    fn rd_amount_sheets_use_method_multiplier_and_type_sheet_does_not() {
        let conn = Connection::open_test_database().unwrap();
        conn.execute_batch(
            "CREATE TABLE project_groups (id INTEGER PRIMARY KEY, name TEXT);
             CREATE TABLE projects (id INTEGER PRIMARY KEY, name TEXT);
             CREATE TABLE methods (
                 id INTEGER PRIMARY KEY, name TEXT, full_name TEXT,
                 amount REAL, multiplier REAL
             );
             CREATE TABLE method_types (id INTEGER PRIMARY KEY, name TEXT);
             CREATE TABLE method_type_links (method_id INTEGER, method_type_id INTEGER);
              CREATE TABLE rd_work_records (
                  id INTEGER PRIMARY KEY, project_id INTEGER, method_id INTEGER,
                  quantity INTEGER, recorded_at TEXT, group_id INTEGER, deleted_at TEXT,
                  status TEXT NOT NULL,
                  instrument_code_snapshot TEXT, instrument_type_snapshot TEXT,
                  subject_user_id INTEGER GENERATED ALWAYS AS (10) VIRTUAL
              );
             INSERT INTO project_groups VALUES (1, 'Lab01');
             INSERT INTO projects VALUES (1, 'Project01');
             INSERT INTO methods VALUES (1, 'Method01', 'Method01 Full', 10.0, 2.5);
             INSERT INTO method_types VALUES (1, 'Type01');
             INSERT INTO method_type_links VALUES (1, 1);
             INSERT INTO rd_work_records VALUES (1, 1, 1, 3, '2026-07-17T10:00:00', 1, NULL, '待取样', 'LC-01', '液相');"
        ).unwrap();
        conn.execute_batch(
            "ALTER TABLE rd_work_records
             ADD COLUMN ownership_status TEXT NOT NULL DEFAULT 'confirmed';
             ALTER TABLE rd_work_records ADD COLUMN project_division_id BIGINT;
             ALTER TABLE rd_work_records ADD COLUMN submitted_division_id BIGINT;
             ALTER TABLE rd_work_records ADD COLUMN execution_division_id BIGINT",
        )
        .unwrap();

        assert_eq!(
            query_sheet3_data(&conn, "2026-07-17", "2026-07-17", None, None, None).unwrap()[0]
                .multiplier,
            2.5
        );
        assert_eq!(
            query_sheet4_data(&conn, "2026-07-17", "2026-07-17", None, None, None).unwrap()[0]
                .multiplier,
            2.5
        );
        assert_eq!(
            query_sheet7_data(&conn, "2026-07-17", "2026-07-17", None, None, None).unwrap()[0]
                .multiplier,
            2.5
        );
        assert_eq!(
            query_sheet8_data(&conn, "2026-07-17", "2026-07-17", None, None, None).unwrap()[0]
                .multiplier,
            2.5
        );
        assert_eq!(
            query_sheet11_data(&conn, "2026-07-17", "2026-07-17", None, None, None).unwrap()[0]
                .multiplier,
            1.0
        );
        assert_eq!(
            query_sheet3_data(&conn, "2026-07-17", "2026-07-17", Some(10), None, None)
                .unwrap()
                .len(),
            1
        );
        assert!(
            query_sheet3_data(&conn, "2026-07-17", "2026-07-17", Some(11), None, None)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            query_sheet3_data(&conn, "2026-07-17", "2026-07-17", None, Some(1), None)
                .unwrap()
                .len(),
            1
        );
        assert!(
            query_sheet3_data(&conn, "2026-07-17", "2026-07-17", None, Some(2), None)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn division_scope_clause_only_contains_numeric_ids() {
        let global_scope = division_scope_clause(None);
        assert!(!global_scope.contains(" IN ("));
        assert!(global_scope.contains("ownership_status"));
        assert!(division_scope_clause(Some(&[])).starts_with(" AND 1=0"));
        let scoped = division_scope_clause(Some(&[4, 12]));
        assert!(scoped.contains(" IN (4,12)"));
        assert!(!scoped.contains("4,12x"));
    }
}
