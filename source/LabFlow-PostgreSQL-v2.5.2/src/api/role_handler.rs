use crate::db::DbPool;
use crate::error::Result;
use crate::models::role::{
    CurrentRoleDataScopeSummary, EffectivePermission, EffectivePermissions, PermissionDef,
    RoleConfigurationSet, RoleCreate, RoleDataScopeSet, RolePermissionSet, RoleTemplateCreate,
    RoleTemplateUpdate, RoleUpdate, PERMISSIONS,
};
use crate::models::trash::DeleteReasonRequest;
use crate::models::ApiResponse;
use crate::repo::role_repo;
use crate::service::authz_service;
use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json, Router,
};

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/roles", axum::routing::get(list).post(create))
        .route(
            "/api/roles/:id",
            axum::routing::put(update).delete(delete_one),
        )
        .route("/api/roles/:id/permissions", axum::routing::put(set_perms))
        .route(
            "/api/roles/:id/configuration",
            axum::routing::put(set_configuration),
        )
        .route(
            "/api/roles/:id/configuration/preview",
            axum::routing::post(preview_configuration),
        )
        .route(
            "/api/users/:id/effective-permissions",
            axum::routing::get(effective_permissions),
        )
        .route(
            "/api/roles/:id/data-scopes",
            axum::routing::put(set_data_scopes),
        )
        .route(
            "/api/roles/current-data-scopes",
            axum::routing::get(current_data_scopes),
        )
        .route(
            "/api/roles/permissions",
            axum::routing::get(permission_whitelist),
        )
        .route(
            "/api/role-templates",
            axum::routing::get(list_templates).post(create_template),
        )
        .route(
            "/api/role-templates/:id",
            axum::routing::put(update_template),
        )
        .route("/api/roles/members", axum::routing::get(permission_members))
        .with_state(pool)
}

async fn permission_members(
    State(pool): State<DbPool>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<crate::models::role::PermissionMember>>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    if !ctx.is_system_admin()
        && !ctx.has_permission("manage:roles")
        && !ctx.has_permission("manage:users")
    {
        return Err(crate::error::AppError::Forbidden(
            "无人员权限查看能力".into(),
        ));
    }
    Ok(Json(ApiResponse::ok(role_repo::permission_members(&pool)?)))
}

async fn create_template(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(body): Json<RoleTemplateCreate>,
) -> Result<Json<ApiResponse<crate::models::role::RoleTemplate>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    if !ctx.is_system_admin() {
        return Err(crate::error::AppError::Forbidden(
            "仅系统管理员可创建角色模板".into(),
        ));
    }
    Ok(Json(ApiResponse::ok(role_repo::create_template(
        &pool,
        &body,
        &ctx.user.username,
    )?)))
}

async fn update_template(
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    Json(body): Json<RoleTemplateUpdate>,
) -> Result<Json<ApiResponse<crate::models::role::RoleTemplate>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    if !ctx.is_system_admin() {
        return Err(crate::error::AppError::Forbidden(
            "仅系统管理员可维护角色模板".into(),
        ));
    }
    Ok(Json(ApiResponse::ok(role_repo::update_template(
        &pool,
        id,
        &body,
        &ctx.user.username,
    )?)))
}

async fn list_templates(
    State(pool): State<DbPool>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<crate::models::role::RoleTemplate>>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    if !ctx.is_system_admin() && !ctx.has_permission("manage:roles") {
        return Err(crate::error::AppError::Forbidden(
            "无角色模板查看权限".into(),
        ));
    }
    Ok(Json(ApiResponse::ok(role_repo::list_templates(&pool)?)))
}

/// GET /api/roles — 列出全部角色（含权限点）
async fn list(
    State(pool): State<DbPool>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<crate::models::role::RoleWithPermissions>>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    if !ctx.is_system_admin() && !ctx.has_permission("manage:roles") {
        return Err(crate::error::AppError::Forbidden("无角色查看权限".into()));
    }
    let roles = role_repo::list_with_permissions(&pool)?;
    Ok(Json(ApiResponse::ok(roles)))
}

/// POST /api/roles — 新建角色
async fn create(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(body): Json<RoleCreate>,
) -> Result<Json<ApiResponse<crate::models::role::RoleWithPermissions>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    if !ctx.is_system_admin() {
        return Err(crate::error::AppError::Forbidden(
            "仅系统管理员可新增自定义角色".into(),
        ));
    }
    let role = role_repo::create(&pool, &body, &ctx.user.username)?;
    Ok(Json(ApiResponse::ok(role)))
}

/// PUT /api/roles/:id — 更新角色基础信息
async fn update(
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    Json(body): Json<RoleUpdate>,
) -> Result<Json<ApiResponse<crate::models::role::RoleWithPermissions>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    if !ctx.is_system_admin() {
        return Err(crate::error::AppError::Forbidden(
            "仅系统管理员可编辑自定义角色".into(),
        ));
    }
    let role = role_repo::update(&pool, id, &body, &ctx.user.username)?;
    Ok(Json(ApiResponse::ok(role)))
}

/// DELETE /api/roles/:id — 删除角色（系统角色拒绝）
async fn delete_one(
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    Query(body): Query<DeleteReasonRequest>,
) -> Result<Json<ApiResponse<()>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    if !ctx.is_system_admin() {
        return Err(crate::error::AppError::Forbidden(
            "仅系统管理员可删除自定义角色".into(),
        ));
    }
    let reason = body
        .reason
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("用户删除");
    role_repo::delete(&pool, id, &ctx.user.username, reason)?;
    Ok(Json(ApiResponse::ok_msg("角色已删除")))
}

/// PUT /api/roles/:id/permissions — 设置角色权限点
async fn set_perms(
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    Json(body): Json<RolePermissionSet>,
) -> Result<Json<ApiResponse<crate::models::role::RoleWithPermissions>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    if !ctx.is_system_admin() {
        return Err(crate::error::AppError::Forbidden(
            "仅系统管理员可设置自定义角色权限".into(),
        ));
    }
    role_repo::list_all(&pool)?
        .into_iter()
        .find(|role| role.id == id)
        .ok_or_else(|| crate::error::AppError::NotFound("角色不存在".into()))?;
    let role = role_repo::set_permissions(&pool, id, &body, &ctx.user.username)?;
    Ok(Json(ApiResponse::ok(role)))
}

/// PUT /api/roles/:id/data-scopes — 整体替换角色的数据可见范围。
async fn set_data_scopes(
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    Json(body): Json<RoleDataScopeSet>,
) -> Result<Json<ApiResponse<crate::models::role::RoleWithPermissions>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    if !ctx.is_system_admin() {
        return Err(crate::error::AppError::Forbidden(
            "仅系统管理员可设置角色数据范围".into(),
        ));
    }
    let role = role_repo::set_data_scopes(&pool, id, &body, &ctx.user.username)?;
    Ok(Json(ApiResponse::ok(role)))
}

async fn set_configuration(
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    Json(body): Json<RoleConfigurationSet>,
) -> Result<Json<ApiResponse<crate::models::role::RoleWithPermissions>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    if !ctx.is_system_admin() {
        return Err(crate::error::AppError::Forbidden(
            "仅系统管理员可保存角色权限和数据范围".into(),
        ));
    }
    Ok(Json(ApiResponse::ok(role_repo::set_configuration(
        &pool,
        id,
        &body,
        &ctx.user.username,
    )?)))
}

async fn preview_configuration(
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    Json(body): Json<RoleConfigurationSet>,
) -> Result<Json<ApiResponse<crate::models::role::RoleWithPermissions>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    if !ctx.is_system_admin() {
        return Err(crate::error::AppError::Forbidden(
            "仅系统管理员可预检角色配置".into(),
        ));
    }
    Ok(Json(ApiResponse::ok(role_repo::preview_configuration(
        &pool,
        id,
        &body,
        &ctx.user.username,
    )?)))
}

async fn effective_permissions(
    State(pool): State<DbPool>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<EffectivePermissions>>> {
    let viewer = authz_service::authenticate(&pool, &headers)?;
    if !viewer.is_system_admin()
        && !viewer.has_permission("manage:users")
        && !viewer.has_permission("manage:roles")
    {
        return Err(crate::error::AppError::Forbidden(
            "无人员权限查看能力".into(),
        ));
    }
    let user = crate::repo::user_repo::find_by_id(&pool, id)?
        .ok_or_else(|| crate::error::AppError::NotFound("用户不存在".into()))?;
    let ctx = authz_service::AuthContext {
        role_names: user.role_names.clone(),
        user,
    };
    let roles: Vec<_> = role_repo::list_with_permissions(&pool)?
        .into_iter()
        .filter(|role| ctx.user.role_ids.contains(&role.id))
        .collect();
    let mut identities = Vec::new();
    if ctx.is_system_admin() {
        identities.push("系统管理员".into());
    }
    if ctx.is_rd_leader() {
        identities.push("研发送样组长".into());
    }
    if ctx.is_analysis_leader() {
        identities.push("分析检测组长".into());
    }
    if ctx.is_rd_public_account() {
        identities.push("研发送样公共账号".into());
    }
    if ctx.is_analysis_public_account() {
        identities.push("分析检测公共账号".into());
    }
    let mut permissions = Vec::new();
    for def in PERMISSIONS {
        let granted = ctx.has_permission(def.key);
        let (usable, explanation) = explain_permission(&ctx, def.key, granted);
        let mut sources: Vec<String> = roles
            .iter()
            .filter(|role| role.permissions.iter().any(|p| p == "*" || p == def.key))
            .map(|role| role.name.clone())
            .collect();
        if ctx.user.is_admin {
            sources.insert(0, "系统管理员身份".into());
        }
        permissions.push(EffectivePermission {
            key: def.key.into(),
            label: def.full_label.into(),
            granted,
            usable,
            sources,
            explanation,
        });
    }
    let mut rules = vec![
        "此处解释功能能力，不替代具体记录的范围、人员关系、状态及服务端鉴权。".into(),
        "角色间功能权限取并集；样品登记的部门与类型在同一角色内取交集，再合并各角色允许结果。"
            .into(),
        "空范围按现有账号、实验室及兼容规则解释，不统一视为全部或无权限。".into(),
        "角色配置保存、帮助维护等管理员专属动作仍校验真实管理员身份；通配权限不替代身份限制。"
            .into(),
    ];
    if ctx.is_analysis_public_account() || ctx.is_rd_public_account() {
        rules.push("公共账号的可选人员及记录范围继续由关联账号和原服务端规则限定；不会因为此页面展示而扩大。".into());
        if ctx.is_system_admin() || ctx.is_analysis_leader() || ctx.is_rd_leader() {
            rules.push("此账号存在历史管理身份与公共用途混合配置，请管理员复核；当前角色和归属保持原样，新增此类组合将被拒绝。".into());
        }
    }
    let work_scope = match ctx.workload_scope() {
        authz_service::RecordScope::Global => "全局授权范围",
        authz_service::RecordScope::AnalysisAll => "分析检测授权人员范围",
        authz_service::RecordScope::PublicCreated => "该公共账号创建的记录",
        authz_service::RecordScope::Lab(_) => "指定实验室范围",
        authz_service::RecordScope::Own => "本人记录",
    };
    rules.push(format!(
        "当前工作量范围：{work_scope}；本人创建者与实际业务人员分别判定。"
    ));
    let conn = pool.get()?;
    let mut scope_names = std::collections::HashMap::new();
    let mut divisions = conn.prepare("SELECT id,name FROM divisions")?;
    for row in divisions.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })? {
        let (id, name) = row?;
        scope_names.insert(format!("division:{id}"), name);
    }
    let mut types = conn.prepare("SELECT type_key,label FROM sample_info_types")?;
    for row in types.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })? {
        let (key, name) = row?;
        scope_names.insert(format!("type:{key}"), name);
    }
    Ok(Json(ApiResponse::ok(EffectivePermissions {
        user_id: ctx.user.id,
        username: ctx.user.username,
        is_active: ctx.user.is_active,
        identities,
        roles,
        scope_names,
        permissions,
        rules,
    })))
}

fn explain_permission(
    ctx: &authz_service::AuthContext,
    key: &str,
    granted: bool,
) -> (bool, String) {
    if !ctx.user.is_active {
        return (false, "账号停用，所有业务请求均拒绝".into());
    }
    if !granted {
        return (false, "未获任何角色授予".into());
    }
    if matches!(
        key,
        "session:extended" | "stats:workload:access" | "stats:rd:access" | "manage:help"
    ) {
        return (false, "历史兼容标识保留，当前不控制该业务；帮助维护按管理员身份及 help:edit，阅读按 help:view".into());
    }
    if key == "manage:auxiliary" && !ctx.user.is_admin {
        return (false, "辅助配置维护还要求账号为系统管理员".into());
    }
    if key == "help:edit" && !ctx.is_system_admin() {
        return (false, "帮助维护还要求系统管理员身份".into());
    }
    if key == "feedback:personnel:edit"
        && !(ctx.is_system_admin() || ctx.is_rd_leader() || ctx.is_analysis_leader())
    {
        return (
            false,
            "还需组长职责；提交限研发组长，审批限分析组长，并校验实验室及不可自审批".into(),
        );
    }
    if key == "feedback:personnel:edit" && !ctx.has_permission("feedback:personnel:view") {
        return (false, "缺少人员变动查看权限".into());
    }
    let portal = if key.starts_with("stats:workload:") {
        Some("stats:portal:workload")
    } else if key.starts_with("stats:rd:") {
        Some("stats:portal:rd")
    } else if key.starts_with("stats:sample-info:") {
        Some("stats:portal:sample-info")
    } else {
        None
    };
    if let Some(portal) = portal {
        if !ctx.has_permission(portal) {
            return (false, format!("能力已授予，缺少对应统计门户权限：{portal}"));
        }
    }
    if key == "manage:roles" {
        return (
            true,
            "可以查看角色；新增、编辑、删除和保存授权仅系统管理员可执行".into(),
        );
    }
    if key == "records:work:portal-scoped" && ctx.is_analysis_public_account() {
        return (
            true,
            "公共账号系统必需开关；人员候选仍受原授权范围约束".into(),
        );
    }
    (
        true,
        "功能能力已授予；具体对象仍受数据范围、人员关系和业务状态限制".into(),
    )
}

/// GET /api/roles/current-data-scopes — UI uses this only to hide unavailable tabs/options.
/// Record queries still enforce the same scopes on the server.
async fn current_data_scopes(
    State(pool): State<DbPool>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<CurrentRoleDataScopeSummary>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    if ctx.is_system_admin() {
        return Ok(Json(ApiResponse::ok(CurrentRoleDataScopeSummary {
            has_configured_scope: false,
            has_division_scope: false,
            has_work_division_scope: false,
            has_sample_info_type_scope: false,
            division_ids: vec![],
            work_division_ids: vec![],
            sample_info_type_keys: vec![],
        })));
    }
    let scopes = authz_service::role_data_scopes(&pool, &ctx)?;
    let work_division_ids =
        crate::repo::role_repo::work_division_scope_ids_for_roles(&pool, &ctx.user.role_ids)?;
    let mut division_ids = Vec::new();
    let mut sample_info_type_keys = Vec::new();
    let mut has_division_scope = false;
    let mut has_sample_info_type_scope = false;
    for scope in &scopes {
        has_division_scope |= !scope.division_ids.is_empty();
        has_sample_info_type_scope |= !scope.sample_info_type_keys.is_empty();
        division_ids.extend(scope.division_ids.iter().copied());
        sample_info_type_keys.extend(scope.sample_info_type_keys.iter().cloned());
    }
    division_ids.sort_unstable();
    division_ids.dedup();
    sample_info_type_keys.sort();
    sample_info_type_keys.dedup();
    Ok(Json(ApiResponse::ok(CurrentRoleDataScopeSummary {
        has_configured_scope: !scopes.is_empty() || !work_division_ids.is_empty(),
        has_division_scope,
        has_work_division_scope: !work_division_ids.is_empty(),
        has_sample_info_type_scope,
        division_ids,
        work_division_ids,
        sample_info_type_keys,
    })))
}

/// GET /api/roles/permissions — 返回权限点白名单（登录即可，无需 admin）
async fn permission_whitelist(
    State(pool): State<DbPool>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<PermissionDef>>>> {
    authz_service::authenticate(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(PERMISSIONS.to_vec())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_explanations_keep_identity_and_portal_dependencies() {
        let pool = crate::db::init_pool("postgres-test");
        crate::db::test_migrations::run(&pool.get().unwrap()).unwrap();
        let mut user = crate::repo::user_repo::find_by_username(&pool, "admin")
            .unwrap()
            .unwrap();
        user.is_admin = false;
        user.role_keys.clear();
        user.role_names.clear();
        user.role_template_names.clear();
        user.role_identity_keys.clear();
        user.permissions = vec!["*".into()];
        let mut ctx = authz_service::AuthContext {
            user,
            role_names: vec![],
        };
        assert!(!explain_permission(&ctx, "help:edit", true).0);
        assert!(!explain_permission(&ctx, "manage:auxiliary", true).0);
        assert!(explain_permission(&ctx, "manage:roles", true)
            .1
            .contains("仅系统管理员"));
        assert!(!explain_permission(&ctx, "feedback:personnel:edit", true).0);
        assert!(!explain_permission(&ctx, "session:extended", true).0);
        ctx.user.permissions = vec!["stats:workload:user-log".into()];
        assert!(!explain_permission(&ctx, "stats:workload:user-log", true).0);
        ctx.user.permissions.push("stats:portal:workload".into());
        assert!(explain_permission(&ctx, "stats:workload:user-log", true).0);
        ctx.user.is_active = false;
        assert!(!explain_permission(&ctx, "stats:workload:user-log", true).0);
    }
}
