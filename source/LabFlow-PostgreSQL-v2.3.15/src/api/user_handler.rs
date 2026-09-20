use crate::config::AppConfig;
use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::trash::DeleteReasonRequest;
use crate::models::user::{LoginRequest, User, UserCreate, UserUpdate};
use crate::models::ApiResponse;
use crate::repo::{role_repo, user_repo};
use crate::service::auth_service;
use crate::service::authz_service::{
    self, AuthContext, ROLE_ANALYSIS_LEADER, ROLE_ANALYST, ROLE_KEY_ANALYSIS_PUBLIC_ACCOUNT,
    ROLE_KEY_LEGACY_PUBLIC_ACCOUNT, ROLE_KEY_RD_PUBLIC_ACCOUNT, ROLE_RD_LEADER, ROLE_RD_SENDER,
    ROLE_SYSTEM_ADMIN,
};
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{header, HeaderMap, Response, StatusCode},
    Json, Router,
};
use calamine::{open_workbook_from_rs, DataType, Reader};
use rust_xlsxwriter::{
    Color, DataValidation, Format, FormatAlign, FormatBorder, Workbook, XlsxError,
};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    io::Cursor,
    sync::Arc,
};

pub fn router(pool: DbPool, config: Arc<AppConfig>) -> Router {
    Router::new()
        .route("/api/users/login", axum::routing::post(login))
        .route("/api/users/me", axum::routing::get(me))
        .route("/api/users/logout", axum::routing::post(logout))
        .route(
            "/api/users/change-password",
            axum::routing::put(change_password),
        )
        .route(
            "/api/users",
            axum::routing::get(list_users).post(create_user),
        )
        .route("/api/users/export", axum::routing::get(export_users))
        .route("/api/users/rd-senders", axum::routing::get(list_rd_senders))
        .route(
            "/api/users/work-detectors",
            axum::routing::get(list_work_detectors),
        )
        .route(
            "/api/users/analysis-public-scope",
            axum::routing::get(analysis_public_scope),
        )
        .route(
            "/api/users/:id",
            axum::routing::put(update_user).delete(delete_user),
        )
        .route(
            "/api/users/import/template",
            axum::routing::get(download_user_import_template),
        )
        .route(
            "/api/users/import",
            axum::routing::post(import_users).layer(DefaultBodyLimit::max(50 * 1024 * 1024)),
        )
        .with_state((pool, config))
}

#[derive(Deserialize)]
struct RdSenderQuery {
    group_id: i64,
}

#[derive(Deserialize)]
struct WorkDetectorQuery {
    group_id: i64,
}

/// Candidate list for the controlled "actual sender" selector in RD entry.
/// This endpoint deliberately avoids exposing the administrator user directory.
async fn list_rd_senders(
    State((pool, _config)): State<(DbPool, Arc<AppConfig>)>,
    headers: HeaderMap,
    Query(query): Query<RdSenderQuery>,
) -> Result<Json<ApiResponse<Vec<User>>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    if !ctx.has_permission("entry:sample") {
        return Err(AppError::Forbidden("无研发送样权限".into()));
    }
    let conn = pool.get()?;
    let assigned_to_group: i64 = conn.query_row(
        "SELECT COUNT(*) FROM user_groups WHERE user_id=?1 AND group_id=?2",
        postgres_compat::params![ctx.user.id, query.group_id],
        |row| row.get(0),
    )?;
    let can_access_group = ctx.is_system_admin()
        || ctx.has_permission("records:rd:portal-all-labs")
        || ctx.user.group_id == Some(query.group_id)
        || assigned_to_group > 0;
    if !can_access_group {
        return Err(AppError::Forbidden("无权查看该实验室送样人员".into()));
    }
    Ok(Json(ApiResponse::ok(user_repo::list_rd_senders_by_group(
        &pool,
        query.group_id,
    )?)))
}

/// Candidate list for the controlled "actual detector" selector in analysis entry.
async fn list_work_detectors(
    State((pool, _config)): State<(DbPool, Arc<AppConfig>)>,
    headers: HeaderMap,
    Query(query): Query<WorkDetectorQuery>,
) -> Result<Json<ApiResponse<Vec<User>>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "entry:workload")?;
    authz_service::require_permission(&ctx, "records:work:select-detector")?;
    if !authz_service::work_group_allowed(&pool, &ctx, query.group_id)? {
        return Err(AppError::Forbidden("无权在该实验室选择检测人员".into()));
    }
    Ok(Json(ApiResponse::ok(user_repo::list_work_detectors(
        &pool,
    )?)))
}

/// Account-card data for the analysis public-account portal.
async fn analysis_public_scope(
    State((pool, _config)): State<(DbPool, Arc<AppConfig>)>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<crate::models::user::AnalysisPublicAccountScope>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "entry:workload")?;
    authz_service::require_permission(&ctx, "records:work:portal-scoped")?;
    if !ctx.is_analysis_public_account() {
        return Err(AppError::Forbidden(
            "仅分析检测公共账号可查看账号范围".into(),
        ));
    }
    Ok(Json(ApiResponse::ok(
        user_repo::analysis_public_account_scope(&pool, ctx.user.id)?,
    )))
}

/// 从 HeaderMap 中提取 JWT claims
fn extract_claims_from_headers(pool: &DbPool, headers: &HeaderMap) -> Result<auth_service::Claims> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| AppError::Validation("未提供登录凭证".into()))?;
    auth_service::verify_active_token(pool, token)
}

/// POST /api/users/login — 登录
async fn login(
    State((pool, _config)): State<(DbPool, Arc<AppConfig>)>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<ApiResponse<crate::models::user::LoginResponse>>> {
    let resp = auth_service::login(&pool, &body)?;
    Ok(Json(ApiResponse::ok(resp)))
}

/// GET /api/users/me — 获取当前用户信息
async fn me(
    State((pool, _config)): State<(DbPool, Arc<AppConfig>)>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<User>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(ctx.user)))
}

/// GET /api/users — 用户列表（管理员）
async fn list_users(
    State((pool, _config)): State<(DbPool, Arc<AppConfig>)>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<User>>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    require_user_manager(&ctx)?;
    let users = user_repo::list_all(&pool)?;
    Ok(Json(ApiResponse::ok(users)))
}

/// GET /api/users/export - export current users using the active role model.
async fn export_users(
    State((pool, _config)): State<(DbPool, Arc<AppConfig>)>,
    headers: HeaderMap,
) -> Result<Response<Body>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    require_user_manager(&ctx)?;
    let users = user_repo::list_all(&pool)?;
    let conn = pool.get()?;
    let role_map = role_repo::list_all(&pool)?
        .into_iter()
        .map(|role| (role.name.clone(), role))
        .collect::<HashMap<_, _>>();
    let rows = users
        .iter()
        .map(|user| {
            Ok(UserExportRow {
                module: export_user_module(&conn, &role_map, user)?,
                username: user.username.clone(),
                division: user
                    .primary_division_name
                    .clone()
                    .or_else(|| user.division_names.first().cloned())
                    .or_else(|| user.division_name.clone())
                    .unwrap_or_default(),
                group: user
                    .group_names
                    .first()
                    .cloned()
                    .or_else(|| user.group_name.clone())
                    .unwrap_or_default(),
                roles: user.role_names.clone(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let bytes = build_user_export_workbook(&rows).map_err(|e| AppError::Internal(e.to_string()))?;
    let filename = format!("用户列表_v{}.xlsx", env!("CARGO_PKG_VERSION"));
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        )
        .header(
            header::CONTENT_DISPOSITION,
            format!(
                "attachment; filename*=UTF-8''{}",
                url_escape::encode_component(&filename)
            ),
        )
        .body(Body::from(bytes))
        .map_err(|e| AppError::Internal(e.to_string()))?)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UserExportModule {
    Rd,
    Work,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UserExportRow {
    module: UserExportModule,
    username: String,
    division: String,
    group: String,
    roles: Vec<String>,
}

fn export_user_module(
    conn: &postgres_compat::Connection,
    role_map: &HashMap<String, crate::models::role::Role>,
    user: &User,
) -> Result<UserExportModule> {
    let mut has_rd_role = false;
    for role_name in &user.role_names {
        if let Some(role) = role_map.get(role_name) {
            if matches!(
                role_import_module_on_conn(conn, role)?,
                Some(UserImportModule::Rd)
            ) {
                has_rd_role = true;
            }
        }
    }
    Ok(
        if has_rd_role || !user.group_names.is_empty() || user.group_name.is_some() {
            UserExportModule::Rd
        } else {
            UserExportModule::Work
        },
    )
}

fn build_user_export_workbook(rows: &[UserExportRow]) -> std::result::Result<Vec<u8>, XlsxError> {
    let role_count = rows
        .iter()
        .map(|row| row.roles.len())
        .max()
        .unwrap_or(0)
        .max(5);
    let mut workbook = Workbook::new();
    let header_format = Format::new()
        .set_bold()
        .set_font_color(Color::White)
        .set_background_color(Color::RGB(0x1976D2))
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center)
        .set_text_wrap();
    let rd_rows = rows
        .iter()
        .filter(|row| row.module == UserExportModule::Rd)
        .collect::<Vec<_>>();
    let work_rows = rows
        .iter()
        .filter(|row| row.module == UserExportModule::Work)
        .collect::<Vec<_>>();
    write_user_export_sheet(
        &mut workbook,
        "研发送样用户导入",
        &rd_rows,
        true,
        role_count,
        &header_format,
    )?;
    write_user_export_sheet(
        &mut workbook,
        "分析检测用户导入",
        &work_rows,
        false,
        role_count,
        &header_format,
    )?;
    workbook.save_to_buffer()
}

fn write_user_export_sheet(
    workbook: &mut Workbook,
    name: &str,
    rows: &[&UserExportRow],
    is_rd: bool,
    role_count: usize,
    header_format: &Format,
) -> std::result::Result<(), XlsxError> {
    let sheet = workbook.add_worksheet();
    sheet.set_name(name)?;
    let fixed_headers = if is_rd {
        vec!["用户名*", "初始密码", "归属部门*", "所属实验室"]
    } else {
        vec!["用户名*", "初始密码", "主归属部门"]
    };
    let role_headers = (1..=role_count)
        .map(|index| format!("角色{index}"))
        .collect::<Vec<_>>();
    for (column, value) in fixed_headers.iter().enumerate() {
        sheet.write_with_format(0, column as u16, *value, header_format)?;
    }
    for (column, value) in role_headers.iter().enumerate() {
        sheet.write_with_format(
            0,
            (fixed_headers.len() + column) as u16,
            value,
            header_format,
        )?;
    }
    sheet.set_freeze_panes(1, 0)?;
    sheet.autofilter(0, 0, 1000, (fixed_headers.len() + role_count - 1) as u16)?;
    for (row_index, row) in rows.iter().enumerate() {
        let row_index = (row_index + 1) as u32;
        sheet.write_string(row_index, 0, &row.username)?;
        sheet.write_string(row_index, 1, "")?;
        sheet.write_string(row_index, 2, &row.division)?;
        let role_start = if is_rd {
            sheet.write_string(row_index, 3, &row.group)?;
            4
        } else {
            3
        };
        for (role_index, role) in row.roles.iter().enumerate() {
            sheet.write_string(row_index, (role_start + role_index) as u16, role)?;
        }
    }
    for (column, width) in (0..fixed_headers.len() + role_count).map(|column| {
        let width = if column == 0 {
            24.0
        } else if column == 1 {
            18.0
        } else {
            22.0
        };
        (column, width)
    }) {
        sheet.set_column_width(column as u16, width)?;
    }
    Ok(())
}

/// PUT /api/users/:id — 更新用户
/// POST /api/users - admin-only user creation with role_ids support.
async fn create_user(
    State((pool, _config)): State<(DbPool, Arc<AppConfig>)>,
    headers: HeaderMap,
    Json(body): Json<UserCreate>,
) -> Result<Json<ApiResponse<User>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    require_user_manager(&ctx)?;

    if body.username.trim().is_empty() || body.password.is_empty() {
        return Err(AppError::Validation("用户名和密码不能为空".into()));
    }

    let mut body = body;
    let mut assigned = body.role_ids.clone();
    if let Some(role_id) = body.role_id {
        assigned.push(role_id);
    }
    assigned.sort_unstable();
    assigned.dedup();
    let role_keys = role_system_keys(&pool, &assigned)?;
    validate_leader_roles(&pool, &ctx, &assigned)?;
    let is_analysis_public_account = role_keys
        .iter()
        .any(|key| key == ROLE_KEY_ANALYSIS_PUBLIC_ACCOUNT);
    let is_other_public_account =
        has_public_account_role(&role_keys) && !is_analysis_public_account;
    let has_rd_role = role_is_rd(&pool, &assigned)?;
    let mut business_division_ids = if !body.business_division_ids.is_empty() {
        body.business_division_ids.clone()
    } else {
        body.division_ids.clone()
    };
    if is_other_public_account {
        body.group_id = None;
        body.division_id = None;
        body.primary_division_id = None;
        body.group_ids.clear();
        body.division_ids.clear();
        body.business_division_ids.clear();
        business_division_ids.clear();
    } else if is_analysis_public_account {
        body.group_id = None;
        body.group_ids.clear();
        if business_division_ids.is_empty()
            && body.primary_division_id.or(body.division_id).is_none()
        {
            return Err(AppError::Validation(
                "分析检测公共账号必须绑定至少一个所属部门".into(),
            ));
        }
    } else if has_rd_role && body.group_ids.is_empty() && body.group_id.is_none() {
        return Err(AppError::Validation(
            "包含研发送样角色时必须设置所属实验室".into(),
        ));
    }
    if business_division_ids.is_empty() {
        if let Some(division_id) = body.division_id {
            business_division_ids.push(division_id);
        }
    }
    if body.group_ids.is_empty() {
        if let Some(group_id) = body.group_id {
            body.group_ids.push(group_id);
        }
    }
    business_division_ids = normalize_ids(&business_division_ids);
    let primary_division_id = body
        .primary_division_id
        .or(body.division_id)
        .or_else(|| business_division_ids.first().copied());
    if let Some(primary) = primary_division_id {
        if !business_division_ids.contains(&primary) {
            return Err(AppError::Validation(
                "主归属部门必须包含在可承担业务部门中".into(),
            ));
        }
    }
    body.business_division_ids = business_division_ids.clone();
    body.division_ids = business_division_ids;
    body.primary_division_id = primary_division_id;
    body.group_ids = normalize_ids(&body.group_ids);
    validate_user_affiliations(&pool, &body.division_ids, &body.group_ids)?;
    body.division_id = body.primary_division_id;
    body.group_id = body.group_ids.first().copied();
    body.role_id = assigned.first().copied();
    body.role_ids = assigned;
    let password_hash = auth_service::hash_password(&body.password)?;
    let user = user_repo::create(
        &pool,
        &body,
        &password_hash,
        Some((ctx.user.id, &ctx.user.username)),
    )?;
    Ok(Json(ApiResponse::ok(user)))
}

fn has_public_account_role(role_keys: &[String]) -> bool {
    role_keys.iter().any(|key| {
        matches!(
            key.as_str(),
            ROLE_KEY_LEGACY_PUBLIC_ACCOUNT
                | ROLE_KEY_RD_PUBLIC_ACCOUNT
                | ROLE_KEY_ANALYSIS_PUBLIC_ACCOUNT
        )
    })
}

fn role_names(pool: &DbPool, role_ids: &[i64]) -> Result<Vec<String>> {
    let conn = pool.get()?;
    let mut names = Vec::with_capacity(role_ids.len());
    for role_id in role_ids {
        let name = conn
            .query_row("SELECT name FROM roles WHERE id=?1", [role_id], |row| {
                row.get(0)
            })
            .map_err(|_| AppError::Validation("请选择有效角色".into()))?;
        names.push(name);
    }
    Ok(names)
}

fn role_system_keys(pool: &DbPool, role_ids: &[i64]) -> Result<Vec<String>> {
    let conn = pool.get()?;
    let mut keys = Vec::with_capacity(role_ids.len());
    for role_id in role_ids {
        let key: String = conn
            .query_row(
                "SELECT COALESCE(system_key,'') FROM roles WHERE id=?1",
                [role_id],
                |row| row.get(0),
            )
            .map_err(|_| AppError::Validation("请选择有效角色".into()))?;
        keys.push(key);
    }
    Ok(keys)
}

fn role_matches_identity(
    pool: &DbPool,
    role_ids: &[i64],
    role_names: &[&str],
    template_names: &[&str],
) -> Result<bool> {
    if role_ids.is_empty() {
        return Ok(false);
    }
    let ids = role_ids
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let names = role_names
        .iter()
        .map(|name| format!("'{}'", name.replace('\'', "''")))
        .collect::<Vec<_>>()
        .join(",");
    let templates = template_names
        .iter()
        .map(|name| format!("'{}'", name.replace('\'', "''")))
        .collect::<Vec<_>>()
        .join(",");
    let conn = pool.get()?;
    conn.query_row(
        &format!(
            "SELECT EXISTS(
                SELECT 1
                  FROM roles r
                  LEFT JOIN role_templates rt ON rt.id=r.template_id
                 WHERE r.id IN ({ids})
                   AND (r.name IN ({names}) OR rt.name IN ({templates}))
            )"
        ),
        [],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

fn role_is_rd(pool: &DbPool, role_ids: &[i64]) -> Result<bool> {
    role_matches_identity(
        pool,
        role_ids,
        &[ROLE_RD_SENDER, ROLE_RD_LEADER],
        &["研发送样员模板", "研发送样组长模板"],
    )
}

fn role_is_analysis_leader(pool: &DbPool, role_ids: &[i64]) -> Result<bool> {
    role_matches_identity(
        pool,
        role_ids,
        &[ROLE_ANALYSIS_LEADER],
        &["分析检测组长模板"],
    )
}

fn leader_assignable_roles(pool: &DbPool, role_ids: &[i64]) -> Result<bool> {
    if role_ids.is_empty() {
        return Ok(false);
    }
    let ids = role_ids
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let conn = pool.get()?;
    conn.query_row(
        &format!(
            "SELECT NOT EXISTS(
                SELECT 1
                  FROM roles r
                  LEFT JOIN role_templates rt ON rt.id=r.template_id
                 WHERE r.id IN ({ids})
                   AND NOT (
                     r.name IN ('分析检测员','研发送样员','研发送样组长')
                     OR rt.name IN ('分析检测员模板','研发送样员模板','研发送样组长模板')
                   )
            )"
        ),
        [],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

// Name-only helper retained for the import-template path. The final user
// creation/update validation still checks the role template provenance in
// `leader_assignable_roles` before persisting assignments.
fn leader_assignable_role(name: &str) -> bool {
    matches!(name, "分析检测员" | "研发送样员" | "研发送样组长")
}

fn leader_assignable_role_on_conn(
    conn: &postgres_compat::Connection,
    role: &crate::models::role::Role,
) -> Result<bool> {
    if leader_assignable_role(&role.name) {
        return Ok(true);
    }
    let Some(template_id) = role.template_id else {
        return Ok(false);
    };
    conn.query_row(
        "SELECT name IN ('分析检测员模板','研发送样员模板','研发送样组长模板')
         FROM role_templates WHERE id=?1 AND deleted_at IS NULL",
        [template_id],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

fn normalize_ids(values: &[i64]) -> Vec<i64> {
    let mut ids = values.to_vec();
    ids.sort_unstable();
    ids.dedup();
    ids
}

fn validate_user_affiliations(
    pool: &DbPool,
    division_ids: &[i64],
    group_ids: &[i64],
) -> Result<()> {
    let conn = pool.get()?;
    for division_id in division_ids {
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM divisions WHERE id=?1 AND is_active=1",
            [division_id],
            |row| row.get(0),
        )?;
        if exists == 0 {
            return Err(AppError::Validation(format!(
                "部门不存在或已停用: {}",
                division_id
            )));
        }
    }
    for group_id in group_ids {
        let division_id: Option<i64> = conn
            .query_row(
                "SELECT division_id FROM project_groups WHERE id=?1",
                [group_id],
                |row| row.get(0),
            )
            .map_err(|_| AppError::Validation(format!("实验室不存在: {}", group_id)))?;
        if let Some(division_id) = division_id {
            if !division_ids.contains(&division_id) {
                return Err(AppError::Validation(
                    "所选实验室必须属于已选择的部门".into(),
                ));
            }
        }
    }
    Ok(())
}

fn require_user_manager(ctx: &AuthContext) -> Result<()> {
    if ctx.is_system_admin() || ctx.has_permission("manage:users") {
        Ok(())
    } else {
        Err(AppError::Forbidden("无用户管理权限".into()))
    }
}

fn validate_leader_roles(pool: &DbPool, ctx: &AuthContext, role_ids: &[i64]) -> Result<()> {
    if ctx.is_system_admin() {
        return Ok(());
    }
    if !leader_assignable_roles(pool, role_ids)? {
        return Err(AppError::Forbidden(
            "分析检测组长只能分配分析检测员、研发送样员和研发送样组长角色".into(),
        ));
    }
    Ok(())
}

fn target_is_protected(pool: &DbPool, target: &User) -> Result<bool> {
    let analysis_leader = role_is_analysis_leader(pool, &target.role_ids)?;
    Ok(target.is_admin
        || target
            .role_names
            .iter()
            .any(|name| name == ROLE_SYSTEM_ADMIN)
        || analysis_leader)
}

async fn update_user(
    State((pool, _config)): State<(DbPool, Arc<AppConfig>)>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    Json(mut body): Json<UserUpdate>,
) -> Result<Json<ApiResponse<User>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    require_user_manager(&ctx)?;
    let target =
        user_repo::find_by_id(&pool, id)?.ok_or_else(|| AppError::NotFound("用户不存在".into()))?;
    if body.is_admin.is_some() {
        return Err(AppError::Validation(
            "管理员权限请通过“系统管理员”角色配置".into(),
        ));
    }
    if id == ctx.user.id && !ctx.is_system_admin() {
        if body.role_id.is_some()
            || body.role_ids.is_some()
            || body.is_admin.is_some()
            || body.is_active.is_some()
            || body.division_id.is_some()
            || body.primary_division_id.is_some()
            || body.group_id.is_some()
            || body.division_ids.is_some()
            || body.business_division_ids.is_some()
            || body.group_ids.is_some()
        {
            return Err(AppError::Forbidden("只能修改自己的用户名和密码".into()));
        }
    } else if !ctx.is_system_admin() {
        if target_is_protected(&pool, &target)? {
            return Err(AppError::Forbidden(
                "不能编辑系统管理员或其他分析检测组长".into(),
            ));
        }
        if body.is_admin == Some(true) {
            return Err(AppError::Forbidden("不能授予系统管理员权限".into()));
        }
    }
    if let Some(ref mut assigned) = body.role_ids {
        assigned.sort_unstable();
        assigned.dedup();
        let names = role_names(&pool, assigned)?;
        let role_keys = role_system_keys(&pool, assigned)?;
        validate_leader_roles(&pool, &ctx, assigned)?;
        let has_system_admin_role = names.iter().any(|name| name == ROLE_SYSTEM_ADMIN);
        let is_analysis_public_account = role_keys
            .iter()
            .any(|key| key == ROLE_KEY_ANALYSIS_PUBLIC_ACCOUNT);
        let is_other_public_account =
            has_public_account_role(&role_keys) && !is_analysis_public_account;
        let has_rd_role = role_is_rd(&pool, assigned)?;
        if is_other_public_account {
            body.group_id = Some(None);
            body.division_id = Some(None);
            body.primary_division_id = Some(None);
            body.group_ids = Some(vec![]);
            body.division_ids = Some(vec![]);
            body.business_division_ids = Some(vec![]);
        } else if is_analysis_public_account {
            body.group_id = Some(None);
            body.group_ids = Some(vec![]);
            let has_division = body
                .business_division_ids
                .as_ref()
                .map(|ids| !ids.is_empty())
                .unwrap_or(!target.division_ids.is_empty() || target.division_id.is_some());
            if !has_division {
                return Err(AppError::Validation(
                    "分析检测公共账号必须绑定至少一个所属部门".into(),
                ));
            }
        } else if has_rd_role
            && !has_system_admin_role
            && body
                .group_ids
                .as_ref()
                .map(|ids| ids.is_empty())
                .unwrap_or(target.group_ids.is_empty())
            && body.group_id.flatten().or(target.group_id).is_none()
        {
            return Err(AppError::Validation(
                "包含研发送样角色时必须设置所属实验室".into(),
            ));
        }
        body.role_id = Some(assigned.first().copied());
    }
    let resulting_analysis_public_account = body
        .role_ids
        .as_ref()
        .map(|ids| role_system_keys(&pool, ids))
        .transpose()?
        .map(|keys| {
            keys.iter()
                .any(|key| key == ROLE_KEY_ANALYSIS_PUBLIC_ACCOUNT)
        })
        .unwrap_or(target.is_analysis_public_account);
    if resulting_analysis_public_account {
        body.group_id = Some(None);
        body.group_ids = Some(vec![]);
    }
    if body.business_division_ids.is_some()
        || body.primary_division_id.is_some()
        || body.division_ids.is_some()
        || body.group_ids.is_some()
        || body.division_id.is_some()
        || body.group_id.is_some()
    {
        let division_ids = body
            .business_division_ids
            .clone()
            .or_else(|| body.division_ids.clone())
            .or_else(|| {
                body.primary_division_id
                    .or(body.division_id)
                    .map(|value| value.into_iter().collect())
            })
            .unwrap_or_else(|| target.business_division_ids.clone());
        let division_ids = normalize_ids(&division_ids);
        let primary_division_id = body
            .primary_division_id
            .or(body.division_id)
            .flatten()
            .or(target.primary_division_id)
            .or_else(|| division_ids.first().copied());
        if let Some(primary) = primary_division_id {
            if !division_ids.contains(&primary) {
                return Err(AppError::Validation(
                    "主归属部门必须包含在可承担业务部门中".into(),
                ));
            }
        }
        let group_ids = body
            .group_ids
            .clone()
            .or_else(|| body.group_id.map(|value| value.into_iter().collect()))
            .unwrap_or_else(|| target.group_ids.clone());
        let group_ids = normalize_ids(&group_ids);
        validate_user_affiliations(&pool, &division_ids, &group_ids)?;
        body.division_ids = Some(division_ids.clone());
        body.business_division_ids = Some(division_ids.clone());
        body.group_ids = Some(group_ids.clone());
        body.primary_division_id = Some(primary_division_id);
        body.division_id = Some(primary_division_id);
        body.group_id = Some(group_ids.first().copied());
    }
    let user = user_repo::update(&pool, id, &body, ctx.user.id, &ctx.user.username)?;
    Ok(Json(ApiResponse::ok(user)))
}

/// DELETE /api/users/:id — 删除用户（软删除）
async fn delete_user(
    State((pool, _config)): State<(DbPool, Arc<AppConfig>)>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    axum::extract::Query(body): axum::extract::Query<DeleteReasonRequest>,
) -> Result<Json<ApiResponse<()>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    require_user_manager(&ctx)?;
    if id == ctx.user.id {
        return Err(AppError::Forbidden("不能停用当前登录账号".into()));
    }
    let target =
        user_repo::find_by_id(&pool, id)?.ok_or_else(|| AppError::NotFound("用户不存在".into()))?;
    if !ctx.is_system_admin() && target_is_protected(&pool, &target)? {
        return Err(AppError::Forbidden(
            "不能停用系统管理员或分析检测组长".into(),
        ));
    }
    let reason = body
        .reason
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("用户删除");
    user_repo::soft_delete(&pool, id, &ctx.user.username, reason)?;
    Ok(Json(ApiResponse::ok_msg("删除成功")))
}

/// POST /api/users/logout — 登出
async fn logout(
    State((pool, _config)): State<(DbPool, Arc<AppConfig>)>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<()>>> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");

    if !token.is_empty() {
        auth_service::logout(&pool, token)?;
    }
    Ok(Json(ApiResponse::ok_msg("已登出")))
}

/// PUT /api/users/change-password — 修改密码
#[derive(Deserialize)]
struct ChangePasswordRequest {
    old_password: String,
    new_password: String,
}

async fn change_password(
    State((pool, _config)): State<(DbPool, Arc<AppConfig>)>,
    headers: HeaderMap,
    Json(body): Json<ChangePasswordRequest>,
) -> Result<Json<ApiResponse<()>>> {
    let claims = extract_claims_from_headers(&pool, &headers)?;

    if body.old_password.is_empty() || body.new_password.is_empty() {
        return Err(AppError::Validation("密码不能为空".into()));
    }
    if body.new_password.len() < 4 {
        return Err(AppError::Validation("新密码至少4位".into()));
    }

    let user = user_repo::find_by_id(&pool, claims.sub)?
        .ok_or_else(|| AppError::NotFound("用户不存在".into()))?;

    // 验证旧密码
    if !auth_service::verify_password(&body.old_password, &user.password) {
        return Err(AppError::Validation("旧密码错误".into()));
    }

    // 哈希新密码
    let new_hash = auth_service::hash_password(&body.new_password)?;

    let conn = pool.get()?;
    conn.execute(
        "UPDATE users SET password = ?1, updated_at = to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS') WHERE id = ?2",
        postgres_compat::params![new_hash, claims.sub],
    )?;

    Ok(Json(ApiResponse::ok_msg("密码修改成功")))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum UserImportModule {
    Rd,
    Work,
}

impl UserImportModule {
    fn sheet_name(&self) -> &'static str {
        match self {
            Self::Rd => "研发送样用户导入",
            Self::Work => "分析检测用户导入",
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Rd => "研发送样",
            Self::Work => "分析检测",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UserImportRow {
    module: UserImportModule,
    row: usize,
    username: String,
    password: String,
    divisions: Vec<String>,
    groups: Vec<String>,
    division: String,
    group: String,
    roles: Vec<String>,
}

fn role_import_module_on_conn(
    conn: &postgres_compat::Connection,
    role: &crate::models::role::Role,
) -> Result<Option<UserImportModule>> {
    let system_key: Option<String> = conn
        .query_row(
            "SELECT NULLIF(system_key,'') FROM roles WHERE id=?1",
            [role.id],
            |row| row.get(0),
        )
        .ok();
    if matches!(
        system_key.as_deref(),
        Some(ROLE_KEY_ANALYSIS_PUBLIC_ACCOUNT)
    ) || matches!(
        role.name.as_str(),
        ROLE_ANALYST | ROLE_ANALYSIS_LEADER | "分析检测公共账号"
    ) {
        return Ok(Some(UserImportModule::Work));
    }
    if matches!(
        system_key.as_deref(),
        Some(ROLE_KEY_LEGACY_PUBLIC_ACCOUNT | ROLE_KEY_RD_PUBLIC_ACCOUNT)
    ) || matches!(
        role.name.as_str(),
        ROLE_RD_SENDER | ROLE_RD_LEADER | "研发送样公共账号"
    ) {
        return Ok(Some(UserImportModule::Rd));
    }
    let template_name: Option<String> = role.template_id.and_then(|template_id| {
        conn.query_row(
            "SELECT name FROM role_templates WHERE id=?1 AND deleted_at IS NULL",
            [template_id],
            |row| row.get(0),
        )
        .ok()
    });
    Ok(match template_name.as_deref() {
        Some("分析检测员模板") | Some("分析检测组长模板") => {
            Some(UserImportModule::Work)
        }
        Some("研发送样员模板") | Some("研发送样组长模板") => {
            Some(UserImportModule::Rd)
        }
        _ => None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IgnoredRoleReason {
    Missing,
    Forbidden,
}

fn resolve_import_roles(
    requested: &[String],
    role_map: &HashMap<String, crate::models::role::Role>,
    can_assign_all: bool,
    assignable_role_ids: &HashSet<i64>,
) -> (Vec<i64>, Vec<(String, IgnoredRoleReason)>) {
    let mut role_ids = Vec::new();
    let mut ignored = Vec::new();
    for role_name in requested {
        match role_map.get(role_name) {
            Some(role) if can_assign_all || assignable_role_ids.contains(&role.id) => {
                role_ids.push(role.id);
            }
            Some(_) => ignored.push((role_name.clone(), IgnoredRoleReason::Forbidden)),
            None => ignored.push((role_name.clone(), IgnoredRoleReason::Missing)),
        }
    }
    role_ids.sort_unstable();
    role_ids.dedup();
    (role_ids, ignored)
}

async fn download_user_import_template(
    State((pool, _config)): State<(DbPool, Arc<AppConfig>)>,
    headers: HeaderMap,
) -> Result<Response<Body>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    require_user_manager(&ctx)?;

    let conn = pool.get()?;
    let mut rd_roles = Vec::new();
    let mut work_roles = Vec::new();
    for role in role_repo::list_all(&pool)? {
        if ctx.is_system_admin() || leader_assignable_role_on_conn(&conn, &role)? {
            match role_import_module_on_conn(&conn, &role)? {
                Some(UserImportModule::Rd) => rd_roles.push(role),
                Some(UserImportModule::Work) => work_roles.push(role),
                None => {}
            }
        }
    }
    let divisions = query_active_names(
        &conn,
        "SELECT name FROM divisions WHERE is_active=1 ORDER BY sort_order, id",
    )?;
    let groups = query_active_names(
        &conn,
        "SELECT name FROM project_groups WHERE show_in_work=1 OR show_in_rd=1 ORDER BY sort_order, id",
    )?;
    let bytes = build_user_import_template(&rd_roles, &work_roles, &divisions, &groups)?;
    let detail = format!(
        "下载用户导入模板: 研发送样角色 {} 个, 分析检测角色 {} 个, {} 个部门, {} 个实验室",
        rd_roles.len(),
        work_roles.len(),
        divisions.len(),
        groups.len()
    );
    crate::repo::audit_repo::log_actor(
        &pool,
        "template",
        "users",
        None,
        ctx.user.id,
        &ctx.user.username,
        &detail,
        "shared",
    )?;

    let version = env!("CARGO_PKG_VERSION");
    let filename = format!("用户导入模板_v{version}.xlsx");
    let ascii_filename = format!("user_import_v{version}.xlsx");
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        )
        .header(
            header::CONTENT_DISPOSITION,
            format!(
                "attachment; filename={ascii_filename}; filename*=UTF-8''{}",
                url_escape::encode_component(&filename)
            ),
        )
        .body(Body::from(bytes))
        .map_err(|error| AppError::Internal(format!("构建模板响应失败: {error}")))?)
}

fn query_active_names(conn: &postgres_compat::Connection, sql: &str) -> Result<Vec<String>> {
    let mut statement = conn.prepare(sql)?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

fn build_user_import_template(
    rd_roles: &[crate::models::role::Role],
    work_roles: &[crate::models::role::Role],
    divisions: &[String],
    groups: &[String],
) -> std::result::Result<Vec<u8>, XlsxError> {
    let mut workbook = Workbook::new();
    let header_format = Format::new()
        .set_bold()
        .set_font_color(Color::White)
        .set_background_color(Color::RGB(0x1976D2))
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center)
        .set_text_wrap();
    let note_format = Format::new()
        .set_background_color(Color::RGB(0xE3F2FD))
        .set_text_wrap();
    let example_format = Format::new()
        .set_font_color(Color::RGB(0x607D8B))
        .set_background_color(Color::RGB(0xF5F5F5));

    let rd_role = rd_roles
        .first()
        .map(|role| role.name.as_str())
        .unwrap_or("");
    let work_role = work_roles
        .first()
        .map(|role| role.name.as_str())
        .unwrap_or("");
    let division = divisions.first().map(String::as_str).unwrap_or("");
    let group = groups.first().map(String::as_str).unwrap_or("");

    {
        let sheet = workbook.add_worksheet();
        sheet.set_name("研发送样用户导入")?;
        let headers = [
            "用户名*",
            "初始密码",
            "归属部门*",
            "所属实验室",
            "角色1",
            "角色2",
            "角色3",
            "角色4",
            "角色5",
        ];
        for (column, value) in headers.iter().enumerate() {
            sheet.write_with_format(0, column as u16, *value, &header_format)?;
        }
        for (column, width) in [24.0, 18.0, 22.0, 24.0, 22.0, 22.0, 22.0, 22.0, 22.0]
            .iter()
            .enumerate()
        {
            sheet.set_column_width(column as u16, *width)?;
        }
        sheet.set_freeze_panes(1, 0)?;
        sheet.autofilter(0, 0, 1000, 8)?;
        let values = [
            "示例-不导入-送样001",
            "123456",
            division,
            group,
            rd_role,
            "",
            "",
            "",
            "",
        ];
        for (column, value) in values.iter().enumerate() {
            sheet.write_with_format(1, column as u16, *value, &example_format)?;
        }
    }

    {
        let sheet = workbook.add_worksheet();
        sheet.set_name("分析检测用户导入")?;
        let headers = [
            "用户名*",
            "初始密码",
            "主归属部门",
            "角色1",
            "角色2",
            "角色3",
            "角色4",
            "角色5",
        ];
        for (column, value) in headers.iter().enumerate() {
            sheet.write_with_format(0, column as u16, *value, &header_format)?;
        }
        for (column, width) in [24.0, 18.0, 22.0, 22.0, 22.0, 22.0, 22.0, 22.0]
            .iter()
            .enumerate()
        {
            sheet.set_column_width(column as u16, *width)?;
        }
        sheet.set_freeze_panes(1, 0)?;
        sheet.autofilter(0, 0, 1000, 7)?;
        let values = [
            "示例-不导入-分析001",
            "123456",
            division,
            work_role,
            "",
            "",
            "",
            "",
        ];
        for (column, value) in values.iter().enumerate() {
            sheet.write_with_format(1, column as u16, *value, &example_format)?;
        }
    }

    {
        let sheet = workbook.add_worksheet();
        sheet.set_name("研发送样角色候选")?;
        for (column, value) in ["角色名称", "角色说明"].iter().enumerate() {
            sheet.write_with_format(0, column as u16, *value, &header_format)?;
        }
        for (row, role) in rd_roles.iter().enumerate() {
            sheet.write_string((row + 1) as u32, 0, &role.name)?;
            sheet.write_string((row + 1) as u32, 1, &role.description)?;
        }
        sheet.set_column_width(0, 24.0)?;
        sheet.set_column_width(1, 52.0)?;
        sheet.set_freeze_panes(1, 0)?;
        sheet.set_hidden(true);
    }

    {
        let sheet = workbook.add_worksheet();
        sheet.set_name("分析检测角色候选")?;
        for (column, value) in ["角色名称", "角色说明"].iter().enumerate() {
            sheet.write_with_format(0, column as u16, *value, &header_format)?;
        }
        for (row, role) in work_roles.iter().enumerate() {
            sheet.write_string((row + 1) as u32, 0, &role.name)?;
            sheet.write_string((row + 1) as u32, 1, &role.description)?;
        }
        sheet.set_column_width(0, 24.0)?;
        sheet.set_column_width(1, 52.0)?;
        sheet.set_freeze_panes(1, 0)?;
        sheet.set_hidden(true);
    }

    write_name_candidate_sheet(&mut workbook, "部门候选", divisions, &header_format)?;
    write_name_candidate_sheet(&mut workbook, "实验室候选", groups, &header_format)?;

    {
        let sheet = workbook.add_worksheet();
        sheet.set_name("填写说明")?;
        sheet.set_column_width(0, 24.0)?;
        sheet.set_column_width(1, 88.0)?;
        sheet.write_with_format(0, 0, "项目", &header_format)?;
        sheet.write_with_format(0, 1, "填写规则", &header_format)?;
        let notes = [
            ("用户名*", "必填，实际登录仍使用用户名；默认跳过同名用户，选择更新模式后仅更新其归属和角色。"),
            ("初始密码", "可留空，留空时使用默认密码 123456；密码至少 6 位。"),
            ("研发送样用户导入", "每个用户只能填写一个归属部门；所属实验室按现有研发送样规则填写。只能选择研发送样角色。"),
            ("分析检测用户导入", "主归属部门仅表示人员组织归属，不控制分析检测数据范围。分析检测数据范围完全由所选角色的数据来源部门决定；不填写实验室。"),
            ("角色1-角色5", "支持多角色。研发送样表只能选择研发送样角色，分析检测表只能选择分析检测角色。角色名称会重新映射为内部角色 ID。"),
            ("数据范围", "分析检测普通用户的可见、录入、统计和导出范围按角色中的分析检测数据来源部门执行；没有显式范围时默认无权查看业务数据。"),
            ("角色未匹配", "角色不存在、已删除、跨业务模块或当前账号无权分配时，该行导入失败并显示具体原因，不再静默混入另一业务。"),
            ("示例行", "用户名以“示例-”开头的行仅用于演示，导入时不会写入数据库。"),
            ("结果报告", "导入完成后分别显示研发送样和分析检测的成功、更新、跳过和错误数量。"),
        ];
        for (row, (item, rule)) in notes.iter().enumerate() {
            sheet.write_with_format((row + 1) as u32, 0, *item, &note_format)?;
            sheet.write_with_format((row + 1) as u32, 1, *rule, &note_format)?;
        }
        sheet.set_hidden(true);
    }

    workbook.define_name(
        "RdRoleOptions",
        "=OFFSET('研发送样角色候选'!$A$2,0,0,MAX(1,COUNTA('研发送样角色候选'!$A:$A)-1),1)",
    )?;
    workbook.define_name(
        "WorkRoleOptions",
        "=OFFSET('分析检测角色候选'!$A$2,0,0,MAX(1,COUNTA('分析检测角色候选'!$A:$A)-1),1)",
    )?;
    workbook.define_name(
        "DivisionOptions",
        "=OFFSET('部门候选'!$A$2,0,0,MAX(1,COUNTA('部门候选'!$A:$A)-1),1)",
    )?;
    workbook.define_name(
        "GroupOptions",
        "=OFFSET('实验室候选'!$A$2,0,0,MAX(1,COUNTA('实验室候选'!$A:$A)-1),1)",
    )?;
    let rd_role_validation = DataValidation::new().allow_list_formula("=RdRoleOptions".into());
    let work_role_validation = DataValidation::new().allow_list_formula("=WorkRoleOptions".into());
    let division_validation = DataValidation::new().allow_list_formula("=DivisionOptions".into());
    let group_validation = DataValidation::new().allow_list_formula("=GroupOptions".into());
    let rd_sheet = workbook.worksheet_from_name("研发送样用户导入")?;
    for column in 4..=8 {
        rd_sheet.add_data_validation(1, column, 1000, column, &rd_role_validation)?;
    }
    rd_sheet.add_data_validation(1, 2, 1000, 2, &division_validation)?;
    rd_sheet.add_data_validation(1, 3, 1000, 3, &group_validation)?;
    let work_sheet = workbook.worksheet_from_name("分析检测用户导入")?;
    for column in 3..=7 {
        work_sheet.add_data_validation(1, column, 1000, column, &work_role_validation)?;
    }
    work_sheet.add_data_validation(1, 2, 1000, 2, &division_validation)?;

    workbook.save_to_buffer()
}

fn write_name_candidate_sheet(
    workbook: &mut Workbook,
    name: &str,
    values: &[String],
    header_format: &Format,
) -> std::result::Result<(), XlsxError> {
    let sheet = workbook.add_worksheet();
    sheet.set_name(name)?;
    sheet.write_with_format(0, 0, "名称", header_format)?;
    for (row, value) in values.iter().enumerate() {
        sheet.write_string((row + 1) as u32, 0, value)?;
    }
    sheet.set_column_width(0, 32.0)?;
    sheet.set_freeze_panes(1, 0)?;
    sheet.set_hidden(true);
    Ok(())
}

fn cell_string(cell: Option<&DataType>) -> String {
    cell.map(ToString::to_string)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn normalize_import_header(value: &str) -> String {
    value.trim().trim_end_matches('*').trim().to_string()
}

fn split_affiliation_values(value: &str) -> Vec<String> {
    value
        .split(['、', ',', '，', ';', '；', '|'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn parse_user_import_sheet(
    range: &calamine::Range<DataType>,
    module: UserImportModule,
) -> Result<Vec<UserImportRow>> {
    let sheet_name = module.sheet_name();
    let mut rows = range.rows();
    let header_row = rows
        .next()
        .ok_or_else(|| AppError::Validation(format!("工作表“{sheet_name}”缺少表头")))?;
    let headers = header_row
        .iter()
        .map(|cell| normalize_import_header(&cell.to_string()))
        .collect::<Vec<_>>();
    let find_column = |names: &[&str]| {
        headers
            .iter()
            .position(|header| names.iter().any(|name| header == name))
    };
    let username_column = find_column(&["用户名", "登录用户名", "账号"])
        .ok_or_else(|| AppError::Validation(format!("工作表“{sheet_name}”缺少“用户名*”列")))?;
    let password_column = find_column(&["初始密码", "密码"]);
    let division_column = match module {
        UserImportModule::Rd => find_column(&["归属部门", "主归属部门", "所属部门", "部门"]),
        UserImportModule::Work => find_column(&["主归属部门", "归属部门", "所属部门", "部门"]),
    };
    if matches!(module, UserImportModule::Rd) && division_column.is_none() {
        return Err(AppError::Validation(format!(
            "工作表“{sheet_name}”缺少“归属部门*”列"
        )));
    }
    let group_column = find_column(&["所属实验室", "实验室"]);
    let role_columns = headers
        .iter()
        .enumerate()
        .filter_map(|(index, header)| {
            (header == "角色" || header.starts_with("角色")).then_some(index)
        })
        .collect::<Vec<_>>();
    if role_columns.is_empty() {
        return Err(AppError::Validation(format!(
            "工作表“{sheet_name}”至少需要一列角色，例如“角色1”"
        )));
    }

    let mut result = Vec::new();
    for (index, row) in rows.enumerate() {
        let row_number = index + 2;
        let username = cell_string(row.get(username_column));
        if username.is_empty() || username.starts_with("示例-") {
            continue;
        }
        let mut roles = role_columns
            .iter()
            .map(|column| cell_string(row.get(*column)))
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        roles.sort();
        roles.dedup();

        let division = division_column
            .map(|column| cell_string(row.get(column)))
            .unwrap_or_default();
        let divisions = split_affiliation_values(&division);
        if divisions.len() > 1 {
            return Err(AppError::Validation(format!(
                "工作表“{sheet_name}”第{row_number}行只能填写一个归属部门"
            )));
        }
        let mut groups = group_column
            .map(|column| split_affiliation_values(&cell_string(row.get(column))))
            .unwrap_or_default();
        groups.sort();
        groups.dedup();
        if matches!(module, UserImportModule::Work) && !groups.is_empty() {
            return Err(AppError::Validation(format!(
                "工作表“{sheet_name}”第{row_number}行不允许填写实验室"
            )));
        }
        result.push(UserImportRow {
            module,
            row: row_number,
            username,
            password: password_column
                .map(|column| cell_string(row.get(column)))
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "123456".into()),
            division: divisions.first().cloned().unwrap_or_default(),
            divisions,
            group: groups.first().cloned().unwrap_or_default(),
            groups,
            roles,
        });
    }
    Ok(result)
}

fn parse_user_import_workbook(bytes: &[u8]) -> Result<Vec<UserImportRow>> {
    let mut workbook: calamine::Xlsx<_> = open_workbook_from_rs(Cursor::new(bytes))
        .map_err(|error| AppError::Validation(format!("无法打开 Excel 文件: {error}")))?;
    let mut result = Vec::new();
    let mut found_new_sheet = false;
    for module in [UserImportModule::Rd, UserImportModule::Work] {
        if let Ok(range) = workbook.worksheet_range(module.sheet_name()) {
            found_new_sheet = true;
            result.extend(parse_user_import_sheet(&range, module)?);
        }
    }
    if !found_new_sheet {
        if let Ok(range) = workbook.worksheet_range("用户导入") {
            result.extend(parse_legacy_user_import_workbook_range(&range)?);
        } else {
            return Err(AppError::Validation(
                "Excel 中缺少“研发送样用户导入”或“分析检测用户导入”工作表".into(),
            ));
        }
    }
    if result.is_empty() {
        return Err(AppError::Validation(
            "模板中没有可导入的用户数据；示例行不会参与导入".into(),
        ));
    }
    let mut usernames = HashMap::new();
    for row in &result {
        if let Some(previous) = usernames.insert(row.username.clone(), row.module) {
            return Err(AppError::Validation(format!(
                "用户名“{}”同时出现在{}和{}工作表中，请只保留一处",
                row.username,
                previous.label(),
                row.module.label()
            )));
        }
    }
    Ok(result)
}

fn parse_legacy_user_import_workbook_range(
    range: &calamine::Range<DataType>,
) -> Result<Vec<UserImportRow>> {
    let mut rows = range.rows();
    let header_row = rows
        .next()
        .ok_or_else(|| AppError::Validation("工作表“用户导入”缺少表头".into()))?;
    let headers = header_row
        .iter()
        .map(|cell| normalize_import_header(&cell.to_string()))
        .collect::<Vec<_>>();
    let find_column = |names: &[&str]| {
        headers
            .iter()
            .position(|header| names.iter().any(|name| header == name))
    };
    let username_column = find_column(&["用户名", "登录用户名", "账号"])
        .ok_or_else(|| AppError::Validation("工作表“用户导入”缺少“用户名*”列".into()))?;
    let password_column = find_column(&["初始密码", "密码"]);
    let primary_division_column = find_column(&["主归属部门"]);
    let legacy_division_column = find_column(&["所属部门", "部门"]);
    let group_column = find_column(&["所属实验室", "实验室"]);
    let business_division_columns = headers
        .iter()
        .enumerate()
        .filter_map(|(index, header)| header.starts_with("可承担业务部门").then_some(index))
        .collect::<Vec<_>>();
    let role_columns = headers
        .iter()
        .enumerate()
        .filter_map(|(index, header)| {
            (header == "角色" || header.starts_with("角色")).then_some(index)
        })
        .collect::<Vec<_>>();
    if role_columns.is_empty() {
        return Err(AppError::Validation(
            "工作表“用户导入”至少需要一列角色，例如“角色1”".into(),
        ));
    }

    let mut result = Vec::new();
    for (index, row) in rows.enumerate() {
        let username = cell_string(row.get(username_column));
        if username.is_empty() || username.starts_with("示例-") {
            continue;
        }
        let mut roles = role_columns
            .iter()
            .map(|column| cell_string(row.get(*column)))
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        roles.sort();
        roles.dedup();
        let mut divisions = business_division_columns
            .iter()
            .flat_map(|column| split_affiliation_values(&cell_string(row.get(*column))))
            .collect::<Vec<_>>();
        let primary_division = primary_division_column
            .or(legacy_division_column)
            .map(|column| cell_string(row.get(column)))
            .filter(|value| !value.is_empty())
            .or_else(|| divisions.first().cloned())
            .unwrap_or_default();
        if !primary_division.is_empty() && !divisions.contains(&primary_division) {
            divisions.push(primary_division.clone());
        }
        divisions.sort();
        divisions.dedup();
        let mut groups = group_column
            .map(|column| split_affiliation_values(&cell_string(row.get(column))))
            .unwrap_or_default();
        groups.sort();
        groups.dedup();
        let module = if groups.is_empty() {
            UserImportModule::Work
        } else {
            UserImportModule::Rd
        };
        result.push(UserImportRow {
            module,
            row: index + 2,
            username,
            password: password_column
                .map(|column| cell_string(row.get(column)))
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "123456".into()),
            division: primary_division,
            divisions,
            group: groups.first().cloned().unwrap_or_default(),
            groups,
            roles,
        });
    }
    Ok(result)
}

async fn import_users(
    State((pool, _config)): State<(DbPool, Arc<AppConfig>)>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<ApiResponse<serde_json::Value>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    require_user_manager(&ctx)?;
    let mut file_data = Vec::new();
    let mut file_name = String::new();
    let mut update_existing = false;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Internal(format!("读取上传文件失败: {}", e)))?
    {
        let field_name = field.name().unwrap_or("").to_string();
        if field_name == "file" {
            file_name = field.file_name().unwrap_or("").to_string();
            file_data = field
                .bytes()
                .await
                .map_err(|e| AppError::Internal(format!("读取文件内容失败: {}", e)))?
                .to_vec();
        } else if field_name == "update_existing" {
            update_existing = field
                .text()
                .await
                .map(|value| value == "true")
                .unwrap_or(false);
        }
    }
    if file_data.is_empty() {
        return Err(AppError::Validation("未选择文件".into()));
    }
    if !file_name.to_ascii_lowercase().ends_with(".xlsx") {
        return Err(AppError::Validation(
            "请选择系统下载的 .xlsx 用户导入模板".into(),
        ));
    }
    let rows = parse_user_import_workbook(&file_data)?;
    let mut created = 0i64;
    let mut updated = 0i64;
    let mut skipped = 0i64;
    let mut roles_mapped = 0i64;
    let mut roles_ignored = 0i64;
    let mut users_without_roles = 0i64;
    let mut module_counts: HashMap<UserImportModule, [i64; 3]> = HashMap::new();
    let mut errors: Vec<String> = vec![];
    let mut warnings: Vec<String> = vec![];
    let conn = pool.get()?;
    let role_map = role_repo::list_all(&pool)?
        .into_iter()
        .map(|role| (role.name.clone(), role))
        .collect::<HashMap<_, _>>();
    let mut role_modules = HashMap::new();
    for role in role_map.values() {
        if let Some(module) = role_import_module_on_conn(&conn, role)? {
            role_modules.insert(role.name.clone(), module);
        }
    }
    let mut assignable_role_ids = HashSet::new();
    for role in role_map.values() {
        if leader_assignable_role_on_conn(&conn, role)? {
            assignable_role_ids.insert(role.id);
        }
    }
    for row in rows {
        if row.password.len() < 6 {
            errors.push(format!("行{}: 密码长度不足 6 位", row.row));
            skipped += 1;
            module_counts.entry(row.module).or_default()[2] += 1;
            continue;
        }
        let existing_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM users WHERE username=?1",
                [&row.username],
                |r| r.get(0),
            )
            .ok();
        if existing_id.is_some() && !update_existing {
            errors.push(format!(
                "行{}: 用户名「{}」已存在，已跳过",
                row.row, row.username
            ));
            skipped += 1;
            module_counts.entry(row.module).or_default()[2] += 1;
            continue;
        }
        let mut row_errors = Vec::new();
        if matches!(row.module, UserImportModule::Rd) {
            if row.division.is_empty() {
                row_errors.push(format!(
                    "{}第{}行: 研发送样用户必须填写唯一归属部门",
                    row.module.label(),
                    row.row
                ));
            }
            if row.divisions.len() > 1 {
                row_errors.push(format!(
                    "{}第{}行: 研发送样用户只能填写一个归属部门",
                    row.module.label(),
                    row.row
                ));
            }
        }
        for role_name in &row.roles {
            if let Some(module) = role_modules.get(role_name) {
                if *module != row.module {
                    row_errors.push(format!(
                        "{}第{}行: 角色「{}」属于{}，不能导入{}用户表",
                        row.module.label(),
                        row.row,
                        role_name,
                        module.label(),
                        row.module.label()
                    ));
                }
            }
        }
        let division_id: Option<i64> = if !row.division.is_empty() {
            match conn.query_row(
                "SELECT id FROM divisions WHERE name=?1",
                [&row.division],
                |r| r.get(0),
            ) {
                Ok(id) => Some(id),
                Err(_) => {
                    row_errors.push(format!("行{}: 部门「{}」不存在", row.row, row.division));
                    None
                }
            }
        } else {
            None
        };
        let group_id: Option<i64> = if !row.group.is_empty() {
            match conn.query_row(
                "SELECT id FROM project_groups WHERE name=?1",
                [&row.group],
                |r| r.get(0),
            ) {
                Ok(id) => Some(id),
                Err(_) => {
                    row_errors.push(format!("行{}: 实验室「{}」不存在", row.row, row.group));
                    None
                }
            }
        } else {
            None
        };
        let mut division_ids = Vec::new();
        for division in &row.divisions {
            match conn.query_row("SELECT id FROM divisions WHERE name=?1", [division], |r| {
                r.get(0)
            }) {
                Ok(id) => division_ids.push(id),
                Err(_) => row_errors.push(format!("部门不存在: {}", division)),
            }
        }
        if division_ids.is_empty() {
            if let Some(division_id) = division_id {
                division_ids.push(division_id);
            }
        }
        let mut group_ids = Vec::new();
        for group in &row.groups {
            match conn.query_row(
                "SELECT id FROM project_groups WHERE name=?1",
                [group],
                |r| r.get(0),
            ) {
                Ok(id) => group_ids.push(id),
                Err(_) => row_errors.push(format!("实验室不存在: {}", group)),
            }
        }
        if group_ids.is_empty() {
            if let Some(group_id) = group_id {
                group_ids.push(group_id);
            }
        }
        let (role_ids, ignored_roles) = resolve_import_roles(
            &row.roles,
            &role_map,
            ctx.is_system_admin(),
            &assignable_role_ids,
        );
        for (role_name, reason) in ignored_roles {
            match reason {
                IgnoredRoleReason::Forbidden => {
                    warnings.push(format!(
                        "行{}: 当前账号无权分配角色「{}」，已跳过并留空",
                        row.row, role_name
                    ));
                }
                IgnoredRoleReason::Missing => {
                    warnings.push(format!(
                        "行{}: 角色「{}」在系统中不存在，已跳过并留空",
                        row.row, role_name
                    ));
                }
            }
            roles_ignored += 1;
        }
        if !row_errors.is_empty() {
            errors.extend(row_errors);
            skipped += 1;
            module_counts.entry(row.module).or_default()[2] += 1;
            continue;
        }
        let has_rd_role = role_is_rd(&pool, &role_ids)?;
        if matches!(row.module, UserImportModule::Rd) && has_rd_role && group_ids.is_empty() {
            errors.push(format!("行{}: 研发送样角色必须设置所属实验室", row.row));
            skipped += 1;
            module_counts.entry(row.module).or_default()[2] += 1;
            continue;
        }
        let mapped_role_count = role_ids.len() as i64;
        let has_no_roles = role_ids.is_empty();
        let data = UserCreate {
            username: row.username.clone(),
            password: row.password.clone(),
            division_id: division_ids.first().copied(),
            primary_division_id: division_ids.first().copied(),
            group_id: group_ids.first().copied(),
            business_division_ids: division_ids.clone(),
            division_ids,
            group_ids,
            role_id: role_ids.first().copied(),
            role_ids: role_ids.clone(),
        };
        if let Some(id) = existing_id {
            let update = crate::models::user::UserUpdate {
                username: None,
                password: None,
                division_id: Some(data.division_id),
                primary_division_id: Some(data.primary_division_id),
                group_id: Some(data.group_id),
                division_ids: Some(data.division_ids.clone()),
                business_division_ids: Some(data.business_division_ids.clone()),
                group_ids: Some(data.group_ids.clone()),
                is_admin: None,
                is_active: None,
                role_id: None,
                role_ids: (!role_ids.is_empty()).then_some(role_ids),
            };
            match user_repo::update(&pool, id, &update, ctx.user.id, &ctx.user.username) {
                Ok(_) => {
                    updated += 1;
                    module_counts.entry(row.module).or_default()[1] += 1;
                    roles_mapped += mapped_role_count;
                }
                Err(error) => {
                    errors.push(format!("行{}: {}", row.row, error));
                    skipped += 1;
                    module_counts.entry(row.module).or_default()[2] += 1;
                }
            }
            continue;
        }
        let hashed = auth_service::hash_password(&row.password)?;
        match user_repo::create(
            &pool,
            &data,
            &hashed,
            Some((ctx.user.id, &ctx.user.username)),
        ) {
            Ok(_) => {
                created += 1;
                module_counts.entry(row.module).or_default()[0] += 1;
                roles_mapped += mapped_role_count;
                if has_no_roles {
                    users_without_roles += 1;
                }
            }
            Err(error) => {
                errors.push(format!("行{}: {}", row.row, error));
                skipped += 1;
                module_counts.entry(row.module).or_default()[2] += 1;
            }
        }
    }
    let detail = format!(
        "批量导入用户: 新增 {} 条, 更新 {} 条, 跳过 {} 条, 映射角色 {} 个, 忽略角色 {} 个, 无角色用户 {} 个",
        created, updated, skipped, roles_mapped, roles_ignored, users_without_roles
    );
    crate::repo::audit_repo::log_actor(
        &pool,
        "import",
        "users",
        None,
        ctx.user.id,
        &ctx.user.username,
        &detail,
        "shared",
    )?;
    let result = serde_json::json!({
        "created": created,
        "updated": updated,
        "skipped": skipped,
        "roles_mapped": roles_mapped,
        "roles_ignored": roles_ignored,
        "users_without_roles": users_without_roles,
        "modules": {
            "rd": {
                "created": module_counts.get(&UserImportModule::Rd).map(|counts| counts[0]).unwrap_or(0),
                "updated": module_counts.get(&UserImportModule::Rd).map(|counts| counts[1]).unwrap_or(0),
                "skipped": module_counts.get(&UserImportModule::Rd).map(|counts| counts[2]).unwrap_or(0)
            },
            "work": {
                "created": module_counts.get(&UserImportModule::Work).map(|counts| counts[0]).unwrap_or(0),
                "updated": module_counts.get(&UserImportModule::Work).map(|counts| counts[1]).unwrap_or(0),
                "skipped": module_counts.get(&UserImportModule::Work).map(|counts| counts[2]).unwrap_or(0)
            }
        },
        "warnings": warnings,
        "errors": errors
    });
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg(test)]
mod user_import_tests {
    use super::*;

    #[test]
    fn parses_multiple_role_columns_and_skips_example_rows() {
        let mut workbook = Workbook::new();
        let sheet = workbook.add_worksheet();
        sheet.set_name("用户导入").unwrap();
        for (column, value) in [
            "用户名*",
            "初始密码",
            "所属部门",
            "所属实验室",
            "角色1",
            "角色2",
        ]
        .iter()
        .enumerate()
        {
            sheet.write_string(0, column as u16, *value).unwrap();
        }
        sheet.write_string(1, 0, "示例-不导入").unwrap();
        sheet.write_string(2, 0, "user001").unwrap();
        sheet.write_string(2, 1, "654321").unwrap();
        sheet.write_string(2, 2, "研究院").unwrap();
        sheet.write_string(2, 3, "实验室01").unwrap();
        sheet.write_string(2, 4, "分析检测员").unwrap();
        sheet.write_string(2, 5, "自定义角色").unwrap();
        let bytes = workbook.save_to_buffer().unwrap();

        let rows = parse_user_import_workbook(&bytes).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].username, "user001");
        assert_eq!(rows[0].password, "654321");
        assert_eq!(rows[0].roles, vec!["分析检测员", "自定义角色"]);
    }

    #[test]
    fn template_contains_current_role_candidates() {
        let rd_roles = vec![crate::models::role::Role {
            id: 7,
            name: "研发送样员".into(),
            description: "研发送样测试".into(),
            is_system: 0,
            sort_order: 10,
            template_id: None,
        }];
        let work_roles = vec![crate::models::role::Role {
            id: 8,
            name: "分析检测员".into(),
            description: "模板候选测试".into(),
            is_system: 1,
            sort_order: 11,
            template_id: None,
        }];
        let bytes = build_user_import_template(
            &rd_roles,
            &work_roles,
            &["研究院".into()],
            &["实验室01".into()],
        )
        .unwrap();
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes.clone())).unwrap();
        let mut workbook_xml = String::new();
        std::io::Read::read_to_string(
            &mut archive.by_name("xl/workbook.xml").unwrap(),
            &mut workbook_xml,
        )
        .unwrap();
        assert_eq!(workbook_xml.matches("<sheet ").count(), 7);
        assert!(workbook_xml.contains("name=\"研发送样用户导入\""));
        assert!(workbook_xml.contains("name=\"分析检测用户导入\""));
        assert!(workbook_xml.contains("name=\"研发送样角色候选\""));
        assert_eq!(workbook_xml.matches("state=\"hidden\"").count(), 5);
        let mut workbook: calamine::Xlsx<_> =
            open_workbook_from_rs(Cursor::new(bytes.clone())).unwrap();
        let rd_range = workbook.worksheet_range("研发送样角色候选").unwrap();
        assert_eq!(
            rd_range.get_value((1, 0)).unwrap().to_string(),
            "研发送样员"
        );
        let work_range = workbook.worksheet_range("分析检测角色候选").unwrap();
        assert_eq!(
            work_range.get_value((1, 0)).unwrap().to_string(),
            "分析检测员"
        );

        let rd_import = workbook.worksheet_range("研发送样用户导入").unwrap();
        assert_eq!(
            rd_import.get_value((0, 2)).unwrap().to_string(),
            "归属部门*"
        );
        assert_eq!(
            rd_import.get_value((0, 3)).unwrap().to_string(),
            "所属实验室"
        );
        let work_import = workbook.worksheet_range("分析检测用户导入").unwrap();
        assert_eq!(
            work_import.get_value((0, 2)).unwrap().to_string(),
            "主归属部门"
        );
        assert!(work_import
            .get_value((0, 3))
            .unwrap()
            .to_string()
            .starts_with("角色"));
        assert!(work_import.get_value((0, 8)).is_none());
    }

    #[test]
    fn parses_both_module_sheets_and_keeps_analysis_without_lab() {
        let mut workbook = Workbook::new();
        let rd = workbook.add_worksheet();
        rd.set_name("研发送样用户导入").unwrap();
        for (column, value) in ["用户名*", "初始密码", "归属部门*", "所属实验室", "角色1"]
            .iter()
            .enumerate()
        {
            rd.write_string(0, column as u16, *value).unwrap();
        }
        for (column, value) in ["rd001", "654321", "研究院", "实验室01", "研发送样员"]
            .iter()
            .enumerate()
        {
            rd.write_string(1, column as u16, *value).unwrap();
        }
        let work = workbook.add_worksheet();
        work.set_name("分析检测用户导入").unwrap();
        for (column, value) in ["用户名*", "初始密码", "主归属部门", "角色1"]
            .iter()
            .enumerate()
        {
            work.write_string(0, column as u16, *value).unwrap();
        }
        for (column, value) in ["work001", "654321", "研究院", "分析检测员"]
            .iter()
            .enumerate()
        {
            work.write_string(1, column as u16, *value).unwrap();
        }
        let bytes = workbook.save_to_buffer().unwrap();
        let rows = parse_user_import_workbook(&bytes).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|row| {
            row.module == UserImportModule::Rd
                && row.username == "rd001"
                && row.division == "研究院"
                && row.groups == vec!["实验室01"]
        }));
        assert!(rows.iter().any(|row| {
            row.module == UserImportModule::Work
                && row.username == "work001"
                && row.groups.is_empty()
        }));
    }

    #[test]
    fn exported_workbook_uses_import_sheet_format() {
        let bytes = build_user_export_workbook(&[
            UserExportRow {
                module: UserExportModule::Rd,
                username: "rd001".into(),
                division: "研究院".into(),
                group: "实验室01".into(),
                roles: vec!["研发送样员".into(), "研发送样组长".into()],
            },
            UserExportRow {
                module: UserExportModule::Work,
                username: "work001".into(),
                division: "研究院".into(),
                group: String::new(),
                roles: vec!["分析检测员".into()],
            },
        ])
        .unwrap();
        let mut workbook: calamine::Xlsx<_> =
            open_workbook_from_rs(Cursor::new(bytes.clone())).unwrap();
        assert!(workbook.worksheet_range("研发送样用户导入").is_ok());
        assert!(workbook.worksheet_range("分析检测用户导入").is_ok());
        let rows = parse_user_import_workbook(&bytes).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|row| {
            row.username == "rd001"
                && row.module == UserImportModule::Rd
                && row.roles == vec!["研发送样员", "研发送样组长"]
        }));
        assert!(rows.iter().any(|row| {
            row.username == "work001"
                && row.module == UserImportModule::Work
                && row.roles == vec!["分析检测员"]
        }));
    }

    #[test]
    fn role_resolution_maps_existing_and_ignores_missing_or_forbidden() {
        let role_map = [
            crate::models::role::Role {
                id: 1,
                name: ROLE_ANALYST.into(),
                description: String::new(),
                is_system: 1,
                sort_order: 1,
                template_id: None,
            },
            crate::models::role::Role {
                id: 2,
                name: "自定义角色A".into(),
                description: String::new(),
                is_system: 0,
                sort_order: 2,
                template_id: None,
            },
        ]
        .into_iter()
        .map(|role| (role.name.clone(), role))
        .collect::<HashMap<_, _>>();
        let requested = vec![
            ROLE_ANALYST.into(),
            "自定义角色A".into(),
            "不存在角色".into(),
        ];

        let (leader_roles, leader_ignored) = resolve_import_roles(
            &requested,
            &role_map,
            false,
            &std::collections::HashSet::from([1]),
        );
        assert_eq!(leader_roles, vec![1]);
        assert_eq!(
            leader_ignored,
            vec![
                ("自定义角色A".into(), IgnoredRoleReason::Forbidden),
                ("不存在角色".into(), IgnoredRoleReason::Missing),
            ]
        );

        let (admin_roles, admin_ignored) = resolve_import_roles(
            &requested,
            &role_map,
            true,
            &std::collections::HashSet::from([1]),
        );
        assert_eq!(admin_roles, vec![1, 2]);
        assert_eq!(
            admin_ignored,
            vec![("不存在角色".into(), IgnoredRoleReason::Missing)]
        );
    }
}
