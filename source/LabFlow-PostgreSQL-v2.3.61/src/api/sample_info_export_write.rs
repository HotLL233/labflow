use super::export_write::Fmt;
use super::sample_info_export_data::*;
use crate::error::{AppError, Result};
use crate::models::sample_info_column::SampleInfoColumn;
/// 样品信息登记导出 — Excel 写入层（独立模块，复用 export_write::Fmt 样式）
/// 根据列配置动态生成表头和数据行
use rust_xlsxwriter::*;

/// Sheet 1: 全部记录明细（动态列配置）
pub fn write_sheet_detail(
    ws: &mut Worksheet,
    rows: &[SampleInfoExportRow],
    columns: &[SampleInfoColumn],
    fmt: &Fmt,
) -> Result<()> {
    ws.set_name("记录明细")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    ws.set_tab_color(Color::RGB(0x2E7D32));

    // 只取 show_in_export=true 的列
    let active_cols: Vec<&SampleInfoColumn> = columns.iter().filter(|c| c.show_in_export).collect();

    // 设置列宽
    for (i, col) in active_cols.iter().enumerate() {
        let w = (col.width as f64).max(8.0) / 10.0;
        ws.set_column_width(i as u16, w)?;
        ws.set_column_format(i as u16, &fmt.fd)?;
        // 写表头
        ws.write_with_format(0u32, i as u16, col.label.as_str(), &fmt.fh)?;
    }

    let mut r = 1u32;
    for row in rows {
        for (i, col) in active_cols.iter().enumerate() {
            let val = get_field_value(row, &col.field_key);
            ws.write_with_format(r, i as u16, &val, &fmt.fd)?;
        }
        r += 1;
    }
    Ok(())
}

/// 从 export row 中提取字段值（预置字段 + extra_fields 中的自定义字段）
fn get_field_value(row: &SampleInfoExportRow, field_key: &str) -> String {
    match field_key {
        "seq_no" => row.seq_no.to_string(),
        "batch_no" => row.batch_no.clone(),
        "user_name" => row.user_name.clone(),
        "lab_name" => row.lab_name.clone(),
        "project_name" => row.project_name.clone(),
        "submitted_at" => row.submitted_at.clone(),
        "detection_date" => row.detection_date.clone(),
        "sampled_by" => row.sampled_by.clone(),
        "sampled_at" => row.sampled_at.clone(),
        "detected_by" => row.detected_by.clone(),
        "detection_type" => row.detection_type.clone(),
        "status" => row.status.clone(),
        "main_components" => row.main_components.clone(),
        "notes" => row.notes.clone(),
        _ => {
            // 自定义字段：从 extra_fields JSON 中提取
            if let Some(ref ef) = row.extra_fields {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(ef) {
                    if let Some(v) = val.get(field_key) {
                        return match v {
                            serde_json::Value::String(s) => s.clone(),
                            serde_json::Value::Number(n) => n.to_string(),
                            serde_json::Value::Bool(b) => b.to_string(),
                            serde_json::Value::Null => String::new(),
                            other => other.to_string(),
                        };
                    }
                }
            }
            String::new()
        }
    }
}

/// 通用：写「名称-数量」型汇总 Sheet
fn write_name_count(
    ws: &mut Worksheet,
    title: &str,
    tab_color: u32,
    col_name: &str,
    rows: &[NameCountRow],
    fmt: &Fmt,
) -> Result<()> {
    ws.set_name(title)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    ws.set_tab_color(Color::RGB(tab_color));
    ws.set_column_width(0, 24.0)?;
    ws.set_column_width(1, 12.0)?;
    ws.set_column_format(0, &fmt.fd)?;
    ws.set_column_format(1, &fmt.fd)?;
    ws.write_with_format(0u32, 0, col_name, &fmt.fh)?;
    ws.write_with_format(0u32, 1, "数量", &fmt.fh)?;
    let mut r = 1u32;
    for row in rows {
        ws.write_with_format(r, 0, row.name.as_str(), &fmt.fd)?;
        ws.write_with_format(r, 1, row.count as f64, &fmt.fd)?;
        r += 1;
    }
    Ok(())
}

/// Sheet 2: 按状态
pub fn write_sheet_by_status(ws: &mut Worksheet, rows: &[NameCountRow], fmt: &Fmt) -> Result<()> {
    write_name_count(ws, "按状态", 0xE65100, "状态", rows, fmt)
}

/// Sheet 3: 按检测类型
pub fn write_sheet_by_type(ws: &mut Worksheet, rows: &[TypeCountRow], fmt: &Fmt) -> Result<()> {
    ws.set_name("按检测类型")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    ws.set_tab_color(Color::RGB(0x6A1B9A));
    ws.set_column_width(0, 24.0)?;
    ws.set_column_width(1, 18.0)?;
    ws.set_column_width(2, 12.0)?;
    ws.set_column_format(0, &fmt.fd)?;
    ws.set_column_format(1, &fmt.fd)?;
    ws.set_column_format(2, &fmt.fd)?;
    ws.write_with_format(0u32, 0, "检测类型", &fmt.fh)?;
    ws.write_with_format(0u32, 1, "类型标识", &fmt.fh)?;
    ws.write_with_format(0u32, 2, "数量", &fmt.fh)?;
    let mut r = 1u32;
    for row in rows {
        ws.write_with_format(r, 0, row.label.as_str(), &fmt.fd)?;
        ws.write_with_format(r, 1, row.type_key.as_str(), &fmt.fd)?;
        ws.write_with_format(r, 2, row.count as f64, &fmt.fd)?;
        r += 1;
    }
    Ok(())
}

/// Sheet 4: 按实验室
pub fn write_sheet_by_lab(ws: &mut Worksheet, rows: &[NameCountRow], fmt: &Fmt) -> Result<()> {
    write_name_count(ws, "按实验室", 0x0277BD, "实验室/车间", rows, fmt)
}

/// Sheet 5: 按项目
pub fn write_sheet_by_project(ws: &mut Worksheet, rows: &[NameCountRow], fmt: &Fmt) -> Result<()> {
    write_name_count(ws, "按项目", 0x00897B, "所属项目", rows, fmt)
}

/// Sheet 6: 按送样人
pub fn write_sheet_by_user(ws: &mut Worksheet, rows: &[NameCountRow], fmt: &Fmt) -> Result<()> {
    write_name_count(ws, "按送样人", 0x558B2F, "送样人", rows, fmt)
}

/// Sheet 7: 按月份
pub fn write_sheet_by_month(ws: &mut Worksheet, rows: &[MonthCountRow], fmt: &Fmt) -> Result<()> {
    ws.set_name("按月份")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    ws.set_tab_color(Color::RGB(0x5E35B1));
    ws.set_column_width(0, 16.0)?;
    ws.set_column_width(1, 12.0)?;
    ws.set_column_format(0, &fmt.fd)?;
    ws.set_column_format(1, &fmt.fd)?;
    ws.write_with_format(0u32, 0, "月份", &fmt.fh)?;
    ws.write_with_format(0u32, 1, "数量", &fmt.fh)?;
    let mut r = 1u32;
    for row in rows {
        ws.write_with_format(r, 0, row.month.as_str(), &fmt.fd)?;
        ws.write_with_format(r, 1, row.count as f64, &fmt.fd)?;
        r += 1;
    }
    Ok(())
}
