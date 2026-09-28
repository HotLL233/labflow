use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::user::{
    AnalysisPublicAccountCandidate, AnalysisPublicAccountScope, User, UserCreate, UserUpdate,
};
use crate::repo::{audit_repo, trash_repo};

/// 基础用户查询 SQL（含 role_id 列，索引 11）
const USER_SELECT: &str =
    "SELECT u.id, u.username, u.password, u.division_id, d.name AS division_name, \
        u.group_id, pg.name AS group_name, u.is_admin, u.is_active, \
        u.created_at, u.updated_at, u.role_id, r.name AS role_name \
 FROM users u \
 LEFT JOIN divisions d ON u.division_id = d.id \
 LEFT JOIN project_groups pg ON u.group_id = pg.id \
 LEFT JOIN roles r ON u.role_id = r.id";

/// 从一行映射为 User（permissions 暂置空，由调用方按需加载）
fn map_user(row: &postgres_compat::Row) -> postgres_compat::Result<User> {
    let role_id: Option<i64> = row.get(11)?;
    Ok(User {
        id: row.get(0)?,
        username: row.get(1)?,
        password: row.get(2)?,
        division_id: row.get(3)?,
        division_name: row.get(4)?,
        primary_division_id: row.get(3)?,
        primary_division_name: row.get(4)?,
        division_ids: vec![],
        division_names: vec![],
        business_division_ids: vec![],
        business_division_names: vec![],
        group_id: row.get(5)?,
        group_name: row.get(6)?,
        group_ids: vec![],
        group_names: vec![],
        is_admin: row.get::<_, i64>(7)? != 0,
        is_active: row.get::<_, i64>(8)? != 0,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        role_id,
        role_name: row.get(12)?,
        role_ids: vec![],
        role_names: vec![],
        role_keys: vec![],
        role_template_names: vec![],
        is_public_account: false,
        is_rd_public_account: false,
        is_analysis_public_account: false,
        affiliation_groups: vec![],
        permissions: vec![],
    })
}

fn hydrate_user_on_conn(conn: &postgres_compat::Connection, user: &mut User) -> Result<()> {
    user.division_ids = user_division_ids_on_conn(conn, user.id).unwrap_or_default();
    user.division_names = user_division_names_on_conn(conn, user.id).unwrap_or_default();
    user.business_division_ids = user.division_ids.clone();
    user.business_division_names = user.division_names.clone();
    user.group_ids = user_group_ids_on_conn(conn, user.id).unwrap_or_default();
    user.group_names = user_group_names_on_conn(conn, user.id).unwrap_or_default();
    user.role_ids = user_role_ids_on_conn(conn, user.id).unwrap_or_default();
    user.role_names = user_role_names_on_conn(conn, user.id).unwrap_or_default();
    user.role_keys = user_role_keys_on_conn(conn, user.id).unwrap_or_default();
    user.role_template_names = user_role_template_names_on_conn(conn, user.id).unwrap_or_default();
    user.is_rd_public_account = user
        .role_keys
        .iter()
        .any(|key| key == "rd_public_account" || key == "public_account");
    user.is_analysis_public_account = user
        .role_keys
        .iter()
        .any(|key| key == "analysis_public_account");
    user.is_public_account = user.is_rd_public_account;
    user.affiliation_groups = affiliation_group_names_on_conn(conn, user.id).unwrap_or_default();
    user.permissions = if !user.role_ids.is_empty() {
        user_perms_multi_on_conn(conn, user.id)?
    } else {
        user_perms_on_conn(conn, user.role_id)?
    };
    Ok(())
}

fn user_division_ids_on_conn(conn: &postgres_compat::Connection, user_id: i64) -> Result<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT division_id FROM user_divisions WHERE user_id=?1 ORDER BY is_primary DESC, division_id",
    )?;
    let rows = stmt.query_map([user_id], |row| row.get::<_, i64>(0))?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

fn user_division_names_on_conn(
    conn: &postgres_compat::Connection,
    user_id: i64,
) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT d.name FROM user_divisions ud JOIN divisions d ON d.id=ud.division_id
         WHERE ud.user_id=?1 ORDER BY ud.is_primary DESC, d.sort_order, d.id",
    )?;
    let rows = stmt.query_map([user_id], |row| row.get::<_, String>(0))?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

fn user_group_ids_on_conn(conn: &postgres_compat::Connection, user_id: i64) -> Result<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT group_id FROM user_groups WHERE user_id=?1 ORDER BY is_primary DESC, group_id",
    )?;
    let rows = stmt.query_map([user_id], |row| row.get::<_, i64>(0))?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

fn user_group_names_on_conn(
    conn: &postgres_compat::Connection,
    user_id: i64,
) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT pg.name FROM user_groups ug JOIN project_groups pg ON pg.id=ug.group_id
         WHERE ug.user_id=?1 ORDER BY ug.is_primary DESC, pg.sort_order, pg.id",
    )?;
    let rows = stmt.query_map([user_id], |row| row.get::<_, String>(0))?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

fn replace_user_affiliations_on_conn(
    conn: &postgres_compat::Connection,
    user_id: i64,
    division_ids: &[i64],
    primary_division_id: Option<i64>,
    group_ids: &[i64],
) -> Result<()> {
    conn.execute("DELETE FROM user_divisions WHERE user_id=?1", [user_id])?;
    for division_id in division_ids {
        conn.execute(
            "INSERT INTO user_divisions(user_id,division_id,is_primary) VALUES(?1,?2,?3)",
            postgres_compat::params![
                user_id,
                division_id,
                if Some(*division_id) == primary_division_id {
                    1
                } else {
                    0
                }
            ],
        )?;
    }
    conn.execute("DELETE FROM user_groups WHERE user_id=?1", [user_id])?;
    for (index, group_id) in group_ids.iter().enumerate() {
        conn.execute(
            "INSERT INTO user_groups(user_id,group_id,is_primary) VALUES(?1,?2,?3)",
            postgres_compat::params![user_id, group_id, if index == 0 { 1 } else { 0 }],
        )?;
    }
    Ok(())
}

/// 加载某角色关联的权限点集合（单角色，v0.4.74 前使用）
fn user_perms_on_conn(
    conn: &postgres_compat::Connection,
    role_id: Option<i64>,
) -> Result<Vec<String>> {
    let rid = match role_id {
        Some(v) => v,
        None => return Ok(vec![]),
    };
    let mut stmt = conn.prepare(
        "SELECT permission_key FROM role_permissions WHERE role_id=?1 ORDER BY permission_key",
    )?;
    let rows = stmt.query_map([rid], |row| row.get::<_, String>(0))?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

/// v0.4.74: 加载用户所有角色的权限点（合并去重）
fn user_perms_multi_on_conn(
    conn: &postgres_compat::Connection,
    user_id: i64,
) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT rp.permission_key FROM user_roles ur
         JOIN role_permissions rp ON rp.role_id = ur.role_id
         WHERE ur.user_id = ?1 ORDER BY rp.permission_key",
    )?;
    let rows = stmt.query_map([user_id], |row| row.get::<_, String>(0))?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

/// v0.4.74: 加载用户的所有角色 id
fn user_role_ids_on_conn(conn: &postgres_compat::Connection, user_id: i64) -> Result<Vec<i64>> {
    let mut stmt =
        conn.prepare("SELECT role_id FROM user_roles WHERE user_id = ?1 ORDER BY role_id")?;
    let rows = stmt.query_map([user_id], |row| row.get::<_, i64>(0))?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

fn user_role_names_on_conn(
    conn: &postgres_compat::Connection,
    user_id: i64,
) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT r.name FROM user_roles ur JOIN roles r ON r.id=ur.role_id
         WHERE ur.user_id=?1 ORDER BY r.sort_order,r.id",
    )?;
    let rows = stmt.query_map([user_id], |row| row.get::<_, String>(0))?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

fn user_role_keys_on_conn(conn: &postgres_compat::Connection, user_id: i64) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT COALESCE(NULLIF(r.system_key,''),'') FROM user_roles ur JOIN roles r ON r.id=ur.role_id
         WHERE ur.user_id=?1 AND COALESCE(r.system_key,'')<>'' ORDER BY r.sort_order,r.id",
    )?;
    let rows = stmt.query_map([user_id], |row| row.get::<_, String>(0))?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

fn user_role_template_names_on_conn(
    conn: &postgres_compat::Connection,
    user_id: i64,
) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT rt.name
           FROM roles r
           JOIN role_templates rt ON rt.id=r.template_id
          WHERE r.id IN (
                SELECT role_id FROM user_roles WHERE user_id=?1
                UNION
                SELECT role_id FROM users WHERE id=?1 AND role_id IS NOT NULL
          )
            AND rt.deleted_at IS NULL
          ORDER BY rt.name",
    )?;
    let rows = stmt.query_map([user_id], |row| row.get::<_, String>(0))?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

fn affiliation_group_names_on_conn(
    conn: &postgres_compat::Connection,
    user_id: i64,
) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT ag.name FROM user_affiliation_group_links link
         JOIN user_affiliation_groups ag ON ag.id=link.affiliation_group_id
         WHERE link.user_id=?1 ORDER BY ag.sort_order,ag.id",
    )?;
    let rows = stmt.query_map([user_id], |row| row.get::<_, String>(0))?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

/// Rebuild fixed user-only affiliations from assigned roles. Business laboratories are never used here.
fn sync_affiliation_groups_on_conn(conn: &postgres_compat::Connection, user_id: i64) -> Result<()> {
    conn.execute(
        "DELETE FROM user_affiliation_group_links WHERE user_id=?1",
        [user_id],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO user_affiliation_group_links(user_id,affiliation_group_id)
         SELECT ?1, ag.id
         FROM user_affiliation_groups ag
         WHERE ag.code IN (
            SELECT DISTINCT CASE
                WHEN EXISTS(SELECT 1 FROM role_permissions rp WHERE rp.role_id=r.id AND rp.permission_key='entry:sample') THEN 'experiment'
                WHEN EXISTS(SELECT 1 FROM role_permissions rp WHERE rp.role_id=r.id AND rp.permission_key='entry:workload') THEN 'analysis'
            END
            FROM roles r
            WHERE r.id IN (
                SELECT role_id FROM user_roles WHERE user_id=?1
                UNION
                SELECT role_id FROM users WHERE id=?1 AND role_id IS NOT NULL
            )
              AND (EXISTS(SELECT 1 FROM role_permissions rp WHERE rp.role_id=r.id AND rp.permission_key IN ('entry:sample','entry:workload')))
         )",
        [user_id],
    )?;
    Ok(())
}

/// 单条查询并回填 permissions
fn find_one<P: postgres_compat::Params>(
    conn: &postgres_compat::Connection,
    sql: &str,
    params: P,
) -> Result<Option<User>> {
    // stmt 在内部块结束即释放，避免与下方 user_perms_on_conn 的二次查询产生活动语句冲突
    let user = {
        let mut stmt = conn.prepare(sql)?;
        let mut rows = stmt.query_map(params, map_user)?;
        rows.next().transpose()?
    };
    let mut user = user;
    if let Some(ref mut u) = user {
        hydrate_user_on_conn(conn, u)?;
    }
    Ok(user)
}

/// 按 username 查找用户（含部门/实验室名称、角色、权限点）
pub fn find_by_username(pool: &DbPool, username: &str) -> Result<Option<User>> {
    let conn = pool.get()?;
    find_one(
        &conn,
        &format!("{} WHERE u.username = ?1", USER_SELECT),
        [username],
    )
}

/// 按 id 查找用户（连接版本，供事务内使用）
fn find_by_id_on_conn(conn: &postgres_compat::Connection, id: i64) -> Result<Option<User>> {
    find_one(conn, &format!("{} WHERE u.id = ?1", USER_SELECT), [id])
}

/// 按 id 查找用户
pub fn find_by_id(pool: &DbPool, id: i64) -> Result<Option<User>> {
    let conn = pool.get()?;
    find_by_id_on_conn(&conn, id)
}

/// 获取所有用户列表（含角色、权限点）
pub fn list_all(pool: &DbPool) -> Result<Vec<User>> {
    let conn = pool.get()?;
    let users = {
        let mut stmt = conn.prepare(&format!("{} ORDER BY u.id ASC", USER_SELECT))?;
        let mut rows = stmt.query_map([], map_user)?;
        let mut v: Vec<User> = vec![];
        while let Some(row) = rows.next() {
            v.push(row?);
        }
        v
    };
    let mut users = users;
    for u in &mut users {
        hydrate_user_on_conn(&conn, u)?;
    }
    Ok(users)
}

/// Active users that may be selected as the visible sender for a particular
/// laboratory. It intentionally returns only users with the RD portal
/// permission and a matching business laboratory.
pub fn list_rd_senders_by_group(pool: &DbPool, group_id: i64) -> Result<Vec<User>> {
    let conn = pool.get()?;
    let users = {
        let mut stmt = conn.prepare(&format!(
            "{} WHERE u.is_active=1 AND u.deleted_at IS NULL AND
             (u.group_id=?1 OR EXISTS(SELECT 1 FROM user_groups ug WHERE ug.user_id=u.id AND ug.group_id=?1))
             AND EXISTS(
               SELECT 1 FROM user_roles ur
               JOIN role_permissions rp ON rp.role_id=ur.role_id
               WHERE ur.user_id=u.id AND rp.permission_key IN ('entry:sample','*')
             )
             ORDER BY u.username ASC",
            USER_SELECT
        ))?;
        stmt.query_map([group_id], map_user)?
            .collect::<std::result::Result<Vec<_>, _>>()?
    };
    let mut users = users;
    for user in &mut users {
        hydrate_user_on_conn(&conn, user)?;
    }
    Ok(users)
}

/// Active, non-public users who can be selected as the actual detector by an
/// analysis public account. Analysis users are not required to be assigned to
/// a business laboratory, so the caller authorizes the laboratory separately.
pub fn list_work_detectors(pool: &DbPool) -> Result<Vec<User>> {
    let conn = pool.get()?;
    let users = {
        let mut stmt = conn.prepare(&format!(
            "{} WHERE u.is_active=1 AND u.deleted_at IS NULL AND u.is_admin=0
             AND EXISTS(
               SELECT 1 FROM user_roles ur
               JOIN role_permissions rp ON rp.role_id=ur.role_id
               WHERE ur.user_id=u.id AND rp.permission_key IN ('entry:workload','*')
             )
             AND NOT EXISTS(
               SELECT 1 FROM user_roles ur
               JOIN roles r ON r.id=ur.role_id
               WHERE ur.user_id=u.id
                 AND r.system_key IN ('public_account','rd_public_account','analysis_public_account','system_admin')
             )
             ORDER BY u.username ASC",
            USER_SELECT
        ))?;
        stmt.query_map([], map_user)?
            .collect::<std::result::Result<Vec<_>, _>>()?
    };
    let mut users = users;
    for user in &mut users {
        hydrate_user_on_conn(&conn, user)?;
    }
    Ok(users)
}

/// The configured departments of an analysis public account are its hard
/// portal boundary. The legacy scalar department is included for installations
/// that have not yet populated user_divisions.
pub fn analysis_public_account_division_ids(pool: &DbPool, user_id: i64) -> Result<Vec<i64>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT DISTINCT division_id FROM (
             SELECT division_id FROM user_divisions WHERE user_id=?1
             UNION
             SELECT division_id FROM users WHERE id=?1 AND division_id IS NOT NULL
         ) configured ORDER BY division_id",
    )?;
    let rows = stmt.query_map([user_id], |row| row.get::<_, i64>(0))?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

/// Builds the exact people cards available to an analysis public account.
/// Business laboratories/project groups are intentionally not part of this
/// personnel selection.
pub fn analysis_public_account_scope(
    pool: &DbPool,
    public_account_user_id: i64,
) -> Result<AnalysisPublicAccountScope> {
    let division_ids = analysis_public_account_division_ids(pool, public_account_user_id)?;
    if division_ids.is_empty() {
        return Ok(AnalysisPublicAccountScope { accounts: vec![] });
    }
    let conn = pool.get()?;
    let division_values = division_ids
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let mut stmt = conn.prepare(
        &format!(
            "SELECT u.id,u.username,r.name
         FROM users u
         JOIN user_roles ur ON ur.user_id=u.id
         JOIN roles r ON r.id=ur.role_id
         LEFT JOIN role_templates rt ON rt.id=r.template_id
         WHERE u.is_active=1 AND u.deleted_at IS NULL AND u.is_admin=0
           AND (u.division_id IN ({division_values}) OR EXISTS(
             SELECT 1 FROM user_divisions ud
             WHERE ud.user_id=u.id AND ud.division_id IN ({division_values})
           ))
            AND (
              r.name IN ('分析检测员','分析检测组长')
              OR rt.name IN ('分析检测员模板','分析检测组长模板')
            )
           AND EXISTS(
             SELECT 1 FROM user_roles workload_ur
             JOIN role_permissions workload_rp ON workload_rp.role_id=workload_ur.role_id
             WHERE workload_ur.user_id=u.id
               AND workload_rp.permission_key IN ('entry:workload','*')
           )
           AND NOT EXISTS(
             SELECT 1 FROM user_roles public_ur
             JOIN roles public_r ON public_r.id=public_ur.role_id
             WHERE public_ur.user_id=u.id
               AND public_r.system_key IN ('public_account','rd_public_account','analysis_public_account','system_admin')
           )
         ORDER BY u.username,r.sort_order,r.id"
        ),
    )?;
    let mut accounts: Vec<AnalysisPublicAccountCandidate> = vec![];
    for row in stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })? {
        let (user_id, username, role_name) = row?;
        if let Some(account) = accounts.iter_mut().find(|account| account.id == user_id) {
            if !account.role_names.contains(&role_name) {
                account.role_names.push(role_name);
            }
        } else {
            accounts.push(AnalysisPublicAccountCandidate {
                id: user_id,
                username,
                role_names: vec![role_name],
            });
        }
    }
    Ok(AnalysisPublicAccountScope { accounts })
}

pub fn analysis_public_account_candidate_allowed(
    pool: &DbPool,
    public_account_user_id: i64,
    candidate_user_id: i64,
) -> Result<bool> {
    let scope = analysis_public_account_scope(pool, public_account_user_id)?;
    Ok(scope
        .accounts
        .iter()
        .any(|account| account.id == candidate_user_id))
}

/// 查询某用户所属角色的权限点（role_id 为 NULL 或角色不存在返回空 vec）
pub fn get_user_permissions(pool: &DbPool, user_id: i64) -> Result<Vec<String>> {
    let conn = pool.get()?;
    let role_ids = user_role_ids_on_conn(&conn, user_id).unwrap_or_default();
    if !role_ids.is_empty() {
        return user_perms_multi_on_conn(&conn, user_id);
    }
    let role_id: Option<i64> =
        match conn.query_row("SELECT role_id FROM users WHERE id=?1", [user_id], |row| {
            row.get::<_, Option<i64>>(0)
        }) {
            Ok(v) => v,
            Err(postgres_compat::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(e.into()),
        };
    user_perms_on_conn(&conn, role_id)
}

/// 创建用户
pub fn create(
    pool: &DbPool,
    data: &UserCreate,
    password_hash: &str,
    actor: Option<(i64, &str)>,
) -> Result<User> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;

    // 唯一性校验
    let exists: i64 = tx.query_row(
        "SELECT COUNT(*) FROM users WHERE username = ?1",
        [&data.username],
        |r| r.get(0),
    )?;
    if exists > 0 {
        return Err(AppError::Conflict(format!(
            "用户名「{}」已存在",
            data.username
        )));
    }

    let role_id = data.role_id.or_else(|| data.role_ids.first().copied());
    let mut division_ids = if !data.business_division_ids.is_empty() {
        data.business_division_ids.clone()
    } else {
        data.division_ids.clone()
    };
    if division_ids.is_empty() {
        if let Some(division_id) = data.division_id {
            division_ids.push(division_id);
        }
    }
    division_ids.sort_unstable();
    division_ids.dedup();
    let mut group_ids = data.group_ids.clone();
    if group_ids.is_empty() {
        if let Some(group_id) = data.group_id {
            group_ids.push(group_id);
        }
    }
    group_ids.sort_unstable();
    group_ids.dedup();
    let primary_division_id = data
        .primary_division_id
        .or(data.division_id)
        .or_else(|| division_ids.first().copied());
    if let Some(primary) = primary_division_id {
        if !division_ids.contains(&primary) {
            return Err(AppError::Validation(
                "主归属部门必须包含在可承担业务部门中".into(),
            ));
        }
    }
    let primary_group_id = group_ids.first().copied();
    tx.execute(
        "INSERT INTO users (username, password, division_id, group_id, role_id) VALUES (?1, ?2, ?3, ?4, ?5)",
        postgres_compat::params![data.username, password_hash, primary_division_id, primary_group_id, role_id],
    )?;

    let id = tx.last_insert_rowid();
    // v0.4.74: 多角色
    let mut assigned = data.role_ids.clone();
    if let Some(rid) = role_id {
        assigned.push(rid);
    }
    assigned.sort_unstable();
    assigned.dedup();
    for rid in assigned {
        tx.execute(
            "INSERT OR IGNORE INTO user_roles (user_id, role_id) VALUES (?1, ?2)",
            postgres_compat::params![id, rid],
        )
        .ok();
    }
    tx.execute(
        "UPDATE users SET is_admin=CASE WHEN EXISTS(
            SELECT 1 FROM user_roles ur JOIN roles r ON r.id=ur.role_id
            WHERE ur.user_id=?1 AND r.system_key='system_admin'
         ) THEN 1 ELSE 0 END WHERE id=?1",
        [id],
    )?;
    replace_user_affiliations_on_conn(&tx, id, &division_ids, primary_division_id, &group_ids)?;
    sync_affiliation_groups_on_conn(&tx, id)?;
    let detail = format!("注册用户#{}：「{}」", id, data.username);
    let created =
        find_by_id_on_conn(&tx, id)?.ok_or_else(|| AppError::Internal("创建用户失败".into()))?;
    let after = serde_json::to_value(&created).unwrap_or(serde_json::Value::Null);
    if let Some((actor_id, actor_name)) = actor {
        audit_repo::log_structured_actor_on_conn(
            &tx,
            "create",
            "users",
            Some(id),
            actor_id,
            actor_name,
            &detail,
            "shared",
            "",
            None,
            Some(&after),
            "management",
        )?;
    } else {
        audit_repo::log_structured_on_conn(
            &tx,
            "create",
            "users",
            Some(id),
            &data.username,
            &detail,
            "shared",
            "",
            None,
            Some(&after),
            "self_registration",
        )?;
    }
    tx.commit()?;
    Ok(created)
}

/// 更新用户
pub fn update(
    pool: &DbPool,
    id: i64,
    data: &UserUpdate,
    actor_id: i64,
    user_name: &str,
) -> Result<User> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;

    let existing =
        find_by_id_on_conn(&tx, id)?.ok_or_else(|| AppError::NotFound("用户不存在".into()))?;
    let password_changed = data
        .password
        .as_ref()
        .map(|value| !value.is_empty())
        .unwrap_or(false);
    let roles_changed = data.role_id.is_some() || data.role_ids.is_some();
    let affiliations_changed = data.business_division_ids.is_some()
        || data.primary_division_id.is_some()
        || data.division_ids.is_some()
        || data.group_ids.is_some()
        || data.division_id.is_some()
        || data.group_id.is_some();
    let mut before = serde_json::to_value(&existing).unwrap_or(serde_json::Value::Null);
    if password_changed {
        if let Some(object) = before.as_object_mut() {
            object.insert("password_changed".into(), serde_json::Value::Bool(false));
        }
    }

    let mut changes: Vec<String> = vec![];

    if let Some(ref un) = data.username {
        if !un.is_empty() && un != &existing.username {
            // 唯一性校验
            let conflict: i64 = tx.query_row(
                "SELECT COUNT(*) FROM users WHERE username = ?1 AND id <> ?2",
                postgres_compat::params![un, id],
                |r| r.get(0),
            )?;
            if conflict > 0 {
                return Err(AppError::Conflict(format!("用户名「{}」已存在", un)));
            }
            tx.execute("UPDATE users SET username = ?1, updated_at = to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS') WHERE id = ?2", postgres_compat::params![un, id])?;
            changes.push(format!("用户名 {} → {}", existing.username, un));
        }
    }

    if let Some(ref pw) = data.password {
        if !pw.is_empty() {
            let hash = bcrypt::hash(pw, bcrypt::DEFAULT_COST)
                .map_err(|e| AppError::Internal(format!("密码哈希失败: {}", e)))?;
            tx.execute("UPDATE users SET password = ?1, updated_at = to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS') WHERE id = ?2", postgres_compat::params![hash, id])?;
            changes.push("密码已更新".into());
        }
    }

    if let Some(did) = data.primary_division_id.or(data.division_id) {
        let val = did.map(|v| v as i64);
        tx.execute("UPDATE users SET division_id = ?1, updated_at = to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS') WHERE id = ?2", postgres_compat::params![val, id])?;
        changes.push("所属部门已更新".into());
    }

    if let Some(gid) = data.group_id {
        let val = gid.map(|v| v as i64);
        tx.execute("UPDATE users SET group_id = ?1, updated_at = to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS') WHERE id = ?2", postgres_compat::params![val, id])?;
        changes.push("实验室已更新".into());
    }

    if affiliations_changed {
        let mut division_ids = data
            .business_division_ids
            .clone()
            .or_else(|| data.division_ids.clone())
            .or_else(|| {
                data.primary_division_id
                    .or(data.division_id)
                    .map(|value| value.into_iter().collect())
            })
            .unwrap_or_else(|| existing.division_ids.clone());
        let primary_division_id = data
            .primary_division_id
            .or(data.division_id)
            .flatten()
            .or(existing.primary_division_id)
            .or_else(|| division_ids.first().copied());
        if let Some(primary) = primary_division_id {
            if !division_ids.contains(&primary) {
                return Err(AppError::Validation(
                    "主归属部门必须包含在可承担业务部门中".into(),
                ));
            }
        }
        let mut group_ids = data
            .group_ids
            .clone()
            .or_else(|| data.group_id.map(|value| value.into_iter().collect()))
            .unwrap_or_else(|| existing.group_ids.clone());
        division_ids.sort_unstable();
        division_ids.dedup();
        group_ids.sort_unstable();
        group_ids.dedup();
        let primary_group_id = group_ids.first().copied();
        tx.execute(
            "UPDATE users SET division_id=?1,group_id=?2,updated_at=to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS') WHERE id=?3",
            postgres_compat::params![primary_division_id, primary_group_id, id],
        )?;
        replace_user_affiliations_on_conn(&tx, id, &division_ids, primary_division_id, &group_ids)?;
        changes.push("所属部门和实验室已更新".into());
    }

    if let Some(rid) = data.role_id {
        tx.execute(
            "UPDATE users SET role_id = ?1, updated_at = to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS') WHERE id = ?2",
            postgres_compat::params![rid, id],
        )?;
        changes.push("角色已更新".into());
    }

    // v0.4.74: 多角色
    if let Some(ref role_ids) = data.role_ids {
        let role_id = role_ids.first().copied();
        tx.execute("DELETE FROM user_roles WHERE user_id = ?1", [id])?;
        for rid in role_ids {
            tx.execute(
                "INSERT OR IGNORE INTO user_roles (user_id, role_id) VALUES (?1, ?2)",
                postgres_compat::params![id, rid],
            )?;
        }
        tx.execute(
            "UPDATE users SET role_id=?1,updated_at=to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS') WHERE id=?2",
            postgres_compat::params![role_id, id],
        )?;
        changes.push("角色已更新".into());
    }

    tx.execute(
        "UPDATE users SET is_admin=CASE WHEN EXISTS(
            SELECT 1 FROM user_roles ur JOIN roles r ON r.id=ur.role_id
            WHERE ur.user_id=?1 AND r.system_key='system_admin'
         ) THEN 1 ELSE 0 END WHERE id=?1",
        [id],
    )?;
    sync_affiliation_groups_on_conn(&tx, id)?;

    if let Some(active) = data.is_active {
        if active != existing.is_active {
            tx.execute("UPDATE users SET is_active = ?1, updated_at = to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS') WHERE id = ?2", postgres_compat::params![active as i64, id])?;
            changes.push(format!("启用状态 {} → {}", existing.is_active, active));
        }
    }

    if roles_changed || affiliations_changed || data.is_active.is_some() {
        tx.execute("DELETE FROM user_sessions WHERE user_id=?1", [id])?;
    }

    if changes.is_empty() {
        drop(tx);
    } else {
        let detail = format!(
            "修改用户#{}「{}」：{}",
            id,
            existing.username,
            changes.join("，")
        );
        let updated = find_by_id_on_conn(&tx, id)?
            .ok_or_else(|| AppError::Internal("更新用户失败".into()))?;
        let mut after = serde_json::to_value(&updated).unwrap_or(serde_json::Value::Null);
        if password_changed {
            if let Some(object) = after.as_object_mut() {
                object.insert("password_changed".into(), serde_json::Value::Bool(true));
            }
        }
        audit_repo::log_structured_actor_on_conn(
            &tx,
            "update",
            "users",
            Some(id),
            actor_id,
            user_name,
            &detail,
            "shared",
            "",
            Some(&before),
            Some(&after),
            "management",
        )?;
        tx.commit()?;
    }

    find_by_id_on_conn(&conn, id)?.ok_or_else(|| AppError::Internal("更新用户失败".into()))
}

/// 软删除用户（is_active=0）
pub fn soft_delete(pool: &DbPool, id: i64, user_name: &str, reason: &str) -> Result<()> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;

    let existing =
        find_by_id_on_conn(&tx, id)?.ok_or_else(|| AppError::NotFound("用户不存在".into()))?;

    tx.execute(
        "UPDATE users SET is_active=0,deleted_at=to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS'),updated_at=to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS') WHERE id=?1 AND deleted_at IS NULL",
        [id],
    )?;

    let detail = format!("删除用户#{}：「{}」", id, existing.username);
    let before = serde_json::to_value(&existing).unwrap_or(serde_json::Value::Null);
    trash_repo::move_to_trash_on_conn(
        &tx,
        "用户",
        "users",
        id,
        "access",
        "shared",
        &format!("{}（用户ID {}）", existing.username, existing.id),
        "",
        &before,
        reason,
        user_name,
        Some(existing.id),
        existing.group_id,
        "保留用户ID和历史审计关联",
        true,
    )?;
    let after = serde_json::json!({"is_active":false,"deleted_at":"now","user_id":id});
    audit_repo::log_structured_on_conn(
        &tx,
        "delete",
        "users",
        Some(id),
        user_name,
        &detail,
        "shared",
        "",
        Some(&before),
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(())
}
