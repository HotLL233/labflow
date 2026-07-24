use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::project::*;
use crate::repo::{audit_repo, trash_repo};

// ── helpers ──

fn parse_comma_i64(s: &str) -> Vec<i64> {
    if s.is_empty() {
        return vec![];
    }
    s.split(',').filter_map(|x| x.trim().parse().ok()).collect()
}

fn parse_comma_str(s: &str) -> Vec<String> {
    if s.is_empty() {
        return vec![];
    }
    s.split(',').map(|x| x.trim().to_string()).collect()
}

fn normalize_project_status(status: Option<&str>) -> &'static str {
    match status.unwrap_or("ongoing").trim() {
        "archived" => "archived",
        _ => "ongoing",
    }
}

const PROJ_SQL: &str =
    "SELECT p.id, p.name, COALESCE(p.notes,''), \
     COALESCE(p.full_name,''), p.sort_order, p.is_active, \
     COALESCE((SELECT string_agg(DISTINCT pll.group_id::text, ',') \
               FROM project_lab_links pll WHERE pll.project_id=p.id), '') as lab_ids_str, \
     COALESCE((SELECT string_agg(DISTINCT pg.name, ',') \
               FROM project_lab_links pll JOIN project_groups pg ON pg.id=pll.group_id \
               WHERE pll.project_id=p.id AND pg.name != '研发项目'), '') as lab_names_str, \
     COALESCE((SELECT string_agg(DISTINCT pml.method_id::text, ',') \
               FROM project_method_links pml WHERE pml.project_id=p.id), '') as method_ids_str, \
     COALESCE((SELECT string_agg(DISTINCT m.name || ' · ' || COALESCE(NULLIF(i.code,''),'待配置'), ',') \
               FROM project_method_links pml JOIN methods m ON m.id=pml.method_id \
               LEFT JOIN instruments i ON i.id=m.instrument_id WHERE pml.project_id=p.id), '') as method_names_str, \
     p.created_at, \
     p.high_item, \
     COALESCE(p.project_status, 'ongoing'), \
     p.archived_at, \
     p.archived_by \
     FROM projects p";

fn row_to_project(row: &postgres_compat::Row) -> postgres_compat::Result<ProjectResponse> {
    let lab_ids_str: String = row.get::<_, String>(6).unwrap_or_default();
    let lab_names_str: String = row.get::<_, String>(7).unwrap_or_default();
    let method_ids_str: String = row.get::<_, String>(8).unwrap_or_default();
    let method_names_str: String = row.get::<_, String>(9).unwrap_or_default();
    Ok(ProjectResponse {
        id: row.get(0)?,
        name: row.get(1)?,
        notes: row.get(2)?,
        full_name: row.get::<_, String>(3).unwrap_or_default(),
        sort_order: row.get::<_, i64>(4).unwrap_or(0),
        is_active: row.get::<_, i64>(5).unwrap_or(1) != 0,
        lab_ids: parse_comma_i64(&lab_ids_str),
        lab_names: parse_comma_str(&lab_names_str),
        method_ids: parse_comma_i64(&method_ids_str),
        method_names: parse_comma_str(&method_names_str),
        created_at: row.get(10)?,
        high_item: row.get::<_, Option<String>>(11).unwrap_or(None),
        project_status: row
            .get::<_, Option<String>>(12)?
            .unwrap_or_else(|| "ongoing".to_string()),
        archived_at: row.get::<_, Option<String>>(13).unwrap_or(None),
        archived_by: row.get::<_, Option<String>>(14).unwrap_or(None),
    })
}

// ── CRUD ──

pub fn list(
    pool: &DbPool,
    group_id: Option<i64>,
    active_only: bool,
    method_type: Option<&str>,
    status: Option<&str>,
) -> Result<Vec<ProjectResponse>> {
    let conn = pool.get()?;
    let mut sql = format!("{} WHERE p.deleted_at IS NULL", PROJ_SQL);
    let mut params: Vec<Box<dyn postgres_compat::types::ToSql>> = vec![];
    if active_only {
        sql.push_str(" AND p.is_active=1 AND COALESCE(p.project_status, 'ongoing')='ongoing'");
    }
    match status.unwrap_or("all") {
        "ongoing" => sql.push_str(" AND COALESCE(p.project_status, 'ongoing')='ongoing'"),
        "archived" => sql.push_str(" AND COALESCE(p.project_status, 'ongoing')='archived'"),
        _ => {}
    }
    if let Some(gid) = group_id {
        if gid > 0 {
            // Filter projects that have this lab link
            sql.push_str(
                " AND p.id IN (SELECT project_id FROM project_lab_links WHERE group_id=?)",
            );
            params.push(Box::new(gid));
        }
    }
    if let Some(mt) = method_type {
        let mt = mt.trim();
        if !mt.is_empty() {
            sql.push_str(
                " AND p.id IN (
                SELECT pml.project_id
                FROM project_method_links pml
                JOIN method_type_links mtl ON pml.method_id = mtl.method_id
                JOIN method_types mt ON mtl.method_type_id = mt.id
                WHERE mt.name = ?
            )",
            );
            params.push(Box::new(mt.to_string()));
        }
    }
    sql.push_str(" ORDER BY p.sort_order ASC, p.id ASC");
    let mut stmt = conn.prepare(&sql)?;
    let refs: Vec<&dyn postgres_compat::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let rows: Vec<ProjectResponse> = stmt
        .query_map(postgres_compat::params_from_iter(refs.iter()), |row| {
            row_to_project(row)
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn get_by_id(pool: &DbPool, id: i64) -> Result<ProjectResponse> {
    let conn = pool.get()?;
    get_by_id_on_conn(&conn, id)
}

fn get_by_id_on_conn(conn: &postgres_compat::Connection, id: i64) -> Result<ProjectResponse> {
    let sql = format!("{} WHERE p.id=?1", PROJ_SQL);
    conn.query_row(&sql, [id], |row| row_to_project(row))
        .map_err(|e| AppError::NotFound(format!("项目不存在: {}", e)))
}

pub fn create(pool: &DbPool, body: &ProjectCreate, user_name: &str) -> Result<ProjectResponse> {
    let mut conn = pool.get()?;
    let name = body.name.trim();
    if name.is_empty() {
        return Err(AppError::Validation("项目名称不能为空".into()));
    }
    let duplicate: i64 = conn.query_row(
        "SELECT COUNT(*) FROM projects WHERE name=?1",
        [name],
        |row| row.get(0),
    )?;
    if duplicate > 0 {
        return Err(AppError::Validation(format!("项目名称已存在：{name}")));
    }
    // 取第一个 lab_id 作为 group_id（项目必须关联至少一个实验室）
    let group_id = body
        .lab_ids
        .as_ref()
        .and_then(|ids| ids.first().copied())
        .ok_or_else(|| AppError::Validation("请至少选择一个实验室".into()))?;
    let nt = body.notes.as_deref().unwrap_or("");
    let fnm = body.full_name.as_deref().unwrap_or("");
    let so = body.sort_order.unwrap_or(0);
    let ia = body.is_active.unwrap_or(true) as i64;
    let status = normalize_project_status(body.project_status.as_deref());
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO projects (group_id, name, full_name, notes, sort_order, is_active, high_item, project_status, archived_at, archived_by) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, CASE WHEN ?8='archived' THEN datetime('now','localtime') ELSE NULL END, CASE WHEN ?8='archived' THEN ?9 ELSE NULL END)",
        postgres_compat::params![group_id, name, fnm, nt, so, ia, body.high_item, status, user_name],
    )?;
    let pid = tx.last_insert_rowid();

    // Insert lab links
    if let Some(ref lab_ids) = body.lab_ids {
        for lid in lab_ids {
            tx.execute(
                "INSERT OR IGNORE INTO project_lab_links (project_id, group_id) VALUES (?1,?2)",
                postgres_compat::params![pid, lid],
            )?;
        }
    }

    // Insert method links
    if let Some(ref method_ids) = body.method_ids {
        for mid in method_ids {
            tx.execute(
                "INSERT OR IGNORE INTO project_method_links (project_id, method_id) VALUES (?1,?2)",
                postgres_compat::params![pid, mid],
            )?;
        }
    }

    let created = get_by_id_on_conn(&tx, pid)?;
    let after = serde_json::to_value(&created).unwrap_or(serde_json::Value::Null);
    audit_repo::log_structured_on_conn(
        &tx,
        "create",
        "projects",
        Some(pid),
        user_name,
        &format!("创建项目「{}」", created.name),
        "shared",
        "",
        None,
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(created)
}

pub fn update(
    pool: &DbPool,
    id: i64,
    body: &ProjectUpdate,
    user_name: &str,
) -> Result<ProjectResponse> {
    let mut conn = pool.get()?;
    let current = get_by_id_on_conn(&conn, id)?;
    if let Some(ref n) = body.name {
        let name = n.trim();
        if name.is_empty() {
            return Err(AppError::Validation("项目名称不能为空".into()));
        }
        let duplicate: i64 = conn.query_row(
            "SELECT COUNT(*) FROM projects WHERE name=?1 AND id<>?2",
            postgres_compat::params![name, id],
            |row| row.get(0),
        )?;
        if duplicate > 0 {
            return Err(AppError::Validation(format!("项目名称已存在：{name}")));
        }
    }
    let before = serde_json::to_value(&current).unwrap_or(serde_json::Value::Null);
    let tx = conn.transaction()?;
    if let Some(ref n) = body.name {
        tx.execute("UPDATE projects SET name=?1 WHERE id=?2", (n.trim(), id))?;
    }
    if let Some(ref n) = body.full_name {
        tx.execute("UPDATE projects SET full_name=?1 WHERE id=?2", (n, id))?;
    }
    if let Some(ref n) = body.notes {
        tx.execute("UPDATE projects SET notes=?1 WHERE id=?2", (n, id))?;
    }
    if let Some(s) = body.sort_order {
        tx.execute("UPDATE projects SET sort_order=?1 WHERE id=?2", (s, id))?;
    }
    if let Some(a) = body.is_active {
        tx.execute(
            "UPDATE projects SET is_active=?1 WHERE id=?2",
            (a as i64, id),
        )?;
    }
    if let Some(ref s) = body.project_status {
        let status = normalize_project_status(Some(s));
        tx.execute(
            "UPDATE projects
             SET project_status=?1,
                 archived_at=CASE WHEN ?1='archived' THEN COALESCE(archived_at, datetime('now','localtime')) ELSE NULL END,
                 archived_by=CASE WHEN ?1='archived' THEN ?2 ELSE NULL END
             WHERE id=?3",
            postgres_compat::params![status, user_name, id],
        )?;
    }
    if let Some(ref hi) = body.high_item {
        let hi_trim = hi.trim().to_string();
        let val = if hi_trim.is_empty() {
            None
        } else {
            Some(hi_trim)
        };
        tx.execute(
            "UPDATE projects SET high_item=?1 WHERE id=?2",
            postgres_compat::params![val, id],
        )?;
    }
    // Replace lab links
    if let Some(ref lab_ids) = body.lab_ids {
        tx.execute("DELETE FROM project_lab_links WHERE project_id=?1", [id])?;
        for lid in lab_ids {
            tx.execute(
                "INSERT OR IGNORE INTO project_lab_links (project_id, group_id) VALUES (?1,?2)",
                postgres_compat::params![id, lid],
            )?;
        }
    }
    // Replace method links
    if let Some(ref method_ids) = body.method_ids {
        tx.execute("DELETE FROM project_method_links WHERE project_id=?1", [id])?;
        for mid in method_ids {
            tx.execute(
                "INSERT OR IGNORE INTO project_method_links (project_id, method_id) VALUES (?1,?2)",
                postgres_compat::params![id, mid],
            )?;
        }
    }
    let action = if body.project_status.as_deref() == Some("archived") {
        "归档项目"
    } else if body.project_status.as_deref() == Some("ongoing") {
        "重新启用项目"
    } else {
        "编辑项目"
    };
    let updated = get_by_id_on_conn(&tx, id)?;
    let after = serde_json::to_value(&updated).unwrap_or(serde_json::Value::Null);
    audit_repo::log_structured_on_conn(
        &tx,
        "update",
        "projects",
        Some(id),
        user_name,
        &format!("{}「{}」", action, updated.name),
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
    let wr_count: i64 = tx.query_row(
        "SELECT COUNT(*) FROM work_records WHERE project_id=?1",
        [id],
        |r| r.get(0),
    )?;
    let rd_count: i64 = tx.query_row(
        "SELECT COUNT(*) FROM rd_work_records WHERE project_id=?1",
        [id],
        |r| r.get(0),
    )?;
    let proj = get_by_id_on_conn(&tx, id)?;
    let before = serde_json::to_value(&proj).unwrap_or(serde_json::Value::Null);
    tx.execute("UPDATE projects SET deleted_at=datetime('now','localtime') WHERE id=?1 AND deleted_at IS NULL", [id])?;
    let dependency = format!("分析检测记录 {wr_count} 条；研发送样记录 {rd_count} 条");
    trash_repo::move_to_trash_on_conn(
        &tx,
        "项目",
        "projects",
        id,
        "master",
        "shared",
        &proj.name,
        "",
        &before,
        reason,
        operator,
        None,
        proj.lab_ids.first().copied(),
        &dependency,
        wr_count + rd_count == 0,
    )?;
    let after = serde_json::json!({"deleted_at":"now","data":before});
    audit_repo::log_structured_on_conn(
        &tx,
        "delete",
        "projects",
        Some(id),
        operator,
        &format!("删除项目「{}」；{}", proj.name, dependency),
        "shared",
        "",
        Some(&before),
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(())
}

// ── 批量系数 ──

pub fn batch_coefficient(
    pool: &DbPool,
    group_id: i64,
    coefficient: f64,
    operator: &str,
) -> Result<i64> {
    let mut conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT p.id,p.name,COALESCE(p.coefficient,1.0) FROM projects p
         JOIN project_lab_links link ON link.project_id=p.id WHERE link.group_id=?1 ORDER BY p.id",
    )?;
    let before_items = stmt
        .query_map([group_id], |row| {
            Ok(serde_json::json!({
                "id":row.get::<_,i64>(0)?,
                "name":row.get::<_,String>(1)?,
                "coefficient":row.get::<_,f64>(2)?,
            }))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(stmt);
    let before = serde_json::json!({"group_id":group_id,"projects":before_items});
    let tx = conn.transaction()?;
    // Update coefficient for projects linked to this group
    let count = tx.execute(
        "UPDATE projects SET coefficient=?1 WHERE id IN (SELECT project_id FROM project_lab_links WHERE group_id=?2)",
        postgres_compat::params![coefficient, group_id],
    )?;
    let after = serde_json::json!({
        "group_id":group_id,
        "coefficient":coefficient,
        "project_count":count,
    });
    audit_repo::log_structured_on_conn(
        &tx,
        "update",
        "projects",
        None,
        operator,
        &format!("批量修改实验室#{}的项目系数，共 {} 个项目", group_id, count),
        "shared",
        "",
        Some(&before),
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(count as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_hides_project_and_reenable_restores_it() {
        let pool = crate::db::init_pool("postgres-test");
        {
            let conn = pool.get().unwrap();
            crate::db::test_migrations::run(&conn).unwrap();
            conn.execute(
                "INSERT INTO project_groups(name,sort_order) VALUES ('实验室A',1)",
                [],
            )
            .unwrap();
        }
        let group_id = pool
            .get()
            .unwrap()
            .query_row(
                "SELECT id FROM project_groups WHERE name='实验室A'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        let created = create(
            &pool,
            &ProjectCreate {
                name: "项目A".into(),
                full_name: None,
                notes: None,
                sort_order: None,
                is_active: Some(true),
                lab_ids: Some(vec![group_id]),
                method_ids: Some(vec![]),
                high_item: None,
                project_status: Some("ongoing".into()),
            },
            "tester",
        )
        .unwrap();
        assert_eq!(list(&pool, None, true, None, None).unwrap().len(), 1);

        update(
            &pool,
            created.id,
            &ProjectUpdate {
                name: None,
                full_name: None,
                notes: None,
                sort_order: None,
                is_active: None,
                lab_ids: None,
                method_ids: None,
                high_item: None,
                project_status: Some("archived".into()),
            },
            "archiver",
        )
        .unwrap();
        assert!(list(&pool, None, true, None, None).unwrap().is_empty());
        let archived = list(&pool, None, false, None, Some("archived")).unwrap();
        assert_eq!(archived.len(), 1);
        assert_eq!(archived[0].archived_by.as_deref(), Some("archiver"));
        assert_eq!(archived[0].lab_ids, vec![group_id]);

        update(
            &pool,
            created.id,
            &ProjectUpdate {
                name: None,
                full_name: None,
                notes: None,
                sort_order: None,
                is_active: None,
                lab_ids: None,
                method_ids: None,
                high_item: None,
                project_status: Some("ongoing".into()),
            },
            "archiver",
        )
        .unwrap();
        assert_eq!(list(&pool, None, true, None, None).unwrap().len(), 1);
    }
}
