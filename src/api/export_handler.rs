use super::{export_data, export_write};
use crate::db::DbPool;
use crate::repo::settings_repo;
use crate::service::authz_service;
use crate::service::stats_service;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::Response;
/// Excel 导出处理器 - v0.3.8 修复版本
/// 实现 10 个 Sheet 的导出功能
/// 修复：错误响应不再被当作 xlsx 下载，改为返回 500 + JSON
use axum::{
    extract::{Query, State},
    routing::get,
    Router,
};
use chrono::Datelike;
use serde::Deserialize;
use serde_json;

/// 从 system_settings 加载导出模板并覆盖 sheet 名称和颜色
fn apply_sheet_cfg(
    ws: &mut rust_xlsxwriter::Worksheet,
    tmpl: &Option<serde_json::Value>,
    sheet_id: &str,
) -> Result<(), String> {
    let t = match tmpl {
        Some(v) => v,
        None => return Ok(()),
    };
    let sheets = match t.get("sheets") {
        Some(v) => v,
        None => return Ok(()),
    };
    let sheet = match sheets.get(sheet_id) {
        Some(v) => v,
        None => return Ok(()),
    };
    if let Some(enabled) = sheet.get("enabled").and_then(|v| v.as_bool()) {
        if !enabled {
            return Ok(());
        }
    }
    if let Some(title) = sheet.get("title").and_then(|v| v.as_str()) {
        ws.set_name(title)
            .map_err(|e| format!("{}名称: {}", sheet_id, e))?;
    }

    // 解析 hex 颜色并设置 tab 颜色
    fn hex_to_rgb(hex: &str) -> Option<u32> {
        let h = hex.trim_start_matches('#');
        if h.len() != 6 {
            return None;
        }
        u32::from_str_radix(h, 16).ok()
    }

    if let Some(color) = sheet.get("color").and_then(|v| v.as_str()) {
        if let Some(rgb) = hex_to_rgb(color) {
            ws.set_tab_color(rust_xlsxwriter::Color::RGB(rgb));
        }
    }
    // 列宽覆盖
    if let Some(columns) = sheet.get("columns") {
        if let Some(obj) = columns.as_object() {
            for (i, (_, col_val)) in obj.iter().enumerate() {
                if let Some(width) = col_val.get("width").and_then(|v| v.as_f64()) {
                    ws.set_column_width(i as u16, width).ok();
                }
            }
        }
    }
    Ok(())
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct ExportQuery {
    /// 起始日期 (YYYY-MM-DD)，默认当月第一天
    pub start: Option<String>,
    /// 结束日期 (YYYY-MM-DD)，默认当月最后一天
    pub end: Option<String>,
    /// 筛选分组 ID（仅 Sheet 1 使用）
    pub group_id: Option<i64>,
}

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/export/excel", get(export_excel))
        .with_state(pool)
}

/// 主导出函数：生成包含 10 个 Sheet 的 Excel 文件
/// 返回 Response（非 Result），内部捕获所有错误
/// 成功 → 200 + xlsx binary；失败 → 500 + JSON
async fn export_excel(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<ExportQuery>,
) -> Response {
    use std::io::Cursor;

    let ctx = match authz_service::authenticate(&pool, &headers).and_then(|ctx| {
        authz_service::require_permission(&ctx, "stats:portal:workload")?;
        Ok(ctx)
    }) {
        Ok(ctx) => ctx,
        Err(error) => return error_response(StatusCode::FORBIDDEN, &error.to_string()),
    };
    let subject_user_id = if ctx.is_system_admin() || ctx.has_permission("stats:workload:view-all")
    {
        None
    } else {
        Some(ctx.user.id)
    };

    // 确定日期范围
    let start_str: String;
    let end_str: String;
    let (start, end) = if let Some(ref s) = q.start {
        start_str = s.clone();
        end_str = q.end.as_ref().cloned().unwrap_or_else(|| s.clone());
        (start_str.as_str(), end_str.as_str())
    } else {
        // 默认当月
        let now = chrono::Local::now();
        start_str = format!("{}-{:02}-01", now.year(), now.month());

        // 计算月末
        let last_day = if now.month() == 12 {
            chrono::NaiveDate::from_ymd_opt(now.year() + 1, 1, 1).and_then(|d| d.pred_opt())
        } else {
            chrono::NaiveDate::from_ymd_opt(now.year(), now.month() + 1, 1)
                .and_then(|d| d.pred_opt())
        };

        end_str = if let Some(d) = last_day {
            d.format("%Y-%m-%d").to_string()
        } else {
            format!("{}-{:02}-28", now.year(), now.month())
        };

        (start_str.as_str(), end_str.as_str())
    };

    // 执行导出，内部捕获所有错误 → Result<Vec<u8>, String>
    let result: std::result::Result<Vec<u8>, String> = (|| {
        let conn = pool.get().map_err(|e| format!("数据库连接失败: {}", e))?;
        let fmt = export_write::Fmt::new();
        let mut wb = rust_xlsxwriter::Workbook::new();

        // v0.4.51: 加载导出模板配置
        let tmpl = settings_repo::get(&pool, "export_template_workload")
            .ok()
            .flatten()
            .and_then(|r| serde_json::from_str::<serde_json::Value>(&r.value).ok());

        tracing::info!(
            "开始导出 Excel: start={}, end={}, group_id={:?}",
            start,
            end,
            q.group_id
        );

        // ========== Sheet 1: 各实验室项目方法对应表 ==========
        match export_data::query_sheet1_data(&conn, start, end, q.group_id, subject_user_id) {
            Ok(data) => {
                tracing::info!("Sheet 1 查询完成: {} 行", data.len());
                let ws = wb.add_worksheet();
                match export_write::write_sheet1(ws, &data, &fmt) {
                    Ok(_) => {
                        apply_sheet_cfg(ws, &tmpl, "sheet1")?;
                        tracing::info!("Sheet 1 写入完成");
                    }
                    Err(e) => return Err(format!("Sheet1写入: {}", e)),
                }
            }
            Err(e) => return Err(format!("Sheet1查询: {}", e)),
        }

        // ========== Sheet 2: 仪器-汇总 ==========
        match export_data::query_sheet2_data(&conn, start, end, subject_user_id) {
            Ok(data) => {
                tracing::info!("Sheet 2 查询完成: {} 行", data.len());
                let ws = wb.add_worksheet();
                export_write::write_sheet2(ws, &data, &fmt)
                    .map_err(|e| format!("Sheet2: {}", e))?;
                apply_sheet_cfg(ws, &tmpl, "sheet2")?;
                tracing::info!("Sheet 2 写入完成");
            }
            Err(e) => return Err(format!("Sheet2查询: {}", e)),
        }

        // ========== Sheet 3: 项目-汇总 ==========
        match export_data::query_sheet3_data(&conn, start, end, subject_user_id) {
            Ok(data) => {
                tracing::info!("Sheet 3 查询完成: {} 行", data.len());
                let ws = wb.add_worksheet();
                export_write::write_sheet3(ws, &data, &fmt)
                    .map_err(|e| format!("Sheet3: {}", e))?;
                apply_sheet_cfg(ws, &tmpl, "sheet3")?;
                tracing::info!("Sheet 3 写入完成");
            }
            Err(e) => return Err(format!("Sheet3查询: {}", e)),
        }

        // ========== Sheet 4: 实验室-汇总 ==========
        match export_data::query_sheet4_data(&conn, start, end, subject_user_id) {
            Ok(data) => {
                tracing::info!("Sheet 4 查询完成: {} 行", data.len());
                let ws = wb.add_worksheet();
                export_write::write_sheet4(ws, &data, &fmt)
                    .map_err(|e| format!("Sheet4: {}", e))?;
                apply_sheet_cfg(ws, &tmpl, "sheet4")?;
                tracing::info!("Sheet 4 写入完成");
            }
            Err(e) => return Err(format!("Sheet4查询: {}", e)),
        }

        // ========== Sheet 5: 人员-汇总（原始记录） ==========
        match export_data::query_sheet5_data(&conn, start, end, subject_user_id) {
            Ok(data) => {
                tracing::info!("Sheet 5 查询完成: {} 行", data.len());
                let ws = wb.add_worksheet();
                export_write::write_sheet5(ws, &data, &fmt, "检测人")
                    .map_err(|e| format!("Sheet5: {}", e))?;
                apply_sheet_cfg(ws, &tmpl, "sheet5")?;
                tracing::info!("Sheet 5 写入完成");
            }
            Err(e) => return Err(format!("Sheet5查询: {}", e)),
        }

        // ========== Sheet 6: 人员汇总表 ==========
        match export_data::query_sheet6_data(&conn, start, end, subject_user_id) {
            Ok(data) => {
                tracing::info!("Sheet 6 查询完成: {} 行", data.len());
                let ws = wb.add_worksheet();
                export_write::write_sheet6(ws, &data, &fmt, "检测人")
                    .map_err(|e| format!("Sheet6: {}", e))?;
                apply_sheet_cfg(ws, &tmpl, "sheet6")?;
                tracing::info!("Sheet 6 写入完成");
            }
            Err(e) => return Err(format!("Sheet6查询: {}", e)),
        }

        // ========== Sheet 7: 实验室总表 ==========
        match export_data::query_sheet7_data(&conn, start, end, subject_user_id) {
            Ok(data) => {
                tracing::info!("Sheet 7 查询完成: {} 行", data.len());
                let ws = wb.add_worksheet();
                export_write::write_sheet7(ws, &data, &fmt)
                    .map_err(|e| format!("Sheet7: {}", e))?;
                apply_sheet_cfg(ws, &tmpl, "sheet7")?;
                tracing::info!("Sheet 7 写入完成");
            }
            Err(e) => return Err(format!("Sheet7查询: {}", e)),
        }

        // ========== Sheet 8: 项目总表 ==========
        match export_data::query_sheet8_data(&conn, start, end, subject_user_id) {
            Ok(data) => {
                tracing::info!("Sheet 8 查询完成: {} 行", data.len());
                let ws = wb.add_worksheet();
                export_write::write_sheet8(ws, &data, &fmt)
                    .map_err(|e| format!("Sheet8: {}", e))?;
                apply_sheet_cfg(ws, &tmpl, "sheet8")?;
                tracing::info!("Sheet 8 写入完成");
            }
            Err(e) => return Err(format!("Sheet8查询: {}", e)),
        }

        // ========== Sheet 9: 仪器汇总表 ==========
        match export_data::query_sheet9_data(&conn, start, end, subject_user_id) {
            Ok(data) => {
                tracing::info!("Sheet 9 查询完成: {} 行", data.len());
                let ws = wb.add_worksheet();
                export_write::write_sheet9(ws, &data, &fmt)
                    .map_err(|e| format!("Sheet9: {}", e))?;
                apply_sheet_cfg(ws, &tmpl, "sheet9")?;
                tracing::info!("Sheet 9 写入完成");
            }
            Err(e) => return Err(format!("Sheet9查询: {}", e)),
        }

        // ========== Sheet 10: 理化汇总表 ==========
        match export_data::query_sheet10_data(&conn, start, end, subject_user_id) {
            Ok(data) => {
                tracing::info!("Sheet 10 查询完成: {} 行", data.len());
                let ws = wb.add_worksheet();
                export_write::write_sheet10(ws, &data, &fmt)
                    .map_err(|e| format!("Sheet10: {}", e))?;
                apply_sheet_cfg(ws, &tmpl, "sheet10")?;
                tracing::info!("Sheet 10 写入完成");
            }
            Err(e) => return Err(format!("Sheet10查询: {}", e)),
        }

        // ========== Sheet 11: 事业部汇总 (v0.4.28) ==========
        {
            let div_data = stats_service::by_division(&pool, start, end, None, subject_user_id)
                .map_err(|e| format!("事业部查询: {}", e))?;
            tracing::info!("Sheet 11 事业部汇总: {} 行", div_data.len());
            let ws = wb.add_worksheet();
            ws.set_name("事业部汇总")
                .map_err(|e| format!("Sheet11: {}", e))?;
            ws.set_column_width(0, 16.0)
                .map_err(|e| format!("Sheet11: {}", e))?; // 事业部
            ws.set_column_width(1, 10.0)
                .map_err(|e| format!("Sheet11: {}", e))?; // 实验室数
            ws.set_column_width(2, 10.0)
                .map_err(|e| format!("Sheet11: {}", e))?; // 检测数量
            ws.set_column_width(3, 10.0)
                .map_err(|e| format!("Sheet11: {}", e))?; // 记录数
            ws.set_column_width(4, 10.0)
                .map_err(|e| format!("Sheet11: {}", e))?; // 系数分
            for col in 0u16..=4u16 {
                ws.set_column_format(col, &fmt.fd)
                    .map_err(|e| format!("Sheet11: {}", e))?;
            }
            // 表头
            let headers = ["事业部", "实验室数", "检测数量", "记录数", "系数分"];
            for (i, h) in headers.iter().enumerate() {
                ws.write_with_format(0, i as u16, *h, &fmt.fh)
                    .map_err(|e| format!("Sheet11: {}", e))?;
            }
            for (i, row) in div_data.iter().enumerate() {
                let r = (i + 1) as u32;
                ws.write_with_format(r, 0, row.division_name.as_str(), &fmt.fd)
                    .map_err(|e| format!("Sheet11: {}", e))?;
                ws.write_with_format(r, 1, row.lab_count as f64, &fmt.fd)
                    .map_err(|e| format!("Sheet11: {}", e))?;
                ws.write_with_format(r, 2, row.total_quantity as f64, &fmt.fd)
                    .map_err(|e| format!("Sheet11: {}", e))?;
                ws.write_with_format(r, 3, row.record_count as f64, &fmt.fd)
                    .map_err(|e| format!("Sheet11: {}", e))?;
                ws.write_with_format(r, 4, row.coefficient_score, &fmt.fd)
                    .map_err(|e| format!("Sheet11: {}", e))?;
            }
            apply_sheet_cfg(ws, &tmpl, "sheet11")?;
            ws.autofit();
            tracing::info!("Sheet 11 事业部汇总写入完成");
        }

        // 保存到内存
        let mut buf = Cursor::new(Vec::new());
        wb.save_to_writer(&mut buf)
            .map_err(|e| format!("保存Excel: {}", e))?;
        Ok(buf.into_inner())
    })();

    match result {
        Ok(data) => {
            // 审计日志：记录导出操作
            let desc = format!("导出Excel，时间范围 {} ~ {}", start, end);
            crate::repo::audit_repo::log_actor(
                &pool,
                "export",
                "work_records",
                None,
                ctx.user.id,
                &ctx.user.username,
                &desc,
                "work",
            )
            .ok();
            let filename = format!("分析检测统计_{}_{}.xlsx", start, end);
            let encoded_filename = format!(
                "attachment; filename*=UTF-8''{}",
                url_escape::encode_component(&filename)
            );
            tracing::info!("Excel 导出完成: {} bytes", data.len());
            Response::builder()
                .header(
                    header::CONTENT_TYPE,
                    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                )
                .header(header::CONTENT_DISPOSITION, encoded_filename)
                .body(axum::body::Body::from(data))
                .unwrap()
        }
        Err(msg) => {
            tracing::error!("导出失败: {}", msg);
            let error_json =
                serde_json::json!({"code": 5000, "message": format!("导出失败: {}", msg)});
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(error_json.to_string()))
                .unwrap()
        }
    }
}

fn error_response(status: StatusCode, message: &str) -> Response {
    let body = serde_json::json!({"code": 1003, "message": message});
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body.to_string()))
        .unwrap()
}
