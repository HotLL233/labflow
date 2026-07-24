use crate::error::Result;
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
    bool,
    f64,
    Option<String>,
);
// (实验室, 项目代号, 仪器, 方法, 单价倍率, 数量, 是否气相, 系数, 高项)

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

/// 人员汇总行（Sheet 6）
#[derive(Debug, Clone, serde::Serialize)]
pub struct PersonSummaryRow {
    pub user_name: String,
    pub method_type: String, // 液相/气相/理化
    pub coefficient: f64,
    pub multiplier: f64,
    pub quantity: i64,
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

// ========== Sheet 1: 各实验室项目方法对应表 ==========

pub fn query_sheet1_data(
    conn: &Connection,
    start: &str,
    end: &str,
    group_id: Option<i64>,
    subject_user_id: Option<i64>,
) -> Result<Vec<FlatRow>> {
    let end_closed = format!("{}T23:59:59", end);

    // v0.3.25 修复：使用 wr.group_id 对应的 project_groups.name 显示单个实验室
    let mut sql = String::from(
        "SELECT COALESCE(pg.name, '未知') as lab_name,
                p.name, COALESCE(m.full_name, m.name), m.name, m.coefficient,
                COALESCE(wr.multiplier, m.multiplier, 1.0),
                SUM(wr.quantity)::BIGINT,
                COALESCE(wr.high_item, p.high_item),
                COALESCE(NULLIF(wr.instrument_code_snapshot,''), '未绑定'),
                COALESCE(NULLIF(wr.instrument_type_snapshot,''), '其他')
         FROM work_records wr
         LEFT JOIN project_groups pg ON pg.id = wr.group_id
         JOIN projects p ON wr.project_id = p.id
         LEFT JOIN methods m ON wr.method_id = m.id
         WHERE wr.deleted_at IS NULL
           AND wr.recorded_at >= ?1
           AND wr.recorded_at <= ?2
           AND (?3 IS NULL OR wr.subject_user_id = ?3)",
    );

    let mut params: Vec<Box<dyn postgres_compat::types::ToSql>> = vec![
        Box::new(start.to_string()),
        Box::new(end_closed),
        Box::new(subject_user_id),
    ];

    if let Some(gid) = group_id {
        sql.push_str(&format!(" AND EXISTS (SELECT 1 FROM project_lab_links pll_f WHERE pll_f.project_id = p.id AND pll_f.group_id = {})", gid));
    }

    sql.push_str(" GROUP BY pg.id, pg.name, p.id, m.id, wr.multiplier, wr.high_item, wr.instrument_code_snapshot, wr.instrument_type_snapshot ORDER BY lab_name, p.name, m.name");

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
            let instrument_type: String = row.get(9).unwrap_or_else(|_| "其他".into());
            let is_gc = instrument_type == "气相";
            Ok((
                lab,
                project_code,
                instrument,
                method,
                multiplier,
                quantity,
                is_gc,
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
) -> Result<Vec<InstrumentDailyRow>> {
    let end_closed = format!("{}T23:59:59", end);

    // v0.3.25 修复：使用 wr.group_id 对应的 project_groups.name 显示单个实验室
    let sql = "SELECT date(wr.recorded_at) AS record_date,
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
           AND (?3 IS NULL OR wr.subject_user_id = ?3)
         GROUP BY LEFT(wr.recorded_at, 10), pg.id, pg.name, m.id, p.id, wr.multiplier, wr.high_item, wr.instrument_code_snapshot
         ORDER BY record_date, m.full_name, lab_name, p.name";

    let mut stmt = conn.prepare(sql)?;
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
) -> Result<Vec<ProjectSummaryRow>> {
    let end_closed = format!("{}T23:59:59", end);

    // v0.3.25 修复：使用 wr.group_id 对应的 project_groups.name 显示单个实验室
    let sql = "SELECT p.name AS project_name,
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
           AND (?3 IS NULL OR wr.subject_user_id = ?3)
         GROUP BY p.id, pg.id, pg.name, m.id, wr.multiplier, wr.high_item, wr.instrument_code_snapshot
         ORDER BY p.name, lab_name, m.name";

    let mut stmt = conn.prepare(sql)?;
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
) -> Result<Vec<LabSummaryRow>> {
    let end_closed = format!("{}T23:59:59", end);

    // v0.3.25 修复：使用 wr.group_id 对应的 project_groups.name 显示单个实验室
    let sql = "SELECT COALESCE(pg.name, '未知') AS lab_name,
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
           AND (?3 IS NULL OR wr.subject_user_id = ?3)
         GROUP BY p.id, pg.id, pg.name, m.id, wr.multiplier, wr.high_item, wr.instrument_code_snapshot
         ORDER BY lab_name, p.name, m.name";

    let mut stmt = conn.prepare(sql)?;
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
) -> Result<Vec<PersonRecordRow>> {
    let end_closed = format!("{}T23:59:59", end);

    // v0.3.25 修复：使用 wr.group_id 对应的 project_groups.name 显示单个实验室
    let sql = "SELECT wr.recorded_at,
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
           AND (?3 IS NULL OR wr.subject_user_id = ?3)
         ORDER BY wr.recorded_at DESC";

    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(
        postgres_compat::params![start, end_closed, subject_user_id],
        |row| {
            Ok(PersonRecordRow {
                recorded_at: row.get(0)?,
                lab: row.get(1)?,
                project: row.get(2)?,
                method: row.get(3).unwrap_or_default(),
                method_type: row.get(4)?,
                multiplier: 1.0,
                quantity: row.get(5)?,
                user_name: row.get(6)?,
                high_item: row.get(7).unwrap_or(None),
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
) -> Result<Vec<PersonSummaryRow>> {
    let end_closed = format!("{}T23:59:59", end);

    let sql = "SELECT wr.user_name,
                COALESCE(mt.name, '其他') AS method_type,
                wr.coefficient_snapshot,
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
           AND (?3 IS NULL OR wr.subject_user_id = ?3)
         GROUP BY wr.user_name, mt.name, wr.coefficient_snapshot
         ORDER BY wr.user_name, mt.name, wr.coefficient_snapshot";

    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(
        postgres_compat::params![start, end_closed, subject_user_id],
        |row| {
            Ok(PersonSummaryRow {
                user_name: row.get(0)?,
                method_type: row.get(1)?,
                coefficient: row.get::<_, f64>(2).unwrap_or(1.0),
                multiplier: 1.0,
                quantity: row.get(3)?,
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
) -> Result<Vec<LabTotalRow>> {
    let end_closed = format!("{}T23:59:59", end);

    // v0.3.25 修复：使用 wr.group_id 对应的 project_groups.name 显示单个实验室
    let sql = "SELECT COALESCE(pg.name, '未知') AS lab_name,
                p.name AS project_name,
                COALESCE(mt.name, '其他') AS method_type,
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
           AND (?3 IS NULL OR wr.subject_user_id = ?3)
         GROUP BY pg.id, pg.name, p.id, mt.name, m.amount, m.multiplier, wr.multiplier
         ORDER BY lab_name, p.name, mt.name";

    let mut stmt = conn.prepare(sql)?;
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
) -> Result<Vec<ProjectTotalRow>> {
    let end_closed = format!("{}T23:59:59", end);

    let sql = "SELECT p.name AS project_name,
                COALESCE(mt.name, '其他') AS method_type,
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
           AND (?3 IS NULL OR wr.subject_user_id = ?3)
         GROUP BY p.id, mt.name, m.amount, m.multiplier, wr.multiplier
         ORDER BY p.name, mt.name";

    let mut stmt = conn.prepare(sql)?;
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
) -> Result<Vec<InstrumentSummaryRow>> {
    let end_closed = format!("{}T23:59:59", end);

    let sql =
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
           AND (?3 IS NULL OR wr.subject_user_id = ?3)
         GROUP BY COALESCE(CAST(wr.instrument_id_snapshot AS TEXT), 'legacy:' || COALESCE(NULLIF(wr.instrument_code_snapshot,''), '未绑定')),
                  COALESCE(NULLIF(wr.instrument_code_snapshot,''), '未绑定'),
                  COALESCE(NULLIF(wr.instrument_type_snapshot,''), '其他')
         ORDER BY total_qty DESC";

    let mut stmt = conn.prepare(sql)?;
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
) -> Result<Vec<PhysChemRow>> {
    let end_closed = format!("{}T23:59:59", end);

    let sql =
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
           AND (?3 IS NULL OR wr.subject_user_id = ?3)
           AND mt.name = '理化'
         GROUP BY wr.method_id, wr.instrument_id_snapshot, m.name, wr.instrument_code_snapshot
         ORDER BY total_qty DESC";

    let mut stmt = conn.prepare(sql)?;
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
    let end_closed = format!("{}T23:59:59", end);

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
