pub mod article_handler;
pub mod audit_handler;
pub mod auth_handler;
pub mod backup_handler;
pub mod data_governance_handler;
pub mod division_handler;
pub mod docx_parser;
pub mod export_data;
pub mod export_handler;
pub mod export_preview_handler;
pub mod export_write;
pub mod group_handler;
pub mod help_attachment_handler;
pub mod help_handler;
pub mod import_handler;
pub mod instrument_handler;
pub mod log_maintenance_handler;
pub mod master_import_handler;
mod master_import_utils;
pub mod method_handler;
pub mod notification_handler;
pub mod organization_handler;
pub mod pdf_parser;
pub mod pdf_render;
pub mod personnel_change_handler;
pub mod personnel_feedback_handler;
pub mod project_handler;
pub mod rd_export_data;
pub mod rd_export_handler;
pub mod rd_export_preview_handler;
pub mod rd_record_column_handler;
pub mod rd_record_handler;
pub mod rd_stats_handler;
pub mod record_handler;
pub mod role_handler;
pub mod sample_info_attachment_handler;
pub mod sample_info_column_handler;
pub mod sample_info_draft_handler;
pub mod sample_info_export_data;
pub mod sample_info_export_handler;
pub mod sample_info_export_write;
pub mod sample_info_handler;
pub mod sample_info_type_handler;
pub mod session_handler;
pub mod settings_handler;
pub mod stats_handler;
pub mod trace_handler;
pub mod trash_handler;
pub mod user_handler;
pub mod xlsx_parser;

use crate::config::AppConfig;
use crate::db::DbPool;
use crate::models::ApiResponse;
use axum::{routing::get, Json, Router};
use serde::Serialize;
use std::sync::Arc;

#[derive(Serialize)]
struct VersionInfo {
    version: &'static str,
}

#[derive(Serialize)]
struct ServerTimeInfo {
    unix_ms: i64,
    beijing_time: String,
}

#[utoipa::path(get, path = "/api/version", responses((status = 200, body = VersionInfo)))]
async fn version() -> Json<VersionInfo> {
    Json(VersionInfo {
        version: env!("CARGO_PKG_VERSION"),
    })
}

/// Health check 端点（Docker HEALTHCHECK / 负载均衡探活）
async fn health_check() -> Json<ApiResponse<&'static str>> {
    Json(ApiResponse::ok("ok"))
}

async fn server_time() -> Json<ApiResponse<ServerTimeInfo>> {
    let now = chrono::Utc::now();
    let offset = chrono::FixedOffset::east_opt(8 * 60 * 60).expect("valid Beijing offset");
    Json(ApiResponse::ok(ServerTimeInfo {
        unix_ms: now.timestamp_millis(),
        beijing_time: now
            .with_timezone(&offset)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string(),
    }))
}

pub fn api_router(pool: DbPool, config: Arc<AppConfig>) -> Router {
    Router::new()
        .route("/api/version", get(version))
        .route("/api/health", get(health_check))
        .route("/api/server-time", get(server_time))
        .merge(group_handler::router(pool.clone()))
        .merge(division_handler::router(pool.clone()))
        .merge(project_handler::router(pool.clone()))
        .merge(method_handler::router(pool.clone()))
        .merge(notification_handler::router(pool.clone()))
        .merge(organization_handler::router(pool.clone()))
        .merge(instrument_handler::router(pool.clone()))
        .merge(record_handler::router(pool.clone()))
        .merge(rd_record_handler::router(pool.clone()))
        .merge(rd_stats_handler::router(pool.clone()))
        .merge(rd_export_handler::router(pool.clone()))
        .merge(rd_export_preview_handler::router(pool.clone()))
        .merge(stats_handler::router(pool.clone()))
        .merge(export_handler::router(pool.clone()))
        .merge(import_handler::router(pool.clone()))
        .merge(master_import_handler::router(pool.clone()))
        .merge(audit_handler::router(pool.clone()))
        .merge(auth_handler::router(pool.clone()))
        .merge(backup_handler::router(config.clone(), pool.clone()))
        .merge(export_preview_handler::router(pool.clone()))
        .merge(help_handler::router(pool.clone()))
        .merge(help_attachment_handler::router(pool.clone()))
        .merge(article_handler::router(pool.clone()))
        .merge(personnel_change_handler::router(pool.clone()))
        .merge(sample_info_handler::router(pool.clone()))
        .merge(sample_info_draft_handler::router(pool.clone()))
        .merge(sample_info_column_handler::router(pool.clone()))
        .merge(sample_info_type_handler::router(pool.clone()))
        .merge(sample_info_export_handler::router(pool.clone()))
        .merge(sample_info_attachment_handler::router(pool.clone()))
        .merge(user_handler::router(pool.clone(), config.clone()))
        .merge(role_handler::router(pool.clone()))
        .merge(rd_record_column_handler::router(pool.clone()))
        .merge(settings_handler::router(pool.clone()))
        .merge(trace_handler::router(pool.clone()))
        .merge(trash_handler::router(pool.clone(), config.clone()))
        .merge(session_handler::router(pool.clone()))
        .merge(data_governance_handler::router(pool.clone()))
        .merge(log_maintenance_handler::router(
            config.clone(),
            pool.clone(),
        ))
}
