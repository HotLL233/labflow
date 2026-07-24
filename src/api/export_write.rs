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

pub const HR: u32 = 0; // 表头行（0-indexed，实际是第1行）

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
const SHEET6_HEADERS: [&str; 12] = [
    "人员",
    "液相数量",
    "液相系数",
    "液相工作量",
    "气相数量",
    "气相系数",
    "气相工作量",
    "理化数量",
    "理化系数",
    "理化工作量",
    "个人工作量汇总",
    "总工作量",
];
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

// ========== 格式定义 ==========

pub struct Fmt {
    pub fh: Format, // 表头格式
    pub fd: Format, // 数据格式
    pub fb: Format, // 加粗格式
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
        Self {
            fh: header,
            fd: data,
            fb: bold,
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
    ws.set_column_width(CC, 18.0)?; // 液相仪器
    ws.set_column_width(CD, 30.0)?; // 检测方法
    ws.set_column_width(CE, 12.0)?; // 检测数量
    ws.set_column_width(CF, 12.0)?; // 液相检测量
    ws.set_column_width(CG, 12.0)?; // 气相检测量
    ws.set_column_width(CH, 15.0)?; // 项目检测总量
    ws.set_column_width(CI, 12.0)?; // 高项

    // 设置全局单元格格式（含空白单元格）—— 必须在写数据前调用
    for col in 0u16..=8u16 {
        ws.set_column_format(col, &fmt.fd)?;
    }

    // 写表头
    let headers = [
        "使用实验室",
        "项目代号",
        "液相仪器",
        "检测方法",
        "检测数量",
        "液相检测量",
        "气相检测量",
        "项目检测总量",
        "高项",
    ];
    for (i, h) in headers.iter().enumerate() {
        ws.write_with_format(HR, i as u16, *h, &fmt.fh)?;
    }

    // 检测分组（按实验室、项目代号分组）
    let mut groups: Vec<(u32, u32, u32, u32, String)> = vec![]; // (start, end, lc_count, gc_count, project_code)
    let mut i = 0usize;
    while i < rows.len() {
        let (ref_lab, ref_proj, _, _, _, _, _, _, _) = &rows[i];
        let start = HR + 1 + i as u32;
        let mut end = start;
        let mut lc = 0u32;
        let mut gc = 0u32;

        while i < rows.len() {
            let (lab, proj, _, _, _, _, is_gc, _, _) = &rows[i];
            if lab != ref_lab || proj != ref_proj {
                break;
            }
            if *is_gc {
                gc += 1;
            } else {
                lc += 1;
            }
            end = HR + 1 + i as u32;
            i += 1;
        }
        groups.push((start, end, lc, gc, ref_proj.clone()));
    }

    // 写数据
    let mut row_idx = HR + 1;
    for (lab, proj, inst, method, _, qty, _, _, high_item) in rows {
        ws.write_with_format(row_idx, CA, lab.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CB, proj.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CC, inst.as_str(), &fmt.fd)?;
        ws.write_with_format(row_idx, CD, method.as_str(), &fmt.fd)?;
        if *qty > 0 {
            ws.write_with_format(row_idx, CE, *qty as f64, &fmt.fd)?;
        }
        ws.write_with_format(row_idx, CI, high_item.as_deref().unwrap_or("-"), &fmt.fd)?;
        row_idx += 1;
    }

    // 合并单元格（实验室、项目代号、汇总列）
    for &(start, end, _, _, ref proj_code) in &groups {
        if start == end {
            continue;
        }
        ws.merge_range(start, CB, end, CB, proj_code.as_str(), &fmt.fd)?;
        for col in [CF, CG, CH] {
            ws.merge_range(start, col, end, col, "", &fmt.fd)?;
        }
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
    for &(start, end, lc_cnt, gc_cnt, _) in &groups {
        let excel_start = start + 1; // Excel行号从1开始
        let excel_end = end + 1;

        // 液相检测量
        if lc_cnt > 0 {
            let lc_last = excel_start + lc_cnt - 1;
            ws.write_formula(
                start,
                CF,
                format!(
                    "=SUM({}{}:{}{})",
                    col_letter(CE),
                    excel_start,
                    col_letter(CE),
                    lc_last
                )
                .as_str(),
            )?;
        }

        // 气相检测量
        if gc_cnt > 0 {
            let gc_start = excel_start + lc_cnt;
            ws.write_formula(
                start,
                CG,
                format!(
                    "=SUM({}{}:{}{})",
                    col_letter(CE),
                    gc_start,
                    col_letter(CE),
                    excel_end
                )
                .as_str(),
            )?;
        }

        // 项目检测总量
        ws.write_formula(
            start,
            CH,
            format!(
                "={}{}+{}{}",
                col_letter(CF),
                excel_start,
                col_letter(CG),
                excel_start
            )
            .as_str(),
        )?;
    }

    // 总计行
    let total_row = row_idx;
    ws.write_with_format(total_row, CA, "总计", &fmt.fb)?;
    for col in [CB, CC, CD] {
        ws.write_with_format(total_row, col, "", &fmt.fb)?;
    }
    for col in [CE, CF, CG, CH] {
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
        "项目",
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
    ws.set_name("项目-汇总")
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
    ws.set_name("实验室-汇总")
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
    ws.set_name(&format!("{}汇总", person_label))
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

// ========== Sheet 6: 人员汇总表 ==========

pub fn write_sheet6(
    ws: &mut Worksheet,
    rows: &[PersonSummaryRow],
    fmt: &Fmt,
    person_label: &str,
) -> Result<()> {
    ws.set_name(&format!("{}汇总表", person_label))
        .map_err(|e| AppError::Internal(e.to_string()))?;
    ws.set_tab_color(Color::RGB(0x00BCD4));

    ws.set_column_width(CA, 12.0)?; // 人员
    ws.set_column_width(CB, 10.0)?; // 液相数量
    ws.set_column_width(CC, 8.0)?; // 液相系数
    ws.set_column_width(CD, 12.0)?; // 液相汇总
    ws.set_column_width(CE, 10.0)?; // 气相数量
    ws.set_column_width(CF, 8.0)?; // 气相系数
    ws.set_column_width(CG, 12.0)?; // 气相汇总
    ws.set_column_width(CH, 10.0)?; // 理化数量
    ws.set_column_width(CI, 8.0)?; // 理化系数
    ws.set_column_width(CJ, 12.0)?; // 理化汇总
    ws.set_column_width(CK, 15.0)?; // 个人汇总
    ws.set_column_width(CL, 15.0)?; // 总额

    for col in 0u16..=11u16 {
        ws.set_column_format(col, &fmt.fd)?;
    }

    let headers = [
        person_label,
        SHEET6_HEADERS[1],
        SHEET6_HEADERS[2],
        SHEET6_HEADERS[3],
        SHEET6_HEADERS[4],
        SHEET6_HEADERS[5],
        SHEET6_HEADERS[6],
        SHEET6_HEADERS[7],
        SHEET6_HEADERS[8],
        SHEET6_HEADERS[9],
        SHEET6_HEADERS[10],
        SHEET6_HEADERS[11],
    ];
    for (i, h) in headers.iter().enumerate() {
        ws.write_with_format(HR, i as u16, *h, &fmt.fh)?;
    }

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

        // 根据 method_type 填充对应列
        if row.method_type.contains("液相") {
            ws.write_with_format(row_idx, CB, row.quantity as f64, &fmt.fd)?;
            ws.write_with_format(row_idx, CC, row.coefficient, &fmt.fd)?;
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
        } else if row.method_type.contains("气相") {
            ws.write_with_format(row_idx, CE, row.quantity as f64, &fmt.fd)?;
            ws.write_with_format(row_idx, CF, row.coefficient, &fmt.fd)?;
            ws.write_formula(
                row_idx,
                CG,
                format!(
                    "={}{}*{}{}",
                    col_letter(CE),
                    row_idx + 1,
                    col_letter(CF),
                    row_idx + 1
                )
                .as_str(),
            )?;
        } else {
            // 理化/质谱等
            ws.write_with_format(row_idx, CH, row.quantity as f64, &fmt.fd)?;
            ws.write_with_format(row_idx, CI, row.coefficient, &fmt.fd)?;
            ws.write_formula(
                row_idx,
                CJ,
                format!(
                    "={}{}*{}{}",
                    col_letter(CH),
                    row_idx + 1,
                    col_letter(CI),
                    row_idx + 1
                )
                .as_str(),
            )?;
        }

        row_idx += 1;
    }

    if row_idx > HR + 1 {
        user_groups.push((user_start, row_idx - 1, current_user));
    }

    // 合并用户列和 CN/CO 列
    for (start, end, user) in &user_groups {
        if *start == *end {
            continue;
        }
        ws.merge_range(*start, CA, *end, CA, user.as_str(), &fmt.fd)?;
        // 个人汇总：merge 后写入公式
        ws.merge_range(*start, CK, *end, CK, "", &fmt.fd)?;
        let personal_sum = format!(
            "=SUM({}{}:{}{})+SUM({}{}:{}{})+SUM({}{}:{}{})",
            col_letter(CD),
            start + 1,
            col_letter(CD),
            end + 1,
            col_letter(CG),
            start + 1,
            col_letter(CG),
            end + 1,
            col_letter(CJ),
            start + 1,
            col_letter(CJ),
            end + 1
        );
        ws.write_formula(*start, CK, personal_sum.as_str())?;
    }

    // 单类型用户
    for (start, end, _user) in &user_groups {
        if *start != *end {
            continue;
        }
        let single_sum = format!(
            "=IF(ISNUMBER({}{}),{}{},0)+IF(ISNUMBER({}{}),{}{},0)+IF(ISNUMBER({}{}),{}{},0)",
            col_letter(CD),
            start + 1,
            col_letter(CD),
            start + 1,
            col_letter(CG),
            start + 1,
            col_letter(CG),
            start + 1,
            col_letter(CJ),
            start + 1,
            col_letter(CJ),
            start + 1
        );
        ws.write_formula(*start, CK, single_sum.as_str())?;
    }

    // 总额
    if row_idx > HR + 1 {
        ws.write_formula(
            HR + 1,
            CL,
            format!(
                "=SUM({}{}:{}{})",
                col_letter(CK),
                HR + 2,
                col_letter(CK),
                row_idx
            )
            .as_str(),
        )?;
    }

    ws.set_freeze_panes(HR + 1, 1)?;
    Ok(())
}

// ========== Sheet 7: 实验室总表 ==========

pub fn write_sheet7(ws: &mut Worksheet, rows: &[LabTotalRow], fmt: &Fmt) -> Result<()> {
    ws.set_name("实验室总表")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    ws.set_tab_color(Color::RGB(0x4CAF50));

    ws.set_column_width(CA, 14.0)?; // 实验室
    ws.set_column_width(CB, 20.0)?; // 项目
    ws.set_column_width(CC, 10.0)?; // 液相数量
    ws.set_column_width(CD, 10.0)?; // 液相单价
    ws.set_column_width(CE, 8.0)?; // 液相单价倍率
    ws.set_column_width(CF, 12.0)?; // 液相汇总
    ws.set_column_width(CG, 10.0)?; // 气相数量
    ws.set_column_width(CH, 10.0)?; // 气相单价
    ws.set_column_width(CI, 8.0)?; // 气相单价倍率
    ws.set_column_width(CJ, 12.0)?; // 气相汇总
    ws.set_column_width(CK, 10.0)?; // 理化数量
    ws.set_column_width(CL, 10.0)?; // 理化单价
    ws.set_column_width(CM, 8.0)?; // 理化单价倍率
    ws.set_column_width(CN, 12.0)?; // 理化汇总
    ws.set_column_width(CO, 15.0)?; // 项目汇总

    // NOTE: 实验室汇总列 moved to col 15 (CP), but we're limited to 14 (CO).
    // Adding one more column... Let me use a hardcoded 15.
    ws.set_column_width(15, 15.0)?; // 实验室汇总 (CP)

    for col in 0u16..=15u16 {
        ws.set_column_format(col, &fmt.fd)?;
    }

    let headers = SHEET7_HEADERS;
    for (i, h) in headers.iter().enumerate() {
        ws.write_with_format(HR, i as u16, *h, &fmt.fh)?;
    }

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

        // 根据 method_type 填充
        if row.method_type.contains("液相") {
            ws.write_with_format(row_idx, CC, row.quantity as f64, &fmt.fd)?;
            ws.write_with_format(row_idx, CD, row.unit_price, &fmt.fd)?;
            ws.write_with_format(row_idx, CE, row.multiplier, &fmt.fd)?;
            // 液相汇总 = 数量 × 单价 × 单价倍率
            ws.write_formula(
                row_idx,
                CF,
                format!(
                    "={}{}*{}{}*{}{}",
                    col_letter(CC),
                    row_idx + 1,
                    col_letter(CD),
                    row_idx + 1,
                    col_letter(CE),
                    row_idx + 1
                )
                .as_str(),
            )?;
        } else if row.method_type.contains("气相") {
            ws.write_with_format(row_idx, CG, row.quantity as f64, &fmt.fd)?;
            ws.write_with_format(row_idx, CH, row.unit_price, &fmt.fd)?;
            ws.write_with_format(row_idx, CI, row.multiplier, &fmt.fd)?;
            ws.write_formula(
                row_idx,
                CJ,
                format!(
                    "={}{}*{}{}*{}{}",
                    col_letter(CG),
                    row_idx + 1,
                    col_letter(CH),
                    row_idx + 1,
                    col_letter(CI),
                    row_idx + 1
                )
                .as_str(),
            )?;
        } else {
            ws.write_with_format(row_idx, CK, row.quantity as f64, &fmt.fd)?;
            ws.write_with_format(row_idx, CL, row.unit_price, &fmt.fd)?;
            ws.write_with_format(row_idx, CM, row.multiplier, &fmt.fd)?;
            ws.write_formula(
                row_idx,
                CN,
                format!(
                    "={}{}*{}{}*{}{}",
                    col_letter(CK),
                    row_idx + 1,
                    col_letter(CL),
                    row_idx + 1,
                    col_letter(CM),
                    row_idx + 1
                )
                .as_str(),
            )?;
        }

        // 项目汇总 = 各类型汇总之和（直接用值，三类型必有一个）
        let mut parts: Vec<String> = Vec::new();
        if row.method_type.contains("液相") {
            parts.push(format!(
                "{}{}*{}{}*{}{}",
                col_letter(CC),
                row_idx + 1,
                col_letter(CD),
                row_idx + 1,
                col_letter(CE),
                row_idx + 1
            ));
        } else {
            parts.push("0".to_string());
        }
        if row.method_type.contains("气相") {
            parts.push(format!(
                "{}{}*{}{}*{}{}",
                col_letter(CG),
                row_idx + 1,
                col_letter(CH),
                row_idx + 1,
                col_letter(CI),
                row_idx + 1
            ));
        } else {
            parts.push("0".to_string());
        }
        if !row.method_type.contains("液相") && !row.method_type.contains("气相") {
            parts.push(format!(
                "{}{}*{}{}*{}{}",
                col_letter(CK),
                row_idx + 1,
                col_letter(CL),
                row_idx + 1,
                col_letter(CM),
                row_idx + 1
            ));
        } else {
            parts.push("0".to_string());
        }
        ws.write_formula(
            row_idx,
            CO,
            format!("={}+{}+{}", parts[0], parts[1], parts[2]).as_str(),
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
        ws.merge_range(start, 15, end, 15, "", &fmt.fd)?;
        ws.write_formula(
            start,
            15,
            format!(
                "=SUM({}{}:{}{})",
                col_letter(CO),
                start + 1,
                col_letter(CO),
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

    ws.set_column_width(CA, 20.0)?; // 项目
    ws.set_column_width(CB, 10.0)?; // 液相数量
    ws.set_column_width(CC, 10.0)?; // 液相单价
    ws.set_column_width(CD, 8.0)?; // 液相单价倍率
    ws.set_column_width(CE, 12.0)?; // 液相汇总
    ws.set_column_width(CF, 10.0)?; // 气相数量
    ws.set_column_width(CG, 10.0)?; // 气相单价
    ws.set_column_width(CH, 8.0)?; // 气相单价倍率
    ws.set_column_width(CI, 12.0)?; // 气相汇总
    ws.set_column_width(CJ, 10.0)?; // 理化数量
    ws.set_column_width(CK, 10.0)?; // 理化单价
    ws.set_column_width(CL, 8.0)?; // 理化单价倍率
    ws.set_column_width(CM, 12.0)?; // 理化汇总
    ws.set_column_width(CN, 15.0)?; // 项目汇总

    for col in 0u16..=13u16 {
        ws.set_column_format(col, &fmt.fd)?;
    }

    let headers = SHEET8_HEADERS;
    for (i, h) in headers.iter().enumerate() {
        ws.write_with_format(HR, i as u16, *h, &fmt.fh)?;
    }

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

        if row.method_type.contains("液相") {
            ws.write_with_format(row_idx, CB, row.quantity as f64, &fmt.fd)?;
            ws.write_with_format(row_idx, CC, row.unit_price, &fmt.fd)?;
            ws.write_with_format(row_idx, CD, row.multiplier, &fmt.fd)?;
            ws.write_formula(
                row_idx,
                CE,
                format!(
                    "={}{}*{}{}*{}{}",
                    col_letter(CB),
                    row_idx + 1,
                    col_letter(CC),
                    row_idx + 1,
                    col_letter(CD),
                    row_idx + 1
                )
                .as_str(),
            )?;
        } else if row.method_type.contains("气相") {
            ws.write_with_format(row_idx, CF, row.quantity as f64, &fmt.fd)?;
            ws.write_with_format(row_idx, CG, row.unit_price, &fmt.fd)?;
            ws.write_with_format(row_idx, CH, row.multiplier, &fmt.fd)?;
            ws.write_formula(
                row_idx,
                CI,
                format!(
                    "={}{}*{}{}*{}{}",
                    col_letter(CF),
                    row_idx + 1,
                    col_letter(CG),
                    row_idx + 1,
                    col_letter(CH),
                    row_idx + 1
                )
                .as_str(),
            )?;
        } else {
            ws.write_with_format(row_idx, CJ, row.quantity as f64, &fmt.fd)?;
            ws.write_with_format(row_idx, CK, row.unit_price, &fmt.fd)?;
            ws.write_with_format(row_idx, CL, row.multiplier, &fmt.fd)?;
            ws.write_formula(
                row_idx,
                CM,
                format!(
                    "={}{}*{}{}*{}{}",
                    col_letter(CJ),
                    row_idx + 1,
                    col_letter(CK),
                    row_idx + 1,
                    col_letter(CL),
                    row_idx + 1
                )
                .as_str(),
            )?;
        }

        // 项目汇总 = 各类型汇总之和
        let mut parts: Vec<String> = Vec::new();
        if row.method_type.contains("液相") {
            parts.push(format!(
                "{}{}*{}{}*{}{}",
                col_letter(CB),
                row_idx + 1,
                col_letter(CC),
                row_idx + 1,
                col_letter(CD),
                row_idx + 1
            ));
        } else {
            parts.push("0".to_string());
        }
        if row.method_type.contains("气相") {
            parts.push(format!(
                "{}{}*{}{}*{}{}",
                col_letter(CF),
                row_idx + 1,
                col_letter(CG),
                row_idx + 1,
                col_letter(CH),
                row_idx + 1
            ));
        } else {
            parts.push("0".to_string());
        }
        if !row.method_type.contains("液相") && !row.method_type.contains("气相") {
            parts.push(format!(
                "{}{}*{}{}*{}{}",
                col_letter(CJ),
                row_idx + 1,
                col_letter(CK),
                row_idx + 1,
                col_letter(CL),
                row_idx + 1
            ));
        } else {
            parts.push("0".to_string());
        }
        ws.write_formula(
            row_idx,
            CN,
            format!("={}+{}+{}", parts[0], parts[1], parts[2]).as_str(),
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
        ws.merge_range(*start, CN, *end, CN, "", &fmt.fd)?;
        ws.write_formula(
            *start,
            CN,
            format!(
                "=SUM({}{}:{}{})+SUM({}{}:{}{})+SUM({}{}:{}{})",
                col_letter(CE),
                start + 1,
                col_letter(CE),
                end + 1,
                col_letter(CI),
                start + 1,
                col_letter(CI),
                end + 1,
                col_letter(CM),
                start + 1,
                col_letter(CM),
                end + 1
            )
            .as_str(),
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

    use std::collections::HashMap;
    let mut type_totals: HashMap<String, i64> = HashMap::new();
    for row in rows {
        *type_totals.entry(row.instrument_type.clone()).or_insert(0) += row.quantity;
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

    for &(start, _, ref itype) in &type_groups {
        if let Some(&total) = type_totals.get(itype) {
            ws.write_with_format(start, CD, total as f64, &fmt.fd)?;
        }
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
    fn personnel_headers_describe_workload_not_money() {
        assert_eq!(SHEET6_HEADERS[3], "液相工作量");
        assert_eq!(SHEET6_HEADERS[6], "气相工作量");
        assert_eq!(SHEET6_HEADERS[9], "理化工作量");
        assert_eq!(SHEET6_HEADERS[10], "个人工作量汇总");
        assert_eq!(SHEET6_HEADERS[11], "总工作量");
        assert!(!SHEET6_HEADERS.iter().any(|header| header.contains("金额")));
    }
}
