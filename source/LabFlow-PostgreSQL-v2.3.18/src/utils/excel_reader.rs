use crate::error::AppError;
use crate::models::import_record::ImportRecord;
use calamine::{open_workbook, DataType, Reader, Xlsx};
use regex::Regex;

// Keywords for column detection
const DATE_KEYWORDS: &[&str] = &[
    "日期",
    "date",
    "时间",
    "time",
    "检测日期",
    "采样日期",
    "报告日期",
    "分析日期",
    "送检日期",
    "检验日期",
];
const BATCH_KEYWORDS: &[&str] = &[
    "批号",
    "batch",
    "lot",
    "编号",
    "序号",
    "样品批号",
    "样品编号",
    "样品号",
    "样品名称",
    "实验编号",
    "样本号",
    "code",
    "id",
];
const USER_KEYWORDS: &[&str] = &[
    "录入人",
    "user",
    "操作人",
    "实验员",
    "送样人",
    "检测人",
    "人员",
    "user_name",
];
const QTY_KEYWORDS: &[&str] = &[
    "数量",
    "qty",
    "quantity",
    "件数",
    "个数",
    "检测数量",
    "送样量",
];
const PROJECT_KEYWORDS: &[&str] = &[
    "项目",
    "project",
    "方法",
    "project_name",
    "检测项目",
    "送样项目",
];
const GROUP_KEYWORDS: &[&str] = &["实验室", "group", "分组", "group_name"];

const SKIP_ROWS: usize = 2;

/// Excel解析结果：包含记录列表、表头检测信息和行统计
#[derive(Debug, Clone)]
pub struct ParseResult {
    pub records: Vec<ImportRecord>,
    pub sheet_name: String,
    pub columns_found: Vec<String>,
    pub total_rows_read: usize,
    pub skipped_rows: usize,
}

// ===================== 日志 =====================

fn app_log(msg: String) {
    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    tracing::info!("[{}] {}", ts, msg);
}

// ===================== 日期/数值处理 =====================

fn year_ok(d: &chrono::NaiveDate) -> bool {
    let y: i32 = d.format("%Y").to_string().parse().unwrap_or(0);
    y >= 1900 && y <= 2100
}

fn try_parse_yyyymmdd(s: &str) -> Option<String> {
    let clean = if let Some(dot) = s.find('.') {
        &s[..dot]
    } else {
        s
    };
    if clean.len() == 8 && clean.chars().all(|c| c.is_ascii_digit()) {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(clean, "%Y%m%d") {
            if year_ok(&d) {
                return Some(d.format("%Y-%m-%d").to_string());
            }
        }
    }
    None
}

fn ensure_date_str(v: &DataType) -> String {
    // Try as string
    if let Some(s) = v.as_string() {
        let s = s.trim().to_string();
        if s.is_empty() {
            return String::new();
        }
        // YYYYMMDD string
        if let Some(d) = try_parse_yyyymmdd(&s) {
            return d;
        }
        // Common formats
        for fmt in &["%Y-%m-%d", "%Y/%m/%d", "%Y.%m.%d"] {
            if let Ok(d) = chrono::NaiveDate::parse_from_str(&s, fmt) {
                if year_ok(&d) {
                    return d.format("%Y-%m-%d").to_string();
                }
            }
        }
        return String::new();
    }
    // Try as float
    if let Some(f) = v.as_f64() {
        let s = format!("{:.0}", f);
        // YYYYMMDD integer → "20250101"
        if let Some(d) = try_parse_yyyymmdd(&s) {
            return d;
        }
        // Excel serial date
        if f >= 40000.0 && f <= 200000.0 {
            if let Some(d) = chrono::NaiveDate::from_ymd_opt(1899, 12, 30)
                .unwrap()
                .checked_add_signed(chrono::Duration::days(f as i64))
            {
                if year_ok(&d) {
                    return d.format("%Y-%m-%d").to_string();
                }
            }
        }
        return String::new();
    }
    String::new()
}

fn get_as_string(v: &DataType) -> String {
    if let Some(s) = v.as_string() {
        return s.trim().to_string();
    }
    if let Some(f) = v.as_f64() {
        if f.fract() == 0.0 && f >= 0.0 {
            return format!("{:.0}", f);
        }
        return f.to_string();
    }
    if let Some(i) = v.as_i64() {
        return i.to_string();
    }
    String::new()
}

// ===================== 检测逻辑（与 rust-tool 一致） =====================

fn detect_data_start_row(range: &calamine::Range<DataType>, max_scan: usize) -> usize {
    let re = Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
    let ncols = (range.width() as usize).min(30);
    let nrows = (range.height() as usize).min(max_scan);
    for row_idx in 0..nrows {
        for col_idx in 0..ncols {
            if let Some(val) = range.get((row_idx, col_idx)) {
                if re.is_match(&ensure_date_str(val)) {
                    return row_idx;
                }
            }
        }
    }
    SKIP_ROWS
}

fn detect_columns(
    range: &calamine::Range<DataType>,
    header_end: usize,
) -> (usize, usize, usize, usize, usize, usize) {
    let max_header_scan: usize = 3;
    let mut date_col = 0usize;
    let mut batch_col = 1usize;
    let mut user_col = usize::MAX;
    let mut qty_col = usize::MAX;
    let mut proj_col = usize::MAX;
    let mut group_col = usize::MAX;

    let h = header_end.min(range.height() as usize);
    let w = (range.width() as usize).min(30);
    if h == 0 || w < 2 {
        return (date_col, batch_col, user_col, qty_col, proj_col, group_col);
    }

    let scan_start = if h > max_header_scan {
        h - max_header_scan
    } else {
        0
    };
    for row_idx in (scan_start..h).rev() {
        for col_idx in 0..w {
            if let Some(val) = range.get((row_idx, col_idx)) {
                let s = val.as_string().unwrap_or_default().to_lowercase();
                if s.trim().is_empty() {
                    continue;
                }

                if date_col == 0 && DATE_KEYWORDS.iter().any(|kw| s.contains(kw)) {
                    date_col = col_idx;
                }
                if batch_col == 1
                    && BATCH_KEYWORDS.iter().any(|kw| s.contains(kw))
                    && col_idx != date_col
                {
                    batch_col = col_idx;
                }
                if user_col == usize::MAX && USER_KEYWORDS.iter().any(|kw| s.contains(kw)) {
                    user_col = col_idx;
                }
                if qty_col == usize::MAX
                    && QTY_KEYWORDS.iter().any(|kw| s.contains(kw))
                    && col_idx != date_col
                    && col_idx != batch_col
                {
                    qty_col = col_idx;
                }
                if proj_col == usize::MAX
                    && PROJECT_KEYWORDS.iter().any(|kw| s.contains(kw))
                    && !matches!(col_idx, 0 | 1)
                {
                    proj_col = col_idx;
                }
                if group_col == usize::MAX
                    && GROUP_KEYWORDS.iter().any(|kw| s.contains(kw))
                    && !matches!(col_idx, 0 | 1)
                {
                    group_col = col_idx;
                }
            }
        }
    }
    (date_col, batch_col, user_col, qty_col, proj_col, group_col)
}

fn process_sheet(range: &calamine::Range<DataType>, sheet_name: &str) -> Option<ParseResult> {
    let auto_skip = detect_data_start_row(range, 50);
    let h = range.height() as usize;
    let w = range.width() as usize;

    app_log(format!(
        "[{}] Auto skip row: {}, Total rows: {}, Cols: {}",
        sheet_name, auto_skip, h, w
    ));

    // 列检测：只扫描表头区域（前 auto_skip 行，从最后 3 行向上）
    let header_rows = auto_skip.min(h);
    let (dc, bc, uc, qc, pc, gc) = if header_rows > 0 {
        detect_columns(range, header_rows)
    } else {
        (
            0usize,
            1usize,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
        )
    };

    app_log(format!(
        "[{}] Detected cols - Date:{}, Batch:{}, User:{}, Qty:{}, Proj:{}, Group:{}",
        sheet_name, dc, bc, uc, qc, pc, gc
    ));

    if dc >= w || bc >= w || qc >= w {
        app_log(format!(
            "[{}] Required column out of range (w={})",
            sheet_name, w
        ));
        return None;
    }

    // 收集检测到的列名
    let mut columns_found = vec![];
    for (idx, label) in &[(dc, "日期"), (bc, "批号"), (qc, "数量")] {
        if *idx < w {
            let v = range
                .get((header_rows.saturating_sub(1), *idx))
                .map(|v| v.as_string().unwrap_or_default().to_string())
                .unwrap_or_default();
            columns_found.push(format!("{}({}):{}", label, idx, v));
        }
    }
    for (idx, label) in &[(uc, "用户"), (pc, "项目"), (gc, "实验室")] {
        if *idx < w {
            let v = range
                .get((header_rows.saturating_sub(1), *idx))
                .map(|v| v.as_string().unwrap_or_default().to_string())
                .unwrap_or_default();
            columns_found.push(format!("{}({}):{}", label, idx, v));
        }
    }

    let re = Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
    let mut records: Vec<ImportRecord> = Vec::new();
    let mut last_date = String::new();
    let prefixes = ["例", "示例", "sample"];
    let mut total_rows_read = 0usize;
    let mut skipped_rows = 0usize;

    for row_idx in auto_skip..h {
        total_rows_read += 1;

        let dv = range
            .get((row_idx, dc))
            .map(|v| ensure_date_str(v))
            .unwrap_or_default();
        let date = if dv.is_empty() {
            last_date.clone()
        } else {
            last_date = dv.clone();
            dv
        };
        if !re.is_match(&date) {
            skipped_rows += 1;
            continue;
        }

        let bv = range
            .get((row_idx, bc))
            .map(|v| get_as_string(v))
            .unwrap_or_default();
        if bv.is_empty() {
            skipped_rows += 1;
            continue;
        }
        if prefixes.iter().any(|p| bv.starts_with(p)) {
            skipped_rows += 1;
            continue;
        }

        let qty_val = range
            .get((row_idx, qc))
            .map(|v| {
                if let Some(i) = v.as_i64() {
                    i
                } else if let Some(f) = v.as_f64() {
                    f as i64
                } else if let Some(s) = v.as_string() {
                    s.trim().parse().unwrap_or(0)
                } else {
                    0
                }
            })
            .unwrap_or(0);

        if qty_val <= 0 {
            skipped_rows += 1;
            continue;
        }

        let project_name = if pc < w {
            let v = range
                .get((row_idx, pc))
                .map(|v| get_as_string(v))
                .unwrap_or_default();
            if v.is_empty() {
                "未分类".to_string()
            } else {
                v
            }
        } else {
            "未分类".to_string()
        };

        let group_name = if gc < w {
            let v = range
                .get((row_idx, gc))
                .map(|v| get_as_string(v))
                .unwrap_or_default();
            if v.is_empty() {
                "默认".to_string()
            } else {
                v
            }
        } else {
            "默认".to_string()
        };

        let user_name = if uc < w {
            let v = range
                .get((row_idx, uc))
                .map(|v| get_as_string(v))
                .unwrap_or_default();
            if !v.is_empty() {
                Some(v)
            } else {
                None
            }
        } else {
            None
        };

        records.push(ImportRecord {
            project_name,
            group_name,
            recorded_at: date,
            batch_no: bv,
            quantity: qty_val,
            user_name,
            extra_info: None,
        });
    }

    app_log(format!(
        "[{}] Parsed {} records, skipped {} rows (total {} read)",
        sheet_name,
        records.len(),
        skipped_rows,
        total_rows_read
    ));

    if records.is_empty() {
        None
    } else {
        Some(ParseResult {
            records,
            sheet_name: sheet_name.to_string(),
            columns_found,
            total_rows_read,
            skipped_rows,
        })
    }
}

// ===================== 主函数 =====================

pub fn read_excel(file_path: &str) -> Result<ParseResult, AppError> {
    let mut workbook: Xlsx<_> = open_workbook(file_path)
        .map_err(|e| AppError::Validation(format!("无法打开Excel文件: {}", e)))?;

    let sheet_names = workbook.sheet_names().to_vec();
    if sheet_names.is_empty() {
        return Err(AppError::Validation("Excel文件中没有工作表".into()));
    }

    // Process first sheet only (aligned with rust-tool behavior)
    let sheet_name = &sheet_names[0];
    app_log(format!("Processing sheet: {}", sheet_name));

    let range = workbook
        .worksheet_range(sheet_name)
        .map_err(|e| AppError::Validation(format!("读取工作表失败: {}", e)))?;

    let result = process_sheet(&range, sheet_name).ok_or_else(|| {
        // 提供更详细的错误信息
        let h = range.height() as usize;
        let w = (range.width() as usize).min(6);
        let preview: Vec<String> = (0..h.min(5))
            .map(|ri| {
                (0..w)
                    .filter_map(|ci| {
                        range
                            .get((ri, ci))
                            .and_then(|v| v.as_string())
                            .map(|s| s.to_string())
                    })
                    .collect::<Vec<_>>()
                    .join(" | ")
            })
            .collect();

        AppError::Validation(format!(
            "未能从Excel中解析到有效数据。\nSheet: {}, 总行数: {}\n前5行预览:\n{}",
            sheet_name,
            h,
            preview.join("\n")
        ))
    })?;

    if result.records.is_empty() {
        return Err(AppError::Validation("无有效数据，请检查Excel格式".into()));
    }

    app_log(format!(
        "Successfully parsed {} records from sheet '{}'",
        result.records.len(),
        sheet_name
    ));
    Ok(result)
}
