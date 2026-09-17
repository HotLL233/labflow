use crate::db::import;
use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::{
    import_record::{ImportRecord, ImportResult},
    ApiResponse,
};
use crate::service::authz_service;
use axum::response::IntoResponse;
use axum::{
    extract::{Multipart, State},
    http::HeaderMap,
    routing::post,
    Router,
};
use calamine::{open_workbook_auto, DataType, Reader};

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/import/excel", post(import_excel))
        .with_state(pool)
}

// ===================== 辅助函数 =====================

/// 将 Excel 单元格值转为字符串
fn cell_to_string(cell: &DataType) -> String {
    match cell {
        DataType::String(s) => s.trim().to_string(),
        DataType::Float(f) => {
            // 尝试判断是否为 Excel 日期序列号(范围约 40000-60000 对应 2009-2064)
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

/// Excel 序列号 → YYYY-MM-DD 字符串
fn excel_serial_to_date(serial: f64) -> String {
    // Excel 日期基准: 1899-12-30, 但 Excel 有 1900-02-29 bug (多算1天)
    let days = (serial as i64) - 2;
    let epoch = chrono::NaiveDate::from_ymd_opt(1899, 12, 30)
        .unwrap_or(chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());
    let date = epoch + chrono::Duration::days(days);
    date.format("%Y-%m-%d").to_string()
}

// ===================== 主处理函数 =====================

async fn import_excel(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<impl IntoResponse> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "manage:data-governance:import")?;
    let mut file_path: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Validation(format!("上传错误: {}", e)))?
    {
        if field.name() == Some("file") {
            let data = field
                .bytes()
                .await
                .map_err(|e| AppError::Validation(format!("读取文件失败: {}", e)))?;
            if data.is_empty() {
                return Err(AppError::Validation("上传文件为空".into()));
            }
            let tmp = std::env::temp_dir().join(format!("import_{}.xlsx", uuid::Uuid::new_v4()));
            std::fs::write(&tmp, &data)
                .map_err(|e| AppError::Internal(format!("临时文件写入失败: {}", e)))?;
            file_path = Some(tmp.to_str().unwrap().to_string());
        }
    }

    let path = file_path.ok_or(AppError::Validation(
        "未收到文件，请选择 .xlsx 文件上传".into(),
    ))?;

    // ===== 使用 calamine 直接读取 xlsx =====
    tracing::info!("Reading Excel with calamine: {}", &path);

    let mut workbook = open_workbook_auto(&path)
        .map_err(|e| AppError::Validation(format!("无法打开 Excel 文件: {}", e)))?;

    let sheet_names = workbook.sheet_names().to_vec();
    if sheet_names.is_empty() {
        std::fs::remove_file(&path).ok();
        return Err(AppError::Validation("Excel 文件中没有工作表".into()));
    }

    let mut records: Vec<ImportRecord> = vec![];
    let mut sheets_info: Vec<String> = vec![];
    let mut total_read = 0usize;
    let mut total_skipped = 0usize;

    for sheet_name in &sheet_names {
        let range = workbook.worksheet_range(sheet_name).map_err(|e| {
            AppError::Validation(format!("读取工作表 '{}' 失败: {}", sheet_name, e))
        })?;

        let mut rows = range.rows();
        let headers: Vec<String> = match rows.next() {
            Some(header_row) => header_row.iter().map(|c| cell_to_string(c)).collect(),
            None => {
                sheets_info.push(format!("{}:空工作表(无表头)", sheet_name));
                continue;
            }
        };

        // 查找日期列和批号列
        let date_col = headers.iter().position(|h| {
            h.contains("日期") || h.to_lowercase().contains("date") || h.contains("时间")
        });
        let batch_no_col = headers.iter().position(|h| {
            h.contains("批号")
                || h.to_lowercase().contains("batch")
                || h.contains("编号")
                || h.contains("样本号")
        });

        let (date_idx, batch_idx) = match (date_col, batch_no_col) {
            (Some(d), Some(b)) => (d, b),
            _ => {
                let found = headers.join(", ");
                sheets_info.push(format!(
                    "{}:无法识别列(需要「日期」和「批号」列, 检测到: {})",
                    sheet_name, found
                ));
                continue;
            }
        };

        let mut sheet_read = 0usize;
        let mut sheet_parsed = 0usize;
        let mut sheet_skipped = 0usize;

        for row in rows {
            sheet_read += 1;
            let date_str = row
                .get(date_idx)
                .map(|c| cell_to_string(c))
                .unwrap_or_default();
            let batch_str = row
                .get(batch_idx)
                .map(|c| cell_to_string(c))
                .unwrap_or_default();

            // 跳过空行和示例行
            if date_str.is_empty()
                || batch_str.is_empty()
                || date_str.contains("示例")
                || batch_str.contains("示例")
            {
                sheet_skipped += 1;
                continue;
            }

            records.push(ImportRecord {
                project_name: "通用项目".to_string(),
                group_name: sheet_name.clone(),
                recorded_at: date_str,
                batch_no: batch_str,
                quantity: 1,
                user_name: None,
                extra_info: None,
            });
            sheet_parsed += 1;
        }

        sheets_info.push(format!(
            "{}:解析{}行/跳过{}行",
            sheet_name, sheet_parsed, sheet_skipped
        ));
        total_read += sheet_read;
        total_skipped += sheet_skipped;
    }

    // Clean up temp file
    std::fs::remove_file(&path).ok();

    if records.is_empty() {
        return Err(AppError::Validation(format!(
            "Excel 中无有效数据。已扫描 {} 个工作表: {}",
            sheet_names.len(),
            sheets_info.join("; ")
        )));
    }

    // ===== 写入数据库 =====
    let conn = pool.get()?;
    let (inserted, updated, db_errors) = import::batch_import(&conn, &records)?;

    // ===== 构建结果 =====
    let mut warnings: Vec<String> = vec![];
    if total_skipped > 0 {
        warnings.push(format!(
            "共读取{}行，跳过{}行无效数据（日期/批号为空或示例行）",
            total_read, total_skipped
        ));
    }

    let result = ImportResult {
        success: true,
        total_rows_read: total_read,
        inserted,
        updated,
        skipped: total_skipped,
        sheet_name: sheet_names.join(", "),
        columns_found: sheets_info,
        errors: db_errors,
        warnings,
    };

    tracing::info!(
        "Import completed: {} sheets, {} inserted, {} updated, {} skipped",
        sheet_names.len(),
        result.inserted,
        result.updated,
        result.skipped
    );

    Ok(axum::Json(ApiResponse {
        code: 0,
        message: result.summary(),
        data: Some(result),
    }))
}
