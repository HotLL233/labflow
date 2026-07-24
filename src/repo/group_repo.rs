use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::group::{GroupCreate, GroupResponse, GroupUpdate};
use crate::repo::{audit_repo, trash_repo};

pub fn list(pool: &DbPool) -> Result<Vec<GroupResponse>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT g.id, g.name, g.sort_order, g.created_at,
                (SELECT COUNT(*) FROM project_lab_links pll_count
                 WHERE pll_count.group_id = g.id) AS project_count,
                (SELECT string_agg(p.name, ',' ORDER BY p.name) FROM project_lab_links pll2
                 JOIN projects p ON p.id = pll2.project_id
                 WHERE pll2.group_id = g.id) AS project_names,
                (SELECT COUNT(*) FROM rd_work_records wr2
                 WHERE wr2.group_id = g.id AND wr2.deleted_at IS NULL AND wr2.status = '待取样') AS rd_record_count,
                g.show_in_work, g.show_in_rd,
                g.division_id, dv.name AS division_name
         FROM project_groups g
         LEFT JOIN divisions dv ON dv.id = g.division_id
         WHERE g.name != '研发项目' AND g.deleted_at IS NULL
         ORDER BY g.sort_order"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(GroupResponse {
            id: row.get(0)?,
            name: row.get(1)?,
            sort_order: row.get(2)?,
            created_at: row.get(3)?,
            project_count: row.get(4)?,
            project_names: row.get(5)?,
            rd_record_count: row.get(6)?,
            show_in_work: row.get::<_, bool>(7).unwrap_or(true),
            show_in_rd: row.get::<_, bool>(8).unwrap_or(true),
            division_id: row.get(9)?,
            division_name: row.get(10)?,
        })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

pub fn get_by_id(pool: &DbPool, id: i64) -> Result<GroupResponse> {
    let conn = pool.get()?;
    get_by_id_on_conn(&conn, id)
}

fn get_by_id_on_conn(conn: &postgres_compat::Connection, id: i64) -> Result<GroupResponse> {
    conn.query_row(
        "SELECT g.id, g.name, g.sort_order, g.created_at,
                (SELECT COUNT(*) FROM project_lab_links pll_count
                 WHERE pll_count.group_id = g.id),
                (SELECT string_agg(p.name, ',' ORDER BY p.name) FROM project_lab_links pll2
                 JOIN projects p ON p.id = pll2.project_id
                 WHERE pll2.group_id = g.id) AS project_names,
                (SELECT COUNT(*) FROM rd_work_records wr2
                 WHERE wr2.group_id = g.id AND wr2.deleted_at IS NULL AND wr2.status = '待取样') AS rd_record_count,
                g.show_in_work, g.show_in_rd,
                g.division_id, dv.name AS division_name
         FROM project_groups g
         LEFT JOIN divisions dv ON dv.id = g.division_id
         WHERE g.id = ?1",
        [id],
        |row| Ok(GroupResponse {
            id: row.get(0)?, name: row.get(1)?, sort_order: row.get(2)?,
            created_at: row.get(3)?, project_count: row.get(4)?,
            project_names: row.get(5)?, rd_record_count: row.get(6)?,
            show_in_work: row.get::<_, bool>(7).unwrap_or(true),
            show_in_rd: row.get::<_, bool>(8).unwrap_or(true),
            division_id: row.get(9)?,
            division_name: row.get(10)?,
        }),
    ).map_err(|e| match e {
        postgres_compat::Error::QueryReturnedNoRows => crate::error::AppError::NotFound("分组不存在".into()),
        _ => e.into(),
    })
}

pub fn create(pool: &DbPool, body: &GroupCreate, operator: &str) -> Result<GroupResponse> {
    let mut conn = pool.get()?;
    let name = body.name.trim();
    if name.is_empty() {
        return Err(AppError::Validation("实验室名称不能为空".into()));
    }
    let duplicate: i64 = conn.query_row(
        "SELECT COUNT(*) FROM project_groups WHERE name=?1",
        [name],
        |row| row.get(0),
    )?;
    if duplicate > 0 {
        return Err(AppError::Validation(format!("实验室名称已存在：{name}")));
    }
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO project_groups (name, sort_order, show_in_work, show_in_rd, division_id) VALUES (?1, ?2, ?3, ?4, ?5)",
        (&body.name, body.sort_order.unwrap_or(0), body.show_in_work.unwrap_or(true), body.show_in_rd.unwrap_or(true), body.division_id),
    )?;
    let id = tx.last_insert_rowid();
    let created = get_by_id_on_conn(&tx, id)?;
    let after = serde_json::to_value(&created).unwrap_or(serde_json::Value::Null);
    audit_repo::log_structured_on_conn(
        &tx,
        "create",
        "project_groups",
        Some(id),
        operator,
        &format!("创建实验室「{}」", body.name),
        "shared",
        "",
        None,
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(created)
}

pub fn update(pool: &DbPool, id: i64, body: &GroupUpdate, operator: &str) -> Result<GroupResponse> {
    let mut conn = pool.get()?;
    let current = get_by_id_on_conn(&conn, id)?;
    if let Some(ref name) = body.name {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::Validation("实验室名称不能为空".into()));
        }
        let duplicate: i64 = conn.query_row(
            "SELECT COUNT(*) FROM project_groups WHERE name=?1 AND id<>?2",
            postgres_compat::params![name, id],
            |row| row.get(0),
        )?;
        if duplicate > 0 {
            return Err(AppError::Validation(format!("实验室名称已存在：{name}")));
        }
    }
    let before = serde_json::to_value(&current).unwrap_or(serde_json::Value::Null);
    let tx = conn.transaction()?;
    if let Some(ref name) = body.name {
        tx.execute(
            "UPDATE project_groups SET name=?1 WHERE id=?2",
            (name.trim(), id),
        )?;
    }
    if let Some(so) = body.sort_order {
        tx.execute(
            "UPDATE project_groups SET sort_order=?1 WHERE id=?2",
            (so, id),
        )?;
    }
    if let Some(v) = body.show_in_work {
        tx.execute(
            "UPDATE project_groups SET show_in_work=?1 WHERE id=?2",
            (v, id),
        )?;
    }
    if let Some(v) = body.show_in_rd {
        tx.execute(
            "UPDATE project_groups SET show_in_rd=?1 WHERE id=?2",
            (v, id),
        )?;
    }
    if let Some(did) = body.division_id {
        // did: Option<i64>；None(内层) 表示显式置空(未分配事业部)，Some(v) 表示指定事业部
        tx.execute(
            "UPDATE project_groups SET division_id=?1 WHERE id=?2",
            (did, id),
        )?;
    }
    let updated = get_by_id_on_conn(&tx, id)?;
    let after = serde_json::to_value(&updated).unwrap_or(serde_json::Value::Null);
    audit_repo::log_structured_on_conn(
        &tx,
        "update",
        "project_groups",
        Some(id),
        operator,
        &format!("编辑实验室「{}」", updated.name),
        "shared",
        "",
        Some(&before),
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(updated)
}

pub fn delete(pool: &DbPool, id: i64, operator: &str, reason: &str) -> Result<()> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let g = get_by_id_on_conn(&tx, id)?;
    let work_count: i64 = tx.query_row(
        "SELECT COUNT(*) FROM work_records WHERE group_id=?1",
        [id],
        |r| r.get(0),
    )?;
    let rd_count: i64 = tx.query_row(
        "SELECT COUNT(*) FROM rd_work_records WHERE group_id=?1",
        [id],
        |r| r.get(0),
    )?;
    let user_count: i64 = tx.query_row(
        "SELECT COUNT(*) FROM users WHERE group_id=?1 AND deleted_at IS NULL",
        [id],
        |r| r.get(0),
    )?;
    let before = serde_json::to_value(&g).unwrap_or(serde_json::Value::Null);
    tx.execute("UPDATE project_groups SET deleted_at=datetime('now','localtime') WHERE id=?1 AND deleted_at IS NULL", [id])?;
    let dependency = format!(
        "关联项目 {} 个；分析记录 {work_count} 条；送样记录 {rd_count} 条；用户 {user_count} 个",
        g.project_count
    );
    trash_repo::move_to_trash_on_conn(
        &tx,
        "实验室",
        "project_groups",
        id,
        "master",
        "shared",
        &g.name,
        "",
        &before,
        reason,
        operator,
        None,
        Some(id),
        &dependency,
        g.project_count + work_count + rd_count + user_count == 0,
    )?;
    let after = serde_json::json!({"deleted_at":"now","data":before});
    audit_repo::log_structured_on_conn(
        &tx,
        "delete",
        "project_groups",
        Some(id),
        operator,
        &format!("删除实验室「{}」；{}", g.name, dependency),
        "shared",
        "",
        Some(&before),
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(())
}
