use super::{export_data, export_write};
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
use serde_json;

fn format_export_date(value: &str) -> String {
    let date_part = value.get(..10).unwrap_or(value);
    chrono::NaiveDate::parse_from_str(date_part, "%Y-%m-%d")
        .map(|date| date.format("%Y.%m.%d").to_string())
        .unwrap_or_else(|_| date_part.replace('-', "."))
}

fn format_export_period(start: &str, end: &str) -> String {
    format!("{}-{}", format_export_date(start), format_export_date(end))
}

fn should_export_sheet(
    raw: Option<&str>,
    sheet_id: &str,
    tmpl: &Option<serde_json::Value>,
) -> Result<bool, String> {
    let valid = [
        "sheet1", "sheet2", "sheet3", "sheet4", "sheet5", "sheet6", "sheet7", "sheet8", "sheet9",
        "sheet10", "sheet11", "sheet12", "sheet13",
    ];
    let selected = raw
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .collect::<Vec<_>>()
        });
    if let Some(selected) = selected.as_ref() {
        if selected.iter().any(|value| !valid.contains(value)) {
            return Err("导出报表选择包含无效报表".into());
        }
    }
    let enabled = tmpl
        .as_ref()
        .and_then(|value| value.get("sheets"))
        .and_then(|sheets| sheets.get(sheet_id))
        .and_then(|sheet| sheet.get("enabled"))
        .and_then(|value| value.as_bool())
        .unwrap_or(true);
    if !enabled {
        if selected
            .as_ref()
            .is_some_and(|items| items.contains(&sheet_id))
        {
            return Err(format!("{}已在导出模板中停用", sheet_id));
        }
        return Ok(false);
    }
    Ok(selected
        .as_ref()
        .map(|items| items.contains(&sheet_id))
        .unwrap_or(true))
}

fn configured_sheet_title(
    tmpl: &Option<serde_json::Value>,
    sheet_id: &str,
    default_title: &str,
    period: &str,
) -> String {
    let base_title = tmpl
        .as_ref()
        .and_then(|value| value.get("sheets"))
        .and_then(|sheets| sheets.get(sheet_id))
        .and_then(|sheet| sheet.get("title"))
        .and_then(|title| title.as_str())
        .filter(|title| !title.trim().is_empty())
        .unwrap_or(default_title);
    format!("{}（{}）", base_title, period)
}

fn fallback_sheet_name(title: &str, sheet_id: &str) -> String {
    let suffix = format!("-{}", sheet_id);
    let max_prefix = 31usize.saturating_sub(suffix.chars().count());
    let prefix = title.chars().take(max_prefix).collect::<String>();
    format!("{}{}", prefix, suffix)
}

/// Export writers use a fixed physical column order. JSON object iteration is not a
/// stable contract (and serde_json may order keys alphabetically), so widths must be
/// looked up by the same explicit order instead of by object position.
fn export_column_order(sheet_id: &str) -> &'static [&'static str] {
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
            "detection_division",
            "sending_division",
            "lab_name",
            "project_name",
            "high_item",
            "method_name",
            "method_type",
            "quantity",
            "user_name",
        ],
        "sheet6" => &[
            "user_name",
            "instrument",
            "method_type",
            "method",
            "coefficient",
            "quantity",
            "workload",
            "total_workload",
        ],
        "sheet12" => &[
            "user_name",
            "auxiliary_method",
            "coefficient",
            "quantity",
            "workload",
            "total_workload",
        ],
        "sheet13" => &[
            "user_name",
            "total_workload",
            "method_type",
            "coefficient",
            "quantity",
            "workload",
        ],
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
            "detection_division",
            "sending_division",
            "lab_count",
            "total_quantity",
            "record_count",
            "coefficient_score",
        ],
        _ => &[],
    }
}

/// 从 system_settings 加载导出模板并覆盖 sheet 名称和颜色
fn apply_sheet_cfg(
    ws: &mut rust_xlsxwriter::Worksheet,
    tmpl: &Option<serde_json::Value>,
    sheet_id: &str,
    fmt: &export_write::Fmt,
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
        if let Err(error) = ws.set_name(title) {
            let fallback = fallback_sheet_name(title, sheet_id);
            ws.set_name(&fallback).map_err(|fallback_error| {
                format!(
                    "{}名称: {}（自动改名为{}仍失败: {}）",
                    sheet_id, error, fallback, fallback_error
                )
            })?;
        }
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
    // All writers keep a fixed physical schema. The template changes only the
    // presentation layer: header text, width and whether Excel hides a column.
    if let Some(columns) = sheet.get("columns") {
        if let Some(obj) = columns.as_object() {
            let ordered_keys = export_column_order(sheet_id);
            for (index, key) in ordered_keys.iter().enumerate() {
                let Some(column) = obj.get(*key) else {
                    continue;
                };
                let index = index as u16;
                if let Some(width) = column.get("width").and_then(|value| value.as_f64()) {
                    ws.set_column_width(index, width).ok();
                }
                if column.get("visible").and_then(|value| value.as_bool()) == Some(false) {
                    ws.set_column_hidden(index).ok();
                }
                if let Some(label) = column
                    .get("label")
                    .and_then(|value| value.as_str())
                    .filter(|value| !value.trim().is_empty())
                {
                    ws.write_with_format(export_write::HR, index, label, &fmt.fh)
                        .map_err(|error| format!("{}列标题: {}", sheet_id, error))?;
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
    /// 逗号分隔的实际检测部门 ID，可多选。
    pub detection_division_ids: Option<String>,
    /// 逗号分隔的送样部门 ID，可多选。
    pub sending_division_ids: Option<String>,
    /// 逗号分隔的导出 Sheet 标识；为空时导出全部报表。
    pub sheet_ids: Option<String>,
    /// 历史兼容字段，alpha.5 起不再作为分析导出口径。
    pub division_id: Option<i64>,
    /// 历史兼容字段，alpha.5 起不再作为分析导出口径。
    pub ownership_basis: Option<String>,
    pub include_pending_ownership: Option<bool>,
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
        authz_service::require_permission(&ctx, "stats:workload:export")?;
        Ok(ctx)
    }) {
        Ok(ctx) => ctx,
        Err(error) => return error_response(StatusCode::FORBIDDEN, &error.to_string()),
    };
    let allowed_division_ids = match authz_service::work_allowed_division_ids(&pool, &ctx) {
        Ok(ids) => ids,
        Err(error) => return error_response(StatusCode::FORBIDDEN, &error.to_string()),
    };
    let subject_user_id = if ctx.can_view_workload_scope() {
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
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| format!("export transaction begin failed: {e}"))?;
        export_data::configure_dimension_filters(
            &tx,
            q.detection_division_ids.as_deref(),
            q.sending_division_ids.as_deref(),
            q.include_pending_ownership.unwrap_or(false),
            allowed_division_ids.as_deref(),
        )
        .map_err(|e| format!("设置检测部门和送样部门筛选失败: {e}"))?;
        let fmt = export_write::Fmt::new();
        let mut wb = rust_xlsxwriter::Workbook::new();

        // v0.4.51: 加载导出模板配置
        let tmpl = settings_repo::get(&pool, "export_template_workload")
            .ok()
            .flatten()
            .and_then(|r| serde_json::from_str::<serde_json::Value>(&r.value).ok());

        tracing::info!(
            "开始导出 Excel: start={}, end={}, group_id={:?}, detection_division_ids={:?}, sending_division_ids={:?}",
            start,
            end,
            q.group_id, q.detection_division_ids, q.sending_division_ids
        );
        let period = format_export_period(start, end);

        // ========== Sheet 1: 各实验室项目方法对应表 ==========
        if should_export_sheet(q.sheet_ids.as_deref(), "sheet1", &tmpl)? {
            match export_data::query_sheet1_data(
                &conn,
                start,
                end,
                q.group_id,
                subject_user_id,
                allowed_division_ids.as_deref(),
            ) {
                Ok(data) => {
                    tracing::info!("Sheet 1 查询完成: {} 行", data.len());
                    let ws = wb.add_worksheet();
                    match export_write::write_sheet1(ws, &data, &fmt) {
                        Ok(_) => {
                            apply_sheet_cfg(ws, &tmpl, "sheet1", &fmt)?;
                            export_write::write_period_title(
                                ws,
                                &configured_sheet_title(
                                    &tmpl,
                                    "sheet1",
                                    "各实验室项目方法对应表",
                                    &period,
                                ),
                                7,
                                &fmt,
                            )
                            .map_err(|e| format!("Sheet1标题: {}", e))?;
                            tracing::info!("Sheet 1 写入完成");
                        }
                        Err(e) => return Err(format!("Sheet1写入: {}", e)),
                    }
                }
                Err(e) => return Err(format!("Sheet1查询: {}", e)),
            }
        }

        // ========== Sheet 2: 仪器-汇总 ==========
        if should_export_sheet(q.sheet_ids.as_deref(), "sheet2", &tmpl)? {
            match export_data::query_sheet2_data(
                &conn,
                start,
                end,
                subject_user_id,
                allowed_division_ids.as_deref(),
            ) {
                Ok(data) => {
                    tracing::info!("Sheet 2 查询完成: {} 行", data.len());
                    let ws = wb.add_worksheet();
                    export_write::write_sheet2(ws, &data, &fmt)
                        .map_err(|e| format!("Sheet2: {}", e))?;
                    apply_sheet_cfg(ws, &tmpl, "sheet2", &fmt)?;
                    export_write::write_period_title(
                        ws,
                        &configured_sheet_title(&tmpl, "sheet2", "仪器-汇总", &period),
                        7,
                        &fmt,
                    )
                    .map_err(|e| format!("Sheet2标题: {}", e))?;
                    tracing::info!("Sheet 2 写入完成");
                }
                Err(e) => return Err(format!("Sheet2查询: {}", e)),
            }
        }

        // ========== Sheet 3: 项目-汇总 ==========
        if should_export_sheet(q.sheet_ids.as_deref(), "sheet3", &tmpl)? {
            match export_data::query_sheet3_data(
                &conn,
                start,
                end,
                subject_user_id,
                allowed_division_ids.as_deref(),
            ) {
                Ok(data) => {
                    tracing::info!("Sheet 3 查询完成: {} 行", data.len());
                    let ws = wb.add_worksheet();
                    export_write::write_sheet3(ws, &data, &fmt)
                        .map_err(|e| format!("Sheet3: {}", e))?;
                    apply_sheet_cfg(ws, &tmpl, "sheet3", &fmt)?;
                    export_write::write_period_title(
                        ws,
                        &configured_sheet_title(&tmpl, "sheet3", "项目-汇总", &period),
                        9,
                        &fmt,
                    )
                    .map_err(|e| format!("Sheet3标题: {}", e))?;
                    tracing::info!("Sheet 3 写入完成");
                }
                Err(e) => return Err(format!("Sheet3查询: {}", e)),
            }
        }

        // ========== Sheet 4: 实验室-汇总 ==========
        if should_export_sheet(q.sheet_ids.as_deref(), "sheet4", &tmpl)? {
            match export_data::query_sheet4_data(
                &conn,
                start,
                end,
                subject_user_id,
                allowed_division_ids.as_deref(),
            ) {
                Ok(data) => {
                    tracing::info!("Sheet 4 查询完成: {} 行", data.len());
                    let ws = wb.add_worksheet();
                    export_write::write_sheet4(ws, &data, &fmt)
                        .map_err(|e| format!("Sheet4: {}", e))?;
                    apply_sheet_cfg(ws, &tmpl, "sheet4", &fmt)?;
                    export_write::write_period_title(
                        ws,
                        &configured_sheet_title(&tmpl, "sheet4", "实验室-汇总", &period),
                        10,
                        &fmt,
                    )
                    .map_err(|e| format!("Sheet4标题: {}", e))?;
                    tracing::info!("Sheet 4 写入完成");
                }
                Err(e) => return Err(format!("Sheet4查询: {}", e)),
            }
        }

        // ========== Sheet 5: 人员-汇总（原始记录） ==========
        if should_export_sheet(q.sheet_ids.as_deref(), "sheet5", &tmpl)? {
            match export_data::query_sheet5_data(
                &conn,
                start,
                end,
                subject_user_id,
                allowed_division_ids.as_deref(),
            ) {
                Ok(data) => {
                    tracing::info!("Sheet 5 查询完成: {} 行", data.len());
                    let ws = wb.add_worksheet();
                    export_write::write_analysis_sheet5(ws, &data, &fmt, "检测人")
                        .map_err(|e| format!("Sheet5: {}", e))?;
                    apply_sheet_cfg(ws, &tmpl, "sheet5", &fmt)?;
                    export_write::write_period_title(
                        ws,
                        &configured_sheet_title(&tmpl, "sheet5", "检测人汇总", &period),
                        7,
                        &fmt,
                    )
                    .map_err(|e| format!("Sheet5标题: {}", e))?;
                    tracing::info!("Sheet 5 写入完成");
                }
                Err(e) => return Err(format!("Sheet5查询: {}", e)),
            }
        }

        // ========== Sheet 6: 人员汇总表 ==========
        if should_export_sheet(q.sheet_ids.as_deref(), "sheet6", &tmpl)? {
            match export_data::query_sheet6_data(
                &conn,
                start,
                end,
                subject_user_id,
                allowed_division_ids.as_deref(),
            ) {
                Ok(data) => {
                    tracing::info!("Sheet 6 查询完成: {} 行", data.len());
                    let ws = wb.add_worksheet();
                    export_write::write_sheet6(ws, &data, &fmt, "检测人")
                        .map_err(|e| format!("Sheet6: {}", e))?;
                    apply_sheet_cfg(ws, &tmpl, "sheet6", &fmt)?;
                    export_write::write_period_title(
                        ws,
                        &configured_sheet_title(&tmpl, "sheet6", "人员工作量明细", &period),
                        export_write::sheet6_last_col(&data),
                        &fmt,
                    )
                    .map_err(|e| format!("Sheet6标题: {}", e))?;
                    tracing::info!("Sheet 6 写入完成");
                }
                Err(e) => return Err(format!("Sheet6查询: {}", e)),
            }
        }

        // ========== Sheet 12: 辅助工作明细表 ==========
        if should_export_sheet(q.sheet_ids.as_deref(), "sheet12", &tmpl)? {
            match export_data::query_auxiliary_work_details(
                &conn,
                start,
                end,
                subject_user_id,
                allowed_division_ids.as_deref(),
            ) {
                Ok(data) => {
                    tracing::info!("辅助工作明细查询完成: {} 行", data.len());
                    let ws = wb.add_worksheet();
                    export_write::write_auxiliary_work_sheet(ws, &data, &fmt)
                        .map_err(|e| format!("辅助工作明细: {}", e))?;
                    apply_sheet_cfg(ws, &tmpl, "sheet12", &fmt)?;
                    export_write::write_period_title(
                        ws,
                        &configured_sheet_title(&tmpl, "sheet12", "辅助工作明细汇总", &period),
                        export_write::sheet12_last_col(&data),
                        &fmt,
                    )
                    .map_err(|e| format!("辅助工作明细标题: {}", e))?;
                }
                Err(e) => return Err(format!("辅助工作明细查询: {}", e)),
            }
        }

        // ========== Sheet 13: 人员工作量汇总（按检测类型逐行） ==========
        if should_export_sheet(q.sheet_ids.as_deref(), "sheet13", &tmpl)? {
            match export_data::query_person_type_workload_data(
                &conn,
                start,
                end,
                subject_user_id,
                allowed_division_ids.as_deref(),
            ) {
                Ok(data) => {
                    tracing::info!("人员工作量逐行汇总查询完成: {} 行", data.len());
                    let ws = wb.add_worksheet();
                    export_write::write_person_type_workload_sheet(ws, &data, &fmt)
                        .map_err(|e| format!("Sheet13: {}", e))?;
                    apply_sheet_cfg(ws, &tmpl, "sheet13", &fmt)?;
                    export_write::write_period_title(
                        ws,
                        &configured_sheet_title(&tmpl, "sheet13", "人员工作量汇总", &period),
                        export_write::sheet13_last_col(),
                        &fmt,
                    )
                    .map_err(|e| format!("Sheet13标题: {}", e))?;
                }
                Err(e) => return Err(format!("Sheet13查询: {}", e)),
            }
        }

        // ========== Sheet 7: 实验室总表 ==========
        if should_export_sheet(q.sheet_ids.as_deref(), "sheet7", &tmpl)? {
            match export_data::query_sheet7_data(
                &conn,
                start,
                end,
                subject_user_id,
                allowed_division_ids.as_deref(),
            ) {
                Ok(data) => {
                    tracing::info!("Sheet 7 查询完成: {} 行", data.len());
                    let ws = wb.add_worksheet();
                    export_write::write_sheet7(ws, &data, &fmt)
                        .map_err(|e| format!("Sheet7: {}", e))?;
                    apply_sheet_cfg(ws, &tmpl, "sheet7", &fmt)?;
                    export_write::write_period_title(
                        ws,
                        &configured_sheet_title(&tmpl, "sheet7", "实验室总表", &period),
                        export_write::sheet7_last_col(&data),
                        &fmt,
                    )
                    .map_err(|e| format!("Sheet7标题: {}", e))?;
                    tracing::info!("Sheet 7 写入完成");
                }
                Err(e) => return Err(format!("Sheet7查询: {}", e)),
            }
        }

        // ========== Sheet 8: 项目总表 ==========
        if should_export_sheet(q.sheet_ids.as_deref(), "sheet8", &tmpl)? {
            match export_data::query_sheet8_data(
                &conn,
                start,
                end,
                subject_user_id,
                allowed_division_ids.as_deref(),
            ) {
                Ok(data) => {
                    tracing::info!("Sheet 8 查询完成: {} 行", data.len());
                    let ws = wb.add_worksheet();
                    export_write::write_sheet8(ws, &data, &fmt)
                        .map_err(|e| format!("Sheet8: {}", e))?;
                    apply_sheet_cfg(ws, &tmpl, "sheet8", &fmt)?;
                    export_write::write_period_title(
                        ws,
                        &configured_sheet_title(&tmpl, "sheet8", "项目总表", &period),
                        export_write::sheet8_last_col(&data),
                        &fmt,
                    )
                    .map_err(|e| format!("Sheet8标题: {}", e))?;
                    tracing::info!("Sheet 8 写入完成");
                }
                Err(e) => return Err(format!("Sheet8查询: {}", e)),
            }
        }

        // ========== Sheet 9: 仪器汇总表 ==========
        if should_export_sheet(q.sheet_ids.as_deref(), "sheet9", &tmpl)? {
            match export_data::query_sheet9_data(
                &conn,
                start,
                end,
                subject_user_id,
                allowed_division_ids.as_deref(),
            ) {
                Ok(data) => {
                    tracing::info!("Sheet 9 查询完成: {} 行", data.len());
                    let ws = wb.add_worksheet();
                    export_write::write_sheet9(ws, &data, &fmt)
                        .map_err(|e| format!("Sheet9: {}", e))?;
                    apply_sheet_cfg(ws, &tmpl, "sheet9", &fmt)?;
                    export_write::write_period_title(
                        ws,
                        &configured_sheet_title(&tmpl, "sheet9", "仪器汇总表", &period),
                        3,
                        &fmt,
                    )
                    .map_err(|e| format!("Sheet9标题: {}", e))?;
                    tracing::info!("Sheet 9 写入完成");
                }
                Err(e) => return Err(format!("Sheet9查询: {}", e)),
            }
        }

        // ========== Sheet 10: 理化汇总表 ==========
        if should_export_sheet(q.sheet_ids.as_deref(), "sheet10", &tmpl)? {
            match export_data::query_sheet10_data(
                &conn,
                start,
                end,
                subject_user_id,
                allowed_division_ids.as_deref(),
            ) {
                Ok(data) => {
                    tracing::info!("Sheet 10 查询完成: {} 行", data.len());
                    let ws = wb.add_worksheet();
                    export_write::write_sheet10(ws, &data, &fmt)
                        .map_err(|e| format!("Sheet10: {}", e))?;
                    apply_sheet_cfg(ws, &tmpl, "sheet10", &fmt)?;
                    export_write::write_period_title(
                        ws,
                        &configured_sheet_title(&tmpl, "sheet10", "理化汇总表", &period),
                        1,
                        &fmt,
                    )
                    .map_err(|e| format!("Sheet10标题: {}", e))?;
                    tracing::info!("Sheet 10 写入完成");
                }
                Err(e) => return Err(format!("Sheet10查询: {}", e)),
            }
        }

        // ========== Sheet 11: 事业部汇总 (v0.4.28) ==========
        if should_export_sheet(q.sheet_ids.as_deref(), "sheet11", &tmpl)? {
            {
                let div_data = export_data::query_department_pair_summary_data(
                    &conn,
                    start,
                    end,
                    subject_user_id,
                    allowed_division_ids.as_deref(),
                )
                .map_err(|e| format!("事业部查询: {}", e))?;
                tracing::info!("Sheet 11 事业部汇总: {} 行", div_data.len());
                let ws = wb.add_worksheet();
                ws.set_name("事业部汇总")
                    .map_err(|e| format!("Sheet11: {}", e))?;
                ws.set_column_width(0, 16.0)
                    .map_err(|e| format!("Sheet11: {}", e))?;
                ws.set_column_width(1, 16.0)
                    .map_err(|e| format!("Sheet11: {}", e))?;
                ws.set_column_width(2, 10.0)
                    .map_err(|e| format!("Sheet11: {}", e))?;
                ws.set_column_width(3, 10.0)
                    .map_err(|e| format!("Sheet11: {}", e))?;
                ws.set_column_width(4, 10.0)
                    .map_err(|e| format!("Sheet11: {}", e))?;
                ws.set_column_width(5, 10.0)
                    .map_err(|e| format!("Sheet11: {}", e))?;
                for col in 0u16..=5u16 {
                    ws.set_column_format(col, &fmt.fd)
                        .map_err(|e| format!("Sheet11: {}", e))?;
                }
                // 表头
                let headers = [
                    "检测部门",
                    "送样部门",
                    "实验室数",
                    "检测数量",
                    "记录数",
                    "系数分",
                ];
                for (i, h) in headers.iter().enumerate() {
                    ws.write_with_format(1, i as u16, *h, &fmt.fh)
                        .map_err(|e| format!("Sheet11: {}", e))?;
                }
                for (i, row) in div_data.iter().enumerate() {
                    let r = (i + 2) as u32;
                    ws.write_with_format(r, 0, row.detection_department.as_str(), &fmt.fd)
                        .map_err(|e| format!("Sheet11: {}", e))?;
                    ws.write_with_format(r, 1, row.sending_department.as_str(), &fmt.fd)
                        .map_err(|e| format!("Sheet11: {}", e))?;
                    ws.write_with_format(r, 2, row.lab_count as f64, &fmt.fd)
                        .map_err(|e| format!("Sheet11: {}", e))?;
                    ws.write_with_format(r, 3, row.total_quantity as f64, &fmt.fd)
                        .map_err(|e| format!("Sheet11: {}", e))?;
                    ws.write_with_format(r, 4, row.record_count as f64, &fmt.fd)
                        .map_err(|e| format!("Sheet11: {}", e))?;
                    ws.write_with_format(r, 5, row.coefficient_score, &fmt.fd)
                        .map_err(|e| format!("Sheet11: {}", e))?;
                }
                apply_sheet_cfg(ws, &tmpl, "sheet11", &fmt)?;
                export_write::write_period_title(
                    ws,
                    &configured_sheet_title(&tmpl, "sheet11", "检测部门-送样部门汇总", &period),
                    5,
                    &fmt,
                )
                .map_err(|e| format!("Sheet11标题: {}", e))?;
                ws.autofit();
                tracing::info!("Sheet 11 事业部汇总写入完成");
            }
        }

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

#[cfg(test)]
mod tests {
    use super::{
        configured_sheet_title, export_column_order, format_export_period, should_export_sheet,
    };
    use serde_json::json;

    #[test]
    fn export_period_uses_dotted_dates() {
        assert_eq!(
            format_export_period("2026-07-01", "2026-07-31"),
            "2026.07.01-2026.07.31"
        );
    }

    #[test]
    fn export_period_accepts_datetime_values() {
        assert_eq!(
            format_export_period("2026-07-01T00:00:00", "2026-07-31T23:59:59"),
            "2026.07.01-2026.07.31"
        );
    }

    #[test]
    fn configured_title_keeps_custom_base_title_and_appends_period() {
        let template = Some(json!({
            "sheets": {
                "sheet1": {"title": "自定义统计表"}
            }
        }));
        assert_eq!(
            configured_sheet_title(&template, "sheet1", "默认统计表", "2026.07.01-2026.07.31"),
            "自定义统计表（2026.07.01-2026.07.31）"
        );
    }

    #[test]
    fn workload_titles_use_configured_template_names() {
        let template = Some(json!({
            "sheets": {
                "sheet6": {"title": "人员汇总表"},
                "sheet12": {"title": "旧辅助表"},
                "sheet13": {"title": "旧人员表"}
            }
        }));
        assert_eq!(
            configured_sheet_title(
                &template,
                "sheet6",
                "人员工作量明细",
                "2026.08.01-2026.08.31"
            ),
            "人员汇总表（2026.08.01-2026.08.31）"
        );
        assert_eq!(
            configured_sheet_title(
                &template,
                "sheet12",
                "辅助工作明细汇总",
                "2026.08.01-2026.08.31"
            ),
            "旧辅助表（2026.08.01-2026.08.31）"
        );
        assert_eq!(
            configured_sheet_title(
                &template,
                "sheet13",
                "人员工作量汇总",
                "2026.08.01-2026.08.31"
            ),
            "旧人员表（2026.08.01-2026.08.31）"
        );
    }

    #[test]
    fn missing_sheet_selection_means_all_reports() {
        assert!(should_export_sheet(None, "sheet1", &None).unwrap());
        assert!(should_export_sheet(Some(""), "sheet11", &None).unwrap());
    }

    #[test]
    fn selected_sheet_selection_is_restricted() {
        assert!(should_export_sheet(Some("sheet5,sheet11"), "sheet5", &None).unwrap());
        assert!(should_export_sheet(Some("sheet5,sheet11"), "sheet11", &None).unwrap());
        assert!(should_export_sheet(Some("sheet13"), "sheet13", &None).unwrap());
        assert!(!should_export_sheet(Some("sheet5,sheet11"), "sheet1", &None).unwrap());
    }

    #[test]
    fn invalid_sheet_selection_is_rejected() {
        assert!(should_export_sheet(Some("sheet5,unknown"), "sheet5", &None).is_err());
    }

    #[test]
    fn disabled_sheet_is_skipped_for_all_reports_and_rejected_when_explicit() {
        let template = Some(json!({
            "sheets": {
                "sheet3": {"enabled": false}
            }
        }));
        assert!(!should_export_sheet(None, "sheet3", &template).unwrap());
        assert!(should_export_sheet(Some("sheet3"), "sheet3", &template).is_err());
    }

    #[test]
    fn workload_sheet_widths_follow_writer_column_order() {
        assert_eq!(
            export_column_order("sheet6"),
            [
                "user_name",
                "instrument",
                "method_type",
                "method",
                "coefficient",
                "quantity",
                "workload",
                "total_workload",
            ]
        );
        assert_eq!(
            export_column_order("sheet12"),
            [
                "user_name",
                "auxiliary_method",
                "coefficient",
                "quantity",
                "workload",
                "total_workload",
            ]
        );
        assert_eq!(
            export_column_order("sheet13"),
            [
                "user_name",
                "total_workload",
                "method_type",
                "coefficient",
                "quantity",
                "workload",
            ]
        );
    }

    #[test]
    fn duplicate_sheet_title_has_a_distinct_fallback() {
        assert_eq!(
            super::fallback_sheet_name("人员工作量汇总", "sheet6"),
            "人员工作量汇总-sheet6"
        );
    }
}
