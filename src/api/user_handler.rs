use crate::config::AppConfig;
use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::trash::DeleteReasonRequest;
use crate::models::user::{LoginRequest, User, UserCreate, UserUpdate};
use crate::models::ApiResponse;
use crate::repo::{role_repo, user_repo};
use crate::service::auth_service;
use crate::service::authz_service::{
    self, AuthContext, ROLE_ANALYSIS_LEADER, ROLE_ANALYST, ROLE_RD_LEADER, ROLE_RD_SENDER,
    ROLE_SYSTEM_ADMIN,
};
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::{header, HeaderMap, Response, StatusCode},
    Json, Router,
};
use calamine::{open_workbook_from_rs, DataType, Reader};
use rust_xlsxwriter::{
    Color, DataValidation, Format, FormatAlign, FormatBorder, Workbook, XlsxError,
};
use serde::Deserialize;
use std::{collections::HashMap, io::Cursor, sync::Arc};

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

/// 从 HeaderMap 中提取 JWT claims
fn extract_claims_from_headers(pool: &DbPool, headers: &HeaderMap) -> Result<auth_service::Claims> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| AppError::Validation("未提供登录凭证".into()))?;
    auth_service::verify_active_token(pool, token)
}

/// 校验管理员权限
fn require_admin(claims: &auth_service::Claims) -> Result<()> {
    if !claims.is_admin {
        return Err(AppError::Forbidden("需要管理员权限".into()));
    }
    Ok(())
}

/// POST /api/users/register — 注册用户
async fn register(
    State((pool, _config)): State<(DbPool, Arc<AppConfig>)>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApiResponse<User>>> {
    let username = body
        .get("username")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Validation("用户名不能为空".into()))?
        .trim()
        .to_string();
    let password = body
        .get("password")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Validation("密码不能为空".into()))?
        .to_string();
    let division_id = body.get("division_id").and_then(|v| v.as_i64());
    let group_id = body.get("group_id").and_then(|v| v.as_i64());

    if username.is_empty() || password.is_empty() {
        return Err(AppError::Validation("用户名和密码不能为空".into()));
    }

    let user = auth_service::register(&pool, &username, &password, division_id, group_id)?;
    Ok(Json(ApiResponse::ok(user)))
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
    let names = role_names(&pool, &assigned)?;
    validate_leader_roles(&ctx, &names)?;
    let has_rd_role = names
        .iter()
        .any(|name| matches!(name.as_str(), ROLE_RD_SENDER | ROLE_RD_LEADER));
    if has_rd_role && body.group_id.is_none() {
        return Err(AppError::Validation(
            "包含研发送样角色时必须设置所属实验室".into(),
        ));
    }
    if !has_rd_role {
        body.group_id = None;
    }
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

fn leader_assignable_role(name: &str) -> bool {
    matches!(name, ROLE_ANALYST | ROLE_RD_SENDER | ROLE_RD_LEADER)
}

fn require_user_manager(ctx: &AuthContext) -> Result<()> {
    if ctx.is_system_admin() || ctx.is_analysis_leader() {
        Ok(())
    } else {
        Err(AppError::Forbidden("无用户管理权限".into()))
    }
}

fn validate_leader_roles(ctx: &AuthContext, names: &[String]) -> Result<()> {
    if ctx.is_system_admin() {
        return Ok(());
    }
    if names.is_empty() || names.iter().any(|name| !leader_assignable_role(name)) {
        return Err(AppError::Forbidden(
            "分析检测组长只能分配分析检测员、研发送样员和研发送样组长角色".into(),
        ));
    }
    Ok(())
}

fn target_is_protected(target: &User) -> bool {
    target.is_admin
        || target
            .role_names
            .iter()
            .any(|name| matches!(name.as_str(), ROLE_SYSTEM_ADMIN | ROLE_ANALYSIS_LEADER))
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
            || body.group_id.is_some()
        {
            return Err(AppError::Forbidden("只能修改自己的用户名和密码".into()));
        }
    } else if !ctx.is_system_admin() {
        if target_is_protected(&target) {
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
        validate_leader_roles(&ctx, &names)?;
        let has_system_admin_role = names.iter().any(|name| name == ROLE_SYSTEM_ADMIN);
        let has_rd_role = names
            .iter()
            .any(|name| matches!(name.as_str(), ROLE_RD_SENDER | ROLE_RD_LEADER));
        if has_rd_role
            && !has_system_admin_role
            && body.group_id.flatten().or(target.group_id).is_none()
        {
            return Err(AppError::Validation(
                "包含研发送样角色时必须设置所属实验室".into(),
            ));
        }
        if !has_rd_role {
            body.group_id = Some(None);
        }
        body.role_id = Some(assigned.first().copied());
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
    if !ctx.is_system_admin() && target_is_protected(&target) {
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct UserImportRow {
    row: usize,
    username: String,
    password: String,
    division: String,
    group: String,
    roles: Vec<String>,
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
) -> (Vec<i64>, Vec<(String, IgnoredRoleReason)>) {
    let mut role_ids = Vec::new();
    let mut ignored = Vec::new();
    for role_name in requested {
        match role_map.get(role_name) {
            Some(role) if can_assign_all || leader_assignable_role(&role.name) => {
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

    let roles = role_repo::list_all(&pool)?
        .into_iter()
        .filter(|role| ctx.is_system_admin() || leader_assignable_role(&role.name))
        .collect::<Vec<_>>();
    let conn = pool.get()?;
    let divisions = query_active_names(
        &conn,
        "SELECT name FROM divisions WHERE is_active=1 ORDER BY sort_order, id",
    )?;
    let groups = query_active_names(
        &conn,
        "SELECT name FROM project_groups WHERE show_in_work=1 OR show_in_rd=1 ORDER BY sort_order, id",
    )?;
    let bytes = build_user_import_template(&roles, &divisions, &groups)?;
    let detail = format!(
        "下载用户导入模板: {} 个可分配角色, {} 个部门, {} 个实验室",
        roles.len(),
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

    let filename = "用户批量导入模板_v0.4.105.xlsx";
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        )
        .header(
            header::CONTENT_DISPOSITION,
            format!(
                "attachment; filename=user_import_v0.4.105.xlsx; filename*=UTF-8''{}",
                url_escape::encode_component(filename)
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
    roles: &[crate::models::role::Role],
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

    {
        let sheet = workbook.add_worksheet();
        sheet.set_name("用户导入")?;
        let headers = [
            "用户名*",
            "初始密码",
            "所属部门",
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
        for (column, width) in [24.0, 18.0, 20.0, 24.0, 22.0, 22.0, 22.0, 22.0, 22.0]
            .iter()
            .enumerate()
        {
            sheet.set_column_width(column as u16, *width)?;
        }
        sheet.set_freeze_panes(1, 0)?;
        sheet.autofilter(0, 0, 1000, 8)?;

        let analyst_role = roles
            .iter()
            .find(|role| role.name == ROLE_ANALYST)
            .or_else(|| roles.first())
            .map(|role| role.name.as_str())
            .unwrap_or("");
        let rd_role = roles
            .iter()
            .find(|role| role.name == ROLE_RD_SENDER)
            .or_else(|| roles.first())
            .map(|role| role.name.as_str())
            .unwrap_or("");
        let division = divisions.first().map(String::as_str).unwrap_or("");
        let group = groups.first().map(String::as_str).unwrap_or("");
        let examples = [
            [
                "示例-不导入-分析001",
                "123456",
                "",
                "",
                analyst_role,
                "",
                "",
                "",
                "",
            ],
            [
                "示例-不导入-送样001",
                "123456",
                division,
                group,
                rd_role,
                "",
                "",
                "",
                "",
            ],
        ];
        for (row, values) in examples.iter().enumerate() {
            for (column, value) in values.iter().enumerate() {
                sheet.write_with_format(
                    (row + 1) as u32,
                    column as u16,
                    *value,
                    &example_format,
                )?;
            }
        }
    }

    {
        let sheet = workbook.add_worksheet();
        sheet.set_name("角色候选")?;
        for (column, value) in ["角色名称", "角色说明"].iter().enumerate() {
            sheet.write_with_format(0, column as u16, *value, &header_format)?;
        }
        for (row, role) in roles.iter().enumerate() {
            sheet.write_string((row + 1) as u32, 0, &role.name)?;
            sheet.write_string((row + 1) as u32, 1, &role.description)?;
        }
        sheet.set_column_width(0, 24.0)?;
        sheet.set_column_width(1, 52.0)?;
        sheet.set_freeze_panes(1, 0)?;
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
            ("所属部门", "所有角色均可填写，用于人员组织归属和用户列表显示。"),
            ("所属实验室", "仅研发送样员或研发送样组长填写，用于研发送样数据范围；分析角色填写后会自动忽略。"),
            ("归属组", "无需填写。分析检测员/组长自动归属“分析组”，研发送样员/组长自动归属“实验组”；仅用于人员归属，不参与项目、实验室和统计关联。"),
            ("角色1-角色5", "支持多角色，每列选择一个系统现有角色。角色名称会在导入时重新映射为内部角色 ID。"),
            ("角色未匹配", "角色不存在、已删除或当前账号无权分配时，只跳过该角色并保持为空，不影响该用户导入。"),
            ("示例行", "用户名以“示例-”开头的行仅用于演示，导入时不会写入数据库。"),
            ("结果报告", "导入完成后会分别显示成功用户、跳过用户、角色映射数量和未匹配角色警告。"),
        ];
        for (row, (item, rule)) in notes.iter().enumerate() {
            sheet.write_with_format((row + 1) as u32, 0, *item, &note_format)?;
            sheet.write_with_format((row + 1) as u32, 1, *rule, &note_format)?;
        }
    }

    workbook.define_name(
        "RoleOptions",
        "=OFFSET('角色候选'!$A$2,0,0,MAX(1,COUNTA('角色候选'!$A:$A)-1),1)",
    )?;
    workbook.define_name(
        "DivisionOptions",
        "=OFFSET('部门候选'!$A$2,0,0,MAX(1,COUNTA('部门候选'!$A:$A)-1),1)",
    )?;
    workbook.define_name(
        "GroupOptions",
        "=OFFSET('实验室候选'!$A$2,0,0,MAX(1,COUNTA('实验室候选'!$A:$A)-1),1)",
    )?;
    let role_validation = DataValidation::new().allow_list_formula("=RoleOptions".into());
    let division_validation = DataValidation::new().allow_list_formula("=DivisionOptions".into());
    let group_validation = DataValidation::new().allow_list_formula("=GroupOptions".into());
    let sheet = workbook.worksheet_from_name("用户导入")?;
    for column in 4..=8 {
        sheet.add_data_validation(1, column, 1000, column, &role_validation)?;
    }
    sheet.add_data_validation(1, 2, 1000, 2, &division_validation)?;
    sheet.add_data_validation(1, 3, 1000, 3, &group_validation)?;

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

fn parse_user_import_workbook(bytes: &[u8]) -> Result<Vec<UserImportRow>> {
    let mut workbook: calamine::Xlsx<_> = open_workbook_from_rs(Cursor::new(bytes))
        .map_err(|error| AppError::Validation(format!("无法打开 Excel 文件: {error}")))?;
    let range = workbook
        .worksheet_range("用户导入")
        .map_err(|error| AppError::Validation(format!("无法读取工作表“用户导入”: {error}")))?;
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
    let division_column = find_column(&["所属部门", "部门"]);
    let group_column = find_column(&["所属实验室", "实验室"]);
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
        result.push(UserImportRow {
            row: index + 2,
            username,
            password: password_column
                .map(|column| cell_string(row.get(column)))
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "123456".into()),
            division: division_column
                .map(|column| cell_string(row.get(column)))
                .unwrap_or_default(),
            group: group_column
                .map(|column| cell_string(row.get(column)))
                .unwrap_or_default(),
            roles,
        });
    }
    if result.is_empty() {
        return Err(AppError::Validation(
            "模板中没有可导入的用户数据；示例行不会参与导入".into(),
        ));
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
    let mut errors: Vec<String> = vec![];
    let mut warnings: Vec<String> = vec![];
    let conn = pool.get()?;
    let role_map = role_repo::list_all(&pool)?
        .into_iter()
        .map(|role| (role.name.clone(), role))
        .collect::<HashMap<_, _>>();
    for row in rows {
        if row.password.len() < 6 {
            errors.push(format!("行{}: 密码长度不足 6 位", row.row));
            skipped += 1;
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
            continue;
        }
        let mut row_errors = Vec::new();
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
        let (role_ids, ignored_roles) =
            resolve_import_roles(&row.roles, &role_map, ctx.is_system_admin());
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
            continue;
        }
        let names = role_names(&pool, &role_ids)?;
        let has_rd_role = names
            .iter()
            .any(|name| matches!(name.as_str(), ROLE_RD_SENDER | ROLE_RD_LEADER));
        if has_rd_role && group_id.is_none() {
            errors.push(format!("行{}: 研发送样角色必须设置所属实验室", row.row));
            skipped += 1;
            continue;
        }
        let mapped_role_count = role_ids.len() as i64;
        let has_no_roles = role_ids.is_empty();
        let data = UserCreate {
            username: row.username.clone(),
            password: row.password.clone(),
            division_id,
            group_id: if has_rd_role { group_id } else { None },
            role_id: role_ids.first().copied(),
            role_ids: role_ids.clone(),
        };
        if let Some(id) = existing_id {
            let update = crate::models::user::UserUpdate {
                username: None,
                password: None,
                division_id: Some(data.division_id),
                group_id: Some(data.group_id),
                is_admin: None,
                is_active: None,
                role_id: None,
                role_ids: (!role_ids.is_empty()).then_some(role_ids),
            };
            match user_repo::update(&pool, id, &update, ctx.user.id, &ctx.user.username) {
                Ok(_) => {
                    updated += 1;
                    roles_mapped += mapped_role_count;
                }
                Err(error) => {
                    errors.push(format!("行{}: {}", row.row, error));
                    skipped += 1;
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
                roles_mapped += mapped_role_count;
                if has_no_roles {
                    users_without_roles += 1;
                }
            }
            Err(error) => {
                errors.push(format!("行{}: {}", row.row, error));
                skipped += 1;
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
        let roles = vec![crate::models::role::Role {
            id: 7,
            name: "自定义角色A".into(),
            description: "模板候选测试".into(),
            is_system: 0,
            sort_order: 10,
        }];
        let bytes =
            build_user_import_template(&roles, &["研究院".into()], &["实验室01".into()]).unwrap();
        let mut workbook: calamine::Xlsx<_> = open_workbook_from_rs(Cursor::new(bytes)).unwrap();
        let range = workbook.worksheet_range("角色候选").unwrap();
        assert_eq!(range.get_value((1, 0)).unwrap().to_string(), "自定义角色A");
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
            },
            crate::models::role::Role {
                id: 2,
                name: "自定义角色A".into(),
                description: String::new(),
                is_system: 0,
                sort_order: 2,
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

        let (leader_roles, leader_ignored) = resolve_import_roles(&requested, &role_map, false);
        assert_eq!(leader_roles, vec![1]);
        assert_eq!(
            leader_ignored,
            vec![
                ("自定义角色A".into(), IgnoredRoleReason::Forbidden),
                ("不存在角色".into(), IgnoredRoleReason::Missing),
            ]
        );

        let (admin_roles, admin_ignored) = resolve_import_roles(&requested, &role_map, true);
        assert_eq!(admin_roles, vec![1, 2]);
        assert_eq!(
            admin_ignored,
            vec![("不存在角色".into(), IgnoredRoleReason::Missing)]
        );
    }
}
