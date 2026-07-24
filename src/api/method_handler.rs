use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::import::ImportMapping;
use crate::models::method::*;
use crate::models::project::*;
use crate::models::trash::DeleteReasonRequest;
use crate::models::ApiResponse;
use crate::repo::method_repo;
use axum::{
    extract::{Multipart, Path, Query, State},
    http::HeaderMap,
    Json, Router,
};
use serde::Deserialize;

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/methods", axum::routing::get(list).post(create))
        .route(
            "/api/methods/:id",
            axum::routing::put(update).delete(delete),
        )
        .route("/api/methods/import", axum::routing::post(method_import))
        .route(
            "/api/method-types",
            axum::routing::get(list_method_types).post(create_method_type),
        )
        .route(
            "/api/method-types/:id",
            axum::routing::put(update_method_type).delete(delete_method_type),
        )
        .route("/api/import/mappings", axum::routing::get(list_mappings))
        .with_state(pool)
}

#[derive(Deserialize)]
pub struct MethodQuery {
    pub type_id: Option<i64>,
}

async fn list(
    State(pool): State<DbPool>,
    axum::extract::Query(q): axum::extract::Query<MethodQuery>,
) -> Result<Json<ApiResponse<Vec<MethodResponse>>>> {
    let items = method_repo::list(&pool, q.type_id)?;
    Ok(Json(ApiResponse::ok(items)))
}

fn require_manager(
    pool: &DbPool,
    headers: &HeaderMap,
) -> Result<crate::service::authz_service::AuthContext> {
    let ctx = crate::service::authz_service::authenticate(pool, headers)?;
    crate::service::authz_service::require_permission(&ctx, "manage:methods")?;
    Ok(ctx)
}

async fn create(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(b): Json<MethodCreate>,
) -> Result<Json<ApiResponse<MethodResponse>>> {
    let ctx = require_manager(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(method_repo::create(
        &pool,
        &b,
        &ctx.user.username,
    )?)))
}

async fn update(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(b): Json<MethodUpdate>,
) -> Result<Json<ApiResponse<MethodResponse>>> {
    let ctx = require_manager(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(method_repo::update(
        &pool,
        id,
        &b,
        &ctx.user.username,
    )?)))
}

async fn delete(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(body): Query<DeleteReasonRequest>,
) -> Result<Json<ApiResponse<()>>> {
    let ctx = require_manager(&pool, &headers)?;
    let reason = body
        .reason
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("用户删除");
    method_repo::delete(&pool, id, &ctx.user.username, reason)?;
    Ok(Json(ApiResponse::ok_msg("删除成功")))
}

// ── 方法类型 ──

async fn list_method_types(
    State(pool): State<DbPool>,
) -> Result<Json<ApiResponse<Vec<MethodType>>>> {
    Ok(Json(ApiResponse::ok(method_repo::list_method_types(
        &pool,
    )?)))
}

async fn create_method_type(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(b): Json<MethodTypeCreate>,
) -> Result<Json<ApiResponse<MethodType>>> {
    let ctx = require_manager(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(method_repo::create_method_type(
        &pool,
        &b,
        &ctx.user.username,
    )?)))
}

async fn update_method_type(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(b): Json<MethodTypeUpdate>,
) -> Result<Json<ApiResponse<MethodType>>> {
    let ctx = require_manager(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(method_repo::update_method_type(
        &pool,
        id,
        &b,
        &ctx.user.username,
    )?)))
}

async fn delete_method_type(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(body): Query<DeleteReasonRequest>,
) -> Result<Json<ApiResponse<()>>> {
    let ctx = require_manager(&pool, &headers)?;
    let reason = body
        .reason
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("用户删除");
    method_repo::delete_method_type(&pool, id, &ctx.user.username, reason)?;
    Ok(Json(ApiResponse::ok_msg("删除成功")))
}

// ── 导入 ──

/// v0.2.17: 方法导入 — 从 project_handler 移入，路由 POST /api/methods/import
async fn method_import(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    mut mp: Multipart,
) -> Result<Json<ApiResponse<ImportSummary>>> {
    require_manager(&pool, &headers)?;
    let _ = (pool, &mut mp);
    return Err(AppError::Validation(
        "旧方法导入已停用，请使用“主数据一键导入”下载新版模板，按方法名称、仪器编号和检测类型导入"
            .into(),
    ));
    #[allow(unreachable_code)]
    {
        let mut tp = String::new();
        while let Ok(Some(f)) = mp.next_field().await {
            if f.name() == Some("file") {
                if let Ok(d) = f.bytes().await {
                    if !d.is_empty() {
                        let p =
                            std::env::temp_dir().join(format!("md_{}.xlsx", uuid::Uuid::new_v4()));
                        if std::fs::write(&p, &d).is_ok() {
                            tp = p.to_string_lossy().to_string();
                        }
                    }
                }
            }
        }
        if tp.is_empty() {
            return Err(AppError::Validation("未收到文件".into()));
        }

        use calamine::{open_workbook, Reader, Xlsx};
        let mut wb: Xlsx<_> =
            open_workbook(&tp).map_err(|e| AppError::Validation(format!("打开失败:{}", e)))?;
        let sh = wb.sheet_names().to_vec();
        if sh.is_empty() {
            return Err(AppError::Validation("无工作表".into()));
        }
        let rng = wb
            .worksheet_range(&sh[0])
            .map_err(|e| AppError::Validation(format!("读取失败:{}", e)))?;
        let rows: Vec<Vec<calamine::DataType>> = rng.rows().map(|r| r.to_vec()).collect();
        if rows.len() < 2 {
            return Err(AppError::Validation("至少2行(表头+数据)".into()));
        }

        let headers: Vec<String> = rows[0]
            .iter()
            .map(|v| v.as_string().unwrap_or_default().trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if headers.is_empty() {
            return Err(AppError::Validation("表头为空".into()));
        }

        // v0.3.0: 映射表驱动的列头路由
        let ncols = headers.len().min(rows[0].len());
        let mut method_items: Vec<(String, String, String)> = Vec::new(); // (header, name, method_type)
        let mut project_items: Vec<String> = Vec::new(); // 研发项目名列表
        let mut group_items: Vec<String> = Vec::new(); // 实验室名列表

        // 加载映射配置
        let mappings = {
            let conn = pool.get()?;
            method_repo::load_mappings_from_conn(&conn)?
        };

        for col_idx in 0..ncols {
            let header = &headers[col_idx];
            let matched = mappings
                .iter()
                .find(|m| wildcard_match(&m.header_pattern, header));
            let target = matched
                .map(|m| m.target_table.as_str())
                .unwrap_or("methods");
            let default_type = matched
                .map(|m| m.default_type.clone())
                .unwrap_or_else(|| "其他".to_string());

            match target {
                "project_groups" => {
                    for r in rows.iter().skip(1) {
                        if let Some(v) = r.get(col_idx) {
                            let val = v.as_string().unwrap_or_default().trim().to_string();
                            if !val.is_empty() && !group_items.contains(&val) {
                                group_items.push(val);
                            }
                        }
                    }
                }
                "projects" => {
                    for r in rows.iter().skip(1) {
                        if let Some(v) = r.get(col_idx) {
                            let val = v.as_string().unwrap_or_default().trim().to_string();
                            if !val.is_empty() && !project_items.contains(&val) {
                                project_items.push(val);
                            }
                        }
                    }
                }
                _ => {
                    // methods (兜底)
                    let method_type = if !default_type.is_empty() {
                        &default_type
                    } else {
                        "其他"
                    };
                    for r in rows.iter().skip(1) {
                        if let Some(v) = r.get(col_idx) {
                            let val = v.as_string().unwrap_or_default().trim().to_string();
                            if !val.is_empty() {
                                if !method_items
                                    .iter()
                                    .any(|(g, n, _)| g == header && n == &val)
                                {
                                    method_items.push((
                                        header.clone(),
                                        val,
                                        method_type.to_string(),
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }

        if method_items.is_empty() && project_items.is_empty() && group_items.is_empty() {
            return Err(AppError::Validation("无有效数据".into()));
        }

        let conn = pool.get()?;
        let summary = method_repo::batch_import_column_split(
            &conn,
            &group_items,
            &project_items,
            &method_items,
        )?;
        std::fs::remove_file(&tp).ok();
        Ok(Json(ApiResponse::ok(summary)))
    }
}

/// v0.3.0: 通配符匹配 — * 匹配任意字符序列
fn wildcard_match(pattern: &str, text: &str) -> bool {
    let p = pattern.trim_matches('*');
    if pattern.starts_with('*') && pattern.ends_with('*') {
        text.contains(p)
    } else if pattern.starts_with('*') {
        text.ends_with(p)
    } else if pattern.ends_with('*') {
        text.starts_with(p)
    } else {
        text == p
    }
}

/// v0.3.0: 获取导入映射配置列表
async fn list_mappings(
    State(pool): State<DbPool>,
) -> Result<Json<ApiResponse<Vec<ImportMapping>>>> {
    Ok(Json(ApiResponse::ok(method_repo::load_mappings(&pool)?)))
}
