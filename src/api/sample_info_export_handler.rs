use super::{export_write, sample_info_export_data, sample_info_export_write};
use crate::db::DbPool;
use crate::repo::sample_info_column_repo;
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

#[derive(Deserialize)]
pub struct SampleInfoExportQuery {
    pub start: Option<String>,
    pub end: Option<String>,
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
    let (subject_user_id, scope_group_id) = match ctx.rd_scope() {
        Ok(RecordScope::Global | RecordScope::AnalysisAll) => (None, None),
        Ok(RecordScope::Lab(group_id)) => (None, Some(group_id)),
        Ok(RecordScope::Own) => (Some(ctx.user.id), None),
        Err(error) => return error_response(StatusCode::FORBIDDEN, &error.to_string()),
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
        let fmt = export_write::Fmt::new();
        let mut wb = rust_xlsxwriter::Workbook::new();

        // 加载列配置
        let columns = sample_info_column_repo::list_active(&pool)
            .map_err(|e| format!("列配置加载: {}", e))?;

        let detail = sample_info_export_data::query_detail(
            &conn,
            start,
            end,
            subject_user_id,
            scope_group_id,
        )
        .map_err(|e| format!("明细查询: {}", e))?;
        let ws1 = wb.add_worksheet();
        sample_info_export_write::write_sheet_detail(ws1, &detail, &columns, &fmt)
            .map_err(|e| format!("Sheet1: {}", e))?;

        let by_status = sample_info_export_data::query_by_status(
            &conn,
            start,
            end,
            subject_user_id,
            scope_group_id,
        )
        .map_err(|e| format!("状态查询: {}", e))?;
        let ws2 = wb.add_worksheet();
        sample_info_export_write::write_sheet_by_status(ws2, &by_status, &fmt)
            .map_err(|e| format!("Sheet2: {}", e))?;

        let by_type = sample_info_export_data::query_by_type(
            &conn,
            start,
            end,
            subject_user_id,
            scope_group_id,
        )
        .map_err(|e| format!("类型查询: {}", e))?;
        let ws3 = wb.add_worksheet();
        sample_info_export_write::write_sheet_by_type(ws3, &by_type, &fmt)
            .map_err(|e| format!("Sheet3: {}", e))?;

        let by_lab = sample_info_export_data::query_by_lab(
            &conn,
            start,
            end,
            subject_user_id,
            scope_group_id,
        )
        .map_err(|e| format!("实验室查询: {}", e))?;
        let ws4 = wb.add_worksheet();
        sample_info_export_write::write_sheet_by_lab(ws4, &by_lab, &fmt)
            .map_err(|e| format!("Sheet4: {}", e))?;

        let by_project = sample_info_export_data::query_by_project(
            &conn,
            start,
            end,
            subject_user_id,
            scope_group_id,
        )
        .map_err(|e| format!("项目查询: {}", e))?;
        let ws5 = wb.add_worksheet();
        sample_info_export_write::write_sheet_by_project(ws5, &by_project, &fmt)
            .map_err(|e| format!("Sheet5: {}", e))?;

        let by_user = sample_info_export_data::query_by_user(
            &conn,
            start,
            end,
            subject_user_id,
            scope_group_id,
        )
        .map_err(|e| format!("送样人查询: {}", e))?;
        let ws6 = wb.add_worksheet();
        sample_info_export_write::write_sheet_by_user(ws6, &by_user, &fmt)
            .map_err(|e| format!("Sheet6: {}", e))?;

        let by_month = sample_info_export_data::query_by_month(
            &conn,
            start,
            end,
            subject_user_id,
            scope_group_id,
        )
        .map_err(|e| format!("月份查询: {}", e))?;
        let ws7 = wb.add_worksheet();
        sample_info_export_write::write_sheet_by_month(ws7, &by_month, &fmt)
            .map_err(|e| format!("Sheet7: {}", e))?;

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
