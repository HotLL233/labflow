use calamine::{open_workbook_from_rs, DataType, Reader};

pub struct XlsxResult {
    pub html: String,
}

/// 简单 HTML 转义
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// 将 Excel 单元格值转为字符串（复用 import_handler 相同逻辑）
fn cell_to_string(cell: &DataType) -> String {
    match cell {
        DataType::String(s) => s.trim().to_string(),
        DataType::Float(f) => {
            if *f > 40000.0 && *f < 60000.0 && f.fract() == 0.0 {
                excel_serial_to_date(*f)
            } else if f.fract() == 0.0 {
                format!("{}", *f as i64)
            } else {
                format!("{}", f)
            }
        }
        DataType::DateTime(f) => excel_serial_to_date(*f),
        DataType::Int(i) => format!("{}", i),
        DataType::Bool(b) => format!("{}", b),
        DataType::DateTimeIso(s) => s.trim().to_string(),
        DataType::Duration(f) => format!("{}", f),
        DataType::DurationIso(s) => s.trim().to_string(),
        DataType::Empty => String::new(),
        DataType::Error(_) => String::new(),
    }
}

fn excel_serial_to_date(serial: f64) -> String {
    let days = (serial as i64) - 2;
    let epoch = chrono::NaiveDate::from_ymd_opt(1899, 12, 30)
        .unwrap_or(chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());
    let date = epoch + chrono::Duration::days(days);
    date.format("%Y-%m-%d").to_string()
}

pub fn parse_xlsx(data: &[u8]) -> Result<XlsxResult, String> {
    let cursor = std::io::Cursor::new(data);
    let mut workbook: calamine::Xlsx<_> =
        open_workbook_from_rs(cursor).map_err(|e| format!("无法打开 Excel 文件: {}", e))?;

    let sheet_names = workbook.sheet_names().to_vec();
    let mut html_parts: Vec<String> = Vec::new();

    for name in &sheet_names {
        if let Ok(range) = workbook.worksheet_range(name) {
            if range.is_empty() {
                continue;
            }

            let total_cols: usize = range.rows().map(|row| row.len()).max().unwrap_or(0);
            if total_cols == 0 {
                continue;
            }

            let mut table_html = String::new();
            if sheet_names.len() > 1 {
                table_html.push_str(&format!("<h2>Sheet: {}</h2>", escape_html(name)));
            }
            table_html.push_str("<table class=\"xlsx-table\" style=\"border-collapse:collapse;width:100%;margin-bottom:16px;\">");

            for (ri, row) in range.rows().enumerate() {
                if ri == 0 {
                    table_html.push_str("<thead><tr>");
                    for cell in row {
                        let val = escape_html(&cell_to_string(cell));
                        table_html.push_str(&format!(
                            "<th style=\"border:1px solid #ccc;padding:6px 10px;background:#f5f5f5;font-weight:600;text-align:left;\">{}</th>", val
                        ));
                    }
                    for _ in row.len()..total_cols {
                        table_html.push_str("<th style=\"border:1px solid #ccc;padding:6px 10px;background:#f5f5f5;\"></th>");
                    }
                    table_html.push_str("</tr></thead><tbody>");
                } else {
                    table_html.push_str("<tr>");
                    for cell in row {
                        let val = escape_html(&cell_to_string(cell));
                        table_html.push_str(&format!(
                            "<td style=\"border:1px solid #ccc;padding:4px 10px;\">{}</td>",
                            val
                        ));
                    }
                    for _ in row.len()..total_cols {
                        table_html.push_str(
                            "<td style=\"border:1px solid #ccc;padding:4px 10px;\"></td>",
                        );
                    }
                    table_html.push_str("</tr>");
                }
            }
            table_html.push_str("</tbody></table>");
            html_parts.push(table_html);
        } else {
            tracing::warn!("无法读取 Sheet: {}", name);
        }
    }

    Ok(XlsxResult {
        html: html_parts.join("\n"),
    })
}
