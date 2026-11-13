use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::division::{Division, DivisionCreate, DivisionResponse, DivisionUpdate};
use crate::repo::{audit_repo, trash_repo};

fn validate_manager_on_conn(
    conn: &postgres_compat::Connection,
    manager_user_id: Option<i64>,
) -> Result<()> {
    let Some(manager_user_id) = manager_user_id else {
        return Ok(());
    };
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM users WHERE id=?1 AND is_active=1 AND deleted_at IS NULL)",
        [manager_user_id],
        |row| row.get(0),
    )?;
    if !exists {
        return Err(AppError::Validation("部门负责人不存在或已停用".into()));
    }
    Ok(())
}

/// 列出全部事业部，并聚合每个事业部下属实验室数量（lab_count）。
/// `project_groups.division_id` 为 NULL 的实验室不计入任何事业部。
pub fn list(pool: &DbPool) -> Result<Vec<DivisionResponse>> {
    list_for_portal(pool, None)
}

pub fn list_for_portal(pool: &DbPool, portal: Option<&str>) -> Result<Vec<DivisionResponse>> {
    let conn = pool.get()?;
    let portal_filter = match portal {
        Some("work") => " AND d.show_in_work=1",
        Some("rd") => " AND d.show_in_rd=1",
        Some("sample_info") => " AND d.show_in_sample_info=1",
        _ => "",
    };
    let sql = format!(
        "SELECT d.id, d.name, d.sort_order, d.color, d.is_active,
                (SELECT COUNT(*) FROM project_groups g WHERE g.division_id = d.id) AS lab_count
                ,d.show_in_work,d.show_in_rd,d.show_in_sample_info,
                d.code,d.manager_user_id,
                COALESCE((SELECT u.username FROM users u WHERE u.id=d.manager_user_id AND u.deleted_at IS NULL),'')
         FROM divisions d
         WHERE d.is_active = 1 AND d.deleted_at IS NULL
         {portal_filter}
         ORDER BY d.sort_order"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| {
        Ok(DivisionResponse {
            id: row.get(0)?,
            name: row.get(1)?,
            sort_order: row.get(2)?,
            color: row.get(3)?,
            is_active: row.get::<_, bool>(4).unwrap_or(true),
            lab_count: row.get(5)?,
            show_in_work: row.get::<_, bool>(6).unwrap_or(true),
            show_in_rd: row.get::<_, bool>(7).unwrap_or(true),
            show_in_sample_info: row.get::<_, bool>(8).unwrap_or(true),
            code: row.get::<_, String>(9).unwrap_or_default(),
            manager_user_id: row.get(10)?,
            manager_username: row.get::<_, String>(11).unwrap_or_default(),
        })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

pub fn get_by_id(pool: &DbPool, id: i64) -> Result<Division> {
    let conn = pool.get()?;
    conn.query_row(
        "SELECT id, name, sort_order, color, is_active, show_in_work, show_in_rd, show_in_sample_info, created_at,
                code,manager_user_id,COALESCE((SELECT username FROM users WHERE id=divisions.manager_user_id AND deleted_at IS NULL),'')
         FROM divisions WHERE id = ?1",
        [id],
        |row| {
            Ok(Division {
                id: row.get(0)?,
                name: row.get(1)?,
                sort_order: row.get(2)?,
                color: row.get(3)?,
                is_active: row.get::<_, bool>(4).unwrap_or(true),
                show_in_work: row.get::<_, bool>(5).unwrap_or(true),
                show_in_rd: row.get::<_, bool>(6).unwrap_or(true),
                show_in_sample_info: row.get::<_, bool>(7).unwrap_or(true),
                created_at: row.get(8)?,
                code: row.get::<_, String>(9).unwrap_or_default(),
                manager_user_id: row.get(10)?,
                manager_username: row.get::<_, String>(11).unwrap_or_default(),
            })
        },
    )
    .map_err(|e| match e {
        postgres_compat::Error::QueryReturnedNoRows => {
            crate::error::AppError::NotFound("事业部不存在".into())
        }
        _ => e.into(),
    })
}

/// 创建事业部（在已有连接上执行，便于上层包裹事务 + 审计）
pub fn create_on_conn(
    conn: &postgres_compat::Connection,
    body: &DivisionCreate,
) -> Result<Division> {
    let code = body.code.trim();
    if code.is_empty() {
        return Err(AppError::Validation("部门编码不能为空".into()));
    }
    validate_manager_on_conn(conn, body.manager_user_id)?;
    conn.execute(
        "INSERT INTO divisions (name, code, manager_user_id, sort_order, color, show_in_work, show_in_rd, show_in_sample_info, is_active) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        (
            &body.name,
            code,
            body.manager_user_id,
            body.sort_order.unwrap_or(0),
            body.color.clone().unwrap_or_else(|| "#1976d2".to_string()),
            body.show_in_work.unwrap_or(true),
            body.show_in_rd.unwrap_or(true),
            body.show_in_sample_info.unwrap_or(true),
            body.is_active.unwrap_or(true),
        ),
    )?;
    let id = conn.last_insert_rowid();
    get_by_id_on_conn(conn, id)
}

pub fn create(pool: &DbPool, body: &DivisionCreate) -> Result<Division> {
    let conn = pool.get()?;
    create_on_conn(&conn, body)
}

pub fn update(pool: &DbPool, id: i64, body: &DivisionUpdate) -> Result<Division> {
    let conn = pool.get()?;
    update_on_conn(&conn, id, body)
}

/// 在已有连接上更新事业部（供事务内使用）
pub fn update_on_conn(
    conn: &postgres_compat::Connection,
    id: i64,
    body: &DivisionUpdate,
) -> Result<Division> {
    if let Some(ref name) = body.name {
        conn.execute("UPDATE divisions SET name=?1 WHERE id=?2", (name, id))?;
    }
    if let Some(ref raw_code) = body.code {
        let code = raw_code.trim();
        if code.is_empty() {
            return Err(AppError::Validation("部门编码不能为空".into()));
        }
        conn.execute("UPDATE divisions SET code=?1 WHERE id=?2", (code, id))?;
    }
    if let Some(manager_user_id) = body.manager_user_id {
        validate_manager_on_conn(conn, manager_user_id)?;
        conn.execute(
            "UPDATE divisions SET manager_user_id=?1 WHERE id=?2",
            (manager_user_id, id),
        )?;
    }
    if let Some(so) = body.sort_order {
        conn.execute("UPDATE divisions SET sort_order=?1 WHERE id=?2", (so, id))?;
    }
    if let Some(ref color) = body.color {
        conn.execute("UPDATE divisions SET color=?1 WHERE id=?2", (color, id))?;
    }
    if let Some(value) = body.show_in_work {
        conn.execute(
            "UPDATE divisions SET show_in_work=?1 WHERE id=?2",
            (value, id),
        )?;
    }
    if let Some(value) = body.show_in_rd {
        conn.execute(
            "UPDATE divisions SET show_in_rd=?1 WHERE id=?2",
            (value, id),
        )?;
    }
    if let Some(value) = body.show_in_sample_info {
        conn.execute(
            "UPDATE divisions SET show_in_sample_info=?1 WHERE id=?2",
            (value, id),
        )?;
    }
    if let Some(value) = body.is_active {
        conn.execute("UPDATE divisions SET is_active=?1 WHERE id=?2", (value, id))?;
    }
    get_by_id_on_conn(conn, id)
}

/// 删除事业部时保留实验室归属关系，恢复后可完整还原。
pub fn delete(pool: &DbPool, id: i64, operator: &str, reason: &str) -> Result<()> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let division = get_by_id_on_conn(&tx, id)?;
    let labs: i64 = tx.query_row(
        "SELECT COUNT(*) FROM project_groups WHERE division_id=?1",
        [id],
        |r| r.get(0),
    )?;
    let records: i64 = tx.query_row("SELECT (SELECT COUNT(*) FROM work_records WHERE division_id=?1)+(SELECT COUNT(*) FROM rd_work_records WHERE division_id=?1)+(SELECT COUNT(*) FROM sample_info_records WHERE division_id=?1)", [id], |r| r.get(0))?;
    let users: i64 = tx.query_row(
        "SELECT COUNT(*) FROM users WHERE division_id=?1 AND deleted_at IS NULL",
        [id],
        |r| r.get(0),
    )?;
    let before = serde_json::to_value(&division).unwrap_or(serde_json::Value::Null);
    tx.execute(
        "UPDATE divisions SET is_active=0, deleted_at=datetime('now','localtime') WHERE id=?1",
        [id],
    )?;
    let dependency = format!("关联实验室 {labs} 个；历史记录 {records} 条；用户 {users} 个");
    trash_repo::move_to_trash_on_conn(
        &tx,
        "部门",
        "divisions",
        id,
        "master",
        "shared",
        &division.name,
        "",
        &before,
        reason,
        operator,
        None,
        None,
        &dependency,
        labs + records + users == 0,
    )?;
    let after = serde_json::json!({"deleted_at":"now","is_active":false,"data":before});
    audit_repo::log_structured_on_conn(
        &tx,
        "delete",
        "divisions",
        Some(id),
        operator,
        &format!("删除部门「{}」；{}", division.name, dependency),
        "shared",
        "",
        Some(&before),
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(())
}

/// 列出已软删除的事业部
pub fn list_deleted(pool: &DbPool) -> Result<Vec<DivisionResponse>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT d.id, d.name, d.sort_order, d.color, d.is_active,
                (SELECT COUNT(*) FROM project_groups g WHERE g.division_id = d.id) AS lab_count
                ,d.show_in_work,d.show_in_rd,d.show_in_sample_info,
                d.code,d.manager_user_id,
                COALESCE((SELECT u.username FROM users u WHERE u.id=d.manager_user_id AND u.deleted_at IS NULL),'')
         FROM divisions d
         WHERE d.is_active = 0
         ORDER BY d.sort_order",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(DivisionResponse {
            id: row.get(0)?,
            name: row.get(1)?,
            sort_order: row.get(2)?,
            color: row.get(3)?,
            is_active: row.get::<_, bool>(4).unwrap_or(false),
            lab_count: row.get(5)?,
            show_in_work: row.get::<_, bool>(6).unwrap_or(true),
            show_in_rd: row.get::<_, bool>(7).unwrap_or(true),
            show_in_sample_info: row.get::<_, bool>(8).unwrap_or(true),
            code: row.get::<_, String>(9).unwrap_or_default(),
            manager_user_id: row.get(10)?,
            manager_username: row.get::<_, String>(11).unwrap_or_default(),
        })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

/// 恢复已软删除的事业部
pub fn restore(pool: &DbPool, id: i64, operator: &str) -> Result<Division> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let rows = tx.execute(
        "UPDATE divisions SET is_active=1, deleted_at=NULL WHERE id=?1 AND is_active=0",
        [id],
    )?;
    trash_repo::mark_restored_on_conn(&tx, "divisions", id, operator)?;
    if rows == 0 {
        return Err(crate::error::AppError::NotFound(
            "事业部不存在或未删除".into(),
        ));
    }
    let div = get_by_id_on_conn(&tx, id)?;
    audit_repo::log_on_conn_with_module(
        &tx,
        "restore",
        "divisions",
        Some(id),
        operator,
        &format!("恢复事业部「{}」", div.name),
        "shared",
    )?;
    tx.commit()?;
    Ok(div)
}

/// 在已有连接上查询事业部（供事务内使用）
pub(crate) fn get_by_id_on_conn(conn: &postgres_compat::Connection, id: i64) -> Result<Division> {
    conn.query_row(
        "SELECT id, name, sort_order, color, is_active, show_in_work, show_in_rd, show_in_sample_info, created_at,
                code,manager_user_id,COALESCE((SELECT username FROM users WHERE id=divisions.manager_user_id AND deleted_at IS NULL),'')
         FROM divisions WHERE id = ?1",
        [id],
        |row| {
            Ok(Division {
                id: row.get(0)?,
                name: row.get(1)?,
                sort_order: row.get(2)?,
                color: row.get(3)?,
                is_active: row.get::<_, bool>(4).unwrap_or(true),
                show_in_work: row.get::<_, bool>(5).unwrap_or(true),
                show_in_rd: row.get::<_, bool>(6).unwrap_or(true),
                show_in_sample_info: row.get::<_, bool>(7).unwrap_or(true),
                created_at: row.get(8)?,
                code: row.get::<_, String>(9).unwrap_or_default(),
                manager_user_id: row.get(10)?,
                manager_username: row.get::<_, String>(11).unwrap_or_default(),
            })
        },
    )
    .map_err(|e| match e {
        postgres_compat::Error::QueryReturnedNoRows => {
            crate::error::AppError::NotFound("事业部不存在".into())
        }
        _ => e.into(),
    })
}

/// 清除事业部关联的所有实验室
pub fn clear_labs(conn: &postgres_compat::Connection, division_id: i64) -> Result<()> {
    conn.execute(
        "UPDATE project_groups SET division_id = NULL WHERE division_id = ?1",
        [division_id],
    )?;
    Ok(())
}

/// 设置事业部关联的实验室列表
pub fn set_division_labs(
    pool: &DbPool,
    division_id: i64,
    group_ids: &[i64],
    user_name: &str,
) -> Result<()> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;

    let mut before_stmt =
        tx.prepare("SELECT id FROM project_groups WHERE division_id=?1 ORDER BY id")?;
    let before_group_ids = before_stmt
        .query_map([division_id], |row| row.get::<_, i64>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(before_stmt);

    // 1) 清除现有关联
    clear_labs(&tx, division_id)?;

    // 2) 设置新关联
    for &gid in group_ids {
        tx.execute(
            "UPDATE project_groups SET division_id = ?1 WHERE id = ?2",
            postgres_compat::params![division_id, gid],
        )?;
    }

    let division = get_by_id_on_conn(&tx, division_id)?;
    let detail = format!(
        "设置事业部「{}」关联 {} 个实验室",
        division.name,
        group_ids.len()
    );
    let before = serde_json::json!({"division_id":division_id,"group_ids":before_group_ids});
    let after = serde_json::json!({"division_id":division_id,"group_ids":group_ids});
    audit_repo::log_structured_on_conn(
        &tx,
        "update",
        "project_groups",
        None,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_pool() -> DbPool {
        let pool = crate::db::init_pool("postgres-test");
        crate::db::test_migrations::run(&pool.get().unwrap()).unwrap();
        pool
    }

    #[test]
    fn sample_info_portal_returns_enabled_departments() {
        let pool = test_pool();
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO divisions(name,code,sort_order,show_in_sample_info,is_active) VALUES('样品信息部门','SI-ON',10,1,1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO divisions(name,code,sort_order,show_in_sample_info,is_active) VALUES('非样品信息部门','SI-OFF',20,0,1)",
            [],
        )
        .unwrap();

        let items = list_for_portal(&pool, Some("sample_info")).unwrap();
        assert!(items.iter().any(|item| item.name == "样品信息部门"));
        assert!(!items.iter().any(|item| item.name == "非样品信息部门"));
    }
}
