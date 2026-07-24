use super::{export_write, rd_export_data};
use crate::db::DbPool;
use crate::service::authz_service;
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
        .route("/api/rd-export/excel", get(export_excel))
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
        authz_service::require_permission(&ctx, "stats:portal:rd")?;
        Ok(ctx)
    }) {
        Ok(ctx) => ctx,
        Err(error) => return error_response(StatusCode::FORBIDDEN, &error.to_string()),
    };
    let (subject_user_id, scope_group_id) = if ctx.is_system_admin()
        || ctx.has_permission("stats:rd:view-all")
    {
        (None, None)
    } else if ctx.has_permission("stats:rd:view-lab") {
        match ctx.user.group_id {
            Some(group_id) => (None, Some(group_id)),
            None => return error_response(StatusCode::FORBIDDEN, "当前统计角色尚未设置所属实验室"),
        }
    } else {
        (Some(ctx.user.id), None)
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

        tracing::info!(
            "开始导出 Excel: start={}, end={}, group_id={:?}",
            start,
            end,
            q.group_id
        );

        // ========== Sheet 1: 各实验室项目方法对应表 ==========
        match rd_export_data::query_sheet1_data(
            &conn,
            start,
            end,
            q.group_id,
            subject_user_id,
            scope_group_id,
        ) {
            Ok(data) => {
                tracing::info!("Sheet 1 查询完成: {} 行", data.len());
                let ws = wb.add_worksheet();
                match export_write::write_sheet1(ws, &data, &fmt) {
                    Ok(_) => tracing::info!("Sheet 1 写入完成"),
                    Err(e) => return Err(format!("Sheet1写入: {}", e)),
                }
            }
            Err(e) => return Err(format!("Sheet1查询: {}", e)),
        }

        // ========== Sheet 2: 仪器-汇总 ==========
        match rd_export_data::query_sheet2_data(&conn, start, end, subject_user_id, scope_group_id)
        {
            Ok(data) => {
                tracing::info!("Sheet 2 查询完成: {} 行", data.len());
                let ws = wb.add_worksheet();
                export_write::write_sheet2(ws, &data, &fmt)
                    .map_err(|e| format!("Sheet2: {}", e))?;
                tracing::info!("Sheet 2 写入完成");
            }
            Err(e) => return Err(format!("Sheet2查询: {}", e)),
        }

        // ========== Sheet 3: 项目-汇总 ==========
        match rd_export_data::query_sheet3_data(&conn, start, end, subject_user_id, scope_group_id)
        {
            Ok(data) => {
                tracing::info!("Sheet 3 查询完成: {} 行", data.len());
                let ws = wb.add_worksheet();
                export_write::write_sheet3(ws, &data, &fmt)
                    .map_err(|e| format!("Sheet3: {}", e))?;
                tracing::info!("Sheet 3 写入完成");
            }
            Err(e) => return Err(format!("Sheet3查询: {}", e)),
        }

        // ========== Sheet 4: 实验室-汇总 ==========
        match rd_export_data::query_sheet4_data(&conn, start, end, subject_user_id, scope_group_id)
        {
            Ok(data) => {
                tracing::info!("Sheet 4 查询完成: {} 行", data.len());
                let ws = wb.add_worksheet();
                export_write::write_sheet4(ws, &data, &fmt)
                    .map_err(|e| format!("Sheet4: {}", e))?;
                tracing::info!("Sheet 4 写入完成");
            }
            Err(e) => return Err(format!("Sheet4查询: {}", e)),
        }

        // ========== Sheet 5: 人员-汇总（原始记录） ==========
        match rd_export_data::query_sheet5_data(&conn, start, end, subject_user_id, scope_group_id)
        {
            Ok(data) => {
                tracing::info!("Sheet 5 查询完成: {} 行", data.len());
                let ws = wb.add_worksheet();
                export_write::write_sheet5(ws, &data, &fmt, "送样人")
                    .map_err(|e| format!("Sheet5: {}", e))?;
                tracing::info!("Sheet 5 写入完成");
            }
            Err(e) => return Err(format!("Sheet5查询: {}", e)),
        }

        // ========== Sheet 6: 人员汇总表 ==========
        match rd_export_data::query_sheet6_data(&conn, start, end, subject_user_id, scope_group_id)
        {
            Ok(data) => {
                tracing::info!("Sheet 6 查询完成: {} 行", data.len());
                let ws = wb.add_worksheet();
                export_write::write_sheet6(ws, &data, &fmt, "送样人")
                    .map_err(|e| format!("Sheet6: {}", e))?;
                tracing::info!("Sheet 6 写入完成");
            }
            Err(e) => return Err(format!("Sheet6查询: {}", e)),
        }

        // ========== Sheet 7: 实验室总表 ==========
        match rd_export_data::query_sheet7_data(&conn, start, end, subject_user_id, scope_group_id)
        {
            Ok(data) => {
                tracing::info!("Sheet 7 查询完成: {} 行", data.len());
                let ws = wb.add_worksheet();
                export_write::write_sheet7(ws, &data, &fmt)
                    .map_err(|e| format!("Sheet7: {}", e))?;
                tracing::info!("Sheet 7 写入完成");
            }
            Err(e) => return Err(format!("Sheet7查询: {}", e)),
        }

        // ========== Sheet 8: 项目总表 ==========
        match rd_export_data::query_sheet8_data(&conn, start, end, subject_user_id, scope_group_id)
        {
            Ok(data) => {
                tracing::info!("Sheet 8 查询完成: {} 行", data.len());
                let ws = wb.add_worksheet();
                export_write::write_sheet8(ws, &data, &fmt)
                    .map_err(|e| format!("Sheet8: {}", e))?;
                tracing::info!("Sheet 8 写入完成");
            }
            Err(e) => return Err(format!("Sheet8查询: {}", e)),
        }

        // ========== Sheet 9: 仪器汇总表 ==========
        match rd_export_data::query_sheet9_data(&conn, start, end, subject_user_id, scope_group_id)
        {
            Ok(data) => {
                tracing::info!("Sheet 9 查询完成: {} 行", data.len());
                let ws = wb.add_worksheet();
                export_write::write_sheet9(ws, &data, &fmt)
                    .map_err(|e| format!("Sheet9: {}", e))?;
                tracing::info!("Sheet 9 写入完成");
            }
            Err(e) => return Err(format!("Sheet9查询: {}", e)),
        }

        // ========== Sheet 10: 理化汇总表 ==========
        match rd_export_data::query_sheet10_data(&conn, start, end, subject_user_id, scope_group_id)
        {
            Ok(data) => {
                tracing::info!("Sheet 10 查询完成: {} 行", data.len());
                let ws = wb.add_worksheet();
                export_write::write_sheet10(ws, &data, &fmt)
                    .map_err(|e| format!("Sheet10: {}", e))?;
                tracing::info!("Sheet 10 写入完成");
            }
            Err(e) => return Err(format!("Sheet10查询: {}", e)),
        }

        // ========== Sheet 11: 类型汇总表 ==========
        match rd_export_data::query_sheet11_data(&conn, start, end, subject_user_id, scope_group_id)
        {
            Ok(data) => {
                tracing::info!("Sheet 11 查询完成: {} 行", data.len());
                let ws = wb.add_worksheet();
                export_write::write_sheet11(ws, &data, &fmt)
                    .map_err(|e| format!("Sheet11: {}", e))?;
                tracing::info!("Sheet 11 写入完成");
            }
            Err(e) => return Err(format!("Sheet11查询: {}", e)),
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
                "rd_work_records",
                None,
                ctx.user.id,
                &ctx.user.username,
                &desc,
                "rd",
            )
            .ok();
            let filename = format!("研发送样统计_{}_{}.xlsx", start, end);
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
