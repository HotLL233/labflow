use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::trash::{TrashEntry, TrashPrecheck};
use crate::repo::audit_repo;

#[derive(Debug, Clone, Copy)]
pub enum TrashScope {
    Global,
    Group(i64),
    User(i64),
}

#[allow(clippy::too_many_arguments)]
pub fn move_to_trash_on_conn(
    conn: &postgres_compat::Connection,
    entity_type: &str,
    table_name: &str,
    record_id: i64,
    category: &str,
    module: &str,
    display_name: &str,
    business_no: &str,
    snapshot: &serde_json::Value,
    reason: &str,
    operator: &str,
    owner_user_id: Option<i64>,
    owner_group_id: Option<i64>,
    dependency_summary: &str,
    can_purge: bool,
) -> Result<i64> {
    let actor_id: Option<i64> = conn
        .query_row(
            "SELECT id FROM users WHERE username=?1 ORDER BY id LIMIT 1",
            [operator],
            |row| row.get(0),
        )
        .ok();
    // The active-entry key is unique.  Use an atomic no-op on conflict so a
    // repeated delete or two concurrent delete requests cannot surface a raw
    // PostgreSQL unique-key error to the user.
    let inserted = conn.execute(
        "INSERT INTO trash_entries
         (entity_type,table_name,record_id,category,module,display_name,business_no,
          snapshot_json,delete_reason,deleted_by_user_id,deleted_by_username,
          owner_user_id,owner_group_id,dependency_summary,can_purge)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)
         ON CONFLICT (table_name,record_id)
         WHERE restored_at IS NULL AND purged_at IS NULL
         DO NOTHING",
        postgres_compat::params![
            entity_type,
            table_name,
            record_id,
            category,
            module,
            display_name,
            business_no,
            snapshot.to_string(),
            reason.trim(),
            actor_id,
            operator,
            owner_user_id,
            owner_group_id,
            dependency_summary,
            can_purge as i64,
        ],
    )?;
    if inserted > 0 {
        return Ok(conn.last_insert_rowid());
    }

    // A legacy or concurrent request may already have an active entry.  It is
    // already in the desired state, so return its idempotent result instead of
    // failing the surrounding deletion transaction.
    match conn.query_row(
        "SELECT id FROM trash_entries
         WHERE table_name=?1 AND record_id=?2 AND restored_at IS NULL AND purged_at IS NULL
         ORDER BY id DESC LIMIT 1",
        postgres_compat::params![table_name, record_id],
        |row| row.get(0),
    ) {
        Ok(id) => Ok(id),
        Err(postgres_compat::Error::QueryReturnedNoRows) => Err(AppError::Conflict(
            "回收站记录状态已发生变化，请刷新页面后重试".into(),
        )),
        Err(error) => Err(error.into()),
    }
}

fn map_entry(row: &postgres_compat::Row) -> postgres_compat::Result<TrashEntry> {
    let snapshot: String = row.get(8)?;
    Ok(TrashEntry {
        id: row.get(0)?,
        entity_type: row.get(1)?,
        table_name: row.get(2)?,
        record_id: row.get(3)?,
        category: row.get(4)?,
        module: row.get(5)?,
        display_name: row.get(6)?,
        business_no: row.get(7)?,
        snapshot: serde_json::from_str(&snapshot).unwrap_or(serde_json::Value::Null),
        delete_reason: row.get(9)?,
        deleted_by_user_id: row.get(10)?,
        deleted_by_username: row.get(11)?,
        owner_user_id: row.get(12)?,
        owner_group_id: row.get(13)?,
        dependency_summary: row.get(14)?,
        can_purge: row.get::<_, i64>(15)? != 0,
        deleted_at: row.get(16)?,
    })
}

const SELECT_ENTRY: &str =
    "SELECT id,entity_type,table_name,record_id,category,module,display_name,business_no,
            snapshot_json,delete_reason,deleted_by_user_id,deleted_by_username,
            owner_user_id,owner_group_id,dependency_summary,can_purge,deleted_at
     FROM trash_entries";

pub fn precheck(pool: &DbPool, table_name: &str, record_id: i64) -> Result<TrashPrecheck> {
    let conn = pool.get()?;
    let (entity_type, display_name, dependency_summary, can_purge) = match table_name {
        "projects" => {
            let name: String = conn.query_row(
                "SELECT name FROM projects WHERE id=?1 AND deleted_at IS NULL",
                [record_id],
                |row| row.get(0),
            )?;
            let work: i64 = conn.query_row(
                "SELECT COUNT(*) FROM work_records WHERE project_id=?1",
                [record_id],
                |row| row.get(0),
            )?;
            let rd: i64 = conn.query_row(
                "SELECT COUNT(*) FROM rd_work_records WHERE project_id=?1",
                [record_id],
                |row| row.get(0),
            )?;
            (
                "项目",
                name,
                format!("分析检测记录 {work} 条；研发送样记录 {rd} 条"),
                work + rd == 0,
            )
        }
        "project_groups" => {
            let name: String = conn.query_row(
                "SELECT name FROM project_groups WHERE id=?1 AND deleted_at IS NULL",
                [record_id],
                |row| row.get(0),
            )?;
            let projects: i64 = conn.query_row(
                "SELECT COUNT(*) FROM project_lab_links WHERE group_id=?1",
                [record_id],
                |row| row.get(0),
            )?;
            let work: i64 = conn.query_row(
                "SELECT COUNT(*) FROM work_records WHERE group_id=?1",
                [record_id],
                |row| row.get(0),
            )?;
            let rd: i64 = conn.query_row(
                "SELECT COUNT(*) FROM rd_work_records WHERE group_id=?1",
                [record_id],
                |row| row.get(0),
            )?;
            let users: i64 = conn.query_row(
                "SELECT COUNT(*) FROM users WHERE group_id=?1 AND deleted_at IS NULL",
                [record_id],
                |row| row.get(0),
            )?;
            (
                "实验室",
                name,
                format!(
                    "关联项目 {projects} 个；分析记录 {work} 条；送样记录 {rd} 条；用户 {users} 个"
                ),
                projects + work + rd + users == 0,
            )
        }
        "divisions" => {
            let name: String = conn.query_row(
                "SELECT name FROM divisions WHERE id=?1 AND deleted_at IS NULL",
                [record_id],
                |row| row.get(0),
            )?;
            let labs: i64 = conn.query_row(
                "SELECT COUNT(*) FROM project_groups WHERE division_id=?1",
                [record_id],
                |row| row.get(0),
            )?;
            let records: i64 = conn.query_row("SELECT (SELECT COUNT(*) FROM work_records WHERE division_id=?1)+(SELECT COUNT(*) FROM rd_work_records WHERE division_id=?1)+(SELECT COUNT(*) FROM sample_info_records WHERE division_id=?1)", [record_id], |row| row.get(0))?;
            let users: i64 = conn.query_row(
                "SELECT COUNT(*) FROM users WHERE division_id=?1 AND deleted_at IS NULL",
                [record_id],
                |row| row.get(0),
            )?;
            (
                "部门",
                name,
                format!("关联实验室 {labs} 个；历史记录 {records} 条；用户 {users} 个"),
                labs + records + users == 0,
            )
        }
        "methods" => {
            let display: String = conn.query_row("SELECT m.name || CASE WHEN COALESCE(i.code,'')='' THEN '' ELSE ' · ' || i.code END FROM methods m LEFT JOIN instruments i ON i.id=m.instrument_id WHERE m.id=?1 AND m.deleted_at IS NULL", [record_id], |row| row.get(0))?;
            let projects: i64 = conn.query_row(
                "SELECT COUNT(*) FROM project_method_links WHERE method_id=?1",
                [record_id],
                |row| row.get(0),
            )?;
            let work: i64 = conn.query_row(
                "SELECT COUNT(*) FROM work_records WHERE method_id=?1",
                [record_id],
                |row| row.get(0),
            )?;
            let rd: i64 = conn.query_row(
                "SELECT COUNT(*) FROM rd_work_records WHERE method_id=?1",
                [record_id],
                |row| row.get(0),
            )?;
            (
                "检测方法",
                display,
                format!("关联项目 {projects} 个；分析记录 {work} 条；送样记录 {rd} 条"),
                projects + work + rd == 0,
            )
        }
        "instruments" => {
            let code: String = conn.query_row(
                "SELECT code FROM instruments WHERE id=?1 AND deleted_at IS NULL",
                [record_id],
                |row| row.get(0),
            )?;
            let methods: i64 = conn.query_row(
                "SELECT COUNT(*) FROM methods WHERE instrument_id=?1",
                [record_id],
                |row| row.get(0),
            )?;
            // 记录快照同样对仪器有外键引用。漏算会让预检显示“可永久清理”，
            // 真正执行删除时撞外键报错，判定与实际约束不一致。
            let records: i64 = conn.query_row(
                "SELECT (SELECT COUNT(*) FROM work_records WHERE instrument_id_snapshot=?1)
                      + (SELECT COUNT(*) FROM rd_work_records WHERE instrument_id_snapshot=?1)",
                [record_id],
                |row| row.get(0),
            )?;
            (
                "仪器",
                code,
                format!("关联方法 {methods} 个；历史记录引用 {records} 条；删除后这些方法暂不可用"),
                methods == 0 && records == 0,
            )
        }
        "method_types" => {
            let name: String = conn.query_row(
                "SELECT name FROM method_types WHERE id=?1 AND deleted_at IS NULL",
                [record_id],
                |row| row.get(0),
            )?;
            let methods: i64 = conn.query_row(
                "SELECT COUNT(*) FROM method_type_links WHERE method_type_id=?1",
                [record_id],
                |row| row.get(0),
            )?;
            (
                "方法类型",
                name,
                format!("关联方法 {methods} 个"),
                methods == 0,
            )
        }
        "users" => {
            let username: String = conn.query_row(
                "SELECT username FROM users WHERE id=?1 AND deleted_at IS NULL",
                [record_id],
                |row| row.get(0),
            )?;
            (
                "用户",
                username,
                "用户ID和历史审计关联将在回收站永久清理时匿名化保留".into(),
                true,
            )
        }
        "roles" => {
            let name: String = conn.query_row(
                "SELECT name FROM roles WHERE id=?1 AND deleted_at IS NULL",
                [record_id],
                |row| row.get(0),
            )?;
            let users: i64 = conn.query_row("SELECT (SELECT COUNT(*) FROM users WHERE role_id=?1)+(SELECT COUNT(*) FROM user_roles WHERE role_id=?1)", [record_id], |row| row.get(0))?;
            ("角色", name, format!("关联用户 {users} 个"), users == 0)
        }
        "sample_info_types" => {
            let row: (String, String) = conn.query_row(
                "SELECT label,type_key FROM sample_info_types WHERE id=?1 AND deleted_at IS NULL",
                [record_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            let records: i64 = conn.query_row(
                "SELECT COUNT(*) FROM sample_info_records WHERE type_key=?1",
                [&row.1],
                |row| row.get(0),
            )?;
            (
                "样品检测类型",
                row.0,
                format!("关联样品记录 {records} 条"),
                records == 0,
            )
        }
        "sample_info_columns" => {
            let label: String = conn.query_row(
                "SELECT label FROM sample_info_columns WHERE id=?1 AND deleted_at IS NULL",
                [record_id],
                |row| row.get(0),
            )?;
            (
                "样品自定义字段",
                label,
                "字段值仍保存在历史记录扩展数据中".into(),
                true,
            )
        }
        "work_records" => {
            let business_no: String = conn.query_row(
                "SELECT business_no FROM work_records WHERE id=?1 AND deleted_at IS NULL",
                [record_id],
                |row| row.get(0),
            )?;
            (
                "分析检测记录",
                business_no,
                "历史审计与记录溯源信息将保留".into(),
                true,
            )
        }
        "rd_work_records" => {
            let business_no: String = conn.query_row(
                "SELECT business_no FROM rd_work_records WHERE id=?1 AND deleted_at IS NULL",
                [record_id],
                |row| row.get(0),
            )?;
            (
                "研发送样记录",
                business_no,
                "历史审计与记录溯源信息将保留".into(),
                true,
            )
        }
        "sample_info_records" => {
            let business_no: String = conn.query_row(
                "SELECT business_no FROM sample_info_records WHERE id=?1 AND deleted_at IS NULL",
                [record_id],
                |row| row.get(0),
            )?;
            let attachments: i64 = conn.query_row(
                "SELECT COUNT(*) FROM sample_info_attachments WHERE record_id=?1",
                [record_id],
                |row| row.get(0),
            )?;
            (
                "样品信息登记",
                business_no,
                format!("关联附件 {attachments} 个；附件及历史审计记录将保留"),
                true,
            )
        }
        "sample_info_attachments" => {
            let name: String = conn.query_row(
                "SELECT file_name FROM sample_info_attachments WHERE id=?1 AND deleted_at IS NULL",
                [record_id],
                |row| row.get(0),
            )?;
            ("样品附件", name, "原始文件保留至永久清理".into(), true)
        }
        "help_documents" => {
            let title: String = conn.query_row(
                "SELECT title FROM help_documents WHERE id=?1 AND deleted_at IS NULL",
                [record_id],
                |row| row.get(0),
            )?;
            ("帮助文档", title, "原始文件保留至永久清理".into(), true)
        }
        "help_articles" => {
            let title: String = conn.query_row(
                "SELECT title FROM help_articles WHERE id=?1 AND deleted_at IS NULL",
                [record_id],
                |row| row.get(0),
            )?;
            ("帮助文章", title, String::new(), true)
        }
        "personnel_change_feedbacks" => {
            let notice: String = conn.query_row("SELECT notice_no FROM personnel_change_feedbacks WHERE id=?1 AND deleted_at IS NULL", [record_id], |row| row.get(0))?;
            (
                "人员变动反馈",
                notice,
                "不影响用户、角色、主数据和统计".into(),
                true,
            )
        }
        _ => return Err(AppError::Validation("不支持该数据类型的删除预检".into())),
    };
    Ok(TrashPrecheck {
        entity_type: entity_type.into(),
        table_name: table_name.into(),
        record_id,
        display_name,
        dependency_summary,
        can_purge,
    })
}

pub fn list(
    pool: &DbPool,
    scope: TrashScope,
    category: Option<&str>,
    module: Option<&str>,
    keyword: Option<&str>,
    page: i64,
    page_size: i64,
) -> Result<(Vec<TrashEntry>, i64)> {
    let conn = pool.get()?;
    let mut clauses = vec![
        "restored_at IS NULL".to_string(),
        "purged_at IS NULL".to_string(),
    ];
    let mut params: Vec<Box<dyn postgres_compat::types::ToSql>> = Vec::new();
    match scope {
        TrashScope::Global => {}
        TrashScope::Group(id) => {
            params.push(Box::new(id));
            let n = params.len();
            clauses.push(format!(
                "(owner_group_id=?{n}
                  OR owner_user_id IN (SELECT id FROM users WHERE group_id=?{n})
                  OR deleted_by_user_id IN (SELECT id FROM users WHERE group_id=?{n}))"
            ));
        }
        TrashScope::User(id) => {
            params.push(Box::new(id));
            let n = params.len();
            clauses.push(format!("(owner_user_id=?{n} OR deleted_by_user_id=?{n})"));
        }
    }
    if let Some(value) = category.filter(|value| !value.trim().is_empty() && *value != "all") {
        params.push(Box::new(value.to_string()));
        clauses.push(format!("category=?{}", params.len()));
    }
    if let Some(value) = module.filter(|value| !value.trim().is_empty() && *value != "all") {
        params.push(Box::new(value.to_string()));
        clauses.push(format!("module=?{}", params.len()));
    }
    if let Some(value) = keyword.filter(|value| !value.trim().is_empty()) {
        params.push(Box::new(format!("%{}%", value.trim())));
        let n = params.len();
        clauses.push(format!("(display_name LIKE ?{n} OR business_no LIKE ?{n})"));
    }
    let where_sql = format!(" WHERE {}", clauses.join(" AND "));
    let refs: Vec<&dyn postgres_compat::types::ToSql> =
        params.iter().map(|value| value.as_ref()).collect();
    let total: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM trash_entries{where_sql}"),
        postgres_compat::params_from_iter(refs.iter()),
        |row| row.get(0),
    )?;
    let limit = page_size.clamp(1, 200);
    let offset = (page.max(1) - 1) * limit;
    let sql = format!(
        "{SELECT_ENTRY}{where_sql} ORDER BY deleted_at DESC,id DESC LIMIT {limit} OFFSET {offset}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(postgres_compat::params_from_iter(refs.iter()), map_entry)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok((rows, total))
}

pub fn get_active(pool: &DbPool, id: i64) -> Result<TrashEntry> {
    let conn = pool.get()?;
    conn.query_row(
        &format!("{SELECT_ENTRY} WHERE id=?1 AND restored_at IS NULL AND purged_at IS NULL"),
        [id],
        map_entry,
    )
    .map_err(|_| AppError::NotFound("回收站条目不存在或已处理".into()))
}

pub fn mark_restored_on_conn(
    conn: &postgres_compat::Connection,
    table_name: &str,
    record_id: i64,
    operator: &str,
) -> Result<()> {
    conn.execute(
        "UPDATE trash_entries SET restored_at=datetime('now','localtime'),
         restored_by_user_id=(SELECT id FROM users WHERE username=?1 ORDER BY id LIMIT 1),
         restored_by_username=?1
         WHERE table_name=?2 AND record_id=?3 AND restored_at IS NULL AND purged_at IS NULL",
        postgres_compat::params![operator, table_name, record_id],
    )?;
    Ok(())
}

pub fn mark_purged_on_conn(
    conn: &postgres_compat::Connection,
    table_name: &str,
    record_id: i64,
    actor_id: i64,
    actor_name: &str,
) -> Result<()> {
    conn.execute(
        "UPDATE trash_entries SET purged_at=datetime('now','localtime'),
         purged_by_user_id=?1,purged_by_username=?2
         WHERE table_name=?3 AND record_id=?4 AND restored_at IS NULL AND purged_at IS NULL",
        postgres_compat::params![actor_id, actor_name, table_name, record_id],
    )?;
    Ok(())
}

fn set_restored(conn: &postgres_compat::Connection, entry: &TrashEntry) -> Result<()> {
    let changed = match entry.table_name.as_str() {
        "users" => conn.execute(
            "UPDATE users SET deleted_at=NULL,is_active=1,updated_at=datetime('now','localtime') WHERE id=?1 AND deleted_at IS NOT NULL",
            [entry.record_id],
        )?,
        "divisions" | "sample_info_types" | "sample_info_columns" => conn.execute(
            &format!("UPDATE {} SET deleted_at=NULL,is_active=1 WHERE id=?1 AND deleted_at IS NOT NULL", entry.table_name),
            [entry.record_id],
        )?,
        "work_records" | "rd_work_records" | "sample_info_records" | "project_groups"
        | "projects" | "methods" | "instruments" | "method_types" | "roles"
        | "help_documents" | "help_articles" | "sample_info_attachments" | "personnel_change_feedbacks" => conn.execute(
            &format!("UPDATE {} SET deleted_at=NULL WHERE id=?1 AND deleted_at IS NOT NULL", entry.table_name),
            [entry.record_id],
        )?,
        "backup_files" => conn.execute(
            "UPDATE backup_file_trash SET deleted_at=NULL,restored_at=datetime('now','localtime')
             WHERE id=?1 AND deleted_at IS NOT NULL",
            [entry.record_id],
        )?,
        _ => return Err(AppError::Validation("不支持恢复该数据类型".into())),
    };
    if changed == 0 {
        return Err(AppError::Conflict("原始数据不存在或已经恢复".into()));
    }
    Ok(())
}

pub fn restore(pool: &DbPool, id: i64, actor_id: i64, actor_name: &str) -> Result<TrashEntry> {
    let entry = get_active(pool, id)?;
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    set_restored(&tx, &entry)?;
    tx.execute(
        "UPDATE trash_entries SET restored_at=datetime('now','localtime'),
         restored_by_user_id=?1,restored_by_username=?2 WHERE id=?3",
        postgres_compat::params![actor_id, actor_name, id],
    )?;
    let before = serde_json::json!({"deleted_at": entry.deleted_at, "data": entry.snapshot});
    let after = serde_json::json!({"deleted_at": null, "data": entry.snapshot});
    audit_repo::log_structured_actor_on_conn(
        &tx,
        "restore",
        &entry.table_name,
        Some(entry.record_id),
        actor_id,
        actor_name,
        &format!(
            "从回收站恢复{}「{}」",
            entry.entity_type, entry.display_name
        ),
        &entry.module,
        &entry.business_no,
        Some(&before),
        Some(&after),
        "recycle_bin",
    )?;
    tx.commit()?;
    Ok(entry)
}

fn hard_delete(conn: &postgres_compat::Connection, entry: &TrashEntry) -> Result<()> {
    if !entry.can_purge {
        return Err(AppError::Conflict(format!(
            "该数据仍有历史引用，不能物理删除：{}",
            entry.dependency_summary
        )));
    }
    match entry.table_name.as_str() {
        "users" => {
            let anonymous_name = format!(
                "deleted_user_{}_{}",
                entry.record_id,
                &uuid::Uuid::new_v4().simple().to_string()[..8]
            );
            let random_password =
                bcrypt::hash(uuid::Uuid::new_v4().to_string(), bcrypt::DEFAULT_COST)
                    .map_err(|error| AppError::Internal(format!("密码匿名化失败: {error}")))?;
            conn.execute(
                "DELETE FROM user_sessions WHERE user_id=?1",
                [entry.record_id],
            )?;
            conn.execute("DELETE FROM user_roles WHERE user_id=?1", [entry.record_id])?;
            conn.execute(
                "UPDATE users SET username=?1,password=?2,is_admin=0,is_active=0,division_id=NULL,
                 group_id=NULL,role_id=NULL,purged_at=datetime('now','localtime'),updated_at=datetime('now','localtime') WHERE id=?3",
                postgres_compat::params![anonymous_name, random_password, entry.record_id],
            )?;
        }
        "roles" => {
            conn.execute(
                "DELETE FROM role_permissions WHERE role_id=?1",
                [entry.record_id],
            )?;
            conn.execute("DELETE FROM roles WHERE id=?1", [entry.record_id])?;
        }
        "methods" => {
            conn.execute(
                "DELETE FROM method_type_links WHERE method_id=?1",
                [entry.record_id],
            )?;
            conn.execute("DELETE FROM methods WHERE id=?1", [entry.record_id])?;
        }
        "method_types" => {
            conn.execute(
                "DELETE FROM method_type_links WHERE method_type_id=?1",
                [entry.record_id],
            )?;
            conn.execute("DELETE FROM method_types WHERE id=?1", [entry.record_id])?;
        }
        "sample_info_columns" => {
            conn.execute(
                "DELETE FROM sample_info_column_visibility WHERE column_id=?1",
                [entry.record_id],
            )?;
            conn.execute(
                "DELETE FROM sample_info_columns WHERE id=?1",
                [entry.record_id],
            )?;
        }
        "sample_info_types" => {
            conn.execute("DELETE FROM sample_info_column_visibility WHERE type_key=(SELECT type_key FROM sample_info_types WHERE id=?1)", [entry.record_id])?;
            conn.execute(
                "DELETE FROM sample_info_types WHERE id=?1",
                [entry.record_id],
            )?;
        }
        "projects" => {
            conn.execute(
                "DELETE FROM project_lab_links WHERE project_id=?1",
                [entry.record_id],
            )?;
            conn.execute(
                "DELETE FROM project_method_links WHERE project_id=?1",
                [entry.record_id],
            )?;
            conn.execute("DELETE FROM projects WHERE id=?1", [entry.record_id])?;
        }
        "work_records"
        | "rd_work_records"
        | "sample_info_records"
        | "project_groups"
        | "instruments"
        | "divisions"
        | "help_documents"
        | "help_articles"
        | "sample_info_attachments"
        | "personnel_change_feedbacks" => {
            conn.execute(
                &format!("DELETE FROM {} WHERE id=?1", entry.table_name),
                [entry.record_id],
            )?;
        }
        "backup_files" => {
            conn.execute(
                "DELETE FROM backup_file_trash WHERE id=?1",
                [entry.record_id],
            )?;
        }
        _ => return Err(AppError::Validation("不支持永久清理该数据类型".into())),
    }
    Ok(())
}

pub fn purge(pool: &DbPool, id: i64, actor_id: i64, actor_name: &str) -> Result<TrashEntry> {
    let entry = get_active(pool, id)?;
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    hard_delete(&tx, &entry).map_err(|error| match error {
        AppError::Database(_) => {
            AppError::Conflict("存在关联数据，无法永久清理；历史数据仍安全保留".into())
        }
        other => other,
    })?;
    tx.execute(
        "UPDATE trash_entries SET purged_at=datetime('now','localtime'),
         purged_by_user_id=?1,purged_by_username=?2 WHERE id=?3",
        postgres_compat::params![actor_id, actor_name, id],
    )?;
    let after = serde_json::json!({"purged": true, "trash_entry_id": id});
    audit_repo::log_structured_actor_on_conn(
        &tx,
        "purge",
        &entry.table_name,
        Some(entry.record_id),
        actor_id,
        actor_name,
        &format!("永久清理{}「{}」", entry.entity_type, entry.display_name),
        &entry.module,
        &entry.business_no,
        Some(&entry.snapshot),
        Some(&after),
        "recycle_bin",
    )?;
    tx.commit()?;
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use super::move_to_trash_on_conn;
    use crate::db::test_migrations;
    use postgres_compat::Connection;

    #[test]
    fn repeated_delete_of_the_same_lab_is_idempotent() {
        let mut conn = Connection::open_test_database().expect("PostgreSQL test database");
        test_migrations::run(&conn).expect("PostgreSQL migrations");
        conn.execute(
            "INSERT INTO project_groups(name,sort_order,show_in_work,show_in_rd,division_id)
             VALUES('重复删除实验室',0,1,1,NULL)",
            [],
        )
        .expect("lab");
        let lab_id: i64 = conn
            .query_row(
                "SELECT id FROM project_groups WHERE name='重复删除实验室'",
                [],
                |row| row.get(0),
            )
            .expect("lab id");

        let tx = conn.transaction().expect("transaction");
        let first = move_to_trash_on_conn(
            &tx,
            "实验室",
            "project_groups",
            lab_id,
            "master",
            "shared",
            "重复删除实验室",
            "",
            &serde_json::json!({"id": lab_id, "name": "重复删除实验室"}),
            "回归测试",
            "test-user",
            None,
            None,
            "",
            true,
        )
        .expect("first delete");
        let second = move_to_trash_on_conn(
            &tx,
            "实验室",
            "project_groups",
            lab_id,
            "master",
            "shared",
            "重复删除实验室",
            "",
            &serde_json::json!({"id": lab_id, "name": "重复删除实验室"}),
            "重复删除",
            "test-user",
            None,
            None,
            "",
            true,
        )
        .expect("repeated delete");
        tx.commit().expect("commit");

        assert_eq!(first, second);
        let active_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM trash_entries
                 WHERE table_name='project_groups' AND record_id=?1
                   AND restored_at IS NULL AND purged_at IS NULL",
                [lab_id],
                |row| row.get(0),
            )
            .expect("active trash count");
        assert_eq!(active_count, 1);
        let active_entry: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM trash_entries
                 WHERE id=?1 AND restored_at IS NULL AND purged_at IS NULL",
                [first],
                |row| row.get(0),
            )
            .expect("active state");
        assert_eq!(active_entry, 1);
    }
}
