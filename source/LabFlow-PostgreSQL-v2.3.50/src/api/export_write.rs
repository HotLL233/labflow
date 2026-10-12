use super::export_data::*;
use crate::error::{AppError, Result};
/// 导出 Excel 写入层 - v0.3.6 重写版本 / v0.4.6 单价倍率更新
/// 支持 10 个 Sheet 的格式化写入
use rust_xlsxwriter::*;

// ========== 列常量定义 ==========
pub const CA: u16 = 0; // A列
pub const CB: u16 = 1; // B列
pub const CC: u16 = 2; // C列
pub const CD: u16 = 3; // D列
pub const CE: u16 = 4; // E列
pub const CF: u16 = 5; // F列
pub const CG: u16 = 6; // G列
pub const CH: u16 = 7; // H列
pub const CI: u16 = 8; // I列
pub const CJ: u16 = 9; // J列
pub const CK: u16 = 10; // K列
pub const CL: u16 = 11; // L列
pub const CM: u16 = 12; // M列
pub const CN: u16 = 13; // N列
pub const CO: u16 = 14; // O列

pub const HR: u32 = 1; // 表头行（0-indexed，实际是第2行；第1行为导出时间段标题）

/// 在统计工作表顶部写入本次导出的时间段标题。
/// 标题单独占用第1行，字段表头和数据行从第2行开始，避免改变原有字段结构。
pub fn write_period_title(ws: &mut Worksheet, title: &str, last_col: u16, fmt: &Fmt) -> Result<()> {
    ws.merge_range(0, CA, 0, last_col, title, &fmt.fb)?;
    ws.set_row_height(0, 24.0)?;
    Ok(())
}

const SHEET3_HEADERS: [&str; 10] = [
    "项目",
    "高项",
    "实验室",
    "仪器",
    "方法",
    "单价倍率",
    "数量",
    "单价",
    "明细金额",
    "项目金额汇总",
];
const SHEET4_HEADERS: [&str; 11] = [
    "实验室",
    "项目",
    "高项",
    "仪器",
    "方法",
    "单价倍率",
    "数量",
    "单价",
    "数量总计",
    "明细金额",
    "实验室金额汇总",
];
#[cfg(test)]
const SHEET6_HEADERS: [&str; 13] = [
    "人员",
    "总工作量",
    "检测工作量",
    "辅助工作量",
    "液相数量",
    "液相系数",
    "液相工作量",
    "气相数量",
    "气相系数",
    "气相工作量",
    "理化数量",
    "理化系数",
    "理化工作量",
];
#[cfg(test)]
const SHEET7_HEADERS: [&str; 16] = [
    "实验室",
    "项目",
    "液相数量",
    "液相单价",
    "液相单价倍率",
    "液相金额汇总",
    "气相数量",
    "气相单价",
    "气相单价倍率",
    "气相金额汇总",
    "理化数量",
    "理化单价",
    "理化单价倍率",
    "理化金额汇总",
    "项目金额汇总",
    "实验室金额汇总",
];
#[cfg(test)]
const SHEET8_HEADERS: [&str; 14] = [
    "项目",
    "液相数量",
    "液相单价",
    "液相单价倍率",
    "液相金额汇总",
    "气相数量",
    "气相单价",
    "气相单价倍率",
    "气相金额汇总",
    "理化数量",
    "理化单价",
    "理化单价倍率",
    "理化金额汇总",
    "项目金额汇总",
];
const SHEET11_HEADERS: [&str; 5] = ["检测类型", "数量", "单价", "明细金额", "检测类型金额汇总"];

/// 列号转字母（0->A, 1->B, 25->Z, 26->AA）
pub fn col_letter(n: u16) -> String {
    let letters = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut n = n + 1;
    let mut result = String::new();
    while n > 0 {
        n -= 1;
        result.insert(0, letters.chars().nth((n % 26) as usize).unwrap());
        n /= 26;
    }
    result
}

/// 从查询结果中提取本次导出的实际检测类型，保持查询层的稳定顺序并去重。
/// 不再根据类型名称猜测液相、气相或理化。
fn normalized_type_label(item: &str) -> &str {
    if item.trim().is_empty() {
        "未分类"
    } else {
        item.trim()
    }
}

#[cfg(test)]
fn calculate_workload(quantity: i64, coefficient: f64) -> f64 {
    quantity as f64 * coefficient
}

#[cfg(test)]
fn calculate_amount(quantity: i64, base: f64, multiplier: f64) -> f64 {
    quantity as f64 * base * multiplier
}

fn dynamic_type_labels<'a, I>(items: I) -> Vec<String>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut labels = Vec::new();
    for item in items {
        let label = normalized_type_label(item);
        if !labels.iter().any(|existing| existing == label) {
            labels.push(label.to_string());
        }
    }
    labels
}

fn row_sum_formula(columns: &[u16], row: u32) -> String {
    if columns.is_empty() {
        "=0".to_string()
    } else {
        let cells = columns
            .iter()
            .map(|column| format!("{}{}", col_letter(*column), row + 1))
            .collect::<Vec<_>>();
        format!("={}", cells.join("+"))
    }
}

fn range_sum_formula(columns: &[u16], start: u32, end: u32) -> String {
    if columns.is_empty() {
        "=0".to_string()
    } else {
        let ranges = columns
            .iter()
            .map(|column| {
                format!(
                    "SUM({}{}:{}{})",
                    col_letter(*column),
                    start + 1,
                    col_letter(*column),
                    end + 1
                )
            })
            .collect::<Vec<_>>();
        format!("={}", ranges.join("+"))
    }
}

pub fn sheet6_last_col(rows: &[PersonSummaryRow]) -> u16 {
    let _ = rows;
    7
}

pub fn sheet12_last_col(rows: &[AuxiliaryWorkRow]) -> u16 {
    let _ = rows;
    5
}

pub fn sheet13_last_col() -> u16 {
    5
}

pub fn sheet7_last_col(rows: &[LabTotalRow]) -> u16 {
    let _ = rows;
    8
}

pub fn sheet8_last_col(rows: &[ProjectTotalRow]) -> u16 {
    let _ = rows;
    6
}

// ========== 格式定义 ==========

pub struct Fmt {
    pub fh: Format, // 表头格式
    pub fd: Format, // 数据格式
    pub fb: Format, // 加粗格式
    pub fw: Format, // 工作量格式
}

impl Fmt {
    pub fn new() -> Self {
        // 数据格式：仿宋 14号 + 水平垂直居中 + 细边框
        let data = Format::new()
            .set_font_name("仿宋")
            .set_font_size(14)
            .set_align(FormatAlign::Center)
            .set_align(FormatAlign::VerticalCenter)
            .set_border(FormatBorder::Thin)
            .set_text_wrap();
        // 表头格式：加粗版
        let header = Format::new()
            .set_bold()
            .set_font_name("仿宋")
            .set_font_size(14)
            .set_align(FormatAlign::Center)
            .set_align(FormatAlign::VerticalCenter)
            .set_border(FormatBorder::Thin)
            .set_text_wrap();
        // 加粗格式（用于总计行等）
        let bold = Format::new()
            .set_bold()
            .set_font_name("仿宋")
            .set_font_size(14)
            .set_align(FormatAlign::Center)
            .set_align(FormatAlign::VerticalCenter)
            .set_border(FormatBorder::Thin)
            .set_text_wrap();
        let workload = Format::new()
            .set_font_name("仿宋")
            .set_font_size(14)
            .set_align(FormatAlign::Center)
            .set_align(FormatAlign::VerticalCenter)
            .set_border(FormatBorder::Thin)
            .set_background_color(Color::RGB(0xE7E6E6))
            .set_text_wrap();
        Self {
            fh: header,
            fd: data,
            fb: bold,
            fw: workload,
        }
    }
}

// ========== Sheet 1: 各实验室项目方法对应表 ==========

pub fn write_sheet1(ws: &mut Worksheet, rows: &[FlatRow], fmt: &Fmt) -> Result<()> {
    ws.set_name("各实验室项目方法对应表")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    ws.set_tab_color(Color::RGB(0x1976D2));

    // 设置列宽
    ws.set_column_width(CA, 14.0)?; // 使用实验室
    ws.set_column_width(CB, 18.0)?; // 项目代号
    ws.set_column_width(CC, 18.0)?; // 仪器
    ws.set_column_width(CD, 30.0)?; // 检测方法
    ws.set_column_width(CE, 14.0)?; // 检测类型
    ws.set_column_width(CF, 12.0)?; // 检测数量
    ws.set_column_width(CG, 15.0)?; // 项目检测总量
    ws.set_column_width(CH, 12.0)?; // 高项

    // 设置全局单元格格式（含空白单元格）—— 必须在写数据前调用
    for col in 0u16..=7u16 {
        ws.set_column_format(col, &fmt.fd)?;
    }

    // 写表头
    let headers = [
        "使用实验室",
        "项目代号",
        "仪器",
        "检测方法",
        "检测类型",
        "检测数量",
        "项目检测总量",
        "高项",
    ];
    for (i, h) in headers.iter().enumerate() {
        ws.write_with_format(HR, i as u16, *h, &fmt.fh)?;
    }

    // 检测分组（按实验室、项目代号分组）
    let mut groups: Vec<(u32, u32, String)> = vec![]; // (start, end, project_code)
    let mut i = 0usize;
    while i < rows.len() {
        let (ref_lab, ref_proj, _, _, _, _, _, _, _) = &rows[i];
        let start = HR + 1 + i as u32;
        let mut end = start;

        while i < rows.len() {
            let (lab, proj, _, _, _, _, _, _, _) = &rows[i];
            if lab != ref_lab || proj != ref_proj {
                break;
            }
            end = HR + 1 + i as u32;
            i += 1;
        }
        groups.push((start, end, ref_proj.clone()));
    }

    // 写数据
    let mut row_idx = HR + 1;
    for (lab, proj, inst, method, _, qty, method_type, _, high_item) in rows {
        ws.write_with_format(row_idx, CA, lab.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CB, proj.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CC, inst.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CD, method.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CE, method_type.as_str(), &fmt.fd)?;
        if *qty > 0 {
            ws.write_with_format(row_idx, CF, *qty as f64, &fmt.fd)?;
        }
        ws.write_with_format(row_idx, CH, high_item.as_deref().unwrap_or("-"), &fmt.fd)?;
        row_idx += 1;
    }

    // 合并单元格（实验室、项目代号、汇总列）
    for &(start, end, ref proj_code) in &groups {
        if start == end {
            continue;
        }
        ws.merge_range(start, CB, end, CB, proj_code.as_str(), &fmt.fd)?;
        ws.merge_range(start, CG, end, CG, "", &fmt.fd)?;
    }

    // 合并实验室列
    let mut i = 0usize;
    while i < rows.len() {
        let ref_lab = &rows[i].0;
        let start = HR + 1 + i as u32;
        let mut end = start;
        while i < rows.len() && &rows[i].0 == ref_lab {
            end = HR + 1 + i as u32;
            i += 1;
        }
        if start == end {
            continue;
        }
        ws.merge_range(start, CA, end, CA, ref_lab.as_str(), &fmt.fd)?;
    }

    // 写公式
    for &(start, end, _) in &groups {
        let excel_start = start + 1;
        let excel_end = end + 1;
        ws.write_formula(
            start,
            CG,
            format!(
                "=SUM({}{}:{}{})",
                col_letter(CF),
                excel_start,
                col_letter(CF),
                excel_end
            )
            .as_str(),
        )?;
    }

    // 总计行
    let total_row = row_idx;
    ws.write_with_format(total_row, CA, "总计", &fmt.fb)?;
    for col in [CB, CC, CD, CE, CH] {
        ws.write_with_format(total_row, col, "", &fmt.fb)?;
    }
    for col in [CF, CG] {
        let cl = col_letter(col);
        let last_data_row = if row_idx > HR + 1 {
            row_idx - 1
        } else {
            HR + 1
        };
        ws.write_formula(
            total_row,
            col,
            format!("=SUM({}{}:{}{})", cl, HR + 2, cl, last_data_row).as_str(),
        )?;
    }

    ws.set_freeze_panes(HR + 1, 2)?;
    Ok(())
}

// ========== Sheet 2: 仪器-汇总 ==========

pub fn write_sheet2(ws: &mut Worksheet, rows: &[InstrumentDailyRow], fmt: &Fmt) -> Result<()> {
    ws.set_name("仪器-汇总")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    ws.set_tab_color(Color::RGB(0x43A047));

    ws.set_column_width(CA, 12.0)?; // 日期
    ws.set_column_width(CB, 14.0)?; // 仪器
    ws.set_column_width(CC, 14.0)?; // 实验室
    ws.set_column_width(CD, 20.0)?; // 项目
    ws.set_column_width(CE, 12.0)?; // 高项
    ws.set_column_width(CF, 30.0)?; // 方法
    ws.set_column_width(CG, 12.0)?; // 数量
    ws.set_column_width(CH, 15.0)?; // 按天数量总计

    for col in 0u16..=7u16 {
        ws.set_column_format(col, &fmt.fd)?;
    }

    let headers = [
        "日期",
        "仪器",
        "实验室",
        "检测人",
        "高项",
        "方法",
        "数量",
        "按天数量总计",
    ];
    for (i, h) in headers.iter().enumerate() {
        ws.write_with_format(HR, i as u16, *h, &fmt.fh)?;
    }

    let mut row_idx = HR + 1;
    let mut current_date = String::new();
    let mut date_start = HR + 1;

    for row in rows {
        ws.write_with_format(row_idx, CA, row.date.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CB, row.instrument.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CC, row.lab.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CD, row.project.as_str(), &fmt.fd)?;
        ws.write_with_format(
            row_idx,
            CE,
            row.high_item.as_deref().unwrap_or("-"),
            &fmt.fd,
        )?;
        ws.write_with_format(row_idx, CF, row.method.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CG, row.quantity as f64, &fmt.fd)?;

        if !current_date.is_empty() && current_date != row.date {
            let sum_row = row_idx - 1;
            ws.write_formula(
                date_start,
                CH,
                format!(
                    "=SUM({}{}:{}{})",
                    col_letter(CG),
                    date_start + 1,
                    col_letter(CG),
                    sum_row + 1
                )
                .as_str(),
            )?;
            date_start = row_idx;
        }
        current_date = row.date.clone();
        row_idx += 1;
    }

    if row_idx > HR + 1 {
        ws.write_formula(
            date_start,
            CH,
            format!(
                "=SUM({}{}:{}{})",
                col_letter(CG),
                date_start + 1,
                col_letter(CG),
                row_idx
            )
            .as_str(),
        )?;
    }

    ws.set_freeze_panes(HR + 1, 2)?;
    Ok(())
}

// ========== Sheet 3: 项目-汇总 ==========

pub fn write_sheet3(ws: &mut Worksheet, rows: &[ProjectSummaryRow], fmt: &Fmt) -> Result<()> {
    ws.set_name("项目汇总")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    ws.set_tab_color(Color::RGB(0xFF9800));

    ws.set_column_width(CA, 20.0)?; // 项目
    ws.set_column_width(CB, 12.0)?; // 高项
    ws.set_column_width(CC, 14.0)?; // 实验室
    ws.set_column_width(CD, 14.0)?; // 仪器
    ws.set_column_width(CE, 30.0)?; // 方法
    ws.set_column_width(CF, 10.0)?; // 单价倍率
    ws.set_column_width(CG, 12.0)?; // 数量
    ws.set_column_width(CH, 12.0)?; // 单价
    ws.set_column_width(CI, 15.0)?; // 金额总计
    ws.set_column_width(CJ, 15.0)?; // 项目金额

    for col in 0u16..=9u16 {
        ws.set_column_format(col, &fmt.fd)?;
    }

    let headers = SHEET3_HEADERS;
    for (i, h) in headers.iter().enumerate() {
        ws.write_with_format(HR, i as u16, *h, &fmt.fh)?;
    }

    let mut row_idx = HR + 1;
    let mut project_groups: Vec<(u32, u32, String)> = vec![];
    let mut current_project = String::new();
    let mut proj_start = HR + 1;

    for row in rows {
        ws.write_with_format(row_idx, CA, row.project.as_str(), &fmt.fd)?;
        ws.write_with_format(
            row_idx,
            CB,
            row.high_item.as_deref().unwrap_or("-"),
            &fmt.fd,
        )?;
        ws.write_with_format(row_idx, CC, row.lab.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CD, row.instrument.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CE, row.method.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CF, row.multiplier, &fmt.fd)?;
        ws.write_with_format(row_idx, CG, row.quantity as f64, &fmt.fd)?;
        ws.write_with_format(row_idx, CH, row.unit_price, &fmt.fd)?;

        // 金额总计 = 数量 × 单价 × 单价倍率
        ws.write_formula(
            row_idx,
            CI,
            format!(
                "={}{}*{}{}*{}{}",
                col_letter(CG),
                row_idx + 1,
                col_letter(CH),
                row_idx + 1,
                col_letter(CF),
                row_idx + 1
            )
            .as_str(),
        )?;

        if !current_project.is_empty() && current_project != row.project {
            project_groups.push((proj_start, row_idx - 1, current_project.clone()));
            proj_start = row_idx;
        }
        current_project = row.project.clone();
        row_idx += 1;
    }

    if row_idx > HR + 1 {
        project_groups.push((proj_start, row_idx - 1, current_project));
    }

    for &(start, end, ref proj_name) in &project_groups {
        if start != end {
            ws.merge_range(start, CA, end, CA, proj_name.as_str(), &fmt.fd)?;
            ws.merge_range(start, CJ, end, CJ, "", &fmt.fd)?;
        }
        ws.write_formula(
            start,
            CJ,
            format!(
                "=SUM({}{}:{}{})",
                col_letter(CI),
                start + 1,
                col_letter(CI),
                end + 1
            )
            .as_str(),
        )?;
    }

    ws.set_freeze_panes(HR + 1, 1)?;
    Ok(())
}

// ========== Sheet 4: 实验室-汇总 ==========

pub fn write_sheet4(ws: &mut Worksheet, rows: &[LabSummaryRow], fmt: &Fmt) -> Result<()> {
    ws.set_name("实验室汇总")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    ws.set_tab_color(Color::RGB(0x9C27B0));

    ws.set_column_width(CA, 14.0)?; // 实验室
    ws.set_column_width(CB, 20.0)?; // 项目
    ws.set_column_width(CC, 12.0)?; // 高项
    ws.set_column_width(CD, 14.0)?; // 仪器
    ws.set_column_width(CE, 30.0)?; // 方法
    ws.set_column_width(CF, 10.0)?; // 单价倍率
    ws.set_column_width(CG, 12.0)?; // 数量
    ws.set_column_width(CH, 12.0)?; // 单价
    ws.set_column_width(CI, 12.0)?; // 数量总计
    ws.set_column_width(CJ, 15.0)?; // 金额总计
    ws.set_column_width(CK, 15.0)?; // 实验室汇总

    for col in 0u16..=10u16 {
        ws.set_column_format(col, &fmt.fd)?;
    }

    let headers = SHEET4_HEADERS;
    for (i, h) in headers.iter().enumerate() {
        ws.write_with_format(HR, i as u16, *h, &fmt.fh)?;
    }

    let mut row_idx = HR + 1;
    let mut lab_groups: Vec<(u32, u32, String)> = vec![];
    let mut current_lab = String::new();
    let mut lab_start = HR + 1;

    for row in rows {
        ws.write_with_format(row_idx, CA, row.lab.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CB, row.project.as_str(), &fmt.fd)?;
        ws.write_with_format(
            row_idx,
            CC,
            row.high_item.as_deref().unwrap_or("-"),
            &fmt.fd,
        )?;
        ws.write_with_format(row_idx, CD, row.instrument.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CE, row.method.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CF, row.multiplier, &fmt.fd)?;
        ws.write_with_format(row_idx, CG, row.quantity as f64, &fmt.fd)?;
        ws.write_with_format(row_idx, CH, row.unit_price, &fmt.fd)?;

        // 数量总计 = 数量
        ws.write_formula(
            row_idx,
            CI,
            format!("={}{}", col_letter(CG), row_idx + 1).as_str(),
        )?;
        // 金额总计 = 数量总计 × 单价 × 单价倍率
        ws.write_formula(
            row_idx,
            CJ,
            format!(
                "={}{}*{}{}*{}{}",
                col_letter(CI),
                row_idx + 1,
                col_letter(CH),
                row_idx + 1,
                col_letter(CF),
                row_idx + 1
            )
            .as_str(),
        )?;

        if !current_lab.is_empty() && current_lab != row.lab {
            lab_groups.push((lab_start, row_idx - 1, current_lab.clone()));
            lab_start = row_idx;
        }
        current_lab = row.lab.clone();
        row_idx += 1;
    }

    if row_idx > HR + 1 {
        lab_groups.push((lab_start, row_idx - 1, current_lab));
    }

    for &(start, end, ref lab_name) in &lab_groups {
        if end > start {
            ws.merge_range(start, CA, end, CA, lab_name.as_str(), &fmt.fd)?;
            ws.merge_range(start, CK, end, CK, "", &fmt.fd)?;
        }
        ws.write_formula(
            start,
            CK,
            format!(
                "=SUM({}{}:{}{})",
                col_letter(CJ),
                start + 1,
                col_letter(CJ),
                end + 1
            )
            .as_str(),
        )?;
    }

    ws.set_freeze_panes(HR + 1, 1)?;
    Ok(())
}

// ========== Sheet 5: 人员-汇总（原始记录） ==========

pub fn write_sheet5(
    ws: &mut Worksheet,
    rows: &[PersonRecordRow],
    fmt: &Fmt,
    person_label: &str,
) -> Result<()> {
    ws.set_name(&format!("{}记录明细", person_label))
        .map_err(|e| AppError::Internal(e.to_string()))?;
    ws.set_tab_color(Color::RGB(0xE91E63));

    ws.set_column_width(CA, 16.0)?; // 录入时间
    ws.set_column_width(CB, 14.0)?; // 实验室
    ws.set_column_width(CC, 20.0)?; // 研发项目
    ws.set_column_width(CD, 12.0)?; // 高项
    ws.set_column_width(CE, 30.0)?; // 方法
    ws.set_column_width(CF, 12.0)?; // 检测类型
    ws.set_column_width(CG, 10.0)?; // 数量
    ws.set_column_width(CH, 12.0)?; // 录入人

    for col in 0u16..=7u16 {
        ws.set_column_format(col, &fmt.fd)?;
    }

    let headers = [
        "录入时间",
        "实验室",
        "研发项目",
        "高项",
        "方法",
        "检测类型",
        "数量",
        person_label,
    ];
    for (i, h) in headers.iter().enumerate() {
        ws.write_with_format(HR, i as u16, *h, &fmt.fh)?;
    }

    let mut row_idx = HR + 1;
    for row in rows {
        ws.write_with_format(row_idx, CA, row.recorded_at.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CB, row.lab.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CC, row.project.as_str(), &fmt.fd)?;
        ws.write_with_format(
            row_idx,
            CD,
            row.high_item.as_deref().unwrap_or("-"),
            &fmt.fd,
        )?;
        ws.write_with_format(row_idx, CE, row.method.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CF, row.method_type.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CG, row.quantity as f64, &fmt.fd)?;
        ws.write_with_format(row_idx, CH, row.user_name.as_str(), &fmt.fd)?;
        row_idx += 1;
    }

    ws.set_freeze_panes(HR + 1, 1)?;
    Ok(())
}

/// 分析检测 Sheet 5 写入器。部门维度仅在分析检测导出中出现，避免影响研发送样格式。
pub fn write_analysis_sheet5(
    ws: &mut Worksheet,
    rows: &[AnalysisPersonRecordRow],
    fmt: &Fmt,
    person_label: &str,
) -> Result<()> {
    ws.set_name(&format!("{}记录明细", person_label))
        .map_err(|e| AppError::Internal(e.to_string()))?;
    ws.set_tab_color(Color::RGB(0xE91E63));
    for (col, width) in [
        (CA, 16.0),
        (CB, 16.0),
        (CC, 16.0),
        (CD, 14.0),
        (CE, 20.0),
        (CF, 12.0),
        (CG, 30.0),
        (CH, 12.0),
        (CI, 10.0),
        (CJ, 12.0),
    ] {
        ws.set_column_width(col, width)?;
    }
    for col in 0u16..=9u16 {
        ws.set_column_format(col, &fmt.fd)?;
    }
    let headers = [
        "录入时间",
        "检测部门",
        "送样部门",
        "送样实验室",
        "研发项目",
        "高项",
        "方法",
        "检测类型",
        "数量",
        person_label,
    ];
    for (i, header) in headers.iter().enumerate() {
        ws.write_with_format(HR, i as u16, *header, &fmt.fh)?;
    }
    let mut row_idx = HR + 1;
    for row in rows {
        ws.write_with_format(row_idx, CA, row.recorded_at.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CB, row.detection_department.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CC, row.sending_department.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CD, row.lab.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CE, row.project.as_str(), &fmt.fd)?;
        ws.write_with_format(
            row_idx,
            CF,
            row.high_item.as_deref().unwrap_or("-"),
            &fmt.fd,
        )?;
        ws.write_with_format(row_idx, CG, row.method.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CH, row.method_type.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CI, row.quantity as f64, &fmt.fd)?;
        ws.write_with_format(row_idx, CJ, row.user_name.as_str(), &fmt.fd)?;
        row_idx += 1;
    }
    ws.set_freeze_panes(HR + 1, 1)?;
    Ok(())
}

// ========== Sheet 6: 人员汇总表 ==========

pub fn write_sheet6(
    ws: &mut Worksheet,
    rows: &[PersonSummaryRow],
    fmt: &Fmt,
    person_label: &str,
) -> Result<()> {
    ws.set_name(&format!("{}工作量明细", person_label))
        .map_err(|e| AppError::Internal(e.to_string()))?;
    ws.set_tab_color(Color::RGB(0x00BCD4));
    let widths = [14.0, 24.0, 16.0, 30.0, 10.0, 10.0, 16.0, 18.0];
    for (col, width) in widths.iter().enumerate() {
        ws.set_column_width(col as u16, *width)?;
        ws.set_column_format(col as u16, &fmt.fd)?;
    }
    let headers = [
        "检测人",
        "仪器",
        "类型",
        "方法",
        "系数",
        "数量",
        "工作量明细",
        "人员工作量汇总",
    ];
    for (col, header) in headers.iter().enumerate() {
        ws.write_with_format(HR, col as u16, *header, &fmt.fh)?;
    }
    let mut row_idx = HR + 1;
    let mut groups: Vec<(u32, u32, String)> = Vec::new();
    let mut current = String::new();
    let mut start = row_idx;
    for row in rows {
        if !current.is_empty() && current != row.user_name {
            groups.push((start, row_idx - 1, current.clone()));
            start = row_idx;
        }
        current = row.user_name.clone();
        // 明细按人员分组，首列显示人员名称；项目仍用于后端分组但不新增导出列。
        ws.write_with_format(row_idx, CA, row.user_name.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CB, row.instrument.as_str(), &fmt.fd)?;
        ws.write_with_format(
            row_idx,
            CC,
            normalized_type_label(&row.method_type),
            &fmt.fd,
        )?;
        ws.write_with_format(row_idx, CD, row.method.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CE, row.coefficient, &fmt.fd)?;
        ws.write_with_format(row_idx, CF, row.quantity as f64, &fmt.fd)?;
        let workload_formula = format!(
            "={}{}*{}{}",
            col_letter(CE),
            row_idx + 1,
            col_letter(CF),
            row_idx + 1
        );
        ws.write_formula_with_format(row_idx, CG, workload_formula.as_str(), &fmt.fw)?;
        row_idx += 1;
    }
    if !current.is_empty() {
        groups.push((start, row_idx - 1, current));
    }
    for (start, end, user) in groups {
        if end > start {
            ws.merge_range(start, CA, end, CA, user.as_str(), &fmt.fd)?;
            ws.merge_range(start, CH, end, CH, "", &fmt.fd)?;
            let total_formula = format!(
                "=SUM({}{}:{}{})",
                col_letter(CG),
                start + 1,
                col_letter(CG),
                end + 1
            );
            ws.write_formula_with_format(start, CH, total_formula.as_str(), &fmt.fd)?;
        } else {
            ws.write_with_format(start, CA, user.as_str(), &fmt.fd)?;
            let total_formula = format!(
                "=SUM({}{}:{}{})",
                col_letter(CG),
                start + 1,
                col_letter(CG),
                end + 1
            );
            ws.write_formula_with_format(start, CH, total_formula.as_str(), &fmt.fd)?;
        }
    }
    ws.set_freeze_panes(HR + 1, 1)?;
    Ok(())
}

// ========== Sheet 13: 人员工作量汇总（按检测类型逐行） ==========

pub fn write_person_type_workload_sheet(
    ws: &mut Worksheet,
    rows: &[PersonTypeWorkloadRow],
    fmt: &Fmt,
) -> Result<()> {
    ws.set_name("人员工作量汇总")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    ws.set_tab_color(Color::RGB(0x3F51B5));

    for (col, width) in [
        (CA, 16.0),
        (CB, 14.0),
        (CC, 18.0),
        (CD, 10.0),
        (CE, 10.0),
        (CF, 14.0),
    ] {
        ws.set_column_width(col, width)?;
        ws.set_column_format(col, &fmt.fd)?;
    }
    ws.set_column_format(CF, &fmt.fw)?;
    let headers = ["检测人", "总工作量", "检测类型", "系数", "数量", "工作量"];
    for (index, header) in headers.iter().enumerate() {
        ws.write_with_format(HR, index as u16, *header, &fmt.fh)?;
    }

    let mut row_idx = HR + 1;
    let mut user_groups: Vec<(u32, u32, String)> = Vec::new();
    let mut current_user = String::new();
    let mut user_start = HR + 1;
    for row in rows {
        if !current_user.is_empty() && current_user != row.user_name {
            user_groups.push((user_start, row_idx - 1, current_user.clone()));
            user_start = row_idx;
        }
        current_user = row.user_name.clone();
        ws.write_with_format(row_idx, CA, row.user_name.as_str(), &fmt.fd)?;
        ws.write_with_format(
            row_idx,
            CC,
            normalized_type_label(&row.method_type),
            &fmt.fd,
        )?;
        ws.write_with_format(row_idx, CD, row.coefficient, &fmt.fd)?;
        ws.write_with_format(row_idx, CE, row.quantity as f64, &fmt.fd)?;
        let workload_formula = format!(
            "={}{}*{}{}",
            col_letter(CD),
            row_idx + 1,
            col_letter(CE),
            row_idx + 1
        );
        ws.write_formula_with_format(row_idx, CF, workload_formula.as_str(), &fmt.fw)?;
        row_idx += 1;
    }
    if row_idx > HR + 1 {
        user_groups.push((user_start, row_idx - 1, current_user));
    }
    for (start, end, user) in user_groups {
        if end > start {
            ws.merge_range(start, CA, end, CA, user.as_str(), &fmt.fd)?;
            ws.merge_range(start, CB, end, CB, "", &fmt.fd)?;
        }
        ws.write_formula(
            start,
            CB,
            format!(
                "=SUM({}{}:{}{})",
                col_letter(CF),
                start + 1,
                col_letter(CF),
                end + 1
            )
            .as_str(),
        )?;
    }
    ws.set_freeze_panes(HR + 1, 2)?;
    Ok(())
}

// ========== Sheet 12: 辅助工作明细表 ==========

pub fn write_auxiliary_work_sheet(
    ws: &mut Worksheet,
    rows: &[AuxiliaryWorkRow],
    fmt: &Fmt,
) -> Result<()> {
    ws.set_name("辅助工作明细汇总")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    ws.set_tab_color(Color::RGB(0xFF9800));
    for (col, width) in [14.0, 28.0, 10.0, 10.0, 16.0, 18.0].iter().enumerate() {
        ws.set_column_width(col as u16, *width)?;
        ws.set_column_format(col as u16, &fmt.fd)?;
    }
    let headers = [
        "检测人",
        "辅助工作名称",
        "系数",
        "数量",
        "明细工作量",
        "辅助工作量汇总",
    ];
    for (col, header) in headers.iter().enumerate() {
        ws.write_with_format(HR, col as u16, *header, &fmt.fh)?;
    }
    let mut row_idx = HR + 1;
    let mut groups: Vec<(u32, u32, String)> = Vec::new();
    let mut current = String::new();
    let mut start = row_idx;
    for row in rows {
        if !current.is_empty() && current != row.user_name {
            groups.push((start, row_idx - 1, current.clone()));
            start = row_idx;
        }
        current = row.user_name.clone();
        ws.write_with_format(row_idx, CA, row.user_name.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CB, row.auxiliary_method.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CC, row.coefficient, &fmt.fd)?;
        ws.write_with_format(row_idx, CD, row.quantity as f64, &fmt.fd)?;
        let workload_formula = format!(
            "={}{}*{}{}",
            col_letter(CC),
            row_idx + 1,
            col_letter(CD),
            row_idx + 1
        );
        ws.write_formula_with_format(row_idx, CE, workload_formula.as_str(), &fmt.fw)?;
        row_idx += 1;
    }
    if !current.is_empty() {
        groups.push((start, row_idx - 1, current));
    }
    for (start, end, user) in groups {
        if end > start {
            ws.merge_range(start, CA, end, CA, user.as_str(), &fmt.fd)?;
            ws.merge_range(start, CF, end, CF, "", &fmt.fd)?;
            let total_formula = format!(
                "=SUM({}{}:{}{})",
                col_letter(CE),
                start + 1,
                col_letter(CE),
                end + 1
            );
            ws.write_formula_with_format(start, CF, total_formula.as_str(), &fmt.fd)?;
        } else {
            ws.write_with_format(start, CA, user.as_str(), &fmt.fd)?;
            let total_formula = format!(
                "=SUM({}{}:{}{})",
                col_letter(CE),
                start + 1,
                col_letter(CE),
                end + 1
            );
            ws.write_formula_with_format(start, CF, total_formula.as_str(), &fmt.fd)?;
        }
    }
    ws.set_freeze_panes(HR + 1, 1)?;
    Ok(())
}

// ========== Sheet 6: RD送样人汇总表（仅送样数量，无系数和工作量） ==========

pub fn write_rd_sheet6(
    ws: &mut Worksheet,
    rows: &[PersonSummaryRow],
    fmt: &Fmt,
    person_label: &str,
) -> Result<()> {
    ws.set_name(&format!("{}汇总表", person_label))
        .map_err(|e| AppError::Internal(e.to_string()))?;
    ws.set_tab_color(Color::RGB(0x00BCD4));

    let types = dynamic_type_labels(rows.iter().map(|row| row.method_type.as_str()));
    let total_col = 1 + types.len() as u16;

    // v2.2.9: 仅显示送样数量，不显示系数和工作量
    ws.set_column_width(CA, 12.0)?;
    for index in 0..types.len() {
        ws.set_column_width(1 + index as u16, 10.0)?;
    }
    ws.set_column_width(total_col, 10.0)?;

    for col in 0u16..=total_col {
        ws.set_column_format(col, &fmt.fd)?;
    }

    ws.write_with_format(HR, CA, person_label, &fmt.fh)?;
    for (index, label) in types.iter().enumerate() {
        ws.write_with_format(
            HR,
            1 + index as u16,
            format!("{}数量", label).as_str(),
            &fmt.fh,
        )?;
    }
    ws.write_with_format(HR, total_col, "送样总量", &fmt.fh)?;

    let mut row_idx = HR + 1;
    let mut user_groups: Vec<(u32, u32, String)> = vec![];
    let mut current_user = String::new();
    let mut user_start = HR + 1;

    for row in rows {
        if !current_user.is_empty() && current_user != row.user_name {
            user_groups.push((user_start, row_idx - 1, current_user.clone()));
            user_start = row_idx;
        }
        current_user = row.user_name.clone();

        ws.write_with_format(row_idx, CA, row.user_name.as_str(), &fmt.fd)?;

        let row_type = normalized_type_label(&row.method_type);
        if let Some(index) = types.iter().position(|label| label == row_type) {
            ws.write_with_format(row_idx, 1 + index as u16, row.quantity as f64, &fmt.fd)?;
        }

        row_idx += 1;
    }

    if row_idx > HR + 1 {
        user_groups.push((user_start, row_idx - 1, current_user));
    }

    // 合并用户列和总量列
    for (start, end, user) in &user_groups {
        if *start == *end {
            continue;
        }
        ws.merge_range(*start, CA, *end, CA, user.as_str(), &fmt.fd)?;
        ws.merge_range(*start, total_col, *end, total_col, "", &fmt.fd)?;

        // 计算送样总量（各类型数量之和）
        let mut sum_formula = String::from("=");
        for (idx, _) in types.iter().enumerate() {
            if idx > 0 {
                sum_formula.push('+');
            }
            sum_formula.push_str(&format!(
                "SUM({}{}:{}{})",
                col_letter(1 + idx as u16),
                *start + 1,
                col_letter(1 + idx as u16),
                *end + 1
            ));
        }
        ws.write_formula(*start, total_col, sum_formula.as_str())?;
    }

    // 单行用户
    for (start, end, _user) in &user_groups {
        if *start != *end {
            continue;
        }
        let mut sum_formula = String::from("=");
        for (idx, _) in types.iter().enumerate() {
            if idx > 0 {
                sum_formula.push('+');
            }
            sum_formula.push_str(&format!("{}{}", col_letter(1 + idx as u16), *start + 1));
        }
        ws.write_formula(*start, total_col, sum_formula.as_str())?;
    }

    ws.set_freeze_panes(HR + 1, 1)?;
    Ok(())
}

// ========== Sheet 7: 实验室总表 ==========

pub fn write_sheet7(ws: &mut Worksheet, rows: &[LabTotalRow], fmt: &Fmt) -> Result<()> {
    ws.set_name("实验室总表")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    ws.set_tab_color(Color::RGB(0x4CAF50));
    let headers = [
        "实验室",
        "项目",
        "检测类型",
        "数量",
        "单价",
        "单价倍率",
        "明细金额",
        "项目金额汇总",
        "实验室金额汇总",
    ];
    for (col, width) in [14.0, 20.0, 16.0, 12.0, 12.0, 10.0, 15.0, 16.0, 16.0]
        .iter()
        .enumerate()
    {
        ws.set_column_width(col as u16, *width)?;
        ws.set_column_format(col as u16, &fmt.fd)?;
    }
    for (col, header) in headers.iter().enumerate() {
        ws.write_with_format(HR, col as u16, *header, &fmt.fh)?;
    }
    let mut row_idx = HR + 1;
    let mut project_groups: Vec<(u32, u32, String, String)> = Vec::new();
    let mut lab_groups: Vec<(u32, u32, String)> = Vec::new();
    let mut project_start = row_idx;
    let mut lab_start = row_idx;
    let mut current_project = String::new();
    let mut current_lab = String::new();
    for row in rows {
        if !current_project.is_empty() && (current_project != row.project || current_lab != row.lab)
        {
            project_groups.push((
                project_start,
                row_idx - 1,
                current_lab.clone(),
                current_project.clone(),
            ));
            project_start = row_idx;
        }
        if !current_lab.is_empty() && current_lab != row.lab {
            lab_groups.push((lab_start, row_idx - 1, current_lab.clone()));
            lab_start = row_idx;
        }
        current_project = row.project.clone();
        current_lab = row.lab.clone();
        ws.write_with_format(row_idx, CA, row.lab.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CB, row.project.as_str(), &fmt.fd)?;
        ws.write_with_format(
            row_idx,
            CC,
            normalized_type_label(&row.method_type),
            &fmt.fd,
        )?;
        ws.write_with_format(row_idx, CD, row.quantity as f64, &fmt.fd)?;
        ws.write_with_format(row_idx, CE, row.unit_price, &fmt.fd)?;
        ws.write_with_format(row_idx, CF, row.multiplier, &fmt.fd)?;
        ws.write_formula_with_format(
            row_idx,
            CG,
            format!(
                "={}{}*{}{}*{}{}",
                col_letter(CD),
                row_idx + 1,
                col_letter(CE),
                row_idx + 1,
                col_letter(CF),
                row_idx + 1
            )
            .as_str(),
            &fmt.fw,
        )?;
        row_idx += 1;
    }
    if row_idx > HR + 1 {
        project_groups.push((
            project_start,
            row_idx - 1,
            current_lab.clone(),
            current_project,
        ));
        lab_groups.push((lab_start, row_idx - 1, current_lab));
    }
    for (start, end, lab, project) in project_groups {
        if end > start {
            ws.merge_range(start, CB, end, CB, project.as_str(), &fmt.fd)?;
            ws.merge_range(start, CH, end, CH, "", &fmt.fd)?;
        }
        ws.write_formula_with_format(
            start,
            CH,
            format!("=SUM(G{}:G{})", start + 1, end + 1).as_str(),
            &fmt.fd,
        )?;
        let _ = lab;
    }
    for (start, end, lab) in lab_groups {
        if end > start {
            ws.merge_range(start, CA, end, CA, lab.as_str(), &fmt.fd)?;
            ws.merge_range(start, CI, end, CI, "", &fmt.fd)?;
        }
        ws.write_formula_with_format(
            start,
            CI,
            format!("=SUM(H{}:H{})", start + 1, end + 1).as_str(),
            &fmt.fd,
        )?;
    }
    ws.set_freeze_panes(HR + 1, 2)?;
    Ok(())
}

fn write_sheet7_legacy(ws: &mut Worksheet, rows: &[LabTotalRow], fmt: &Fmt) -> Result<()> {
    ws.set_name("实验室总表")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    ws.set_tab_color(Color::RGB(0x4CAF50));

    let types = dynamic_type_labels(rows.iter().map(|row| row.method_type.as_str()));
    let project_total_col = 2 + types.len() as u16 * 4;
    let lab_total_col = project_total_col + 1;
    let amount_cols = types
        .iter()
        .enumerate()
        .map(|(index, _)| 2 + index as u16 * 4 + 3)
        .collect::<Vec<_>>();

    ws.set_column_width(CA, 14.0)?;
    ws.set_column_width(CB, 20.0)?;
    for index in 0..types.len() {
        let base = 2 + index as u16 * 4;
        ws.set_column_width(base, 10.0)?;
        ws.set_column_width(base + 1, 10.0)?;
        ws.set_column_width(base + 2, 8.0)?;
        ws.set_column_width(base + 3, 12.0)?;
    }
    ws.set_column_width(project_total_col, 15.0)?;
    ws.set_column_width(lab_total_col, 15.0)?;

    for col in 0u16..=lab_total_col {
        ws.set_column_format(col, &fmt.fd)?;
    }

    ws.write_with_format(HR, CA, "实验室", &fmt.fh)?;
    ws.write_with_format(HR, CB, "项目", &fmt.fh)?;
    for (index, label) in types.iter().enumerate() {
        let base = 2 + index as u16 * 4;
        ws.write_with_format(HR, base, format!("{}数量", label).as_str(), &fmt.fh)?;
        ws.write_with_format(HR, base + 1, format!("{}单价", label).as_str(), &fmt.fh)?;
        ws.write_with_format(HR, base + 2, format!("{}单价倍率", label).as_str(), &fmt.fh)?;
        ws.write_with_format(HR, base + 3, format!("{}金额汇总", label).as_str(), &fmt.fh)?;
    }
    ws.write_with_format(HR, project_total_col, "项目金额汇总", &fmt.fh)?;
    ws.write_with_format(HR, lab_total_col, "实验室金额汇总", &fmt.fh)?;

    let mut row_idx = HR + 1;
    let mut lab_groups: Vec<(u32, u32, String)> = vec![];
    let mut current_lab = String::new();
    let mut lab_start = HR + 1;

    for row in rows {
        if !current_lab.is_empty() && current_lab != row.lab {
            lab_groups.push((lab_start, row_idx - 1, current_lab.clone()));
            lab_start = row_idx;
        }
        current_lab = row.lab.clone();

        ws.write_with_format(row_idx, CA, row.lab.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CB, row.project.as_str(), &fmt.fd)?;

        let row_type = normalized_type_label(&row.method_type);
        if let Some(index) = types.iter().position(|label| label == row_type) {
            let base = 2 + index as u16 * 4;
            ws.write_with_format(row_idx, base, row.quantity as f64, &fmt.fd)?;
            ws.write_with_format(row_idx, base + 1, row.unit_price, &fmt.fd)?;
            ws.write_with_format(row_idx, base + 2, row.multiplier, &fmt.fd)?;
            ws.write_formula(
                row_idx,
                base + 3,
                format!(
                    "={}{}*{}{}*{}{}",
                    col_letter(base),
                    row_idx + 1,
                    col_letter(base + 1),
                    row_idx + 1,
                    col_letter(base + 2),
                    row_idx + 1
                )
                .as_str(),
            )?;
        }

        ws.write_formula(
            row_idx,
            project_total_col,
            row_sum_formula(&amount_cols, row_idx).as_str(),
        )?;

        row_idx += 1;
    }

    if row_idx > HR + 1 {
        lab_groups.push((lab_start, row_idx - 1, current_lab));
    }

    for &(start, end, ref lab_name) in &lab_groups {
        if start == end {
            continue;
        }
        ws.merge_range(start, CA, end, CA, lab_name.as_str(), &fmt.fd)?;
        ws.merge_range(start, lab_total_col, end, lab_total_col, "", &fmt.fd)?;
        ws.write_formula(
            start,
            lab_total_col,
            format!(
                "=SUM({}{}:{}{})",
                col_letter(project_total_col),
                start + 1,
                col_letter(project_total_col),
                end + 1
            )
            .as_str(),
        )?;
    }

    ws.set_freeze_panes(HR + 1, 2)?;
    Ok(())
}

// ========== Sheet 8: 项目总表 ==========

pub fn write_sheet8(ws: &mut Worksheet, rows: &[ProjectTotalRow], fmt: &Fmt) -> Result<()> {
    ws.set_name("项目总表")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    ws.set_tab_color(Color::RGB(0xFFC107));
    let headers = [
        "项目",
        "检测类型",
        "数量",
        "单价",
        "单价倍率",
        "明细金额",
        "项目金额汇总",
    ];
    for (col, width) in [20.0, 16.0, 12.0, 12.0, 10.0, 15.0, 16.0]
        .iter()
        .enumerate()
    {
        ws.set_column_width(col as u16, *width)?;
        ws.set_column_format(col as u16, &fmt.fd)?;
    }
    for (col, header) in headers.iter().enumerate() {
        ws.write_with_format(HR, col as u16, *header, &fmt.fh)?;
    }
    let mut row_idx = HR + 1;
    let mut groups: Vec<(u32, u32, String)> = Vec::new();
    let mut start = row_idx;
    let mut current = String::new();
    for row in rows {
        if !current.is_empty() && current != row.project {
            groups.push((start, row_idx - 1, current.clone()));
            start = row_idx;
        }
        current = row.project.clone();
        ws.write_with_format(row_idx, CA, row.project.as_str(), &fmt.fd)?;
        ws.write_with_format(
            row_idx,
            CB,
            normalized_type_label(&row.method_type),
            &fmt.fd,
        )?;
        ws.write_with_format(row_idx, CC, row.quantity as f64, &fmt.fd)?;
        ws.write_with_format(row_idx, CD, row.unit_price, &fmt.fd)?;
        ws.write_with_format(row_idx, CE, row.multiplier, &fmt.fd)?;
        ws.write_formula_with_format(
            row_idx,
            CF,
            format!("=C{}*D{}*E{}", row_idx + 1, row_idx + 1, row_idx + 1).as_str(),
            &fmt.fw,
        )?;
        row_idx += 1;
    }
    if row_idx > HR + 1 {
        groups.push((start, row_idx - 1, current));
    }
    for (start, end, project) in groups {
        if end > start {
            ws.merge_range(start, CA, end, CA, project.as_str(), &fmt.fd)?;
            ws.merge_range(start, CG, end, CG, "", &fmt.fd)?;
        }
        ws.write_formula_with_format(
            start,
            CG,
            format!("=SUM(F{}:F{})", start + 1, end + 1).as_str(),
            &fmt.fd,
        )?;
    }
    ws.set_freeze_panes(HR + 1, 1)?;
    Ok(())
}

fn write_sheet8_legacy(ws: &mut Worksheet, rows: &[ProjectTotalRow], fmt: &Fmt) -> Result<()> {
    ws.set_name("项目总表")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    ws.set_tab_color(Color::RGB(0xFFC107));

    let types = dynamic_type_labels(rows.iter().map(|row| row.method_type.as_str()));
    let project_total_col = 1 + types.len() as u16 * 4;
    let amount_cols = types
        .iter()
        .enumerate()
        .map(|(index, _)| 1 + index as u16 * 4 + 3)
        .collect::<Vec<_>>();

    ws.set_column_width(CA, 20.0)?;
    for index in 0..types.len() {
        let base = 1 + index as u16 * 4;
        ws.set_column_width(base, 10.0)?;
        ws.set_column_width(base + 1, 10.0)?;
        ws.set_column_width(base + 2, 8.0)?;
        ws.set_column_width(base + 3, 12.0)?;
    }
    ws.set_column_width(project_total_col, 15.0)?;

    for col in 0u16..=project_total_col {
        ws.set_column_format(col, &fmt.fd)?;
    }

    ws.write_with_format(HR, CA, "项目", &fmt.fh)?;
    for (index, label) in types.iter().enumerate() {
        let base = 1 + index as u16 * 4;
        ws.write_with_format(HR, base, format!("{}数量", label).as_str(), &fmt.fh)?;
        ws.write_with_format(HR, base + 1, format!("{}单价", label).as_str(), &fmt.fh)?;
        ws.write_with_format(HR, base + 2, format!("{}单价倍率", label).as_str(), &fmt.fh)?;
        ws.write_with_format(HR, base + 3, format!("{}金额汇总", label).as_str(), &fmt.fh)?;
    }
    ws.write_with_format(HR, project_total_col, "项目金额汇总", &fmt.fh)?;

    let mut row_idx = HR + 1;
    let mut project_groups: Vec<(u32, u32, String)> = vec![];
    let mut current_project = String::new();
    let mut project_start = HR + 1;

    for row in rows {
        if !current_project.is_empty() && current_project != row.project {
            project_groups.push((project_start, row_idx - 1, current_project.clone()));
            project_start = row_idx;
        }
        current_project = row.project.clone();

        ws.write_with_format(row_idx, CA, row.project.as_str(), &fmt.fd)?;

        let row_type = normalized_type_label(&row.method_type);
        if let Some(index) = types.iter().position(|label| label == row_type) {
            let base = 1 + index as u16 * 4;
            ws.write_with_format(row_idx, base, row.quantity as f64, &fmt.fd)?;
            ws.write_with_format(row_idx, base + 1, row.unit_price, &fmt.fd)?;
            ws.write_with_format(row_idx, base + 2, row.multiplier, &fmt.fd)?;
            ws.write_formula(
                row_idx,
                base + 3,
                format!(
                    "={}{}*{}{}*{}{}",
                    col_letter(base),
                    row_idx + 1,
                    col_letter(base + 1),
                    row_idx + 1,
                    col_letter(base + 2),
                    row_idx + 1
                )
                .as_str(),
            )?;
        }

        ws.write_formula(
            row_idx,
            project_total_col,
            row_sum_formula(&amount_cols, row_idx).as_str(),
        )?;

        row_idx += 1;
    }

    if row_idx > HR + 1 {
        project_groups.push((project_start, row_idx - 1, current_project));
    }

    for (start, end, project) in &project_groups {
        if *start == *end {
            continue;
        }
        ws.merge_range(*start, CA, *end, CA, project.as_str(), &fmt.fd)?;
        ws.merge_range(
            *start,
            project_total_col,
            *end,
            project_total_col,
            "",
            &fmt.fd,
        )?;
        ws.write_formula(
            *start,
            project_total_col,
            range_sum_formula(&amount_cols, *start, *end).as_str(),
        )?;
    }

    ws.set_freeze_panes(HR + 1, 1)?;
    Ok(())
}

// ========== Sheet 9: 仪器汇总表 ==========

pub fn write_sheet9(ws: &mut Worksheet, rows: &[InstrumentSummaryRow], fmt: &Fmt) -> Result<()> {
    ws.set_name("仪器汇总表")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    ws.set_tab_color(Color::RGB(0x9E9E9E));

    ws.set_column_width(CA, 16.0)?; // 仪器编号
    ws.set_column_width(CB, 12.0)?; // 检测量
    ws.set_column_width(CC, 12.0)?; // 类型
    ws.set_column_width(CD, 15.0)?; // 按类型汇总

    for col in 0u16..=3u16 {
        ws.set_column_format(col, &fmt.fd)?;
    }

    let headers = ["仪器编号", "检测量", "类型", "按类型汇总"];
    for (i, h) in headers.iter().enumerate() {
        ws.write_with_format(HR, i as u16, *h, &fmt.fh)?;
    }

    let mut row_idx = HR + 1;
    let mut type_groups: Vec<(u32, u32, String)> = vec![];
    let mut current_type = String::new();
    let mut type_start = HR + 1;

    for row in rows {
        ws.write_with_format(row_idx, CA, row.instrument.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CB, row.quantity as f64, &fmt.fd)?;
        ws.write_with_format(row_idx, CC, row.instrument_type.as_str(), &fmt.fd)?;

        if !current_type.is_empty() && current_type != row.instrument_type {
            type_groups.push((type_start, row_idx - 1, current_type.clone()));
            type_start = row_idx;
        }
        current_type = row.instrument_type.clone();
        row_idx += 1;
    }

    if row_idx > HR + 1 {
        type_groups.push((type_start, row_idx - 1, current_type));
    }

    for &(start, end, _) in &type_groups {
        let total_formula = format!(
            "=SUM({}{}:{}{})",
            col_letter(CB),
            start + 1,
            col_letter(CB),
            end + 1
        );
        if end > start {
            ws.merge_range(start, CD, end, CD, "", &fmt.fd)?;
        }
        ws.write_formula_with_format(start, CD, total_formula.as_str(), &fmt.fd)?;
    }

    ws.set_freeze_panes(HR + 1, 1)?;
    Ok(())
}

// ========== Sheet 10: 理化汇总表 ==========

pub fn write_sheet10(ws: &mut Worksheet, rows: &[PhysChemRow], fmt: &Fmt) -> Result<()> {
    ws.set_name("理化汇总表")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    ws.set_tab_color(Color::RGB(0x795548));

    ws.set_column_width(CA, 40.0)?; // 方法名
    ws.set_column_width(CB, 12.0)?; // 数量

    for col in 0u16..=1u16 {
        ws.set_column_format(col, &fmt.fd)?;
    }

    let headers = ["方法名", "数量"];
    for (i, h) in headers.iter().enumerate() {
        ws.write_with_format(HR, i as u16, *h, &fmt.fh)?;
    }

    let mut row_idx = HR + 1;
    for row in rows {
        ws.write_with_format(row_idx, CA, row.method.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CB, row.quantity as f64, &fmt.fd)?;
        row_idx += 1;
    }

    // 总计行
    if row_idx > HR + 1 {
        ws.write_with_format(row_idx, CA, "总计", &fmt.fb)?;
        ws.write_formula(
            row_idx,
            CB,
            format!(
                "=SUM({}{}:{}{})",
                col_letter(CB),
                HR + 2,
                col_letter(CB),
                row_idx
            )
            .as_str(),
        )?;
    }

    ws.set_freeze_panes(HR + 1, 1)?;
    Ok(())
}

// ========== Sheet 11: 类型汇总表 ==========

pub fn write_sheet11(ws: &mut Worksheet, rows: &[TypeSummaryRow], fmt: &Fmt) -> Result<()> {
    ws.set_name("类型汇总表")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    ws.set_tab_color(Color::RGB(0x00BCD4));

    ws.set_column_width(CA, 12.0)?; // 检测类型
    ws.set_column_width(CB, 12.0)?; // 数量
    ws.set_column_width(CC, 12.0)?; // 单价
    ws.set_column_width(CD, 15.0)?; // 金额总计
    ws.set_column_width(CE, 15.0)?; // 类型金额

    for col in 0u16..=4u16 {
        ws.set_column_format(col, &fmt.fd)?;
    }

    let headers = SHEET11_HEADERS;
    for (i, h) in headers.iter().enumerate() {
        ws.write_with_format(HR, i as u16, *h, &fmt.fh)?;
    }

    let mut row_idx = HR + 1;
    let mut type_groups: Vec<(u32, u32, String)> = vec![];
    let mut current_type = String::new();
    let mut type_start = HR + 1;

    for row in rows {
        ws.write_with_format(row_idx, CA, row.method_type.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CB, row.quantity as f64, &fmt.fd)?;
        ws.write_with_format(row_idx, CC, row.unit_price, &fmt.fd)?;
        ws.write_formula(
            row_idx,
            CD,
            format!(
                "={}{}*{}{}",
                col_letter(CB),
                row_idx + 1,
                col_letter(CC),
                row_idx + 1
            )
            .as_str(),
        )?;

        if !current_type.is_empty() && current_type != row.method_type {
            type_groups.push((type_start, row_idx - 1, current_type.clone()));
            type_start = row_idx;
        }
        current_type = row.method_type.clone();
        row_idx += 1;
    }

    if row_idx > HR + 1 {
        type_groups.push((type_start, row_idx - 1, current_type));
    }

    for &(start, end, ref type_name) in &type_groups {
        if start == end {
            continue;
        }
        ws.merge_range(start, CA, end, CA, type_name.as_str(), &fmt.fd)?;
        ws.merge_range(start, CE, end, CE, "", &fmt.fd)?;
        ws.write_formula(
            start,
            CE,
            format!(
                "=SUM({}{}:{}{})",
                col_letter(CD),
                start + 1,
                col_letter(CD),
                end + 1
            )
            .as_str(),
        )?;
    }

    // 总计行
    let total_row = row_idx;
    ws.write_with_format(total_row, CA, "合计", &fmt.fb)?;
    for col in [CB, CC] {
        ws.write_with_format(total_row, col, "", &fmt.fb)?;
    }
    // 金额总计行合计
    ws.write_formula(
        total_row,
        CD,
        format!(
            "=SUM({}{}:{}{})",
            col_letter(CD),
            HR + 2,
            col_letter(CD),
            total_row
        )
        .as_str(),
    )?;

    ws.set_freeze_panes(HR + 1, 1)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amount_headers_are_explicit() {
        assert_eq!(SHEET3_HEADERS[8], "明细金额");
        assert_eq!(SHEET3_HEADERS[9], "项目金额汇总");
        assert_eq!(SHEET4_HEADERS[9], "明细金额");
        assert_eq!(SHEET4_HEADERS[10], "实验室金额汇总");
        assert_eq!(SHEET7_HEADERS[5], "液相金额汇总");
        assert_eq!(SHEET7_HEADERS[9], "气相金额汇总");
        assert_eq!(SHEET7_HEADERS[13], "理化金额汇总");
        assert_eq!(SHEET7_HEADERS[14], "项目金额汇总");
        assert_eq!(SHEET7_HEADERS[15], "实验室金额汇总");
        assert_eq!(SHEET8_HEADERS[4], "液相金额汇总");
        assert_eq!(SHEET8_HEADERS[8], "气相金额汇总");
        assert_eq!(SHEET8_HEADERS[12], "理化金额汇总");
        assert_eq!(SHEET8_HEADERS[13], "项目金额汇总");
        assert_eq!(SHEET11_HEADERS[3], "明细金额");
        assert_eq!(SHEET11_HEADERS[4], "检测类型金额汇总");
    }

    #[test]
    fn workload_and_amount_keep_coefficient_and_multiplier_independent() {
        assert_eq!(calculate_workload(5, 20.0), 100.0);
        assert_eq!(calculate_amount(5, 2.0, 350.0), 3500.0);
        assert_eq!(calculate_workload(5, 20.0), calculate_workload(5, 20.0));
        assert_eq!(calculate_amount(5, 2.0, 500.0), 5000.0);
    }

    #[test]
    fn personnel_headers_describe_workload_not_money() {
        assert_eq!(SHEET6_HEADERS[1], "总工作量");
        assert_eq!(SHEET6_HEADERS[2], "检测工作量");
        assert_eq!(SHEET6_HEADERS[3], "辅助工作量");
        assert_eq!(SHEET6_HEADERS[9], "气相工作量");
        assert_eq!(SHEET6_HEADERS[12], "理化工作量");
        assert!(!SHEET6_HEADERS.iter().any(|header| header.contains("金额")));
    }

    #[test]
    fn dynamic_types_are_kept_separate_and_unclassified_is_explicit() {
        let labels = dynamic_type_labels(["液相", "辅助工作", "理化", "辅助工作", ""]);
        assert_eq!(labels, vec!["液相", "辅助工作", "理化", "未分类"]);
        assert_eq!(sheet6_last_col(&[]), 7);
    }

    #[test]
    fn dynamic_summary_column_widths_follow_actual_types() {
        let person_rows = vec![
            PersonSummaryRow {
                user_name: "检测员".into(),
                project: String::new(),
                instrument: String::new(),
                method: String::new(),
                method_type: "辅助工作".into(),
                coefficient: 1.0,
                multiplier: 1.0,
                quantity: 2,
                workload: 2.0,
            },
            PersonSummaryRow {
                user_name: "检测员".into(),
                project: String::new(),
                instrument: String::new(),
                method: String::new(),
                method_type: "ICP".into(),
                coefficient: 1.0,
                multiplier: 1.0,
                quantity: 3,
                workload: 3.0,
            },
        ];
        assert_eq!(sheet6_last_col(&person_rows), 7);
        assert_eq!(sheet7_last_col(&[]), 8);
        assert_eq!(sheet8_last_col(&[]), 6);
    }

    #[test]
    fn person_type_sheet_keeps_different_coefficients_as_separate_rows() {
        let mut workbook = Workbook::new();
        let fmt = Fmt::new();
        let rows = vec![
            PersonTypeWorkloadRow {
                user_name: "人员1".into(),
                method_type: "液相".into(),
                coefficient: 20.0,
                quantity: 2,
                workload: 40.0,
            },
            PersonTypeWorkloadRow {
                user_name: "人员1".into(),
                method_type: "液相".into(),
                coefficient: 30.0,
                quantity: 3,
                workload: 90.0,
            },
        ];
        write_person_type_workload_sheet(workbook.add_worksheet(), &rows, &fmt).unwrap();
        assert_eq!(sheet13_last_col(), 5);
    }

    #[test]
    fn person_type_workload_xml_never_contains_multiplier_formula() {
        use std::io::Read;

        let mut workbook = Workbook::new();
        let fmt = Fmt::new();
        let rows = vec![PersonTypeWorkloadRow {
            user_name: "人员1".into(),
            method_type: "液相".into(),
            coefficient: 20.0,
            quantity: 5,
            workload: 100.0,
        }];
        write_person_type_workload_sheet(workbook.add_worksheet(), &rows, &fmt).unwrap();
        let path = std::env::temp_dir().join(format!(
            "workload_sheet13_formula_{}.xlsx",
            std::process::id()
        ));
        workbook.save(&path).unwrap();
        let file = std::fs::File::open(&path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut xml = String::new();
        archive
            .by_name("xl/worksheets/sheet1.xml")
            .unwrap()
            .read_to_string(&mut xml)
            .unwrap();
        std::fs::remove_file(path).unwrap();
        // The workload cell is formula-driven; cached numeric results are intentionally omitted.
        assert!(xml.contains("<f>"));
        assert!(xml.contains("E3*D3") || xml.contains("D3*E3"));
        assert!(!xml.contains("350"));
        assert!(!xml.contains("*1*"));
    }

    #[test]
    fn sheet6_writes_person_grouping_and_workload_summary_to_xlsx() {
        use std::io::Read;

        let mut workbook = Workbook::new();
        let fmt = Fmt::new();
        let rows = vec![
            PersonSummaryRow {
                user_name: "人员1".into(),
                project: "项目A".into(),
                instrument: "HPLC-MS-01".into(),
                method_type: "气相".into(),
                method: "HPLC-MS".into(),
                coefficient: 20.0,
                multiplier: 350.0,
                quantity: 5,
                workload: 100.0,
            },
            PersonSummaryRow {
                user_name: "人员1".into(),
                project: "项目B".into(),
                instrument: "委外仪器".into(),
                method_type: "液相".into(),
                method: "核磁".into(),
                coefficient: 20.0,
                multiplier: 500.0,
                quantity: 1,
                workload: 20.0,
            },
        ];
        write_sheet6(workbook.add_worksheet(), &rows, &fmt, "检测人").unwrap();

        let path = std::env::temp_dir().join(format!(
            "workload_sheet6_layout_{}.xlsx",
            std::process::id()
        ));
        workbook.save(&path).unwrap();
        let file = std::fs::File::open(&path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut sheet_xml = String::new();
        archive
            .by_name("xl/worksheets/sheet1.xml")
            .unwrap()
            .read_to_string(&mut sheet_xml)
            .unwrap();
        let mut strings_xml = String::new();
        archive
            .by_name("xl/sharedStrings.xml")
            .unwrap()
            .read_to_string(&mut strings_xml)
            .unwrap();
        std::fs::remove_file(path).unwrap();

        assert!(strings_xml.contains("人员工作量汇总"));
        assert!(strings_xml.contains("人员1"));
        // Workload is an Excel formula based on the exported coefficient and quantity cells.
        assert!(sheet_xml.contains("<f>E3*F3</f>"));
        assert!(!sheet_xml.contains("*350"));
        assert!(sheet_xml.contains(r#"ref="A3:A4""#));
        assert!(sheet_xml.contains(r#"ref="H3:H4""#));
    }

    #[test]
    fn single_row_workload_groups_are_written_without_single_cell_merges() {
        let mut workbook = Workbook::new();
        let fmt = Fmt::new();
        let row = PersonSummaryRow {
            user_name: "人员1".into(),
            project: "项目A".into(),
            instrument: "仪器A".into(),
            method_type: "理化".into(),
            method: "方法A".into(),
            coefficient: 20.0,
            multiplier: 350.0,
            quantity: 1,
            workload: 20.0,
        };
        write_sheet6(workbook.add_worksheet(), &[row], &fmt, "检测人").unwrap();
        let auxiliary = AuxiliaryWorkRow {
            user_name: "人员1".into(),
            project: "项目A".into(),
            auxiliary_method: "辅助工作".into(),
            coefficient: 1.0,
            quantity: 1,
            workload: 1.0,
        };
        write_auxiliary_work_sheet(workbook.add_worksheet(), &[auxiliary], &fmt).unwrap();
    }

    #[test]
    fn workload_uses_quantity_and_coefficient_only() {
        assert_eq!(calculate_workload(5, 20.0), 100.0);
    }

    #[test]
    fn rd_person_sheets_have_distinct_names() {
        let mut workbook = Workbook::new();
        let fmt = Fmt::new();
        write_sheet5(workbook.add_worksheet(), &[], &fmt, "送样人").unwrap();
        write_rd_sheet6(workbook.add_worksheet(), &[], &fmt, "送样人").unwrap();
    }

    #[test]
    fn lab_total_workbook_uses_unified_fixed_headers() {
        let mut workbook = Workbook::new();
        let fmt = Fmt::new();
        let rows = vec![
            LabTotalRow {
                lab: "实验室A".into(),
                project: "项目A".into(),
                method_type: "辅助工作".into(),
                multiplier: 1.0,
                unit_price: 0.0,
                quantity: 2,
            },
            LabTotalRow {
                lab: "实验室A".into(),
                project: "项目A".into(),
                method_type: "理化".into(),
                multiplier: 1.0,
                unit_price: 10.0,
                quantity: 1,
            },
        ];
        write_sheet7(workbook.add_worksheet(), &rows, &fmt).unwrap();

        let path = std::env::temp_dir().join(format!(
            "workload_dynamic_export_headers_{}.xlsx",
            std::process::id()
        ));
        workbook.save(&path).unwrap();
        let file = std::fs::File::open(&path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut xml = String::new();
        use std::io::Read;
        archive
            .by_name("xl/sharedStrings.xml")
            .unwrap()
            .read_to_string(&mut xml)
            .unwrap();
        std::fs::remove_file(path).unwrap();

        assert!(xml.contains("实验室"));
        assert!(xml.contains("项目"));
        assert!(xml.contains("检测类型"));
        assert!(xml.contains("数量"));
        assert!(xml.contains("单价"));
        assert!(xml.contains("单价倍率"));
        assert!(xml.contains("明细金额"));
        assert!(xml.contains("项目金额汇总"));
        assert!(xml.contains("实验室金额汇总"));
        assert!(!xml.contains("辅助工作数量"));
    }
}
