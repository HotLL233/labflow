use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::role::{
    is_valid_permission, PermissionMember, Role, RoleConfigurationSet, RoleCreate, RoleDataScope,
    RoleDataScopeSet, RolePermissionSet, RoleTemplate, RoleTemplateCreate, RoleTemplateUpdate,
    RoleUpdate, RoleWithPermissions,
};
use crate::repo::{audit_repo, trash_repo};
use postgres_compat::OptionalExtension;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// 角色基础信息 SQL
const ROLE_SELECT: &str =
    "SELECT id, name, description, is_system, sort_order, template_id FROM roles";

/// 查询单个角色（不含权限点）
fn get_role_on_conn(conn: &postgres_compat::Connection, id: i64) -> Result<Role> {
    conn.query_row(&format!("{} WHERE id=?1", ROLE_SELECT), [id], |row| {
        Ok(Role {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            is_system: row.get(3)?,
            sort_order: row.get(4)?,
            template_id: row.get(5)?,
        })
    })
    .map_err(|e| match e {
        postgres_compat::Error::QueryReturnedNoRows => AppError::NotFound("角色不存在".into()),
        _ => e.into(),
    })
}

/// 查询角色权限点集合
fn get_permissions_on_conn(
    conn: &postgres_compat::Connection,
    role_id: i64,
) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT permission_key FROM role_permissions WHERE role_id=?1 ORDER BY permission_key",
    )?;
    let rows = stmt.query_map([role_id], |row| row.get::<_, String>(0))?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

fn get_template_name_on_conn(
    conn: &postgres_compat::Connection,
    template_id: Option<i64>,
) -> Result<Option<String>> {
    let Some(template_id) = template_id else {
        return Ok(None);
    };
    Ok(conn
        .query_row(
            "SELECT name FROM role_templates WHERE id=?1",
            [template_id],
            |row| row.get(0),
        )
        .optional()?)
}

pub fn list_templates(pool: &DbPool) -> Result<Vec<RoleTemplate>> {
    let conn = pool.get()?;
    let mut stmt = conn
        .prepare("SELECT id FROM role_templates WHERE deleted_at IS NULL ORDER BY sort_order,id")?;
    let ids = stmt
        .query_map([], |row| row.get::<_, i64>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    ids.into_iter()
        .map(|id| template_on_conn(&conn, id))
        .collect()
}

fn template_on_conn(conn: &postgres_compat::Connection, id: i64) -> Result<RoleTemplate> {
    let mut template = conn.query_row(
        "SELECT id,name,description,is_system,sort_order,identity_key,is_active FROM role_templates WHERE id=?1 AND deleted_at IS NULL",
        [id], |row| Ok(RoleTemplate {
            id: row.get(0)?, name: row.get(1)?, description: row.get(2)?,
            is_system: row.get(3)?, sort_order: row.get(4)?, identity_key: row.get(5)?,
            is_active: row.get::<_, i64>(6)? != 0, permissions: vec![],
            configuration_revision: String::new(), reference_count: 0,
        }),
    ).optional()?.ok_or_else(|| AppError::NotFound("角色模板不存在".into()))?;
    let mut stmt = conn.prepare("SELECT permission_key FROM role_template_permissions WHERE template_id=?1 ORDER BY permission_key")?;
    template.permissions = stmt
        .query_map([id], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    template.reference_count = conn.query_row(
        "SELECT COUNT(*) FROM roles WHERE template_id=?1 AND deleted_at IS NULL",
        [id],
        |row| row.get(0),
    )?;
    let snapshot = serde_json::json!([
        template.id,
        template.name,
        template.description,
        template.is_system,
        template.sort_order,
        template.identity_key,
        template.is_active,
        template.permissions
    ]);
    template.configuration_revision =
        format!("{:x}", Sha256::digest(snapshot.to_string().as_bytes()));
    Ok(template)
}

fn validate_name(name: &str) -> Result<()> {
    if name.trim().is_empty() {
        return Err(AppError::Validation("名称不能为空".into()));
    }
    Ok(())
}

fn validate_purpose(purpose: &str) -> Result<()> {
    if !matches!(purpose, "personal" | "public_rd" | "public_analysis") {
        return Err(AppError::Validation("请选择有效的账号用途".into()));
    }
    Ok(())
}

// Administrative role-purpose and account-assignment writes share this transaction
// lock so concurrent requests cannot each validate an obsolete half of the identity.
pub fn lock_identity_configuration(conn: &postgres_compat::Connection) -> Result<()> {
    conn.query_row("SELECT TRUE FROM pg_advisory_xact_lock(2414001)", [], |r| {
        r.get::<_, bool>(0)
    })?;
    Ok(())
}

pub fn validate_role_assignment_on_conn(
    conn: &postgres_compat::Connection,
    role_ids: &[i64],
    prior_ids: &[i64],
    legacy_admin: bool,
) -> Result<()> {
    let mut has_public = false;
    let mut has_management = legacy_admin;
    let mut prior_public = false;
    let mut prior_management = legacy_admin;
    let mut new_sensitive = false;
    for id in role_ids {
        let (purpose,management):(String,bool)=conn.query_row(
            "SELECT labflow_role_purpose(r.role_purpose,r.system_key,r.is_system),labflow_role_identity(r.system_key,r.is_system,t.identity_key,r.name) IN ('system_admin','analysis_leader','rd_leader') FROM roles r LEFT JOIN role_templates t ON t.id=r.template_id WHERE r.id=?1 AND r.deleted_at IS NULL",
            [id],|r|Ok((r.get(0)?,r.get(1)?)),
        ).optional()?.ok_or_else(||AppError::Validation("请选择有效角色".into()))?;
        let public = purpose != "personal";
        has_public |= public;
        has_management |= management;
        if prior_ids.contains(id) {
            prior_public |= public;
            prior_management |= management;
        } else {
            new_sensitive |= public || management;
        }
    }
    if has_public && has_management && (!prior_public || !prior_management || new_sensitive) {
        return Err(AppError::Validation("公共用途不能与系统管理员或组长身份新增组合；请核对所选角色。已有历史混合配置保持原样，不会自动删除角色或归属。".into()));
    }
    Ok(())
}

pub fn create_template(
    pool: &DbPool,
    body: &RoleTemplateCreate,
    operator: &str,
) -> Result<RoleTemplate> {
    validate_name(&body.name)?;
    if body.base_template_id.is_some() && body.source_role_id.is_some() {
        return Err(AppError::Validation(
            "模板来源和角色来源只能选择一个".into(),
        ));
    }
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let (identity, inherited) = if let Some(id) = body.base_template_id {
        tx.query_row(
            "SELECT id FROM role_templates WHERE id=?1 AND deleted_at IS NULL FOR UPDATE",
            [id],
            |r| r.get::<_, i64>(0),
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound("角色模板不存在".into()))?;
        let base = template_on_conn(&tx, id)?;
        if !base.is_active {
            return Err(AppError::Validation("停用模板不能用于创建新模板".into()));
        }
        if body.base_revision.as_deref() != Some(base.configuration_revision.as_str()) {
            return Err(AppError::Conflict(
                "来源模板已变更，请重新读取后再创建".into(),
            ));
        }
        (base.identity_key, base.permissions)
    } else if let Some(id) = body.source_role_id {
        tx.query_row(
            "SELECT id FROM roles WHERE id=?1 AND deleted_at IS NULL FOR UPDATE",
            [id],
            |r| r.get::<_, i64>(0),
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound("角色不存在".into()))?;
        let base = with_permissions(&tx, get_role_on_conn(&tx, id)?)?;
        if body.source_revision.as_deref() != Some(base.configuration_revision.as_str()) {
            return Err(AppError::Conflict(
                "来源角色已变更，请重新读取后再创建".into(),
            ));
        }
        // Administrator identity is fixed and cannot be cloned into a custom template.
        let identity = if base.identity_key == "system_admin" {
            "ordinary".into()
        } else {
            base.identity_key
        };
        (identity, base.permissions)
    } else {
        ("ordinary".into(), vec![])
    };
    let mut permissions = body.permissions.clone().unwrap_or(inherited.clone());
    permissions.sort();
    permissions.dedup();
    for key in &permissions {
        if !is_valid_permission(key) && !inherited.contains(key) {
            return Err(AppError::Validation(format!("非法权限点: {key}")));
        }
    }
    let exists: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM role_templates WHERE name=?1)",
        [&body.name],
        |r| r.get(0),
    )?;
    if exists {
        return Err(AppError::Conflict(format!("模板名「{}」已存在", body.name)));
    }
    tx.execute("INSERT INTO role_templates(name,description,is_system,sort_order,identity_key,is_active) VALUES(?1,?2,0,?3,?4,1)",postgres_compat::params![body.name,body.description,body.sort_order,identity])?;
    let id = tx.last_insert_rowid();
    for key in &permissions {
        tx.execute(
            "INSERT INTO role_template_permissions(template_id,permission_key) VALUES(?1,?2)",
            postgres_compat::params![id, key],
        )?;
    }
    let result = template_on_conn(&tx, id)?;
    let after = serde_json::to_value(&result).unwrap_or_default();
    audit_repo::log_structured_on_conn(
        &tx,
        "create",
        "role_templates",
        Some(id),
        operator,
        &format!("创建角色模板「{}」", result.name),
        "shared",
        "",
        None,
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(result)
}

pub fn update_template(
    pool: &DbPool,
    id: i64,
    body: &RoleTemplateUpdate,
    operator: &str,
) -> Result<RoleTemplate> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    tx.query_row(
        "SELECT id FROM role_templates WHERE id=?1 AND deleted_at IS NULL FOR UPDATE",
        [id],
        |r| r.get::<_, i64>(0),
    )
    .optional()?
    .ok_or_else(|| AppError::NotFound("角色模板不存在".into()))?;
    let current = template_on_conn(&tx, id)?;
    if current.is_system == 1 {
        return Err(AppError::Forbidden("内置模板只读，请复制后修改".into()));
    }
    if body.expected_revision != current.configuration_revision {
        return Err(AppError::Conflict(
            "模板已被其他操作修改，请重新读取后核对差异".into(),
        ));
    }
    let name = body.name.as_deref().unwrap_or(&current.name);
    validate_name(name)?;
    let exists: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM role_templates WHERE name=?1 AND id<>?2)",
        postgres_compat::params![name, id],
        |r| r.get(0),
    )?;
    if exists {
        return Err(AppError::Conflict(format!("模板名「{name}」已存在")));
    }
    let mut permissions = body
        .permissions
        .clone()
        .unwrap_or_else(|| current.permissions.clone());
    permissions.sort();
    permissions.dedup();
    for key in &permissions {
        if !is_valid_permission(key) && !current.permissions.contains(key) {
            return Err(AppError::Validation(format!("非法权限点: {key}")));
        }
    }
    tx.execute(
        "UPDATE role_templates SET name=?1,description=?2,sort_order=?3,is_active=?4 WHERE id=?5",
        postgres_compat::params![
            name,
            body.description.as_deref().unwrap_or(&current.description),
            body.sort_order.unwrap_or(current.sort_order),
            if body.is_active.unwrap_or(current.is_active) {
                1i64
            } else {
                0
            },
            id
        ],
    )?;
    tx.execute(
        "DELETE FROM role_template_permissions WHERE template_id=?1",
        [id],
    )?;
    for key in &permissions {
        tx.execute(
            "INSERT INTO role_template_permissions(template_id,permission_key) VALUES(?1,?2)",
            postgres_compat::params![id, key],
        )?;
    }
    let result = template_on_conn(&tx, id)?;
    let before = serde_json::to_value(&current).unwrap_or_default();
    let after = serde_json::to_value(&result).unwrap_or_default();
    audit_repo::log_structured_on_conn(
        &tx,
        "update",
        "role_templates",
        Some(id),
        operator,
        &format!("修改角色模板「{}」（不回写既有角色）", result.name),
        "shared",
        "",
        Some(&before),
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(result)
}

pub fn permission_members(pool: &DbPool) -> Result<Vec<PermissionMember>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare("SELECT u.id,u.username,u.is_active,COALESCE((SELECT json_agg(r.name ORDER BY r.sort_order,r.id) FROM labflow_effective_user_roles ur JOIN roles r ON r.id=ur.role_id WHERE ur.user_id=u.id),'[]'::json)::TEXT FROM users u WHERE u.deleted_at IS NULL ORDER BY u.username,u.id")?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    rows.map(|row| {
        let (id, username, active, names) = row?;
        Ok(PermissionMember {
            id,
            username,
            is_active: active != 0,
            role_names: serde_json::from_str(&names)
                .map_err(|e| AppError::Internal(format!("角色数据无效: {e}")))?,
        })
    })
    .collect()
}

fn get_division_scope_ids_on_conn(
    conn: &postgres_compat::Connection,
    role_id: i64,
) -> Result<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT division_id FROM role_division_scopes WHERE role_id=?1 ORDER BY division_id",
    )?;
    Ok(stmt
        .query_map([role_id], |row| row.get::<_, i64>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?)
}

fn get_work_division_scope_ids_on_conn(
    conn: &postgres_compat::Connection,
    role_id: i64,
) -> Result<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT division_id FROM role_work_division_scopes WHERE role_id=?1 ORDER BY division_id",
    )?;
    Ok(stmt
        .query_map([role_id], |row| row.get::<_, i64>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?)
}

fn get_sample_info_type_scope_keys_on_conn(
    conn: &postgres_compat::Connection,
    role_id: i64,
) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT type_key FROM role_sample_info_type_scopes WHERE role_id=?1 ORDER BY type_key",
    )?;
    Ok(stmt
        .query_map([role_id], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?)
}

/// 聚合角色 + 权限点
fn with_permissions(conn: &postgres_compat::Connection, role: Role) -> Result<RoleWithPermissions> {
    let permissions = get_permissions_on_conn(conn, role.id)?;
    let (role_purpose, identity_key): (String,String) = conn.query_row(
        "SELECT labflow_role_purpose(r.role_purpose,r.system_key,r.is_system),labflow_role_identity(r.system_key,r.is_system,t.identity_key,r.name) FROM roles r LEFT JOIN role_templates t ON t.id=r.template_id WHERE r.id=?1",
        [role.id],
        |row| Ok((row.get(0)?,row.get(1)?)),
    )?;
    let required = role_purpose == "public_analysis";
    let is_public_account = role_purpose != "personal";
    let member_count = conn.query_row("SELECT COUNT(DISTINCT u.id) FROM users u JOIN labflow_effective_user_roles ur ON ur.user_id=u.id WHERE u.deleted_at IS NULL AND ur.role_id=?1", [role.id], |row| row.get(0))?;
    let mut result = RoleWithPermissions {
        id: role.id,
        name: role.name,
        description: role.description,
        is_system: role.is_system,
        sort_order: role.sort_order,
        template_id: role.template_id,
        template_name: get_template_name_on_conn(conn, role.template_id)?,
        permissions,
        division_scope_ids: get_division_scope_ids_on_conn(conn, role.id)?,
        sample_info_type_scope_keys: get_sample_info_type_scope_keys_on_conn(conn, role.id)?,
        work_division_scope_ids: get_work_division_scope_ids_on_conn(conn, role.id)?,
        configuration_revision: String::new(),
        required_permissions: if required {
            vec!["records:work:portal-scoped".into()]
        } else {
            vec![]
        },
        is_public_account,
        role_purpose,
        purpose_locked: role.is_system == 1,
        identity_key,
        member_count,
    };
    let snapshot = serde_json::json!([
        result.id,
        result.name,
        result.description,
        result.template_id,
        result.sort_order,
        result.role_purpose,
        result.identity_key,
        result.permissions,
        result.division_scope_ids,
        result.work_division_scope_ids,
        result.sample_info_type_scope_keys
    ]);
    result.configuration_revision =
        format!("{:x}", Sha256::digest(snapshot.to_string().as_bytes()));
    Ok(result)
}

/// Returns scope sets per role. Callers must evaluate each role separately and then union the
/// permitted record sets; combining a role's department with another role's type is unsafe.
pub fn data_scopes_for_roles(pool: &DbPool, role_ids: &[i64]) -> Result<Vec<RoleDataScope>> {
    if role_ids.is_empty() {
        return Ok(Vec::new());
    }
    let conn = pool.get()?;
    let ids = role_ids
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let mut stmt = conn.prepare(&format!(
        "SELECT role_id,'division' AS kind,division_id AS id,NULL::TEXT AS type_key FROM role_division_scopes WHERE role_id IN ({ids})
         UNION ALL SELECT role_id,'work',division_id,NULL::TEXT FROM role_work_division_scopes WHERE role_id IN ({ids})
         UNION ALL SELECT role_id,'type',NULL::BIGINT,type_key FROM role_sample_info_type_scopes WHERE role_id IN ({ids})
         ORDER BY role_id,kind,id,type_key"
    ))?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<i64>>(2)?,
            row.get::<_, Option<String>>(3)?,
        ))
    })?;
    let mut by_role = HashMap::new();
    for row in rows {
        let (role_id, kind, division_id, type_key) = row?;
        let scope = by_role.entry(role_id).or_insert_with(|| RoleDataScope {
            role_id,
            division_ids: Vec::new(),
            work_division_ids: Vec::new(),
            sample_info_type_keys: Vec::new(),
        });
        match kind.as_str() {
            "division" => scope.division_ids.push(division_id.unwrap()),
            "work" => scope.work_division_ids.push(division_id.unwrap()),
            "type" => scope.sample_info_type_keys.push(type_key.unwrap()),
            _ => unreachable!("scope kind comes from the query literals"),
        }
    }
    // Preserve input order and each role's department/type pairing, including duplicate role IDs.
    Ok(role_ids
        .iter()
        .filter_map(|id| by_role.get(id).cloned())
        .collect())
}

pub fn work_division_scope_ids_for_roles(pool: &DbPool, role_ids: &[i64]) -> Result<Vec<i64>> {
    if role_ids.is_empty() {
        return Ok(Vec::new());
    }
    let conn = pool.get()?;
    let ids = role_ids
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let mut stmt = conn.prepare(&format!("SELECT DISTINCT division_id FROM role_work_division_scopes WHERE role_id IN ({ids}) ORDER BY division_id"))?;
    let rows = stmt.query_map([], |row| row.get::<_, i64>(0))?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

/// 列出全部角色（含聚合权限点），按 sort_order, id 排序
pub fn list_with_permissions(pool: &DbPool) -> Result<Vec<RoleWithPermissions>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(&format!(
        "{} WHERE deleted_at IS NULL ORDER BY sort_order, id",
        ROLE_SELECT
    ))?;
    let roles = stmt.query_map([], |row| {
        Ok(Role {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            is_system: row.get(3)?,
            sort_order: row.get(4)?,
            template_id: row.get(5)?,
        })
    })?;
    let roles: Vec<Role> = roles.collect::<std::result::Result<Vec<_>, _>>()?;
    let mut out = Vec::with_capacity(roles.len());
    for r in roles {
        out.push(with_permissions(&conn, r)?);
    }
    Ok(out)
}

/// 列出全部角色（仅基础信息），供用户编辑下拉使用
pub fn list_all(pool: &DbPool) -> Result<Vec<Role>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(&format!(
        "{} WHERE deleted_at IS NULL ORDER BY sort_order, id",
        ROLE_SELECT
    ))?;
    let roles = stmt.query_map([], |row| {
        Ok(Role {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            is_system: row.get(3)?,
            sort_order: row.get(4)?,
            template_id: row.get(5)?,
        })
    })?;
    Ok(roles.collect::<std::result::Result<Vec<_>, _>>()?)
}

/// 查询角色权限点
pub fn get_permissions(pool: &DbPool, role_id: i64) -> Result<Vec<String>> {
    let conn = pool.get()?;
    get_permissions_on_conn(&conn, role_id)
}

/// 新建角色并写入权限点（校验权限点合法性）
pub fn create(pool: &DbPool, body: &RoleCreate, operator: &str) -> Result<RoleWithPermissions> {
    validate_name(&body.name)?;
    let purpose = body.role_purpose.as_deref().unwrap_or("personal");
    validate_purpose(purpose)?;
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let exists: i64 = tx.query_row(
        "SELECT COUNT(*) FROM roles WHERE name=?1",
        [&body.name],
        |r| r.get(0),
    )?;
    if exists > 0 {
        return Err(AppError::Conflict(format!("角色名「{}」已存在", body.name)));
    }
    let template = if let Some(id) = body.template_id {
        tx.query_row(
            "SELECT id FROM role_templates WHERE id=?1 AND deleted_at IS NULL FOR UPDATE",
            [id],
            |r| r.get::<_, i64>(0),
        )
        .optional()?
        .ok_or_else(|| AppError::Validation("角色模板不存在或已停用".into()))?;
        let template = template_on_conn(&tx, id)?;
        if !template.is_active {
            return Err(AppError::Validation("角色模板已停用".into()));
        }
        if body
            .template_revision
            .as_deref()
            .is_some_and(|revision| revision != template.configuration_revision)
        {
            return Err(AppError::Conflict(
                "角色模板已变更，请重新读取后核对默认权限".into(),
            ));
        }
        Some(template)
    } else {
        None
    };
    if purpose != "personal"
        && template.as_ref().is_some_and(|t| {
            matches!(
                t.identity_key.as_str(),
                "analysis_leader" | "rd_leader" | "system_admin"
            )
        })
    {
        return Err(AppError::Validation(
            "组长或系统管理员身份不能配置公共用途，请使用普通人员模板".into(),
        ));
    }
    let inherited = template
        .as_ref()
        .map(|t| t.permissions.clone())
        .unwrap_or_default();
    let mut permissions = if body.permissions.is_empty() && !body.permissions_override {
        inherited.clone()
    } else {
        body.permissions.clone()
    };
    if purpose == "public_analysis" {
        permissions.push("records:work:portal-scoped".into());
    }
    permissions.sort();
    permissions.dedup();
    for p in &permissions {
        if !is_valid_permission(p) && !inherited.contains(p) {
            return Err(AppError::Validation(format!("非法权限点: {}", p)));
        }
    }
    tx.execute(
        "INSERT INTO roles (name, description, is_system, sort_order, template_id,role_purpose) VALUES (?1,?2,0,?3,?4,?5)",
        postgres_compat::params![body.name, body.description, body.sort_order, body.template_id,purpose],
    )?;
    let id = tx.last_insert_rowid();
    for p in &permissions {
        tx.execute(
            "INSERT INTO role_permissions (role_id, permission_key) VALUES (?1,?2)",
            postgres_compat::params![id, p],
        )?;
    }
    let created = with_permissions(&tx, get_role_on_conn(&tx, id)?)?;
    let after = serde_json::to_value(&created).unwrap_or(serde_json::Value::Null);
    audit_repo::log_structured_on_conn(
        &tx,
        "create",
        "roles",
        Some(id),
        operator,
        &format!("创建角色「{}」", created.name),
        "shared",
        "",
        None,
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(created)
}

/// 更新角色基础信息（系统角色不可改名）
pub fn update(
    pool: &DbPool,
    id: i64,
    body: &RoleUpdate,
    operator: &str,
) -> Result<RoleWithPermissions> {
    let mut conn = pool.get()?;
    let role = get_role_on_conn(&conn, id)?;
    if role.is_system == 1 {
        return Err(AppError::Forbidden("五个固定角色不能修改".into()));
    }
    let mut sets: Vec<String> = vec![];
    let mut params: Vec<Box<dyn postgres_compat::types::ToSql>> = vec![];
    if let Some(ref n) = body.name {
        if role.is_system == 1 {
            return Err(AppError::Forbidden("系统角色不可改名".into()));
        }
        if !n.is_empty() && n != &role.name {
            // 唯一性校验
            let conflict: i64 = conn.query_row(
                "SELECT COUNT(*) FROM roles WHERE name=?1 AND id<>?2",
                postgres_compat::params![n, id],
                |r| r.get(0),
            )?;
            if conflict > 0 {
                return Err(AppError::Conflict(format!("角色名「{}」已存在", n)));
            }
            sets.push("name=?1".to_string());
            params.push(Box::new(n.clone()));
        }
    }
    if let Some(ref d) = body.description {
        sets.push(format!("description=?{}", params.len() + 1));
        params.push(Box::new(d.clone()));
    }
    if let Some(so) = body.sort_order {
        sets.push(format!("sort_order=?{}", params.len() + 1));
        params.push(Box::new(so));
    }
    if sets.is_empty() {
        return Err(AppError::Validation("没有需要更新的字段".into()));
    }
    params.push(Box::new(id));
    let sql = format!(
        "UPDATE roles SET {} WHERE id=?{}",
        sets.join(","),
        params.len()
    );
    let before_role = with_permissions(&conn, role)?;
    let before = serde_json::to_value(&before_role).unwrap_or(serde_json::Value::Null);
    let tx = conn.transaction()?;
    tx.execute(
        &sql,
        postgres_compat::params_from_iter(params.iter().map(|p| p.as_ref())),
    )?;
    let updated = with_permissions(&tx, get_role_on_conn(&tx, id)?)?;
    let after = serde_json::to_value(&updated).unwrap_or(serde_json::Value::Null);
    audit_repo::log_structured_on_conn(
        &tx,
        "update",
        "roles",
        Some(id),
        operator,
        &format!("修改角色「{}」", updated.name),
        "shared",
        "",
        Some(&before),
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(updated)
}

/// 删除角色（系统角色禁止删除；有关联用户时禁止删除）
pub fn delete(pool: &DbPool, id: i64, operator: &str, reason: &str) -> Result<()> {
    let mut conn = pool.get()?;
    let role = get_role_on_conn(&conn, id)?;
    if role.is_system == 1 {
        return Err(AppError::Forbidden("系统角色不可删除".into()));
    }
    let used: i64 = conn.query_row(
        "SELECT
            (SELECT COUNT(*) FROM users WHERE role_id=?1) +
            (SELECT COUNT(*) FROM user_roles WHERE role_id=?1)",
        [id],
        |r| r.get(0),
    )?;
    if used > 0 {
        return Err(AppError::Conflict("该角色下仍有用户，无法删除".into()));
    }
    let role_full = with_permissions(&conn, role.clone())?;
    let tx = conn.transaction()?;
    let before = serde_json::to_value(&role_full).unwrap_or(serde_json::Value::Null);
    tx.execute("UPDATE roles SET deleted_at=datetime('now','localtime') WHERE id=?1 AND deleted_at IS NULL", [id])?;
    trash_repo::move_to_trash_on_conn(
        &tx,
        "角色",
        "roles",
        id,
        "access",
        "shared",
        &role.name,
        "",
        &before,
        reason,
        operator,
        None,
        None,
        "未分配用户",
        true,
    )?;
    let after = serde_json::json!({"deleted_at":"now","data":before});
    audit_repo::log_structured_on_conn(
        &tx,
        "delete",
        "roles",
        Some(id),
        operator,
        &format!("删除角色「{}」", role.name),
        "shared",
        "",
        Some(&before),
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(())
}

/// Legacy endpoints retain their partial-update contract and share the transaction lock.
pub fn set_permissions(
    pool: &DbPool,
    role_id: i64,
    body: &RolePermissionSet,
    operator: &str,
) -> Result<RoleWithPermissions> {
    write_configuration(
        pool,
        role_id,
        Some(&body.permissions),
        None,
        None,
        None,
        operator,
        false,
    )
}

pub fn set_data_scopes(
    pool: &DbPool,
    role_id: i64,
    body: &RoleDataScopeSet,
    operator: &str,
) -> Result<RoleWithPermissions> {
    write_configuration(pool, role_id, None, Some(body), None, None, operator, false)
}

pub fn set_configuration(
    pool: &DbPool,
    role_id: i64,
    body: &RoleConfigurationSet,
    operator: &str,
) -> Result<RoleWithPermissions> {
    configure(pool, role_id, body, operator, false)
}

pub fn preview_configuration(
    pool: &DbPool,
    role_id: i64,
    body: &RoleConfigurationSet,
    operator: &str,
) -> Result<RoleWithPermissions> {
    configure(pool, role_id, body, operator, true)
}

fn configure(
    pool: &DbPool,
    role_id: i64,
    body: &RoleConfigurationSet,
    operator: &str,
    preview: bool,
) -> Result<RoleWithPermissions> {
    let scopes = RoleDataScopeSet {
        division_ids: body.division_ids.clone(),
        work_division_ids: body.work_division_ids.clone(),
        sample_info_type_keys: body.sample_info_type_keys.clone(),
    };
    write_configuration(
        pool,
        role_id,
        Some(&body.permissions),
        Some(&scopes),
        Some(&body.expected_revision),
        Some(body),
        operator,
        preview,
    )
}

fn write_configuration(
    pool: &DbPool,
    role_id: i64,
    permissions: Option<&Vec<String>>,
    scopes: Option<&RoleDataScopeSet>,
    expected_revision: Option<&str>,
    fields: Option<&RoleConfigurationSet>,
    operator: &str,
    preview: bool,
) -> Result<RoleWithPermissions> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    lock_identity_configuration(&tx)?;
    tx.query_row(
        "SELECT id FROM roles WHERE id=?1 AND deleted_at IS NULL FOR UPDATE",
        [role_id],
        |row| row.get::<_, i64>(0),
    )
    .optional()?
    .ok_or_else(|| AppError::NotFound("角色不存在".into()))?;
    let current = with_permissions(&tx, get_role_on_conn(&tx, role_id)?)?;
    if expected_revision.is_some_and(|revision| revision != current.configuration_revision) {
        return Err(AppError::Conflict(
            "角色配置已被其他操作修改，尚未保存本次内容；请重新读取配置并核对差异。".into(),
        ));
    }
    let next_purpose = fields
        .and_then(|f| f.role_purpose.as_deref())
        .unwrap_or(&current.role_purpose);
    validate_purpose(next_purpose)?;
    let next_name = fields
        .and_then(|f| f.name.as_deref())
        .unwrap_or(&current.name);
    let next_description = fields
        .and_then(|f| f.description.as_deref())
        .unwrap_or(&current.description);
    let next_sort = fields
        .and_then(|f| f.sort_order)
        .unwrap_or(current.sort_order);
    validate_name(next_name)?;
    if current.purpose_locked
        && (next_purpose != current.role_purpose
            || next_name != current.name
            || next_description != current.description
            || next_sort != current.sort_order)
    {
        return Err(AppError::Forbidden(
            "固定系统角色的名称、用途与基本资料不能修改".into(),
        ));
    }
    if next_name != current.name {
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM roles WHERE name=?1 AND id<>?2)",
            postgres_compat::params![next_name, role_id],
            |r| r.get(0),
        )?;
        if exists {
            return Err(AppError::Conflict(format!("角色名「{next_name}」已存在")));
        }
    }
    if next_purpose != current.role_purpose && next_purpose != "personal" {
        if matches!(
            current.identity_key.as_str(),
            "system_admin" | "analysis_leader" | "rd_leader"
        ) {
            return Err(AppError::Validation(
                "组长或系统管理员身份不能配置公共用途".into(),
            ));
        }
        let mut stmt = tx.prepare("SELECT u.id,u.username FROM users u WHERE u.deleted_at IS NULL AND u.id IN (SELECT user_id FROM labflow_effective_user_roles WHERE role_id=?1) ORDER BY u.id FOR UPDATE")?;
        let members = stmt
            .query_map([role_id], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut conflicts = Vec::new();
        for (user_id, name) in members {
            let incompatible: bool = tx.query_row("SELECT u.is_admin=1 OR EXISTS(SELECT 1 FROM labflow_effective_user_roles ur JOIN roles r ON r.id=ur.role_id LEFT JOIN role_templates t ON t.id=r.template_id WHERE ur.user_id=u.id AND (labflow_role_identity(r.system_key,r.is_system,t.identity_key,r.name) IN ('system_admin','analysis_leader','rd_leader'))) OR (?2='public_analysis' AND NOT EXISTS(SELECT 1 FROM user_divisions WHERE user_id=u.id) AND u.division_id IS NULL) FROM users u WHERE u.id=?1",postgres_compat::params![user_id,next_purpose],|r|r.get(0))?;
            if incompatible {
                conflicts.push(name);
            }
        }
        if !conflicts.is_empty() {
            return Err(AppError::Validation(format!("用途变更未保存：{} 位成员存在管理员/组长身份冲突或缺少公共检测所属部门：{}。请先在用户管理核对，原归属保持不变。",conflicts.len(),conflicts.into_iter().take(10).collect::<Vec<_>>().join("、"))));
        }
    }
    let mut next_permissions = permissions
        .cloned()
        .unwrap_or_else(|| current.permissions.clone());
    if next_purpose == "public_analysis" {
        next_permissions.push("records:work:portal-scoped".into());
    }
    next_permissions.sort();
    next_permissions.dedup();
    for permission in &next_permissions {
        // Preserve stored compatibility keys; a client cannot introduce an unknown key.
        if !is_valid_permission(permission) && !current.permissions.contains(permission) {
            return Err(AppError::Validation(format!("非法权限点: {permission}")));
        }
    }
    let mut division_ids = scopes
        .map(|v| v.division_ids.clone())
        .unwrap_or_else(|| current.division_scope_ids.clone());
    let mut work_division_ids = scopes
        .map(|v| v.work_division_ids.clone())
        .unwrap_or_else(|| current.work_division_scope_ids.clone());
    let mut type_keys: Vec<String> = scopes
        .map(|v| {
            v.sample_info_type_keys
                .iter()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .collect()
        })
        .unwrap_or_else(|| current.sample_info_type_scope_keys.clone());
    division_ids.sort_unstable();
    division_ids.dedup();
    work_division_ids.sort_unstable();
    work_division_ids.dedup();
    type_keys.sort();
    type_keys.dedup();
    if scopes.is_some() {
        for (ids, prior, label) in [
            (&division_ids, &current.division_scope_ids, "研发来源部门"),
            (
                &work_division_ids,
                &current.work_division_scope_ids,
                "分析来源部门",
            ),
        ] {
            for id in ids {
                let active: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM divisions WHERE id=?1 AND deleted_at IS NULL AND is_active=1)", [id], |row| row.get(0))?;
                if !active && !prior.contains(id) {
                    return Err(AppError::Validation(format!("{label}不存在或已停用: {id}")));
                }
            }
        }
        for key in &type_keys {
            let active: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM sample_info_types WHERE type_key=?1 AND deleted_at IS NULL AND is_active=1)", [key], |row| row.get(0))?;
            if !active && !current.sample_info_type_scope_keys.contains(key) {
                return Err(AppError::Validation(format!(
                    "样品登记类型不存在或已停用: {key}"
                )));
            }
        }
    }
    if next_permissions == current.permissions
        && division_ids == current.division_scope_ids
        && work_division_ids == current.work_division_scope_ids
        && type_keys == current.sample_info_type_scope_keys
        && next_purpose == current.role_purpose
        && next_name == current.name
        && next_description == current.description
        && next_sort == current.sort_order
    {
        if preview {
            tx.rollback()?;
        } else {
            tx.commit()?;
        }
        return Ok(current);
    }
    let before = serde_json::to_value(&current).unwrap_or(serde_json::Value::Null);
    tx.execute(
        "UPDATE roles SET name=?1,description=?2,sort_order=?3,role_purpose=COALESCE(?4,role_purpose) WHERE id=?5",
        postgres_compat::params![
            next_name,
            next_description,
            next_sort,
            fields.and_then(|fields| fields.role_purpose.as_deref()),
            role_id
        ],
    )?;
    tx.execute("DELETE FROM role_permissions WHERE role_id=?1", [role_id])?;
    for permission in &next_permissions {
        tx.execute(
            "INSERT INTO role_permissions(role_id,permission_key) VALUES(?1,?2)",
            postgres_compat::params![role_id, permission],
        )?;
    }
    tx.execute(
        "DELETE FROM role_division_scopes WHERE role_id=?1",
        [role_id],
    )?;
    tx.execute(
        "DELETE FROM role_work_division_scopes WHERE role_id=?1",
        [role_id],
    )?;
    tx.execute(
        "DELETE FROM role_sample_info_type_scopes WHERE role_id=?1",
        [role_id],
    )?;
    for id in &division_ids {
        tx.execute(
            "INSERT INTO role_division_scopes(role_id,division_id) VALUES(?1,?2)",
            postgres_compat::params![role_id, id],
        )?;
    }
    for id in &work_division_ids {
        tx.execute(
            "INSERT INTO role_work_division_scopes(role_id,division_id) VALUES(?1,?2)",
            postgres_compat::params![role_id, id],
        )?;
    }
    for key in &type_keys {
        tx.execute(
            "INSERT INTO role_sample_info_type_scopes(role_id,type_key) VALUES(?1,?2)",
            postgres_compat::params![role_id, key],
        )?;
    }
    let updated = with_permissions(&tx, get_role_on_conn(&tx, role_id)?)?;
    if preview {
        // The same validation and writes are exercised, but preview never records an
        // audit entry, revokes a session, or persists any configuration change.
        tx.rollback()?;
        return Ok(updated);
    }
    let after = serde_json::to_value(&updated).unwrap_or(serde_json::Value::Null);
    audit_repo::log_structured_on_conn(
        &tx,
        "update",
        "roles",
        Some(role_id),
        operator,
        &format!("修改角色「{}」权限与数据范围", updated.name),
        "shared",
        "",
        Some(&before),
        Some(&after),
        "management",
    )?;
    tx.execute("DELETE FROM user_sessions WHERE user_id IN (SELECT user_id FROM labflow_effective_user_roles WHERE role_id=?1)", [role_id])?;
    tx.commit()?;
    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{init_pool, test_migrations};
    use crate::repo::user_repo;

    fn test_pool() -> DbPool {
        let pool = init_pool("postgres-test");
        test_migrations::run(&pool.get().unwrap()).unwrap();
        pool
    }

    #[test]
    fn v2414_configuration_preview_rolls_back_configuration_audit_and_sessions() {
        let pool = test_pool();
        let request:RoleCreate=serde_json::from_value(serde_json::json!({"name":format!("preview-role-{}",uuid::Uuid::new_v4()),"permissions":["entry:sample"]})).unwrap();
        let role = create(&pool, &request, "admin").unwrap();
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO divisions(name) VALUES(?1)",
            [format!("preview-department-{}", uuid::Uuid::new_v4())],
        )
        .unwrap();
        let division = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO users(username,password,role_id) VALUES(?1,'unused',?2)",
            postgres_compat::params![format!("preview-member-{}", uuid::Uuid::new_v4()), role.id],
        )
        .unwrap();
        let user_id = conn.last_insert_rowid();
        let token = format!("preview-session-{}", uuid::Uuid::new_v4());
        conn.execute("INSERT INTO user_sessions(user_id,token,expires_at) VALUES(?1,?2,'2099-12-31 23:59:59')",postgres_compat::params![user_id,token]).unwrap();
        let counts = || -> (i64, i64) {
            conn.query_row("SELECT (SELECT COUNT(*) FROM audit_log WHERE table_name='roles' AND record_id=?1),(SELECT COUNT(*) FROM user_sessions WHERE user_id=?2 AND token=?3)",postgres_compat::params![role.id,user_id,token],|row|Ok((row.get(0)?,row.get(1)?))).unwrap()
        };
        let before = counts();
        let body:RoleConfigurationSet=serde_json::from_value(serde_json::json!({"expected_revision":role.configuration_revision,"permissions":["entry:workload"],"division_ids":[division],"work_division_ids":[division],"sample_info_type_keys":[],"role_purpose":"public_analysis"})).unwrap();
        assert!(matches!(
            preview_configuration(&pool, role.id, &body, "admin"),
            Err(AppError::Validation(_))
        ));
        assert_eq!(counts(), before);
        assert_eq!(
            with_permissions(&conn, get_role_on_conn(&conn, role.id).unwrap())
                .unwrap()
                .configuration_revision,
            role.configuration_revision
        );
        conn.execute(
            "UPDATE users SET division_id=?1 WHERE id=?2",
            postgres_compat::params![division, user_id],
        )
        .unwrap();
        let predicted = preview_configuration(&pool, role.id, &body, "admin").unwrap();
        assert_eq!(predicted.role_purpose, "public_analysis");
        assert_eq!(predicted.division_scope_ids, vec![division]);
        assert_eq!(predicted.work_division_scope_ids, vec![division]);
        assert_eq!(
            predicted.required_permissions,
            vec!["records:work:portal-scoped"]
        );
        assert!(predicted.permissions.contains(&"entry:workload".into()));
        assert_ne!(
            predicted.configuration_revision,
            role.configuration_revision
        );
        assert_eq!(counts(), before);
        assert_eq!(
            with_permissions(&conn, get_role_on_conn(&conn, role.id).unwrap())
                .unwrap()
                .configuration_revision,
            role.configuration_revision
        );
        assert!(
            !user_repo::find_by_id(&pool, user_id)
                .unwrap()
                .unwrap()
                .is_analysis_public_account
        );
        let saved = set_configuration(&pool, role.id, &body, "admin").unwrap();
        assert_eq!(
            saved.configuration_revision,
            predicted.configuration_revision
        );
        assert_eq!(counts(), (before.0 + 1, 0));
    }

    #[test]
    fn v2414_template_rename_and_stop_preserve_roles_and_identity() {
        let pool = test_pool();
        let base = list_templates(&pool)
            .unwrap()
            .into_iter()
            .find(|t| t.identity_key == "analysis_leader")
            .unwrap();
        let request:RoleTemplateCreate=serde_json::from_value(serde_json::json!({"name":format!("template-{}",uuid::Uuid::new_v4()),"base_template_id":base.id,"base_revision":base.configuration_revision})).unwrap();
        let template = create_template(&pool, &request, "admin").unwrap();
        let request:RoleCreate=serde_json::from_value(serde_json::json!({"name":format!("role-{}",uuid::Uuid::new_v4()),"template_id":template.id,"template_revision":template.configuration_revision})).unwrap();
        let role = create(&pool, &request, "admin").unwrap();
        assert_eq!(role.identity_key, "analysis_leader");
        let request:RoleTemplateUpdate=serde_json::from_value(serde_json::json!({"expected_revision":template.configuration_revision,"name":format!("renamed-{}",uuid::Uuid::new_v4()),"permissions":[],"is_active":false})).unwrap();
        let stopped = update_template(&pool, template.id, &request, "admin").unwrap();
        assert_eq!(stopped.identity_key, "analysis_leader");
        assert_eq!(stopped.reference_count, 1);
        assert!(!stopped.is_active);
        assert!(matches!(
            update_template(&pool, template.id, &request, "admin"),
            Err(AppError::Conflict(_))
        ));
        let unchanged = list_with_permissions(&pool)
            .unwrap()
            .into_iter()
            .find(|r| r.id == role.id)
            .unwrap();
        assert_eq!(unchanged.permissions, role.permissions);
        assert_eq!(
            unchanged.configuration_revision,
            role.configuration_revision
        );
        assert_eq!(unchanged.identity_key, "analysis_leader");
        let request:RoleCreate=serde_json::from_value(serde_json::json!({"name":format!("blocked-{}",uuid::Uuid::new_v4()),"template_id":template.id})).unwrap();
        assert!(matches!(
            create(&pool, &request, "admin"),
            Err(AppError::Validation(_))
        ));
        let request:RoleCreate=serde_json::from_value(serde_json::json!({"name":format!("public-leader-{}",uuid::Uuid::new_v4()),"template_id":base.id,"role_purpose":"public_analysis"})).unwrap();
        assert!(matches!(
            create(&pool, &request, "admin"),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn v2414_explicit_personal_overrides_custom_legacy_key_and_legacy_primary_falls_back() {
        let pool = test_pool();
        let conn = pool.get().unwrap();
        conn.execute("INSERT INTO roles(name,is_system,system_key,role_purpose) VALUES(?1,0,'analysis_public_account',NULL)",[format!("legacy-purpose-{}",uuid::Uuid::new_v4())]).unwrap();
        let role_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO role_permissions(role_id,permission_key) VALUES(?1,'entry:workload')",
            [role_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO users(username,password,role_id) VALUES(?1,'unused',?2)",
            postgres_compat::params![format!("legacy-person-{}", uuid::Uuid::new_v4()), role_id],
        )
        .unwrap();
        let user_id = conn.last_insert_rowid();
        drop(conn);
        let user = user_repo::find_by_id(&pool, user_id).unwrap().unwrap();
        assert_eq!(user.role_ids, vec![role_id]);
        assert!(user.is_analysis_public_account);
        let role = list_with_permissions(&pool)
            .unwrap()
            .into_iter()
            .find(|r| r.id == role_id)
            .unwrap();
        let assert_legacy_purpose = |role: &RoleWithPermissions| {
            let conn = pool.get().unwrap();
            let raw_purpose: Option<String> = conn
                .query_row(
                    "SELECT role_purpose FROM roles WHERE id=?1",
                    [role_id],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(raw_purpose, None);
            assert_eq!(role.role_purpose, "public_analysis");
            assert!(
                user_repo::find_by_id(&pool, user_id)
                    .unwrap()
                    .unwrap()
                    .is_analysis_public_account
            );
        };
        let changed_permissions = set_permissions(
            &pool,
            role_id,
            &RolePermissionSet {
                permissions: vec!["entry:workload".into(), "entry:sample".into()],
            },
            "admin",
        )
        .unwrap();
        assert_ne!(
            changed_permissions.configuration_revision,
            role.configuration_revision
        );
        assert_legacy_purpose(&changed_permissions);
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO divisions(name,is_active) VALUES(?1,1)",
            [format!("legacy-purpose-scope-{}", uuid::Uuid::new_v4())],
        )
        .unwrap();
        let division_id = conn.last_insert_rowid();
        drop(conn);
        let changed_scopes = set_data_scopes(
            &pool,
            role_id,
            &RoleDataScopeSet {
                division_ids: vec![division_id],
                work_division_ids: vec![],
                sample_info_type_keys: vec![],
            },
            "admin",
        )
        .unwrap();
        assert_ne!(
            changed_scopes.configuration_revision,
            changed_permissions.configuration_revision
        );
        assert_legacy_purpose(&changed_scopes);
        let omitted_purpose:RoleConfigurationSet=serde_json::from_value(serde_json::json!({"expected_revision":changed_scopes.configuration_revision,"permissions":changed_scopes.permissions,"division_ids":[division_id],"work_division_ids":[division_id],"sample_info_type_keys":[]})).unwrap();
        let role = set_configuration(&pool, role_id, &omitted_purpose, "admin").unwrap();
        assert_ne!(
            role.configuration_revision,
            changed_scopes.configuration_revision
        );
        assert_legacy_purpose(&role);
        let request:RoleConfigurationSet=serde_json::from_value(serde_json::json!({"expected_revision":role.configuration_revision,"permissions":role.permissions,"division_ids":[],"work_division_ids":[],"sample_info_type_keys":[],"role_purpose":"personal"})).unwrap();
        let saved = set_configuration(&pool, role_id, &request, "admin").unwrap();
        assert_eq!(saved.role_purpose, "personal");
        let user = user_repo::find_by_id(&pool, user_id).unwrap().unwrap();
        assert!(!user.is_analysis_public_account);
        assert!(user
            .role_keys
            .iter()
            .any(|key| key == "analysis_public_account"));
        let context = crate::service::authz_service::AuthContext {
            role_names: user.role_names.clone(),
            user,
        };
        assert!(!context.is_analysis_public_account());
        assert!(user_repo::list_work_detectors(&pool)
            .unwrap()
            .iter()
            .any(|u| u.id == user_id));
        let saved = set_permissions(
            &pool,
            role_id,
            &RolePermissionSet {
                permissions: vec!["entry:workload".into()],
            },
            "admin",
        )
        .unwrap();
        assert_eq!(saved.role_purpose, "personal");
    }

    #[test]
    fn v2414_public_purpose_validates_members_atomically_and_keeps_affiliations() {
        let pool = test_pool();
        let request:RoleCreate=serde_json::from_value(serde_json::json!({"name":format!("purpose-{}",uuid::Uuid::new_v4()),"permissions":["entry:workload"]})).unwrap();
        let role = create(&pool, &request, "admin").unwrap();
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO users(username,password,role_id) VALUES(?1,'unused',?2)",
            postgres_compat::params![format!("purpose-member-{}", uuid::Uuid::new_v4()), role.id],
        )
        .unwrap();
        let user_id = conn.last_insert_rowid();
        drop(conn);
        let request:RoleConfigurationSet=serde_json::from_value(serde_json::json!({"expected_revision":role.configuration_revision,"permissions":[],"division_ids":[],"work_division_ids":[],"sample_info_type_keys":[],"role_purpose":"public_analysis"})).unwrap();
        assert!(matches!(
            set_configuration(&pool, role.id, &request, "admin"),
            Err(AppError::Validation(_))
        ));
        let current = list_with_permissions(&pool)
            .unwrap()
            .into_iter()
            .find(|r| r.id == role.id)
            .unwrap();
        assert_eq!(current.configuration_revision, role.configuration_revision);
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO divisions(name,is_active) VALUES(?1,1)",
            [format!("purpose-department-{}", uuid::Uuid::new_v4())],
        )
        .unwrap();
        let division = conn.last_insert_rowid();
        conn.execute(
            "UPDATE users SET division_id=?1 WHERE id=?2",
            postgres_compat::params![division, user_id],
        )
        .unwrap();
        drop(conn);
        let saved = set_configuration(&pool, role.id, &request, "admin").unwrap();
        assert_eq!(
            saved.required_permissions,
            vec!["records:work:portal-scoped"]
        );
        assert!(saved
            .permissions
            .contains(&"records:work:portal-scoped".into()));
        let user = user_repo::find_by_id(&pool, user_id).unwrap().unwrap();
        assert!(user.is_analysis_public_account);
        assert_eq!(user.division_id, Some(division));
        let leader = list_with_permissions(&pool)
            .unwrap()
            .into_iter()
            .find(|r| r.identity_key == "analysis_leader")
            .unwrap();
        let update: crate::models::user::UserUpdate =
            serde_json::from_value(serde_json::json!({"role_ids":[role.id,leader.id]})).unwrap();
        assert!(matches!(
            user_repo::update(&pool, user_id, &update, 1, "admin"),
            Err(AppError::Validation(_))
        ));
        assert_eq!(
            user_repo::find_by_id(&pool, user_id)
                .unwrap()
                .unwrap()
                .role_ids,
            vec![role.id]
        );
    }

    #[test]
    fn configuration_is_atomic_revisioned_and_legacy_updates_preserve_other_fields() {
        let pool = test_pool();
        let role = create(
            &pool,
            &RoleCreate {
                role_purpose: None,
                template_revision: None,
                permissions_override: false,
                name: format!("atomic-{}", uuid::Uuid::new_v4()),
                description: String::new(),
                permissions: vec!["entry:sample".into()],
                sort_order: 0,
                template_id: None,
            },
            "admin",
        )
        .unwrap();
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO divisions(name,is_active) VALUES(?1,1)",
            [format!("atomic-scope-{}", role.id)],
        )
        .unwrap();
        let department = conn.last_insert_rowid();
        drop(conn);
        let mut request = RoleConfigurationSet {
            role_purpose: None,
            name: None,
            description: None,
            sort_order: None,
            expected_revision: role.configuration_revision.clone(),
            permissions: vec!["entry:workload".into()],
            division_ids: vec![department],
            work_division_ids: vec![-1],
            sample_info_type_keys: vec![],
        };
        assert!(set_configuration(&pool, role.id, &request, "admin").is_err());
        let unchanged = list_with_permissions(&pool)
            .unwrap()
            .into_iter()
            .find(|r| r.id == role.id)
            .unwrap();
        assert_eq!(
            unchanged.configuration_revision,
            role.configuration_revision
        );
        assert_eq!(unchanged.permissions, role.permissions);
        assert!(unchanged.division_scope_ids.is_empty());
        request.work_division_ids = vec![department];
        let saved = set_configuration(&pool, role.id, &request, "admin").unwrap();
        assert_eq!(saved.permissions, vec!["entry:workload"]);
        assert_eq!(saved.division_scope_ids, vec![department]);
        assert!(matches!(
            set_configuration(&pool, role.id, &request, "admin"),
            Err(AppError::Conflict(_))
        ));
        let legacy = set_permissions(
            &pool,
            role.id,
            &RolePermissionSet {
                permissions: vec!["entry:sample".into()],
            },
            "admin",
        )
        .unwrap();
        assert_eq!(legacy.division_scope_ids, saved.division_scope_ids);
        assert_eq!(
            legacy.work_division_scope_ids,
            saved.work_division_scope_ids
        );
        assert_ne!(legacy.configuration_revision, saved.configuration_revision);
        let legacy_scopes = set_data_scopes(
            &pool,
            role.id,
            &RoleDataScopeSet {
                division_ids: vec![],
                work_division_ids: vec![],
                sample_info_type_keys: vec![],
            },
            "admin",
        )
        .unwrap();
        assert_eq!(legacy_scopes.permissions, legacy.permissions);
        assert!(legacy_scopes.division_scope_ids.is_empty());
    }

    #[test]
    fn stored_compatibility_permissions_and_public_required_switch_survive_configuration_save() {
        let pool = test_pool();
        let role = create(
            &pool,
            &RoleCreate {
                role_purpose: None,
                template_revision: None,
                permissions_override: false,
                name: format!("compatibility-{}", uuid::Uuid::new_v4()),
                description: String::new(),
                permissions: vec![],
                sort_order: 0,
                template_id: None,
            },
            "admin",
        )
        .unwrap();
        pool.get().unwrap().execute("INSERT INTO role_permissions(role_id,permission_key) VALUES(?1,'legacy:stored-capability')", [role.id]).unwrap();
        let stored = list_with_permissions(&pool)
            .unwrap()
            .into_iter()
            .find(|r| r.id == role.id)
            .unwrap();
        let updated = set_configuration(
            &pool,
            role.id,
            &RoleConfigurationSet {
                role_purpose: None,
                name: None,
                description: None,
                sort_order: None,
                expected_revision: stored.configuration_revision,
                permissions: vec!["legacy:stored-capability".into(), "entry:sample".into()],
                division_ids: vec![],
                work_division_ids: vec![],
                sample_info_type_keys: vec![],
            },
            "admin",
        )
        .unwrap();
        assert!(updated
            .permissions
            .iter()
            .any(|p| p == "legacy:stored-capability"));
        assert!(set_permissions(
            &pool,
            role.id,
            &RolePermissionSet {
                permissions: vec!["unknown:new-key".into()]
            },
            "admin"
        )
        .is_err());
        let public_role = list_with_permissions(&pool)
            .unwrap()
            .into_iter()
            .find(|r| r.is_public_account && !r.required_permissions.is_empty())
            .unwrap();
        let public = set_permissions(
            &pool,
            public_role.id,
            &RolePermissionSet {
                permissions: vec![],
            },
            "admin",
        )
        .unwrap();
        assert_eq!(public.permissions, vec!["records:work:portal-scoped"]);
        assert_eq!(public.required_permissions, public.permissions);
    }

    #[test]
    fn fixed_roles_custom_roles_and_multi_role_union_are_preserved() {
        let pool = test_pool();
        let roles = list_with_permissions(&pool).unwrap();
        let fixed: Vec<_> = roles.iter().filter(|role| role.is_system == 1).collect();
        // 系统内置角色包含管理员、分析检测员/组长、研发送样员/组长和两类公共账号。
        assert_eq!(fixed.len(), 7);

        assert!(fixed.iter().any(|role| role.name == "研发送样公共账号"));
        let analysis_public = fixed
            .iter()
            .find(|role| role.name == "分析检测公共账号")
            .unwrap();
        assert!(analysis_public
            .permissions
            .iter()
            .any(|value| value == "records:work:select-detector"));

        let analyst = fixed.iter().find(|role| role.name == "分析检测员").unwrap();
        assert!(analyst
            .permissions
            .iter()
            .any(|value| value == "sample:collect"));
        assert!(analyst
            .permissions
            .iter()
            .any(|value| value == "sample:complete"));
        assert!(!analyst
            .permissions
            .iter()
            .any(|value| value == "manage:projects"));

        let leader = fixed
            .iter()
            .find(|role| role.name == "分析检测组长")
            .unwrap();
        assert!(leader
            .permissions
            .iter()
            .any(|value| value == "manage:master-import"));
        assert!(leader
            .permissions
            .iter()
            .any(|value| value == "manage:users"));
        assert!(leader.permissions.iter().any(|value| value == "help:edit"));

        assert!(matches!(
            update(
                &pool,
                analyst.id,
                &RoleUpdate {
                    name: None,
                    description: Some("changed".into()),
                    sort_order: None,
                },
                "admin",
            ),
            Err(AppError::Forbidden(_))
        ));
        let updated = set_permissions(
            &pool,
            analyst.id,
            &RolePermissionSet {
                permissions: vec!["entry:workload".into(), "manage:help".into()],
            },
            "admin",
        )
        .unwrap();
        assert_eq!(updated.permissions, vec!["entry:workload", "manage:help"]);

        test_migrations::run(&pool.get().unwrap()).unwrap();
        let persisted = list_with_permissions(&pool)
            .unwrap()
            .into_iter()
            .find(|role| role.id == analyst.id)
            .unwrap();
        assert_eq!(persisted.permissions, vec!["entry:workload", "manage:help"]);

        let custom = create(
            &pool,
            &RoleCreate {
                role_purpose: None,
                template_revision: None,
                permissions_override: false,
                name: "CustomRole01".into(),
                description: "custom".into(),
                permissions: vec!["manage:help".into()],
                sort_order: 20,
                template_id: None,
            },
            "admin",
        )
        .unwrap();
        assert_eq!(custom.is_system, 0);
        let rd_sender_id = fixed
            .iter()
            .find(|role| role.name == "研发送样员")
            .unwrap()
            .id;
        let user_id = {
            let conn = pool.get().unwrap();
            conn.execute(
                "INSERT INTO users(username,password,is_admin,is_active,role_id) VALUES('multi_role_user','hash',0,1,?1)",
                [custom.id],
            ).unwrap();
            let user_id = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO user_roles(user_id,role_id) VALUES(?1,?2)",
                postgres_compat::params![user_id, custom.id],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO user_roles(user_id,role_id) VALUES(?1,?2)",
                postgres_compat::params![user_id, rd_sender_id],
            )
            .unwrap();
            user_id
        };
        let user = user_repo::find_by_id(&pool, user_id).unwrap().unwrap();
        assert_eq!(user.role_ids.len(), 2);
        assert!(user.permissions.iter().any(|value| value == "manage:help"));
        assert!(user.permissions.iter().any(|value| value == "entry:sample"));

        test_migrations::run(&pool.get().unwrap()).unwrap();
        assert!(list_with_permissions(&pool)
            .unwrap()
            .iter()
            .any(|role| { role.name == "CustomRole01" && role.is_system == 0 }));
    }

    #[test]
    fn role_data_scopes_are_structured_and_replaced_atomically() {
        let pool = test_pool();
        let conn = pool.get().unwrap();
        let suffix = uuid::Uuid::new_v4().simple().to_string();
        conn.execute(
            "INSERT INTO divisions(name,sort_order,color,is_active) VALUES(?1,0,'#1976d2',1)",
            [format!("范围部门-{suffix}")],
        )
        .unwrap();
        let division_id = conn.last_insert_rowid();
        let type_key = format!("scope_{}", &suffix[..12]);
        conn.execute(
            "INSERT INTO sample_info_types(type_key,label,description,color,sort_order,is_active) VALUES(?1,?2,'','#1976d2',0,1)",
            postgres_compat::params![type_key, format!("范围类型-{suffix}")],
        ).unwrap();
        let role = create(
            &pool,
            &RoleCreate {
                role_purpose: None,
                template_revision: None,
                permissions_override: false,
                name: format!("范围角色-{suffix}"),
                description: String::new(),
                permissions: vec!["entry:sample-info".into()],
                sort_order: 99,
                template_id: None,
            },
            "admin",
        )
        .unwrap();
        let updated = set_data_scopes(
            &pool,
            role.id,
            &RoleDataScopeSet {
                division_ids: vec![division_id, division_id],
                work_division_ids: vec![division_id, division_id],
                sample_info_type_keys: vec![type_key.clone(), type_key.clone()],
            },
            "admin",
        )
        .unwrap();
        assert_eq!(updated.division_scope_ids, vec![division_id]);
        assert_eq!(updated.work_division_scope_ids, vec![division_id]);
        assert_eq!(updated.sample_info_type_scope_keys, vec![type_key.clone()]);
        let scopes = data_scopes_for_roles(&pool, &[role.id]).unwrap();
        assert_eq!(scopes.len(), 1);
        assert_eq!(scopes[0].division_ids, vec![division_id]);
        assert_eq!(scopes[0].sample_info_type_keys, vec![type_key]);
    }

    #[test]
    fn batched_scopes_keep_roles_paired_and_work_ranges_deduplicated() {
        let pool = test_pool();
        let conn = pool.get().unwrap();
        let mut divisions = Vec::new();
        for name in ["batch-department-a", "batch-department-b"] {
            conn.execute("INSERT INTO divisions(name) VALUES(?1)", [name])
                .unwrap();
            divisions.push(conn.last_insert_rowid());
        }
        let mut roles = Vec::new();
        for (index, name) in ["batch-role-a", "batch-role-b", "batch-empty-role"]
            .iter()
            .enumerate()
        {
            conn.execute("INSERT INTO roles(name,is_system) VALUES(?1,0)", [*name])
                .unwrap();
            let role = conn.last_insert_rowid();
            roles.push(role);
            if index < 2 {
                let key = format!("batch-type-{index}");
                conn.execute(
                    "INSERT INTO sample_info_types(type_key,label) VALUES(?1,?1)",
                    [&key],
                )
                .unwrap();
                conn.execute(
                    "INSERT INTO role_division_scopes(role_id,division_id) VALUES(?1,?2)",
                    [role, divisions[index]],
                )
                .unwrap();
                conn.execute(
                    "INSERT INTO role_sample_info_type_scopes(role_id,type_key) VALUES(?1,?2)",
                    postgres_compat::params![role, key],
                )
                .unwrap();
                conn.execute(
                    "INSERT INTO role_work_division_scopes(role_id,division_id) VALUES(?1,?2)",
                    [role, divisions[0]],
                )
                .unwrap();
            }
        }
        drop(conn);
        let scopes =
            data_scopes_for_roles(&pool, &[roles[1], roles[2], roles[0], roles[1]]).unwrap();
        assert_eq!(
            scopes.iter().map(|scope| scope.role_id).collect::<Vec<_>>(),
            vec![roles[1], roles[0], roles[1]]
        );
        assert_eq!(scopes[0].division_ids, vec![divisions[1]]);
        assert_eq!(scopes[0].sample_info_type_keys, vec!["batch-type-1"]);
        assert_eq!(scopes[1].division_ids, vec![divisions[0]]);
        assert_eq!(scopes[1].sample_info_type_keys, vec!["batch-type-0"]);
        assert_eq!(
            work_division_scope_ids_for_roles(&pool, &roles).unwrap(),
            vec![divisions[0]]
        );
        assert!(data_scopes_for_roles(&pool, &[]).unwrap().is_empty());
        assert!(work_division_scope_ids_for_roles(&pool, &[])
            .unwrap()
            .is_empty());
    }
}
