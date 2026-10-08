use axum::http::HeaderMap;

use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::role::RoleDataScope;
use crate::models::user::User;
use crate::repo::{organization_repo, role_repo, user_repo};
use crate::service::auth_service;
use postgres_compat::OptionalExtension;

pub const ROLE_SYSTEM_ADMIN: &str = "系统管理员";
pub const ROLE_ANALYST: &str = "分析检测员";
pub const ROLE_ANALYSIS_LEADER: &str = "分析检测组长";
pub const ROLE_RD_SENDER: &str = "研发送样员";
pub const ROLE_RD_LEADER: &str = "研发送样组长";
pub const ROLE_KEY_SYSTEM_ADMIN: &str = "system_admin";
pub const ROLE_KEY_LEGACY_PUBLIC_ACCOUNT: &str = "public_account";
pub const ROLE_KEY_RD_PUBLIC_ACCOUNT: &str = "rd_public_account";
pub const ROLE_KEY_ANALYSIS_PUBLIC_ACCOUNT: &str = "analysis_public_account";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordScope {
    Own,
    /// A public analysis account can review records it submitted, while the
    /// selected detector remains the business owner of each record.
    PublicCreated,
    AnalysisAll,
    Lab(i64),
    Global,
}

#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user: User,
    pub role_names: Vec<String>,
}

impl AuthContext {
    pub fn has_permission(&self, permission: &str) -> bool {
        self.user.is_admin
            || self
                .user
                .permissions
                .iter()
                .any(|value| value == "*" || value == permission)
    }

    pub fn is_system_admin(&self) -> bool {
        self.user.is_admin
            || self
                .user
                .role_keys
                .iter()
                .any(|key| key == ROLE_KEY_SYSTEM_ADMIN)
    }

    pub fn is_analysis_member(&self) -> bool {
        self.has_permission("entry:workload")
    }

    pub fn is_analysis_leader(&self) -> bool {
        self.role_names
            .iter()
            .any(|name| name == ROLE_ANALYSIS_LEADER)
            || self
                .user
                .role_template_names
                .iter()
                .any(|name| name == "分析检测组长模板")
    }

    pub fn is_rd_sender(&self) -> bool {
        self.has_permission("entry:sample")
    }

    pub fn is_rd_leader(&self) -> bool {
        self.role_names.iter().any(|name| name == ROLE_RD_LEADER)
            || self
                .user
                .role_template_names
                .iter()
                .any(|name| name == "研发送样组长模板")
    }

    pub fn is_rd_public_account(&self) -> bool {
        self.user
            .role_keys
            .iter()
            .any(|key| key == ROLE_KEY_RD_PUBLIC_ACCOUNT || key == ROLE_KEY_LEGACY_PUBLIC_ACCOUNT)
    }

    pub fn is_analysis_public_account(&self) -> bool {
        self.user
            .role_keys
            .iter()
            .any(|key| key == ROLE_KEY_ANALYSIS_PUBLIC_ACCOUNT)
    }

    /// Kept for older callers that meant the RD public-account workflow.
    pub fn is_public_account(&self) -> bool {
        self.is_rd_public_account()
    }

    pub fn workload_scope(&self) -> RecordScope {
        if self.is_system_admin() {
            RecordScope::Global
        } else if self.has_permission("records:work:view-all")
            || self.has_permission("stats:workload:view-all")
        {
            RecordScope::Global
        } else if self.has_permission("records:work:view-scope") {
            RecordScope::AnalysisAll
        } else if self.is_analysis_public_account() {
            RecordScope::PublicCreated
        } else {
            RecordScope::Own
        }
    }

    pub fn can_view_workload_scope(&self) -> bool {
        self.is_system_admin()
            || self.has_permission("records:work:view-scope")
            || self.has_permission("records:work:view-all")
            || self.has_permission("stats:workload:view-all")
    }

    pub fn rd_scope(&self) -> Result<RecordScope> {
        if self.is_system_admin() || self.has_permission("records:rd:view-all") {
            return Ok(RecordScope::Global);
        }
        if self.has_permission("records:rd:view-lab") {
            return self
                .user
                .group_id
                .map(RecordScope::Lab)
                .ok_or_else(|| AppError::Validation("研发送样组长尚未设置所属实验室".into()));
        }
        Ok(RecordScope::Own)
    }
}

fn bearer_token(headers: &HeaderMap) -> Result<&str> {
    headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or_else(|| AppError::Forbidden("未提供登录凭证".into()))
}

pub fn authenticate(pool: &DbPool, headers: &HeaderMap) -> Result<AuthContext> {
    let claims = auth_service::verify_active_token(pool, bearer_token(headers)?)?;
    let user = user_repo::find_by_id(pool, claims.sub)?
        .ok_or_else(|| AppError::Forbidden("用户不存在，请重新登录".into()))?;
    if !user.is_active {
        return Err(AppError::Forbidden("该账号已停用".into()));
    }
    if user.username != claims.username {
        return Err(AppError::Forbidden("用户名已变更，请重新登录".into()));
    }
    let role_names = if user.is_admin {
        vec![ROLE_SYSTEM_ADMIN.to_string()]
    } else {
        user.role_names.clone()
    };
    Ok(AuthContext { user, role_names })
}

pub fn require_permission(ctx: &AuthContext, permission: &str) -> Result<()> {
    if ctx.has_permission(permission) {
        Ok(())
    } else {
        Err(AppError::Forbidden(format!("缺少权限: {permission}")))
    }
}

/// Loads configured data scopes per role. Callers must not flatten unrelated roles into a
/// department/type cross product: each role scope represents one independent allowance.
pub fn role_data_scopes(pool: &DbPool, ctx: &AuthContext) -> Result<Vec<RoleDataScope>> {
    if ctx.is_system_admin() {
        return Ok(vec![]);
    }
    role_repo::data_scopes_for_roles(pool, &ctx.user.role_ids)
}

/// The RD portal is authorized only by formal role data-source scopes. Legacy
/// department-role assignments remain readable for governance and migration,
/// but never grant business data access.
pub fn rd_allowed_division_ids(pool: &DbPool, ctx: &AuthContext) -> Result<Option<Vec<i64>>> {
    if ctx.is_system_admin() {
        return Ok(None);
    }
    let mut ids = role_data_scopes(pool, ctx)?
        .into_iter()
        .flat_map(|scope| scope.division_ids)
        .collect::<Vec<_>>();
    ids.sort_unstable();
    ids.dedup();
    let has_legacy_department_role =
        !organization_repo::assigned_department_role_division_ids(pool, ctx.user.id, "rd")?
            .is_empty();
    if !ids.is_empty() {
        return Ok(Some(ids));
    }
    if has_legacy_department_role {
        return Ok(Some(Vec::new()));
    }
    Ok(None)
}

/// Returns the formal role range for the analysis/detection portal. A legacy
/// department-role assignment is deliberately converted to an empty range so
/// it cannot fall through to the old unrestricted compatibility behavior.
pub fn work_allowed_division_ids(pool: &DbPool, ctx: &AuthContext) -> Result<Option<Vec<i64>>> {
    if ctx.is_system_admin() {
        return Ok(None);
    }
    if ctx.is_analysis_public_account() {
        let mut ids = user_repo::analysis_public_account_division_ids(pool, ctx.user.id)?;
        ids.sort_unstable();
        ids.dedup();
        return Ok(Some(ids));
    }
    let mut ids = role_repo::work_division_scope_ids_for_roles(pool, &ctx.user.role_ids)?;
    ids.sort_unstable();
    ids.dedup();
    if !ids.is_empty() {
        return Ok(Some(ids));
    }
    let has_legacy_department_role =
        !organization_repo::assigned_department_role_division_ids(pool, ctx.user.id, "work")?
            .is_empty();
    if has_legacy_department_role
        || ctx
            .user
            .role_template_names
            .iter()
            .any(|name| name == "分析检测员模板" || name == "分析检测组长模板")
    {
        return Ok(Some(Vec::new()));
    }
    // Analysis/detection data ranges are role-driven. A non-admin role without
    // an explicit range must fail closed instead of inheriting unrestricted access.
    Ok(Some(Vec::new()))
}

pub fn work_record_authorization_division_sql(alias: &str) -> String {
    format!(
        "COALESCE({alias}.detection_division_id,{alias}.execution_division_id,{alias}.division_id,(SELECT division_id FROM project_groups WHERE id={alias}.group_id))"
    )
}

pub fn work_record_sending_division_sql(alias: &str) -> String {
    format!(
        "COALESCE({alias}.sending_division_id,{alias}.execution_division_id,{alias}.division_id,(SELECT division_id FROM project_groups WHERE id={alias}.group_id))"
    )
}

pub fn work_division_allowed(
    pool: &DbPool,
    ctx: &AuthContext,
    division_id: Option<i64>,
) -> Result<bool> {
    Ok(match work_allowed_division_ids(pool, ctx)? {
        None => true,
        Some(ids) => division_id.is_some_and(|id| ids.contains(&id)),
    })
}

pub fn work_group_allowed(pool: &DbPool, ctx: &AuthContext, group_id: i64) -> Result<bool> {
    let group = crate::repo::group_repo::get_by_id(pool, group_id)?;
    work_division_allowed(pool, ctx, group.division_id)
}

pub fn work_allowed_group_ids(pool: &DbPool, ctx: &AuthContext) -> Result<Option<Vec<i64>>> {
    let Some(division_ids) = work_allowed_division_ids(pool, ctx)? else {
        return Ok(None);
    };
    if division_ids.is_empty() {
        return Ok(Some(Vec::new()));
    }
    let conn = pool.get()?;
    let ids = division_ids
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let mut stmt = conn.prepare(&format!(
        "SELECT id FROM project_groups WHERE deleted_at IS NULL AND division_id IN ({ids})"
    ))?;
    Ok(Some(
        stmt.query_map([], |row| row.get::<_, i64>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?,
    ))
}

pub fn work_method_allowed(pool: &DbPool, ctx: &AuthContext, method_id: i64) -> Result<bool> {
    let Some(division_ids) = work_allowed_division_ids(pool, ctx)? else {
        return Ok(true);
    };
    if division_ids.is_empty() {
        return Ok(false);
    }
    let conn = pool.get()?;
    let ids = division_ids
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let count: i64 = conn.query_row(
        &format!(
            "SELECT COUNT(*) FROM project_method_links pml
             JOIN projects p ON p.id=pml.project_id
             JOIN project_lab_links pll ON pll.project_id=p.id
             JOIN project_groups pg ON pg.id=pll.group_id
             LEFT JOIN common_method_division_scopes cmds ON cmds.method_id=pml.method_id AND cmds.division_id=pg.division_id
             WHERE pml.method_id=?1 AND p.deleted_at IS NULL AND pg.deleted_at IS NULL
               AND pg.division_id IN ({ids})
               AND (NOT COALESCE((SELECT is_common FROM methods WHERE id=pml.method_id),false)
                    OR NOT EXISTS(SELECT 1 FROM common_method_division_scopes scoped WHERE scoped.method_id=pml.method_id)
                    OR cmds.division_id IS NOT NULL)"
        ),
        [method_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

pub fn work_method_allowed_for_group(
    pool: &DbPool,
    ctx: &AuthContext,
    method_id: i64,
    group_id: i64,
) -> Result<bool> {
    if !work_group_allowed(pool, ctx, group_id)? {
        return Ok(false);
    }
    let conn = pool.get()?;
    let count: i64 = conn.query_row(
        "SELECT COUNT(*)
         FROM project_method_links pml
         JOIN project_lab_links pll ON pll.project_id=pml.project_id
         JOIN project_groups pg ON pg.id=pll.group_id
         JOIN methods m ON m.id=pml.method_id
         LEFT JOIN common_method_division_scopes cmds ON cmds.method_id=m.id AND cmds.division_id=pg.division_id
         WHERE pml.method_id=?1 AND pll.group_id=?2 AND pg.deleted_at IS NULL
           AND (m.is_common=0 OR NOT EXISTS(SELECT 1 FROM common_method_division_scopes scoped WHERE scoped.method_id=m.id) OR cmds.division_id IS NOT NULL)",
        postgres_compat::params![method_id, group_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

pub fn work_method_allowed_for_division(
    pool: &DbPool,
    ctx: &AuthContext,
    method_id: i64,
    division_id: Option<i64>,
) -> Result<bool> {
    let Some(division_id) = division_id else {
        return Ok(false);
    };
    if !work_division_allowed(pool, ctx, Some(division_id))? {
        return Ok(false);
    }
    let conn = pool.get()?;
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM methods m
         LEFT JOIN common_method_division_scopes cmds ON cmds.method_id=m.id AND cmds.division_id=?2
         WHERE m.id=?1 AND m.deleted_at IS NULL AND m.is_active=1
           AND (m.is_common=0 OR NOT EXISTS(SELECT 1 FROM common_method_division_scopes scoped WHERE scoped.method_id=m.id) OR cmds.division_id IS NOT NULL)",
        postgres_compat::params![method_id, division_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

pub fn work_record_allowed(pool: &DbPool, ctx: &AuthContext, record_id: i64) -> Result<bool> {
    let conn = pool.get()?;
    let division_id: Option<i64> = conn.query_row(
        "SELECT COALESCE(wr.detection_division_id, wr.execution_division_id, wr.division_id, pg.division_id) FROM work_records wr LEFT JOIN project_groups pg ON pg.id=wr.group_id WHERE wr.id=?1",
        [record_id],
        |row| row.get(0),
    )?;
    work_division_allowed(pool, ctx, division_id)
}

pub fn rd_division_allowed(
    pool: &DbPool,
    ctx: &AuthContext,
    division_id: Option<i64>,
) -> Result<bool> {
    Ok(match rd_allowed_division_ids(pool, ctx)? {
        None => true,
        Some(ids) => division_id.is_some_and(|id| ids.contains(&id)),
    })
}

pub fn rd_group_allowed(pool: &DbPool, ctx: &AuthContext, group_id: i64) -> Result<bool> {
    let group = crate::repo::group_repo::get_by_id(pool, group_id)?;
    rd_division_allowed(pool, ctx, group.division_id)
}

/// A department can submit work for a project only when it owns the project or
/// has been explicitly configured as a collaboration department.  The selected
/// execution laboratory is validated separately; project collaboration never
/// grants access to an unrelated laboratory.
pub fn rd_project_party_allowed(pool: &DbPool, ctx: &AuthContext, project_id: i64) -> Result<bool> {
    if ctx.is_system_admin() {
        return Ok(true);
    }
    let Some(division_ids) = rd_allowed_division_ids(pool, ctx)? else {
        // Legacy roles retain their existing behavior until they are migrated
        // to an explicit department role.
        return Ok(true);
    };
    if division_ids.is_empty() {
        return Ok(false);
    }
    let joined = division_ids
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let conn = pool.get()?;
    conn.query_row(
        &format!(
            "SELECT EXISTS(
                SELECT 1 FROM projects p
                 WHERE p.id=?1 AND p.is_active=1 AND p.deleted_at IS NULL
                   AND (
                     COALESCE(p.project_division_id,(SELECT division_id FROM project_groups WHERE id=p.group_id)) IN ({joined})
                     OR EXISTS(SELECT 1 FROM project_collaboration_divisions pcd WHERE pcd.project_id=p.id AND pcd.division_id IN ({joined}))
                   )
            )"
        ),
        [project_id],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

/// Whether a department is the owning department of a project or has been
/// explicitly configured as a collaboration department. This checks business
/// configuration only; callers still apply their own role data-scope checks.
pub fn project_allows_division(
    pool: &DbPool,
    project_id: i64,
    division_id: Option<i64>,
) -> Result<bool> {
    let Some(division_id) = division_id else {
        return Ok(false);
    };
    let conn = pool.get()?;
    conn.query_row(
        "SELECT EXISTS(
            SELECT 1
              FROM projects p
             WHERE p.id=?1
               AND p.is_active=1
               AND p.deleted_at IS NULL
               AND COALESCE(p.project_status,'ongoing')='ongoing'
               AND (
                    COALESCE(p.project_division_id,(SELECT division_id FROM project_groups WHERE id=p.group_id))=?2
                    OR EXISTS(
                        SELECT 1 FROM project_collaboration_divisions pcd
                         WHERE pcd.project_id=p.id AND pcd.division_id=?2
                    )
               )
        )",
        postgres_compat::params![project_id, division_id],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

/// Validates the execution target for a project business record and returns
/// the laboratory's real execution department. A project/laboratory link does
/// not itself authorize cross-department execution.
pub fn require_project_execution_group(
    pool: &DbPool,
    project_id: i64,
    group_id: i64,
) -> Result<i64> {
    let conn = pool.get()?;
    let division_id: Option<i64> = conn
        .query_row(
            "SELECT division_id FROM project_groups WHERE id=?1 AND deleted_at IS NULL",
            [group_id],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    let Some(division_id) = division_id else {
        let group_exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM project_groups WHERE id=?1 AND deleted_at IS NULL)",
            [group_id],
            |row| row.get(0),
        )?;
        return Err(AppError::Validation(
            if group_exists {
                "所选实验室尚未配置执行部门"
            } else {
                "所选实验室不存在或已停用"
            }
            .into(),
        ));
    };
    let linked: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM project_lab_links WHERE project_id=?1 AND group_id=?2)",
        postgres_compat::params![project_id, group_id],
        |row| row.get(0),
    )?;
    if !linked {
        return Err(AppError::Validation("所选实验室未关联当前项目".into()));
    }
    if !project_allows_division(pool, project_id, Some(division_id))? {
        return Err(AppError::Validation(
            "所选实验室所属部门不是该项目的归属部门或协作部门".into(),
        ));
    }
    Ok(division_id)
}

/// Returns `None` for users with no formal scope and no legacy department-role
/// assignment. A configured role is evaluated as one scope unit so multi-role
/// users cannot accidentally gain a cross-product of another role's department
/// and type permissions. Legacy department roles are intentionally non-authorizing.
pub fn sample_info_scope_allows(
    pool: &DbPool,
    ctx: &AuthContext,
    division_id: Option<i64>,
    type_key: &str,
    created_by_user_id: Option<i64>,
) -> Result<Option<bool>> {
    if ctx.is_system_admin() {
        return Ok(Some(true));
    }
    let scopes: Vec<_> = role_data_scopes(pool, ctx)?
        .into_iter()
        .filter(|scope| !scope.division_ids.is_empty() || !scope.sample_info_type_keys.is_empty())
        .collect();
    let has_legacy_department_role = !organization_repo::assigned_department_role_division_ids(
        pool,
        ctx.user.id,
        "sample_info",
    )?
    .is_empty();
    if scopes.is_empty() && !has_legacy_department_role {
        return Ok(None);
    }
    if created_by_user_id == Some(ctx.user.id) {
        return Ok(Some(true));
    }
    let allowed_by_role_scope = scopes.into_iter().any(|scope| {
        let division_matches = scope.division_ids.is_empty()
            || division_id.is_some_and(|id| scope.division_ids.contains(&id));
        let type_matches = scope.sample_info_type_keys.is_empty()
            || scope
                .sample_info_type_keys
                .iter()
                .any(|key| key == type_key);
        division_matches && type_matches
    });
    Ok(Some(allowed_by_role_scope))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique(prefix: &str) -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        format!("{prefix}_{nanos}")
    }

    #[test]
    fn project_execution_group_requires_owner_or_collaboration_department() {
        let pool = crate::db::init_pool("postgres-test");
        crate::db::test_migrations::run(&pool.get().expect("connection")).expect("migrations");
        let conn = pool.get().expect("connection");
        let suffix = unique("project_collaboration");

        conn.execute(
            "INSERT INTO divisions(name,sort_order,color,is_active) VALUES(?1,1,'#1976d2',1)",
            postgres_compat::params![format!("owner_{suffix}")],
        )
        .expect("owner division");
        let owner_division_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO divisions(name,sort_order,color,is_active) VALUES(?1,2,'#1976d2',1)",
            postgres_compat::params![format!("collaborator_{suffix}")],
        )
        .expect("collaboration division");
        let collaboration_division_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO divisions(name,sort_order,color,is_active) VALUES(?1,3,'#1976d2',1)",
            postgres_compat::params![format!("unrelated_{suffix}")],
        )
        .expect("unrelated division");
        let unrelated_division_id = conn.last_insert_rowid();

        for (name, division_id) in [
            (format!("owner_lab_{suffix}"), owner_division_id),
            (
                format!("collaboration_lab_{suffix}"),
                collaboration_division_id,
            ),
            (format!("unrelated_lab_{suffix}"), unrelated_division_id),
        ] {
            conn.execute(
                "INSERT INTO project_groups(name,division_id) VALUES(?1,?2)",
                postgres_compat::params![name, division_id],
            )
            .expect("laboratory");
        }
        let owner_group_id: i64 = conn
            .query_row(
                "SELECT id FROM project_groups WHERE name=?1",
                postgres_compat::params![format!("owner_lab_{suffix}")],
                |row| row.get(0),
            )
            .expect("owner laboratory");
        let collaboration_group_id: i64 = conn
            .query_row(
                "SELECT id FROM project_groups WHERE name=?1",
                postgres_compat::params![format!("collaboration_lab_{suffix}")],
                |row| row.get(0),
            )
            .expect("collaboration laboratory");
        let unrelated_group_id: i64 = conn
            .query_row(
                "SELECT id FROM project_groups WHERE name=?1",
                postgres_compat::params![format!("unrelated_lab_{suffix}")],
                |row| row.get(0),
            )
            .expect("unrelated laboratory");
        conn.execute(
            "INSERT INTO projects(group_id,name,project_division_id) VALUES(?1,?2,?3)",
            postgres_compat::params![
                owner_group_id,
                format!("project_{suffix}"),
                owner_division_id
            ],
        )
        .expect("project");
        let project_id = conn.last_insert_rowid();
        for group_id in [owner_group_id, collaboration_group_id, unrelated_group_id] {
            conn.execute(
                "INSERT INTO project_lab_links(project_id,group_id) VALUES(?1,?2)",
                postgres_compat::params![project_id, group_id],
            )
            .expect("project laboratory link");
        }
        conn.execute(
            "INSERT INTO project_collaboration_divisions(project_id,division_id) VALUES(?1,?2)",
            postgres_compat::params![project_id, collaboration_division_id],
        )
        .expect("collaboration division link");
        drop(conn);

        assert!(project_allows_division(&pool, project_id, Some(owner_division_id)).unwrap());
        assert!(
            project_allows_division(&pool, project_id, Some(collaboration_division_id)).unwrap()
        );
        assert!(!project_allows_division(&pool, project_id, Some(unrelated_division_id)).unwrap());
        assert_eq!(
            require_project_execution_group(&pool, project_id, owner_group_id).unwrap(),
            owner_division_id
        );
        assert_eq!(
            require_project_execution_group(&pool, project_id, collaboration_group_id).unwrap(),
            collaboration_division_id
        );
        assert!(require_project_execution_group(&pool, project_id, unrelated_group_id).is_err());
    }

    #[test]
    fn legacy_department_roles_do_not_grant_rd_or_sample_info_data_access() {
        let pool = crate::db::init_pool("postgres-test");
        crate::db::test_migrations::run(&pool.get().expect("connection")).expect("migrations");
        let conn = pool.get().expect("connection");
        let suffix = uuid::Uuid::new_v4().simple().to_string();

        conn.execute(
            "INSERT INTO divisions(name,sort_order,color,is_active) VALUES(?1,1,'#1976d2',1)",
            [format!("legacy_scope_{suffix}")],
        )
        .expect("division");
        let division_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO roles(name,description,is_system,sort_order) VALUES(?1,'',0,99)",
            [format!("legacy_role_{suffix}")],
        )
        .expect("role");
        let role_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO role_permissions(role_id,permission_key) VALUES(?1,'entry:sample')",
            [role_id],
        )
        .expect("role permission");
        conn.execute(
            "INSERT INTO role_permissions(role_id,permission_key) VALUES(?1,'entry:sample-info')",
            [role_id],
        )
        .expect("sample info permission");
        conn.execute(
            "INSERT INTO users(username,password,is_admin,is_active,division_id) VALUES(?1,'hash',0,1,?2)",
            postgres_compat::params![format!("legacy_scope_user_{suffix}"), division_id],
        )
        .expect("user");
        let user_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO user_roles(user_id,role_id) VALUES(?1,?2)",
            postgres_compat::params![user_id, role_id],
        )
        .expect("user role");
        conn.execute(
            "INSERT INTO department_roles(role_id,division_id,business_domain) VALUES(?1,?2,'all')",
            postgres_compat::params![role_id, division_id],
        )
        .expect("legacy department role");
        let department_role_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO user_department_roles(user_id,department_role_id) VALUES(?1,?2)",
            postgres_compat::params![user_id, department_role_id],
        )
        .expect("legacy department role assignment");

        let user = crate::repo::user_repo::find_by_id(&pool, user_id)
            .expect("load user")
            .expect("user exists");
        let ctx = AuthContext {
            role_names: user.role_names.clone(),
            user,
        };

        assert_eq!(
            rd_allowed_division_ids(&pool, &ctx).expect("rd scope"),
            Some(vec![])
        );
        assert!(!rd_division_allowed(&pool, &ctx, Some(division_id)).expect("rd access"));
        assert_eq!(
            work_allowed_division_ids(&pool, &ctx).expect("work scope"),
            Some(vec![])
        );
        assert!(!work_division_allowed(&pool, &ctx, Some(division_id)).expect("work access"));
        assert_eq!(
            sample_info_scope_allows(&pool, &ctx, Some(division_id), "default", None,)
                .expect("sample info scope"),
            Some(false)
        );
    }
}

/// v2.2.8 性能优化：批量查询实验室允许的方法ID，避免 N+1 查询
pub fn batch_work_methods_allowed_for_group(
    pool: &DbPool,
    _ctx: &AuthContext,
    group_id: i64,
) -> Result<std::collections::HashSet<i64>> {
    let conn = pool.get()?;

    // 获取该实验室关联的所有项目
    let project_ids: Vec<i64> = conn
        .prepare("SELECT DISTINCT project_id FROM project_lab_links WHERE group_id = ?1")?
        .query_map([group_id], |row| row.get(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    if project_ids.is_empty() {
        return Ok(std::collections::HashSet::new());
    }

    let placeholders = project_ids
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect::<Vec<_>>()
        .join(",");

    let sql = format!(
        "SELECT DISTINCT pml.method_id
         FROM project_method_links pml
         JOIN methods m ON m.id = pml.method_id
         JOIN instruments i ON i.id = m.instrument_id
         WHERE pml.project_id IN ({})
           AND m.is_active = 1
           AND i.is_active = 1",
        placeholders
    );

    let params: Vec<Box<dyn postgres_compat::types::ToSql>> = project_ids
        .iter()
        .map(|id| Box::new(*id) as Box<dyn postgres_compat::types::ToSql>)
        .collect();

    let mut stmt = conn.prepare(&sql)?;
    let method_ids = stmt
        .query_map(
            postgres_compat::params_from_iter(params.iter().map(|p| p.as_ref())),
            |row| row.get(0),
        )?
        .collect::<std::result::Result<std::collections::HashSet<_>, _>>()?;

    Ok(method_ids)
}
