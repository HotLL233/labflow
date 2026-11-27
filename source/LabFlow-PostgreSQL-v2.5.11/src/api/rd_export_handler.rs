use super::{export_write, rd_export_data};
use crate::db::DbPool;
use crate::repo::settings_repo;
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
use std::collections::BTreeMap;

fn template_sheet_enabled(template: &Option<serde_json::Value>, sheet_id: &str) -> bool {
    template
        .as_ref()
        .and_then(|value| value.get("sheets"))
        .and_then(|sheets| sheets.get(sheet_id))
        .and_then(|sheet| sheet.get("enabled"))
        .and_then(|value| value.as_bool())
        .unwrap_or(true)
}

fn template_columns(sheet_id: &str) -> &'static [&'static str] {
    match sheet_id {
        "sheet1" => &[
            "lab_name",
            "project_name",
            "instrument",
            "method_name",
            "method_type",
            "quantity",
            "project_total",
            "high_item",
        ],
        "sheet2" => &[
            "date",
            "instrument",
            "lab_name",
            "project_name",
            "high_item",
            "method_name",
            "quantity",
            "daily_total",
        ],
        "sheet3" => &[
            "project_name",
            "high_item",
            "lab_name",
            "instrument",
            "method_name",
            "multiplier",
            "quantity",
            "unit_price",
            "detail_amount",
            "project_total",
        ],
        "sheet4" => &[
            "lab_name",
            "project_name",
            "high_item",
            "instrument",
            "method_name",
            "multiplier",
            "quantity",
            "unit_price",
            "total_qty",
            "detail_amount",
            "lab_total",
        ],
        "sheet5" => &[
            "recorded_at",
            "lab_name",
            "project_name",
            "high_item",
            "method_name",
            "method_type",
            "quantity",
            "user_name",
        ],
        "sheet6" => &["user_name", "method_type", "quantity", "total_qty"],
        "sheet7" => &[
            "lab_name",
            "project_name",
            "method_type",
            "quantity",
            "unit_price",
            "multiplier",
            "detail_amount",
            "project_total",
            "lab_total",
        ],
        "sheet8" => &[
            "project_name",
            "method_type",
            "quantity",
            "unit_price",
            "multiplier",
            "detail_amount",
            "project_total",
        ],
        "sheet9" => &["instrument", "quantity", "instrument_type", "type_total"],
        "sheet10" => &["method_name", "quantity"],
        "sheet11" => &[
            "method_type",
            "quantity",
            "unit_price",
            "detail_amount",
            "type_total",
        ],
        _ => &[],
    }
}

fn apply_template_sheet(
    worksheet: &mut rust_xlsxwriter::Worksheet,
    template: &Option<serde_json::Value>,
    sheet_id: &str,
    fmt: &export_write::Fmt,
) -> std::result::Result<(), String> {
    let Some(sheet) = template
        .as_ref()
        .and_then(|value| value.get("sheets"))
        .and_then(|sheets| sheets.get(sheet_id))
    else {
        return Ok(());
    };
    if let Some(title) = sheet
        .get("title")
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
    {
        worksheet
            .set_name(title)
            .map_err(|error| format!("{}名称: {}", sheet_id, error))?;
    }
    if let Some(color) = sheet.get("color").and_then(|value| value.as_str()) {
        if let Ok(rgb) = u32::from_str_radix(color.trim_start_matches('#'), 16) {
            worksheet.set_tab_color(rust_xlsxwriter::Color::RGB(rgb));
        }
    }
    let Some(columns) = sheet.get("columns").and_then(|value| value.as_object()) else {
        return Ok(());
    };
    for (index, key) in template_columns(sheet_id).iter().enumerate() {
        let Some(column) = columns.get(*key) else {
            continue;
        };
        let index = index as u16;
        if let Some(width) = column.get("width").and_then(|value| value.as_f64()) {
            worksheet.set_column_width(index, width).ok();
        }
        if column.get("visible").and_then(|value| value.as_bool()) == Some(false) {
            worksheet.set_column_hidden(index).ok();
        }
        if let Some(label) = column
            .get("label")
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
        {
            worksheet
                .write_with_format(export_write::HR, index, label, &fmt.fh)
                .map_err(|error| format!("{}列标题: {}", sheet_id, error))?;
        }
    }
    Ok(())
}

fn query_custom_fields(
    conn: &postgres_compat::Connection,
    start: &str,
    end: &str,
    subject_user_id: Option<i64>,
    scope_group_id: Option<i64>,
    allowed_division_ids: Option<&[i64]>,
) -> std::result::Result<(Vec<(String, String, String)>, Vec<BTreeMap<String, String>>), String> {
    let mut configured = conn.prepare(
        "SELECT name,label,option_detail_rules FROM rd_record_columns WHERE is_predefined=0 AND is_active=1 AND show_in_export=1 ORDER BY sort_order,id",
    ).map_err(|error| error.to_string())?;
    let custom: Vec<(String, String, String)> = configured
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .map_err(|error| error.to_string())?
        .collect::<std::result::Result<Vec<(String, String, String)>, _>>()
        .map_err(|error| error.to_string())?;
    if custom.is_empty() {
        return Ok((custom, Vec::new()));
    }
    let end_closed = crate::api::export_data::end_of_day_bound(end);
    let division_scope = match allowed_division_ids {
        None => String::new(),
        Some([]) => " AND 1=0".to_string(),
        Some(ids) => format!(
            " AND COALESCE(wr.execution_division_id,wr.division_id,(SELECT division_id FROM project_groups WHERE id=wr.group_id)) IN ({})",
            ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
        ),
    };
    let mut statement = conn.prepare(&format!(
        "SELECT wr.business_no, wr.extra_fields FROM rd_work_records wr WHERE wr.deleted_at IS NULL AND wr.recorded_at>=?1 AND wr.recorded_at<=?2 AND (?3 IS NULL OR wr.subject_user_id=?3) AND (?4 IS NULL OR wr.group_id=?4){division_scope} ORDER BY wr.recorded_at DESC"
    )).map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(
            postgres_compat::params![start, end_closed, subject_user_id, scope_group_id],
            |row| {
                let business_no: String = row.get(0)?;
                let value: Option<String> = row.get(1)?;
                Ok((business_no, value.unwrap_or_else(|| "{}".into())))
            },
        )
        .map_err(|error| error.to_string())?;
    let values = rows
        .filter_map(|row| row.ok())
        .map(|(business_no, raw)| {
            let json = serde_json::from_str::<serde_json::Value>(&raw).unwrap_or_default();
            let mut result = BTreeMap::new();
            result.insert("业务编号".into(), business_no);
            for (key, label, raw_rules) in &custom {
                let value = json
                    .get(key)
                    .map(|value| value.to_string().trim_matches('"').to_string())
                    .unwrap_or_default();
                let has_trigger_rule = serde_json::from_str::<
                    Vec<crate::models::rd_record_column::RdOptionDetailRule>,
                >(raw_rules)
                .unwrap_or_default()
                .iter()
                .any(|rule| rule.trigger_value == value);
                let detail = json
                    .get(format!("{}__detail", key))
                    .map(|value| value.to_string().trim_matches('"').to_string())
                    .unwrap_or_default();
                result.insert(
                    label.clone(),
                    if has_trigger_rule && !detail.trim().is_empty() {
                        format!("{}（{}）", value, detail)
                    } else {
                        value
                    },
                );
            }
            result
        })
        .collect();
    Ok((custom, values))
}

fn write_custom_fields_sheet(
    workbook: &mut rust_xlsxwriter::Workbook,
    fields: &[(String, String, String)],
    rows: &[BTreeMap<String, String>],
) -> std::result::Result<(), String> {
    if fields.is_empty() {
        return Ok(());
    }
    let sheet = workbook.add_worksheet();
    sheet
        .set_name("自定义字段记录")
        .map_err(|error| error.to_string())?;
    sheet
        .write_string(0, 0, "业务编号")
        .map_err(|error| error.to_string())?;
    for (index, (_, label, _)) in fields.iter().enumerate() {
        sheet
            .write_string(0, (index + 1) as u16, label)
            .map_err(|error| error.to_string())?;
    }
    for (row_index, row) in rows.iter().enumerate() {
        sheet
            .write_string(
                (row_index + 1) as u32,
                0,
                row.get("业务编号").map(String::as_str).unwrap_or_default(),
            )
            .map_err(|error| error.to_string())?;
        for (column_index, (_, label, _)) in fields.iter().enumerate() {
            sheet
                .write_string(
                    (row_index + 1) as u32,
                    (column_index + 1) as u16,
                    row.get(label).map(String::as_str).unwrap_or_default(),
                )
                .map_err(|error| error.to_string())?;
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
    pub division_id: Option<i64>,
    pub ownership_basis: Option<String>,
    pub include_pending_ownership: Option<bool>,
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
    let allowed_division_ids = match authz_service::rd_allowed_division_ids(&pool, &ctx) {
        Ok(ids) => ids,
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
    let result: std::result::Result<Vec<u8>, String> =
        (|| {
            let conn = pool.get().map_err(|e| format!("数据库连接失败: {}", e))?;
            let tx = conn
                .unchecked_transaction()
                .map_err(|e| format!("export transaction begin failed: {e}"))?;
            let ownership_basis = match q.ownership_basis.as_deref().unwrap_or("execution") {
                "execution" | "submitted" | "project" => {
                    q.ownership_basis.as_deref().unwrap_or("execution")
                }
                _ => return Err("导出统计口径只能是 execution、submitted 或 project".into()),
            };
            tx.execute(
                "SELECT set_config('workload.export_ownership_basis',?1,true)",
                [ownership_basis],
            )
            .map_err(|e| format!("设置导出统计口径失败: {e}"))?;
            tx.execute(
                "SELECT set_config('workload.export_division_id',?1,true)",
                [q.division_id
                    .map(|id| id.to_string())
                    .as_deref()
                    .unwrap_or("0")],
            )
            .map_err(|e| format!("设置导出部门过滤失败: {e}"))?;
            tx.execute(
                "SELECT set_config('workload.export_include_pending_ownership',?1,true)",
                [q.include_pending_ownership.unwrap_or(false).to_string()],
            )
            .map_err(|e| format!("设置历史记录过滤失败: {e}"))?;
            let fmt = export_write::Fmt::new();
            let mut wb = rust_xlsxwriter::Workbook::new();
            let template = settings_repo::get(&pool, "export_template_rd")
                .ok()
                .flatten()
                .and_then(|setting| serde_json::from_str::<serde_json::Value>(&setting.value).ok());

            tracing::info!(
            "开始导出 Excel: start={}, end={}, group_id={:?}, division_id={:?}, ownership_basis={}",
            start,
            end,
            q.group_id, q.division_id, ownership_basis
        );

            // ========== Sheet 1: 各实验室项目方法对应表 ==========
            match rd_export_data::query_sheet1_data(
                &conn,
                start,
                end,
                q.group_id,
                subject_user_id,
                scope_group_id,
                allowed_division_ids.as_deref(),
            ) {
                Ok(data) => {
                    tracing::info!("Sheet 1 查询完成: {} 行", data.len());
                    if template_sheet_enabled(&template, "sheet1") {
                        let ws = wb.add_worksheet();
                        match export_write::write_sheet1(ws, &data, &fmt) {
                            Ok(_) => apply_template_sheet(ws, &template, "sheet1", &fmt)?,
                            Err(e) => return Err(format!("Sheet1写入: {}", e)),
                        }
                        tracing::info!("Sheet 1 写入完成");
                    }
                }
                Err(e) => return Err(format!("Sheet1查询: {}", e)),
            }

            // ========== Sheet 2: 仪器-汇总 ==========
            match rd_export_data::query_sheet2_data(
                &conn,
                start,
                end,
                subject_user_id,
                scope_group_id,
                allowed_division_ids.as_deref(),
            ) {
                Ok(data) => {
                    tracing::info!("Sheet 2 查询完成: {} 行", data.len());
                    if template_sheet_enabled(&template, "sheet2") {
                        let ws = wb.add_worksheet();
                        export_write::write_sheet2(ws, &data, &fmt)
                            .map_err(|e| format!("Sheet2: {}", e))?;
                        apply_template_sheet(ws, &template, "sheet2", &fmt)?;
                        tracing::info!("Sheet 2 写入完成");
                    }
                }
                Err(e) => return Err(format!("Sheet2查询: {}", e)),
            }

            // ========== Sheet 3: 项目-汇总 ==========
            match rd_export_data::query_sheet3_data(
                &conn,
                start,
                end,
                subject_user_id,
                scope_group_id,
                allowed_division_ids.as_deref(),
            ) {
                Ok(data) => {
                    tracing::info!("Sheet 3 查询完成: {} 行", data.len());
                    if template_sheet_enabled(&template, "sheet3") {
                        let ws = wb.add_worksheet();
                        export_write::write_sheet3(ws, &data, &fmt)
                            .map_err(|e| format!("Sheet3: {}", e))?;
                        apply_template_sheet(ws, &template, "sheet3", &fmt)?;
                        tracing::info!("Sheet 3 写入完成");
                    }
                }
                Err(e) => return Err(format!("Sheet3查询: {}", e)),
            }

            // ========== Sheet 4: 实验室-汇总 ==========
            match rd_export_data::query_sheet4_data(
                &conn,
                start,
                end,
                subject_user_id,
                scope_group_id,
                allowed_division_ids.as_deref(),
            ) {
                Ok(data) => {
                    tracing::info!("Sheet 4 查询完成: {} 行", data.len());
                    if template_sheet_enabled(&template, "sheet4") {
                        let ws = wb.add_worksheet();
                        export_write::write_sheet4(ws, &data, &fmt)
                            .map_err(|e| format!("Sheet4: {}", e))?;
                        apply_template_sheet(ws, &template, "sheet4", &fmt)?;
                        tracing::info!("Sheet 4 写入完成");
                    }
                }
                Err(e) => return Err(format!("Sheet4查询: {}", e)),
            }

            // ========== Sheet 5: 人员-汇总（原始记录） ==========
            match rd_export_data::query_sheet5_data(
                &conn,
                start,
                end,
                subject_user_id,
                scope_group_id,
                allowed_division_ids.as_deref(),
            ) {
                Ok(data) => {
                    tracing::info!("Sheet 5 查询完成: {} 行", data.len());
                    if template_sheet_enabled(&template, "sheet5") {
                        let ws = wb.add_worksheet();
                        export_write::write_sheet5(ws, &data, &fmt, "送样人")
                            .map_err(|e| format!("Sheet5: {}", e))?;
                        apply_template_sheet(ws, &template, "sheet5", &fmt)?;
                        tracing::info!("Sheet 5 写入完成");
                    }
                }
                Err(e) => return Err(format!("Sheet5查询: {}", e)),
            }

            // ========== Sheet 6: 送样人汇总表（统计送样量，不含工作量系数） ==========
            match rd_export_data::query_sheet6_data(
                &conn,
                start,
                end,
                subject_user_id,
                scope_group_id,
                allowed_division_ids.as_deref(),
            ) {
                Ok(data) => {
                    tracing::info!("Sheet 6 查询完成: {} 行", data.len());
                    if template_sheet_enabled(&template, "sheet6") {
                        let ws = wb.add_worksheet();
                        export_write::write_rd_sheet6(ws, &data, &fmt, "送样人")
                            .map_err(|e| format!("Sheet6: {}", e))?;
                        apply_template_sheet(ws, &template, "sheet6", &fmt)?;
                        tracing::info!("Sheet 6 写入完成");
                    }
                }
                Err(e) => return Err(format!("Sheet6查询: {}", e)),
            }

            // ========== Sheet 7: 实验室总表 ==========
            match rd_export_data::query_sheet7_data(
                &conn,
                start,
                end,
                subject_user_id,
                scope_group_id,
                allowed_division_ids.as_deref(),
            ) {
                Ok(data) => {
                    tracing::info!("Sheet 7 查询完成: {} 行", data.len());
                    if template_sheet_enabled(&template, "sheet7") {
                        let ws = wb.add_worksheet();
                        export_write::write_sheet7(ws, &data, &fmt)
                            .map_err(|e| format!("Sheet7: {}", e))?;
                        apply_template_sheet(ws, &template, "sheet7", &fmt)?;
                        tracing::info!("Sheet 7 写入完成");
                    }
                }
                Err(e) => return Err(format!("Sheet7查询: {}", e)),
            }

            // ========== Sheet 8: 项目总表 ==========
            match rd_export_data::query_sheet8_data(
                &conn,
                start,
                end,
                subject_user_id,
                scope_group_id,
                allowed_division_ids.as_deref(),
            ) {
                Ok(data) => {
                    tracing::info!("Sheet 8 查询完成: {} 行", data.len());
                    if template_sheet_enabled(&template, "sheet8") {
                        let ws = wb.add_worksheet();
                        export_write::write_sheet8(ws, &data, &fmt)
                            .map_err(|e| format!("Sheet8: {}", e))?;
                        apply_template_sheet(ws, &template, "sheet8", &fmt)?;
                        tracing::info!("Sheet 8 写入完成");
                    }
                }
                Err(e) => return Err(format!("Sheet8查询: {}", e)),
            }

            // ========== Sheet 9: 仪器汇总表 ==========
            match rd_export_data::query_sheet9_data(
                &conn,
                start,
                end,
                subject_user_id,
                scope_group_id,
                allowed_division_ids.as_deref(),
            ) {
                Ok(data) => {
                    tracing::info!("Sheet 9 查询完成: {} 行", data.len());
                    if template_sheet_enabled(&template, "sheet9") {
                        let ws = wb.add_worksheet();
                        export_write::write_sheet9(ws, &data, &fmt)
                            .map_err(|e| format!("Sheet9: {}", e))?;
                        apply_template_sheet(ws, &template, "sheet9", &fmt)?;
                        tracing::info!("Sheet 9 写入完成");
                    }
                }
                Err(e) => return Err(format!("Sheet9查询: {}", e)),
            }

            // ========== Sheet 10: 理化汇总表 ==========
            match rd_export_data::query_sheet10_data(
                &conn,
                start,
                end,
                subject_user_id,
                scope_group_id,
                allowed_division_ids.as_deref(),
            ) {
                Ok(data) => {
                    tracing::info!("Sheet 10 查询完成: {} 行", data.len());
                    if template_sheet_enabled(&template, "sheet10") {
                        let ws = wb.add_worksheet();
                        export_write::write_sheet10(ws, &data, &fmt)
                            .map_err(|e| format!("Sheet10: {}", e))?;
                        apply_template_sheet(ws, &template, "sheet10", &fmt)?;
                        tracing::info!("Sheet 10 写入完成");
                    }
                }
                Err(e) => return Err(format!("Sheet10查询: {}", e)),
            }

            // ========== Sheet 11: 类型汇总表 ==========
            match rd_export_data::query_sheet11_data(
                &conn,
                start,
                end,
                subject_user_id,
                scope_group_id,
                allowed_division_ids.as_deref(),
            ) {
                Ok(data) => {
                    tracing::info!("Sheet 11 查询完成: {} 行", data.len());
                    if template_sheet_enabled(&template, "sheet11") {
                        let ws = wb.add_worksheet();
                        export_write::write_sheet11(ws, &data, &fmt)
                            .map_err(|e| format!("Sheet11: {}", e))?;
                        apply_template_sheet(ws, &template, "sheet11", &fmt)?;
                        tracing::info!("Sheet 11 写入完成");
                    }
                }
                Err(e) => return Err(format!("Sheet11查询: {}", e)),
            }

            let (custom_fields, custom_rows) = query_custom_fields(
                &conn,
                start,
                end,
                subject_user_id,
                scope_group_id,
                allowed_division_ids.as_deref(),
            )?;
            write_custom_fields_sheet(&mut wb, &custom_fields, &custom_rows)?;

            // 保存到内存
            tx.commit()
                .map_err(|e| format!("export transaction commit failed: {e}"))?;
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
