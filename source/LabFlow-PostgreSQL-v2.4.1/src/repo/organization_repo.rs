use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::organization::{
    DepartmentRole, DepartmentRoleCreate, DepartmentRoleUpdate, GovernanceOverview, LegacyRoleGap,
    OwnershipConfirmation, OwnershipMigrationSummary, OwnershipPendingRecord, PermissionAuditRow,
    PublicAccountAuditRow,
};
use crate::repo::{audit_repo, user_repo};

fn normalized_domain(value: &str) -> Result<&str> {
    match value.trim() {
        "all" | "work" | "rd" | "sample_info" => Ok(value.trim()),
        _ => Err(AppError::Validation(
            "业务域只能是 all、work、rd 或 sample_info".into(),
        )),
    }
}

fn map_department_role(row: &postgres_compat::Row<'_>) -> postgres_compat::Result<DepartmentRole> {
    let user_ids: String = row.get::<_, String>(9).unwrap_or_default();
    let user_names: String = row.get::<_, String>(10).unwrap_or_default();
    Ok(DepartmentRole {
        id: row.get(0)?,
        role_id: row.get(1)?,
        role_name: row.get(2)?,
        template_name: row.get(3)?,
        division_id: row.get(4)?,
        division_name: row.get(5)?,
        business_domain: row.get(6)?,
        is_department_admin: row.get::<_, bool>(7).unwrap_or(false),
        is_active: row.get::<_, bool>(8).unwrap_or(true),
        user_ids: user_ids
            .split(',')
            .filter_map(|value| value.parse::<i64>().ok())
            .collect(),
        user_names: user_names
            .split(',')
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect(),
    })
}

const SELECT_DEPARTMENT_ROLES: &str =
    "SELECT dr.id,dr.role_id,r.name,rt.name,dr.division_id,d.name,dr.business_domain,
            dr.is_department_admin,dr.is_active,
            COALESCE((SELECT string_agg(udr.user_id::text,',' ORDER BY udr.user_id)
                      FROM user_department_roles udr WHERE udr.department_role_id=dr.id),''),
            COALESCE((SELECT string_agg(u.username,',' ORDER BY u.username)
                      FROM user_department_roles udr JOIN users u ON u.id=udr.user_id
                      WHERE udr.department_role_id=dr.id), '')
     FROM department_roles dr
     JOIN roles r ON r.id=dr.role_id
     JOIN divisions d ON d.id=dr.division_id
     LEFT JOIN role_templates rt ON rt.id=r.template_id";

pub fn list_department_roles(
    pool: &DbPool,
    allowed_division_ids: Option<&[i64]>,
) -> Result<Vec<DepartmentRole>> {
    let conn = pool.get()?;
    let mut sql = SELECT_DEPARTMENT_ROLES.to_string();
    if let Some(ids) = allowed_division_ids {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let joined = ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
        sql.push_str(&format!(" WHERE dr.division_id IN ({joined})"));
    }
    sql.push_str(" ORDER BY d.sort_order, r.sort_order, dr.id");
    let mut stmt = conn.prepare(&sql)?;
    Ok(stmt
        .query_map([], map_department_role)?
        .collect::<std::result::Result<Vec<_>, _>>()?)
}

pub fn department_admin_division_ids(pool: &DbPool, user_id: i64) -> Result<Vec<i64>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT DISTINCT dr.division_id
         FROM user_department_roles udr
         JOIN department_roles dr ON dr.id=udr.department_role_id
         WHERE udr.user_id=?1 AND dr.is_active=1 AND dr.is_department_admin=1
         ORDER BY dr.division_id",
    )?;
    Ok(stmt
        .query_map([user_id], |row| row.get::<_, i64>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?)
}

/// Explicit department-role assignments are a restrictive business boundary.
/// `all` applies to every domain, while domain-specific assignments only apply
/// to their matching workflow.
pub fn assigned_department_role_division_ids(
    pool: &DbPool,
    user_id: i64,
    business_domain: &str,
) -> Result<Vec<i64>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT DISTINCT dr.division_id
         FROM user_department_roles udr
         JOIN department_roles dr ON dr.id=udr.department_role_id
         WHERE udr.user_id=?1 AND dr.is_active=1
           AND dr.business_domain IN ('all',?2)
         ORDER BY dr.division_id",
    )?;
    Ok(stmt
        .query_map(postgres_compat::params![user_id, business_domain], |row| {
            row.get::<_, i64>(0)
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?)
}

pub fn create_department_role(
    pool: &DbPool,
    body: &DepartmentRoleCreate,
    operator: &str,
) -> Result<DepartmentRole> {
    let domain = normalized_domain(&body.business_domain)?;
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let role_exists: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM roles WHERE id=?1 AND deleted_at IS NULL)",
        [body.role_id],
        |row| row.get(0),
    )?;
    let division_exists: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM divisions WHERE id=?1 AND is_active=1 AND deleted_at IS NULL)",
        [body.division_id],
        |row| row.get(0),
    )?;
    if !role_exists || !division_exists {
        return Err(AppError::Validation("角色或部门不存在/已停用".into()));
    }
    tx.execute(
        "INSERT INTO department_roles(role_id,division_id,business_domain,is_department_admin)
         VALUES(?1,?2,?3,?4)",
        postgres_compat::params![
            body.role_id,
            body.division_id,
            domain,
            body.is_department_admin
        ],
    )?;
    let id = tx.last_insert_rowid();
    audit_repo::log_structured_on_conn(
        &tx,
        "create",
        "department_roles",
        Some(id),
        operator,
        "创建部门角色",
        "shared",
        "",
        None,
        Some(
            &serde_json::json!({"role_id":body.role_id,"division_id":body.division_id,"business_domain":domain,"is_department_admin":body.is_department_admin}),
        ),
        "management",
    )?;
    tx.commit()?;
    list_department_roles(pool, None)?
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| AppError::NotFound("部门角色创建后未找到".into()))
}

pub fn update_department_role(
    pool: &DbPool,
    id: i64,
    body: &DepartmentRoleUpdate,
    operator: &str,
) -> Result<DepartmentRole> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let before = list_department_roles(pool, None)?
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| AppError::NotFound("部门角色不存在".into()))?;
    if let Some(domain) = body.business_domain.as_deref() {
        tx.execute(
            "UPDATE department_roles SET business_domain=?1,updated_at=datetime('now','localtime') WHERE id=?2",
            postgres_compat::params![normalized_domain(domain)?, id],
        )?;
    }
    if let Some(value) = body.is_department_admin {
        tx.execute("UPDATE department_roles SET is_department_admin=?1,updated_at=datetime('now','localtime') WHERE id=?2", postgres_compat::params![value,id])?;
    }
    if let Some(value) = body.is_active {
        tx.execute("UPDATE department_roles SET is_active=?1,updated_at=datetime('now','localtime') WHERE id=?2", postgres_compat::params![value,id])?;
    }
    audit_repo::log_structured_on_conn(
        &tx,
        "update",
        "department_roles",
        Some(id),
        operator,
        "更新部门角色",
        "shared",
        "",
        Some(&serde_json::to_value(&before).unwrap_or_default()),
        None,
        "management",
    )?;
    tx.commit()?;
    list_department_roles(pool, None)?
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| AppError::NotFound("部门角色不存在".into()))
}

pub fn set_department_role_users(
    pool: &DbPool,
    id: i64,
    user_ids: &[i64],
    operator: &str,
) -> Result<DepartmentRole> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let (role_id, division_id): (i64, i64) = tx
        .query_row(
            "SELECT role_id,division_id FROM department_roles WHERE id=?1 AND is_active=1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| AppError::NotFound("部门角色不存在或已停用".into()))?;
    let before_ids: Vec<i64> = tx
        .prepare("SELECT user_id FROM user_department_roles WHERE department_role_id=?1")?
        .query_map([id], |row| row.get(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for user_id in user_ids {
        let allowed: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM user_divisions WHERE user_id=?1 AND division_id=?2)",
            postgres_compat::params![user_id, division_id],
            |row| row.get(0),
        )?;
        if !allowed {
            return Err(AppError::Validation(format!(
                "用户 {user_id} 不属于该部门的可承担业务部门"
            )));
        }
    }
    tx.execute(
        "DELETE FROM user_department_roles WHERE department_role_id=?1",
        [id],
    )?;
    for user_id in user_ids {
        tx.execute(
            "INSERT INTO user_department_roles(user_id,department_role_id) VALUES(?1,?2)",
            postgres_compat::params![user_id, id],
        )?;
        let has_direct_or_existing_role: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM user_roles WHERE user_id=?1 AND role_id=?2)",
            postgres_compat::params![user_id, role_id],
            |row| row.get(0),
        )?;
        tx.execute(
            "INSERT INTO user_roles(user_id,role_id) VALUES(?1,?2) ON CONFLICT DO NOTHING",
            postgres_compat::params![user_id, role_id],
        )?;
        if !has_direct_or_existing_role {
            tx.execute(
                "INSERT INTO department_role_managed_user_roles(user_id,role_id) VALUES(?1,?2) ON CONFLICT DO NOTHING",
                postgres_compat::params![user_id, role_id],
            )?;
        }
    }
    for user_id in &before_ids {
        if user_ids.contains(user_id) {
            continue;
        }
        let still_assigned: bool = tx.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM user_department_roles udr
                JOIN department_roles dr ON dr.id=udr.department_role_id
                WHERE udr.user_id=?1 AND dr.role_id=?2 AND dr.is_active=1
            )",
            postgres_compat::params![user_id, role_id],
            |row| row.get(0),
        )?;
        let managed_by_department_role: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM department_role_managed_user_roles WHERE user_id=?1 AND role_id=?2)",
            postgres_compat::params![user_id, role_id],
            |row| row.get(0),
        )?;
        if managed_by_department_role && !still_assigned {
            tx.execute(
                "DELETE FROM user_roles WHERE user_id=?1 AND role_id=?2",
                postgres_compat::params![user_id, role_id],
            )?;
            tx.execute(
                "DELETE FROM department_role_managed_user_roles WHERE user_id=?1 AND role_id=?2",
                postgres_compat::params![user_id, role_id],
            )?;
        }
    }
    audit_repo::log_structured_on_conn(
        &tx,
        "assign",
        "department_roles",
        Some(id),
        operator,
        "更新部门角色人员",
        "shared",
        "",
        Some(&serde_json::json!({"user_ids":before_ids})),
        Some(&serde_json::json!({"user_ids":user_ids})),
        "management",
    )?;
    tx.commit()?;
    list_department_roles(pool, None)?
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| AppError::NotFound("部门角色不存在".into()))
}

fn string_list_for_role(
    conn: &postgres_compat::Connection,
    sql: &str,
    role_id: i64,
) -> Result<Vec<String>> {
    Ok(conn
        .prepare(sql)?
        .query_map([role_id], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?)
}

/// Exposes the effective department-role setup in a human-auditable form.
/// The report deliberately shows both generic department scopes and the
/// analysis-specific department scope because they govern different workflows.
pub fn permission_audit_rows(pool: &DbPool) -> Result<Vec<PermissionAuditRow>> {
    let conn = pool.get()?;
    let department_roles = list_department_roles(pool, None)?;
    let mut rows = Vec::with_capacity(department_roles.len());
    for item in department_roles {
        rows.push(PermissionAuditRow {
            department_role_id: item.id,
            division_name: item.division_name,
            business_domain: item.business_domain,
            role_name: item.role_name,
            template_name: item.template_name.unwrap_or_else(|| "自定义".into()),
            is_active: item.is_active,
            is_department_admin: item.is_department_admin,
            assigned_users: item.user_names,
            permissions: string_list_for_role(
                &conn,
                "SELECT permission_key FROM role_permissions WHERE role_id=?1 ORDER BY permission_key",
                item.role_id,
            )?,
            division_scope_names: string_list_for_role(
                &conn,
                "SELECT d.name FROM role_division_scopes s JOIN divisions d ON d.id=s.division_id WHERE s.role_id=?1 ORDER BY d.sort_order,d.id",
                item.role_id,
            )?,
            work_division_scope_names: string_list_for_role(
                &conn,
                "SELECT d.name FROM role_work_division_scopes s JOIN divisions d ON d.id=s.division_id WHERE s.role_id=?1 ORDER BY d.sort_order,d.id",
                item.role_id,
            )?,
            sample_info_type_scope_names: string_list_for_role(
                &conn,
                "SELECT COALESCE(t.label,s.type_key) FROM role_sample_info_type_scopes s LEFT JOIN sample_info_types t ON t.type_key=s.type_key WHERE s.role_id=?1 ORDER BY s.type_key",
                item.role_id,
            )?,
        });
    }
    Ok(rows)
}

pub fn public_account_audit_rows(pool: &DbPool) -> Result<Vec<PublicAccountAuditRow>> {
    let conn = pool.get()?;
    let accounts = conn
        .prepare(
            "SELECT DISTINCT u.id,u.username
             FROM users u
             JOIN user_roles ur ON ur.user_id=u.id
             JOIN roles r ON r.id=ur.role_id
             WHERE u.deleted_at IS NULL AND r.system_key='analysis_public_account'
             ORDER BY u.username",
        )?
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut rows = Vec::with_capacity(accounts.len());
    for (user_id, username) in accounts {
        let division_names = conn
            .prepare(
                "SELECT d.name FROM divisions d WHERE d.id IN (
                     SELECT division_id FROM user_divisions WHERE user_id=?1
                     UNION SELECT division_id FROM users WHERE id=?1 AND division_id IS NOT NULL
                 ) ORDER BY d.sort_order,d.id",
            )?
            .query_map([user_id], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let scope = user_repo::analysis_public_account_scope(pool, user_id)?;
        rows.push(PublicAccountAuditRow {
            username,
            division_names,
            candidate_users: scope
                .accounts
                .into_iter()
                .map(|user| user.username)
                .collect(),
        });
    }
    Ok(rows)
}

pub fn ownership_migration_summary(pool: &DbPool) -> Result<Vec<OwnershipMigrationSummary>> {
    let conn = pool.get()?;
    let definitions = [
        ("work", "work_records"),
        ("rd", "rd_work_records"),
        ("sample_info", "sample_info_records"),
    ];
    let mut rows = Vec::with_capacity(definitions.len());
    for (record_type, table) in definitions {
        let total_records: i64 = conn.query_row(
            &format!("SELECT COUNT(*) FROM {table} WHERE deleted_at IS NULL"),
            [],
            |row| row.get(0),
        )?;
        let pending_records: i64 = conn.query_row(
            &format!("SELECT COUNT(*) FROM {table} WHERE deleted_at IS NULL AND ownership_status='pending_confirmation'"),
            [],
            |row| row.get(0),
        )?;
        let manually_confirmed_records: i64 = conn.query_row(
            "SELECT COUNT(*) FROM record_ownership_confirmations WHERE record_type=?1",
            [record_type],
            |row| row.get(0),
        )?;
        rows.push(OwnershipMigrationSummary {
            record_type: record_type.into(),
            total_records,
            confirmed_records: total_records - pending_records,
            pending_records,
            manually_confirmed_records,
        });
    }
    Ok(rows)
}

/// Direct user-role rows with no active department-role mapping are retained
/// for backward compatibility, but must be reviewed before company-wide use.
pub fn legacy_role_gaps(pool: &DbPool) -> Result<Vec<LegacyRoleGap>> {
    let conn = pool.get()?;
    let rows = conn
        .prepare(
            "SELECT u.id,u.username,u.is_active,r.id,r.name,COALESCE(rt.name,''),COALESCE(d.name,'')
             FROM user_roles ur
             JOIN users u ON u.id=ur.user_id
             JOIN roles r ON r.id=ur.role_id
             LEFT JOIN role_templates rt ON rt.id=r.template_id
             LEFT JOIN divisions d ON d.id=u.division_id
             WHERE u.deleted_at IS NULL AND r.deleted_at IS NULL
               AND COALESCE(r.system_key,'') NOT IN ('system_admin','public_account','rd_public_account','analysis_public_account')
               AND NOT EXISTS(
                 SELECT 1 FROM user_department_roles udr
                 JOIN department_roles dr ON dr.id=udr.department_role_id
                 WHERE udr.user_id=u.id AND dr.role_id=r.id AND dr.is_active=1
               )
             ORDER BY u.username,r.sort_order,r.id",
        )?
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, bool>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut gaps = Vec::with_capacity(rows.len());
    for (user_id, username, is_active, role_id, role_name, template_name, primary_division_name) in
        rows
    {
        let business_division_names = conn
            .prepare(
                "SELECT d.name FROM user_divisions ud JOIN divisions d ON d.id=ud.division_id WHERE ud.user_id=?1 ORDER BY d.sort_order,d.id",
            )?
            .query_map([user_id], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        gaps.push(LegacyRoleGap {
            user_id,
            username,
            is_active,
            role_id,
            role_name,
            template_name,
            primary_division_name,
            business_division_names,
        });
    }
    Ok(gaps)
}

pub fn governance_overview(pool: &DbPool) -> Result<GovernanceOverview> {
    Ok(GovernanceOverview {
        permission_audit: permission_audit_rows(pool)?,
        public_accounts: public_account_audit_rows(pool)?,
        ownership_migration: ownership_migration_summary(pool)?,
        legacy_role_gaps: legacy_role_gaps(pool)?,
    })
}

pub fn pending_ownership(
    pool: &DbPool,
    record_type: Option<&str>,
) -> Result<Vec<OwnershipPendingRecord>> {
    let conn = pool.get()?;
    let accepted = ["work", "rd", "sample_info"];
    if let Some(value) = record_type {
        if !accepted.contains(&value) {
            return Err(AppError::Validation("无效的记录类型".into()));
        }
    }
    let mut sql = String::from(
        "SELECT
            'work' AS record_type,
            id AS record_id,
            COALESCE(business_no,'') AS business_no,
            '' AS status,
            project_division_id,
            execution_division_id,
            execution_group_id,
            NULL::BIGINT AS submitted_division_id,
            created_at
         FROM work_records WHERE deleted_at IS NULL AND ownership_status='pending_confirmation'
         UNION ALL
         SELECT
            'rd',id,COALESCE(business_no,''),COALESCE(status,''),
            project_division_id,execution_division_id,execution_group_id,submitted_division_id,created_at
         FROM rd_work_records WHERE deleted_at IS NULL AND ownership_status='pending_confirmation'
         UNION ALL
         SELECT
            'sample_info',id,COALESCE(business_no,''),status,
            project_division_id,execution_division_id,execution_group_id,submitted_division_id,created_at
         FROM sample_info_records WHERE deleted_at IS NULL AND ownership_status='pending_confirmation'"
    );
    if let Some(value) = record_type {
        sql = format!("SELECT * FROM ({sql}) ownership WHERE ownership.record_type=?1 ORDER BY ownership.created_at DESC");
        let mut stmt = conn.prepare(&sql)?;
        return Ok(stmt
            .query_map([value], map_pending)?
            .collect::<std::result::Result<Vec<_>, _>>()?);
    }
    sql.push_str(" ORDER BY created_at DESC");
    let mut stmt = conn.prepare(&sql)?;
    Ok(stmt
        .query_map([], map_pending)?
        .collect::<std::result::Result<Vec<_>, _>>()?)
}

fn map_pending(row: &postgres_compat::Row<'_>) -> postgres_compat::Result<OwnershipPendingRecord> {
    Ok(OwnershipPendingRecord {
        record_type: row.get(0)?,
        record_id: row.get(1)?,
        business_no: row.get(2)?,
        status: row.get(3)?,
        project_division_id: row.get(4)?,
        execution_division_id: row.get(5)?,
        execution_group_id: row.get(6)?,
        submitted_division_id: row.get(7)?,
        created_at: row.get(8)?,
    })
}

pub fn confirm_ownership(
    pool: &DbPool,
    record_type: &str,
    record_id: i64,
    body: &OwnershipConfirmation,
    operator_id: i64,
    operator: &str,
) -> Result<()> {
    if !["work", "rd", "sample_info"].contains(&record_type) {
        return Err(AppError::Validation("无效的记录类型".into()));
    }
    let table = match record_type {
        "work" => "work_records",
        "rd" => "rd_work_records",
        _ => "sample_info_records",
    };
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let exists: bool = tx.query_row(
        &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id=?1 AND deleted_at IS NULL)"),
        [record_id],
        |row| row.get(0),
    )?;
    if !exists {
        return Err(AppError::NotFound("待确认记录不存在".into()));
    }
    let execution_division_id = body
        .execution_division_id
        .ok_or_else(|| AppError::Validation("确认归属时必须指定执行部门".into()))?;
    for division_id in [
        body.project_division_id,
        Some(execution_division_id),
        body.submitted_division_id,
    ]
    .into_iter()
    .flatten()
    {
        let valid: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM divisions WHERE id=?1 AND is_active=1 AND deleted_at IS NULL)",
            [division_id],
            |row| row.get(0),
        )?;
        if !valid {
            return Err(AppError::Validation(format!(
                "部门不存在或已停用：{division_id}"
            )));
        }
    }
    if record_type != "work" && body.submitted_division_id.is_none() {
        return Err(AppError::Validation(
            "确认归属时必须指定送样/登记部门".into(),
        ));
    }
    if let Some(group_id) = body.execution_group_id {
        let group_matches: bool = if record_type == "work" {
            tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM project_groups WHERE id=?1 AND deleted_at IS NULL)",
                [group_id],
                |row| row.get(0),
            )?
        } else {
            tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM project_groups WHERE id=?1 AND division_id=?2 AND deleted_at IS NULL)",
                postgres_compat::params![group_id, execution_division_id],
                |row| row.get(0),
            )?
        };
        if !group_matches {
            return Err(AppError::Validation(
                "执行实验室不存在，或不属于所选执行部门".into(),
            ));
        }
    }
    let mut confirmed_detection_division_id: Option<i64> = None;
    let mut confirmed_sending_division_id = body.submitted_division_id;
    if record_type == "work" && body.execution_group_id.is_none() {
        return Err(AppError::Validation(
            "确认分析记录归属时必须指定执行实验室，以确定送样部门".into(),
        ));
    }
    if record_type == "work" {
        let execution_group_id = body.execution_group_id.expect("validated above");
        let (sending_division_id, sending_division_name, sending_group_name): (
            Option<i64>,
            String,
            String,
        ) = tx.query_row(
            "SELECT g.division_id, COALESCE(d.name,''), COALESCE(g.name,'')
                 FROM project_groups g
                 LEFT JOIN divisions d ON d.id=g.division_id
                 WHERE g.id=?1 AND g.deleted_at IS NULL
                   AND d.is_active=1 AND d.deleted_at IS NULL",
            [execution_group_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        let sending_division_id = sending_division_id.ok_or_else(|| {
            AppError::Validation("执行实验室未配置所属送样部门，无法确认分析记录归属".into())
        })?;
        let detection_division_name: String = tx.query_row(
            "SELECT COALESCE(name,'') FROM divisions WHERE id=?1 AND deleted_at IS NULL",
            [execution_division_id],
            |row| row.get(0),
        )?;
        confirmed_detection_division_id = Some(execution_division_id);
        confirmed_sending_division_id = Some(sending_division_id);
        tx.execute(
            "UPDATE work_records
             SET project_division_id=?1,
                 execution_division_id=?2,
                 execution_division_name_snapshot=?3,
                 execution_group_id=?4,
                 execution_group_name_snapshot=?5,
                 detection_division_id=?2,
                 detection_division_name_snapshot=?3,
                 sending_division_id=?6,
                 sending_division_name_snapshot=?7,
                 sending_group_id=?4,
                 sending_group_name_snapshot=?5,
                 ownership_status='confirmed'
             WHERE id=?8",
            postgres_compat::params![
                body.project_division_id,
                body.execution_division_id,
                detection_division_name,
                execution_group_id,
                sending_group_name,
                sending_division_id,
                sending_division_name,
                record_id
            ],
        )?;
    } else {
        tx.execute(
            &format!(
                "UPDATE {table}
                 SET project_division_id=?1, execution_division_id=?2, execution_group_id=?3,
                     submitted_division_id=?4, ownership_status='confirmed'
                 WHERE id=?5"
            ),
            postgres_compat::params![
                body.project_division_id,
                body.execution_division_id,
                body.execution_group_id,
                body.submitted_division_id,
                record_id
            ],
        )?;
    }
    tx.execute(
        "INSERT INTO record_ownership_confirmations(record_type,record_id,project_division_id,execution_division_id,execution_group_id,submitted_division_id,confirmed_by_user_id,confirmed_by_username,note)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)
         ON CONFLICT(record_type,record_id) DO UPDATE SET project_division_id=excluded.project_division_id,execution_division_id=excluded.execution_division_id,execution_group_id=excluded.execution_group_id,submitted_division_id=excluded.submitted_division_id,confirmed_by_user_id=excluded.confirmed_by_user_id,confirmed_by_username=excluded.confirmed_by_username,note=excluded.note,confirmed_at=to_char(CURRENT_TIMESTAMP,'YYYY-MM-DD HH24:MI:SS')",
        postgres_compat::params![record_type,record_id,body.project_division_id,body.execution_division_id,body.execution_group_id,body.submitted_division_id,operator_id,operator,&body.note],
    )?;
    audit_repo::log_structured_on_conn(
        &tx,
        "ownership_confirm",
        table,
        Some(record_id),
        operator,
        "确认历史记录归属",
        "shared",
        "",
        None,
        Some(
            &serde_json::json!({"record_type":record_type,"project_division_id":body.project_division_id,"execution_division_id":body.execution_division_id,"execution_group_id":body.execution_group_id,"detection_division_id":confirmed_detection_division_id,"sending_division_id":confirmed_sending_division_id,"submitted_division_id":body.submitted_division_id,"note":body.note}),
        ),
        "governance",
    )?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn governance_overview_reports_department_assignments_and_legacy_direct_roles() {
        let pool = crate::db::init_pool("postgres-test");
        crate::db::test_migrations::run(&pool.get().expect("connection")).expect("migrations");
        let conn = pool.get().expect("connection");
        conn.execute(
            "INSERT INTO divisions(name,sort_order,color,is_active) VALUES('审计部门',1,'#1976d2',1)",
            [],
        )
        .expect("division");
        let division_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO roles(name,description,is_system,sort_order) VALUES('审计测试角色','',0,99)",
            [],
        )
        .expect("role");
        let role_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO role_permissions(role_id,permission_key) VALUES(?1,'entry:workload')",
            [role_id],
        )
        .expect("permission");
        conn.execute(
            "INSERT INTO users(username,password,is_admin,is_active,division_id) VALUES('审计部门人员','hash',0,1,?1)",
            [division_id],
        )
        .expect("department user");
        let department_user_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO user_divisions(user_id,division_id) VALUES(?1,?2)",
            postgres_compat::params![department_user_id, division_id],
        )
        .expect("user division");
        conn.execute(
            "INSERT INTO department_roles(role_id,division_id,business_domain) VALUES(?1,?2,'work')",
            postgres_compat::params![role_id, division_id],
        )
        .expect("department role");
        let department_role_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO user_department_roles(user_id,department_role_id) VALUES(?1,?2)",
            postgres_compat::params![department_user_id, department_role_id],
        )
        .expect("assignment");
        conn.execute(
            "INSERT INTO users(username,password,is_admin,is_active) VALUES('旧直接角色人员','hash',0,1)",
            [],
        )
        .expect("legacy user");
        let legacy_user_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO user_roles(user_id,role_id) VALUES(?1,?2)",
            postgres_compat::params![legacy_user_id, role_id],
        )
        .expect("legacy assignment");

        let report = governance_overview(&pool).expect("overview");
        assert!(report
            .permission_audit
            .iter()
            .any(|row| row.department_role_id == department_role_id
                && row.assigned_users == vec!["审计部门人员"]));
        assert!(report
            .legacy_role_gaps
            .iter()
            .any(|row| row.user_id == legacy_user_id && row.role_id == role_id));
        assert_eq!(report.ownership_migration.len(), 3);
    }

    #[test]
    fn confirming_work_ownership_populates_detection_and_sending_snapshots() {
        let pool = crate::db::init_pool("postgres-test");
        crate::db::test_migrations::run(&pool.get().expect("connection")).expect("migrations");
        let conn = pool.get().expect("connection");
        let suffix = uuid::Uuid::new_v4().simple().to_string();

        conn.execute(
            "INSERT INTO divisions(name,sort_order,color,is_active) VALUES(?1,1,'#1976d2',1)",
            [format!("Detection-{suffix}")],
        )
        .expect("detection division");
        let detection_division_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO divisions(name,sort_order,color,is_active) VALUES(?1,2,'#1976d2',1)",
            [format!("Sending-{suffix}")],
        )
        .expect("sending division");
        let sending_division_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO project_groups(name,division_id) VALUES(?1,?2)",
            postgres_compat::params![format!("Lab-{suffix}"), sending_division_id],
        )
        .expect("sending laboratory");
        let group_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO projects(group_id,name) VALUES(?1,?2)",
            postgres_compat::params![group_id, format!("Project-{suffix}")],
        )
        .expect("project");
        let project_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO work_records(project_id,user_name,quantity,recorded_at,group_id,ownership_status)
             VALUES(?1,'detector',1,'2026-08-14 10:00:00',?2,'pending_confirmation')",
            postgres_compat::params![project_id, group_id],
        )
        .expect("pending work record");
        let record_id = conn.last_insert_rowid();

        confirm_ownership(
            &pool,
            "work",
            record_id,
            &OwnershipConfirmation {
                project_division_id: None,
                execution_division_id: Some(detection_division_id),
                execution_group_id: Some(group_id),
                submitted_division_id: None,
                note: "repair historical dimensions".into(),
            },
            1,
            "admin",
        )
        .expect("confirm ownership");

        let row = conn
            .query_row(
                "SELECT detection_division_id,detection_division_name_snapshot,
                        sending_division_id,sending_division_name_snapshot,
                        sending_group_id,sending_group_name_snapshot,ownership_status
                 FROM work_records WHERE id=?1",
                [record_id],
                |row| {
                    Ok((
                        row.get::<_, Option<i64>>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<i64>>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                    ))
                },
            )
            .expect("confirmed dimensions");
        assert_eq!(row.0, Some(detection_division_id));
        assert_eq!(row.1, format!("Detection-{suffix}"));
        assert_eq!(row.2, Some(sending_division_id));
        assert_eq!(row.3, format!("Sending-{suffix}"));
        assert_eq!(row.4, Some(group_id));
        assert_eq!(row.5, format!("Lab-{suffix}"));
        assert_eq!(row.6, "confirmed");
    }
}
