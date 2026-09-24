use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::role::{
    is_valid_permission, Role, RoleCreate, RoleDataScope, RoleDataScopeSet, RolePermissionSet,
    RoleTemplate, RoleUpdate, RoleWithPermissions,
};
use crate::repo::{audit_repo, trash_repo};
use postgres_compat::OptionalExtension;

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
    let mut stmt = conn.prepare(
        "SELECT id,name,description,is_system,sort_order FROM role_templates WHERE deleted_at IS NULL ORDER BY sort_order,id",
    )?;
    let templates = stmt.query_map([], |row| {
        let id: i64 = row.get(0)?;
        let mut permissions_stmt = conn.prepare("SELECT permission_key FROM role_template_permissions WHERE template_id=?1 ORDER BY permission_key")?;
        let permissions = permissions_stmt.query_map([id], |item| item.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(RoleTemplate { id, name: row.get(1)?, description: row.get(2)?, is_system: row.get(3)?, sort_order: row.get(4)?, permissions })
    })?;
    Ok(templates.collect::<std::result::Result<Vec<_>, _>>()?)
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
    Ok(RoleWithPermissions {
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
    })
}

/// Returns scope sets per role. Callers must evaluate each role separately and then union the
/// permitted record sets; combining a role's department with another role's type is unsafe.
pub fn data_scopes_for_roles(pool: &DbPool, role_ids: &[i64]) -> Result<Vec<RoleDataScope>> {
    let conn = pool.get()?;
    let mut scopes = Vec::new();
    for role_id in role_ids {
        let division_ids = get_division_scope_ids_on_conn(&conn, *role_id)?;
        let sample_info_type_keys = get_sample_info_type_scope_keys_on_conn(&conn, *role_id)?;
        if !division_ids.is_empty() || !sample_info_type_keys.is_empty() {
            scopes.push(RoleDataScope {
                role_id: *role_id,
                division_ids,
                sample_info_type_keys,
                work_division_ids: vec![],
            });
        }
    }
    Ok(scopes)
}

pub fn work_division_scope_ids_for_roles(pool: &DbPool, role_ids: &[i64]) -> Result<Vec<i64>> {
    let conn = pool.get()?;
    let mut ids = Vec::new();
    for role_id in role_ids {
        ids.extend(get_work_division_scope_ids_on_conn(&conn, *role_id)?);
    }
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
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
    let mut conn = pool.get()?;
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM roles WHERE name=?1",
        [&body.name],
        |r| r.get(0),
    )?;
    if exists > 0 {
        return Err(AppError::Conflict(format!("角色名「{}」已存在", body.name)));
    }
    let template_permissions = if let Some(template_id) = body.template_id {
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM role_templates WHERE id=?1 AND deleted_at IS NULL)",
            [template_id],
            |r| r.get(0),
        )?;
        if !exists {
            return Err(AppError::Validation("角色模板不存在或已停用".into()));
        }
        let mut stmt = conn.prepare("SELECT permission_key FROM role_template_permissions WHERE template_id=?1 ORDER BY permission_key")?;
        stmt.query_map([template_id], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?
    } else {
        Vec::new()
    };
    let permissions = if body.permissions.is_empty() {
        template_permissions
    } else {
        body.permissions.clone()
    };
    for p in &permissions {
        if !is_valid_permission(p) {
            return Err(AppError::Validation(format!("非法权限点: {}", p)));
        }
    }
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO roles (name, description, is_system, sort_order, template_id) VALUES (?1,?2,0,?3,?4)",
        postgres_compat::params![body.name, body.description, body.sort_order, body.template_id],
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

/// 整体替换角色权限点
pub fn set_permissions(
    pool: &DbPool,
    role_id: i64,
    body: &RolePermissionSet,
    operator: &str,
) -> Result<RoleWithPermissions> {
    let mut conn = pool.get()?;
    let current = with_permissions(&conn, get_role_on_conn(&conn, role_id)?)?;
    let is_analysis_public_account: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM roles WHERE id=?1 AND system_key='analysis_public_account')",
        [role_id],
        |row| row.get(0),
    )?;
    let mut permissions = body.permissions.clone();
    permissions.sort();
    permissions.dedup();
    if is_analysis_public_account
        && !permissions
            .iter()
            .any(|permission| permission == "records:work:portal-scoped")
    {
        // Required internal switch for the public-account personnel selection flow.
        permissions.push("records:work:portal-scoped".into());
    }
    for p in &permissions {
        if !is_valid_permission(p) {
            return Err(AppError::Validation(format!("非法权限点: {}", p)));
        }
    }
    let before = serde_json::to_value(&current).unwrap_or(serde_json::Value::Null);
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM role_permissions WHERE role_id=?1", [role_id])?;
    for p in &permissions {
        tx.execute(
            "INSERT INTO role_permissions (role_id, permission_key) VALUES (?1,?2)",
            postgres_compat::params![role_id, p],
        )?;
    }
    tx.execute(
        "DELETE FROM user_sessions
         WHERE user_id IN (
           SELECT id FROM users WHERE role_id=?1
           UNION
           SELECT user_id FROM user_roles WHERE role_id=?1
         )",
        [role_id],
    )?;
    let updated = with_permissions(&tx, get_role_on_conn(&tx, role_id)?)?;
    let after = serde_json::to_value(&updated).unwrap_or(serde_json::Value::Null);
    audit_repo::log_structured_on_conn(
        &tx,
        "update",
        "roles",
        Some(role_id),
        operator,
        &format!("修改角色「{}」权限", updated.name),
        "shared",
        "",
        Some(&before),
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(updated)
}

/// Replaces the configured data-view scopes for one role.
pub fn set_data_scopes(
    pool: &DbPool,
    role_id: i64,
    body: &RoleDataScopeSet,
    operator: &str,
) -> Result<RoleWithPermissions> {
    let mut conn = pool.get()?;
    let current = with_permissions(&conn, get_role_on_conn(&conn, role_id)?)?;

    let mut division_ids = body.division_ids.clone();
    division_ids.sort_unstable();
    division_ids.dedup();
    let mut work_division_ids = body.work_division_ids.clone();
    work_division_ids.sort_unstable();
    work_division_ids.dedup();
    for division_id in &division_ids {
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM divisions WHERE id=?1 AND deleted_at IS NULL AND is_active=1)",
            [division_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(AppError::Validation(format!(
                "部门范围不存在或已停用: {}",
                division_id
            )));
        }
    }
    for division_id in &work_division_ids {
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM divisions WHERE id=?1 AND deleted_at IS NULL",
            [*division_id],
            |row| row.get(0),
        )?;
        if exists == 0 {
            return Err(AppError::Validation(format!(
                "分析检测部门不存在: {}",
                division_id
            )));
        }
    }
    let mut type_keys: Vec<String> = body
        .sample_info_type_keys
        .iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect();
    type_keys.sort();
    type_keys.dedup();
    for type_key in &type_keys {
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sample_info_types WHERE type_key=?1 AND deleted_at IS NULL AND is_active=1)",
            [type_key],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(AppError::Validation(format!(
                "样品信息登记类型不存在或已停用: {}",
                type_key
            )));
        }
    }

    let before = serde_json::to_value(&current).unwrap_or(serde_json::Value::Null);
    let tx = conn.transaction()?;
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
    for division_id in &division_ids {
        tx.execute(
            "INSERT INTO role_division_scopes(role_id,division_id) VALUES(?1,?2)",
            postgres_compat::params![role_id, division_id],
        )?;
    }
    for division_id in &work_division_ids {
        tx.execute(
            "INSERT INTO role_work_division_scopes(role_id,division_id) VALUES(?1,?2)",
            postgres_compat::params![role_id, division_id],
        )?;
    }
    for type_key in &type_keys {
        tx.execute(
            "INSERT INTO role_sample_info_type_scopes(role_id,type_key) VALUES(?1,?2)",
            postgres_compat::params![role_id, type_key],
        )?;
    }
    tx.execute(
        "DELETE FROM user_sessions
         WHERE user_id IN (
           SELECT id FROM users WHERE role_id=?1
           UNION
           SELECT user_id FROM user_roles WHERE role_id=?1
         )",
        [role_id],
    )?;
    let updated = with_permissions(&tx, get_role_on_conn(&tx, role_id)?)?;
    let after = serde_json::to_value(&updated).unwrap_or(serde_json::Value::Null);
    audit_repo::log_structured_on_conn(
        &tx,
        "update",
        "roles",
        Some(role_id),
        operator,
        &format!("修改角色「{}」数据查看范围", updated.name),
        "shared",
        "",
        Some(&before),
        Some(&after),
        "management",
    )?;
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
}
