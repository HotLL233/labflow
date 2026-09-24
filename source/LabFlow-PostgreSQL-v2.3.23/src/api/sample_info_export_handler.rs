use super::{export_write, sample_info_export_data, sample_info_export_write};
use crate::db::DbPool;
use crate::models::sample_info::SampleInfoScopeFilter;
use crate::repo::sample_info_column_repo;
use crate::repo::settings_repo;
use crate::service::authz_service::{self, RecordScope};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::Response;
/// 样品信息登记导出处理器（独立接口，不接 /stats）
use axum::{
    extract::{Query, State},
    routing::get,
    Router,
};
use chrono::Datelike;
use serde::Deserialize;

fn template_sheet_enabled(template: &Option<serde_json::Value>, sheet_id: &str) -> bool {
    template
        .as_ref()
        .and_then(|value| value.get("sheets"))
        .and_then(|sheets| sheets.get(sheet_id))
        .and_then(|sheet| sheet.get("enabled"))
        .and_then(|value| value.as_bool())
        .unwrap_or(true)
}

fn apply_template_sheet(
    worksheet: &mut rust_xlsxwriter::Worksheet,
    template: &Option<serde_json::Value>,
    sheet_id: &str,
    column_keys: &[&str],
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
        let raw = color.trim_start_matches('#');
        if let Ok(rgb) = u32::from_str_radix(raw, 16) {
            worksheet.set_tab_color(rust_xlsxwriter::Color::RGB(rgb));
        }
    }
    let Some(columns) = sheet.get("columns").and_then(|value| value.as_object()) else {
        return Ok(());
    };
    for (index, key) in column_keys.iter().enumerate() {
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
                .write_with_format(0, index, label, &fmt.fh)
                .map_err(|error| format!("{}列标题: {}", sheet_id, error))?;
        }
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct SampleInfoExportQuery {
    pub start: Option<String>,
    pub end: Option<String>,
    pub type_key: Option<String>,
    /// v2.3.19: 状态筛选。此前导出不支持 status，导致导出结果与页面列表筛选条件不一致。
    pub status: Option<String>,
    pub division_id: Option<i64>,
    pub ownership_basis: Option<String>,
    pub include_pending_ownership: Option<bool>,
}

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/sample-info/export", get(export_excel))
        .with_state(pool)
}

async fn export_excel(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<SampleInfoExportQuery>,
) -> Response {
    use std::io::Cursor;

    let ctx = match authz_service::authenticate(&pool, &headers).and_then(|ctx| {
        authz_service::require_permission(&ctx, "entry:sample-info")?;
        Ok(ctx)
    }) {
        Ok(ctx) => ctx,
        Err(error) => return error_response(StatusCode::FORBIDDEN, &error.to_string()),
    };
    let role_scopes = match authz_service::role_data_scopes(&pool, &ctx) {
        Ok(scopes) => scopes,
        Err(error) => return error_response(StatusCode::FORBIDDEN, &error.to_string()),
    };
    let scoped_export_filters: Vec<SampleInfoScopeFilter> = if role_scopes.is_empty() {
        vec![]
    } else {
        let mut filters: Vec<SampleInfoScopeFilter> = role_scopes
            .into_iter()
            .map(|scope| SampleInfoScopeFilter {
                division_ids: scope.division_ids,
                type_keys: scope.sample_info_type_keys,
                created_by_user_id: None,
                business_user_id: None,
            })
            .collect();
        filters.push(SampleInfoScopeFilter {
            division_ids: vec![],
            type_keys: vec![],
            created_by_user_id: Some(ctx.user.id),
            business_user_id: Some(ctx.user.id),
        });
        filters
    };
    let (subject_user_id, scope_group_id) = if !scoped_export_filters.is_empty() {
        (None, None)
    } else {
        match ctx.rd_scope() {
            Ok(RecordScope::Global | RecordScope::AnalysisAll) => (None, None),
            Ok(RecordScope::Lab(group_id)) => (None, Some(group_id)),
            Ok(RecordScope::Own | RecordScope::PublicCreated) => (Some(ctx.user.id), None),
            Err(error) => return error_response(StatusCode::FORBIDDEN, &error.to_string()),
        }
    };

    let start_str: String;
    let end_str: String;
    let (start, end) = if let Some(ref s) = q.start {
        start_str = s.clone();
        end_str = q.end.as_ref().cloned().unwrap_or_else(|| s.clone());
        (start_str.as_str(), end_str.as_str())
    } else {
        let now = chrono::Local::now();
        start_str = format!("{}-{:02}-01", now.year(), now.month());
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

    let result: std::result::Result<Vec<u8>, String> = (|| {
        let conn = pool.get().map_err(|e| format!("数据库连接失败: {}", e))?;
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| format!("export transaction begin failed: {e}"))?;
        let ownership_basis = match q.ownership_basis.as_deref().unwrap_or("submitted") {
            "execution" | "submitted" | "project" => {
                q.ownership_basis.as_deref().unwrap_or("submitted")
            }
            _ => return Err("样品信息导出口径只能是 execution、submitted 或 project".into()),
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
        tx.execute(
            "SELECT set_config('workload.export_status',?1,true)",
            [q.status.as_deref().unwrap_or("")],
        )
        .map_err(|e| format!("设置导出状态过滤失败: {e}"))?;
        let fmt = export_write::Fmt::new();
        let mut wb = rust_xlsxwriter::Workbook::new();
        let template = settings_repo::get(&pool, "export_template_sample_info")
            .ok()
            .flatten()
            .and_then(|setting| serde_json::from_str::<serde_json::Value>(&setting.value).ok());
        let type_key = q
            .type_key
            .as_deref()
            .filter(|value| !value.trim().is_empty());

        // 指定类型时采用该类型的导出列；全部类型仅保留各类型公共列。
        let columns = if let Some(type_key) = type_key {
            sample_info_column_repo::list_active_by_type(&pool, type_key)
        } else {
            sample_info_column_repo::list_active_common_for_export(&pool)
        }
        .map_err(|e| format!("列配置加载: {}", e))?;

        let raw_detail = sample_info_export_data::query_detail(
            &conn,
            start,
            end,
            subject_user_id,
            scope_group_id,
            type_key,
        )
        .map_err(|e| format!("明细查询: {}", e))?;
        let detail =
            sample_info_export_data::filter_by_role_scopes(raw_detail, &scoped_export_filters);
        if template_sheet_enabled(&template, "detail") {
            let detail_keys = columns
                .iter()
                .filter(|column| column.show_in_export)
                .map(|column| column.field_key.as_str())
                .collect::<Vec<_>>();
            let ws1 = wb.add_worksheet();
            sample_info_export_write::write_sheet_detail(ws1, &detail, &columns, &fmt)
                .map_err(|e| format!("Sheet1: {}", e))?;
            apply_template_sheet(ws1, &template, "detail", &detail_keys, &fmt)?;
        }

        let by_status = if scoped_export_filters.is_empty() {
            sample_info_export_data::query_by_status(
                &conn,
                start,
                end,
                subject_user_id,
                scope_group_id,
                type_key,
            )
            .map_err(|e| format!("状态查询: {}", e))?
        } else {
            sample_info_export_data::summarize_by_status(&detail)
        };
        if template_sheet_enabled(&template, "by_status") {
            let ws2 = wb.add_worksheet();
            sample_info_export_write::write_sheet_by_status(ws2, &by_status, &fmt)
                .map_err(|e| format!("Sheet2: {}", e))?;
            apply_template_sheet(ws2, &template, "by_status", &["status", "count"], &fmt)?;
        }

        let by_type = if scoped_export_filters.is_empty() {
            sample_info_export_data::query_by_type(
                &conn,
                start,
                end,
                subject_user_id,
                scope_group_id,
                type_key,
            )
            .map_err(|e| format!("类型查询: {}", e))?
        } else {
            sample_info_export_data::summarize_by_type(&detail)
        };
        if template_sheet_enabled(&template, "by_type") {
            let ws3 = wb.add_worksheet();
            sample_info_export_write::write_sheet_by_type(ws3, &by_type, &fmt)
                .map_err(|e| format!("Sheet3: {}", e))?;
            apply_template_sheet(
                ws3,
                &template,
                "by_type",
                &["type_name", "type_key", "count"],
                &fmt,
            )?;
        }

        let by_lab = if scoped_export_filters.is_empty() {
            sample_info_export_data::query_by_lab(
                &conn,
                start,
                end,
                subject_user_id,
                scope_group_id,
                type_key,
            )
            .map_err(|e| format!("实验室查询: {}", e))?
        } else {
            sample_info_export_data::summarize_by_lab(&detail)
        };
        if template_sheet_enabled(&template, "by_lab") {
            let ws4 = wb.add_worksheet();
            sample_info_export_write::write_sheet_by_lab(ws4, &by_lab, &fmt)
                .map_err(|e| format!("Sheet4: {}", e))?;
            apply_template_sheet(ws4, &template, "by_lab", &["lab_name", "count"], &fmt)?;
        }

        let by_project = if scoped_export_filters.is_empty() {
            sample_info_export_data::query_by_project(
                &conn,
                start,
                end,
                subject_user_id,
                scope_group_id,
                type_key,
            )
            .map_err(|e| format!("项目查询: {}", e))?
        } else {
            sample_info_export_data::summarize_by_project(&detail)
        };
        if template_sheet_enabled(&template, "by_project") {
            let ws5 = wb.add_worksheet();
            sample_info_export_write::write_sheet_by_project(ws5, &by_project, &fmt)
                .map_err(|e| format!("Sheet5: {}", e))?;
            apply_template_sheet(
                ws5,
                &template,
                "by_project",
                &["project_name", "count"],
                &fmt,
            )?;
        }

        let by_user = if scoped_export_filters.is_empty() {
            sample_info_export_data::query_by_user(
                &conn,
                start,
                end,
                subject_user_id,
                scope_group_id,
                type_key,
            )
            .map_err(|e| format!("送样人查询: {}", e))?
        } else {
            sample_info_export_data::summarize_by_user(&detail)
        };
        if template_sheet_enabled(&template, "by_user") {
            let ws6 = wb.add_worksheet();
            sample_info_export_write::write_sheet_by_user(ws6, &by_user, &fmt)
                .map_err(|e| format!("Sheet6: {}", e))?;
            apply_template_sheet(ws6, &template, "by_user", &["user_name", "count"], &fmt)?;
        }

        let by_month = if scoped_export_filters.is_empty() {
            sample_info_export_data::query_by_month(
                &conn,
                start,
                end,
                subject_user_id,
                scope_group_id,
                type_key,
            )
            .map_err(|e| format!("月份查询: {}", e))?
        } else {
            sample_info_export_data::summarize_by_month(&detail)
        };
        if template_sheet_enabled(&template, "by_month") {
            let ws7 = wb.add_worksheet();
            sample_info_export_write::write_sheet_by_month(ws7, &by_month, &fmt)
                .map_err(|e| format!("Sheet7: {}", e))?;
            apply_template_sheet(ws7, &template, "by_month", &["month", "count"], &fmt)?;
        }

        tx.commit()
            .map_err(|e| format!("export transaction commit failed: {e}"))?;
        let mut buf = Cursor::new(Vec::new());
        wb.save_to_writer(&mut buf)
            .map_err(|e| format!("保存Excel: {}", e))?;
        Ok(buf.into_inner())
    })();

    match result {
        Ok(data) => {
            let desc = format!("导出样品信息登记，时间范围 {} ~ {}", start, end);
            crate::repo::audit_repo::log_actor(
                &pool,
                "export",
                "sample_info_records",
                None,
                ctx.user.id,
                &ctx.user.username,
                &desc,
                "sample_info",
            )
            .ok();
            let filename = format!("样品信息登记_{}_{}.xlsx", start, end);
            let encoded_filename = format!(
                "attachment; filename*=UTF-8''{}",
                url_escape::encode_component(&filename)
            );
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
            tracing::error!("样品信息导出失败: {}", msg);
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
    let error_json = serde_json::json!({"code": status.as_u16(), "message": message});
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(error_json.to_string()))
        .unwrap()
}
