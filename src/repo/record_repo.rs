use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::record::{RecordCreate, RecordResponse, RecordUpdate};
use crate::repo::{audit_repo, trace_repo, trash_repo};

const SELECT_RECORD: &str = "SELECT wr.id, wr.business_no, wr.project_id, wr.method_id,
            COALESCE(NULLIF(wr.project_name_snapshot,''), p.name),
            COALESCE(NULLIF(wr.lab_name_snapshot,''), pg.name, '未知'),
            wr.user_name, wr.quantity, wr.multiplier, wr.recorded_at, wr.created_at,
            wr.deleted_at,
            COALESCE(NULLIF(wr.method_name_snapshot,''), NULLIF(m.full_name,''), NULLIF(m.name,'')),
            (SELECT string_agg(DISTINCT mt.name, ',')
             FROM method_type_links mtl JOIN method_types mt ON mtl.method_type_id=mt.id
             WHERE mtl.method_id=wr.method_id),
            COALESCE(NULLIF(wr.instrument_code_snapshot,''), i.code, ''),
            COALESCE(NULLIF(wr.instrument_type_snapshot,''), i.instrument_type, ''),
            COALESCE(NULLIF(wr.high_item_snapshot,''), wr.high_item, p.high_item),
            COALESCE(wr.coefficient_snapshot,1.0), wr.subject_user_id, wr.created_by_user_id
     FROM work_records wr
     JOIN projects p ON wr.project_id=p.id
     LEFT JOIN methods m ON wr.method_id=m.id
     LEFT JOIN instruments i ON i.id=m.instrument_id
     LEFT JOIN project_groups pg ON pg.id=wr.group_id";

fn map_record(row: &postgres_compat::Row<'_>) -> postgres_compat::Result<RecordResponse> {
    Ok(RecordResponse {
        id: row.get(0)?,
        business_no: row.get::<_, String>(1).unwrap_or_default(),
        project_id: row.get(2)?,
        method_id: row.get(3)?,
        project_name: row.get(4)?,
        group_name: row.get(5)?,
        user_name: row.get(6)?,
        quantity: row.get(7)?,
        multiplier: row.get::<_, f64>(8).unwrap_or(1.0),
        recorded_at: row.get(9)?,
        created_at: row.get(10)?,
        deleted_at: row.get(11)?,
        method_name: row.get(12)?,
        method_type: row.get(13)?,
        instrument_code: row.get(14)?,
        instrument_type: row.get(15)?,
        high_item: row.get(16)?,
        coefficient_snapshot: row.get::<_, f64>(17).unwrap_or(1.0),
        subject_user_id: row.get(18)?,
        created_by_user_id: row.get(19)?,
    })
}

fn snapshot(record: &RecordResponse) -> serde_json::Value {
    serde_json::json!({
        "business_no": record.business_no, "project_id": record.project_id,
        "project_name": record.project_name, "method_id": record.method_id,
        "method_name": record.method_name, "instrument_code": record.instrument_code,
        "instrument_type": record.instrument_type, "lab_name": record.group_name,
        "user_name": record.user_name, "quantity": record.quantity,
        "multiplier": record.multiplier,
        "recorded_at": record.recorded_at, "high_item": record.high_item,
        "coefficient_snapshot": record.coefficient_snapshot, "deleted_at": record.deleted_at,
    })
}

fn structured_log(
    conn: &postgres_compat::Connection,
    action: &str,
    record: &RecordResponse,
    operator: &str,
    detail: &str,
    before: Option<&serde_json::Value>,
    after: Option<&serde_json::Value>,
) -> Result<()> {
    audit_repo::log_structured_on_conn(
        conn,
        action,
        "work_records",
        Some(record.id),
        operator,
        detail,
        "work",
        &record.business_no,
        before,
        after,
        "record",
    )?;
    trace_repo::log_event_on_conn(
        conn,
        "work",
        "work_records",
        record.id,
        &record.business_no,
        action,
        None,
        None,
        operator,
        detail,
        before,
        after,
    )
}

pub(crate) fn validate_record_bindings(
    conn: &postgres_compat::Connection,
    project_id: i64,
    method_id: Option<i64>,
    group_id: Option<i64>,
) -> Result<()> {
    let project_exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM projects WHERE id=?1 AND is_active=1 AND COALESCE(project_status,'ongoing')='ongoing'",
        [project_id],
        |row| row.get(0),
    )?;
    if project_exists == 0 {
        return Err(AppError::Validation("项目不存在或已归档".into()));
    }

    if let Some(method_id) = method_id {
        let valid_method: i64 = conn.query_row(
            "SELECT COUNT(*)
             FROM methods m
             JOIN instruments i ON i.id=m.instrument_id
             JOIN project_method_links pml ON pml.method_id=m.id
             WHERE m.id=?1 AND pml.project_id=?2 AND m.is_active=1 AND i.is_active=1",
            postgres_compat::params![method_id, project_id],
            |row| row.get(0),
        )?;
        if valid_method == 0 {
            return Err(AppError::Validation(
                "检测方法不存在、未关联当前项目，或绑定仪器已停用".into(),
            ));
        }
    }

    if let Some(group_id) = group_id {
        let valid_lab: i64 = conn.query_row(
            "SELECT COUNT(*) FROM project_lab_links WHERE project_id=?1 AND group_id=?2",
            postgres_compat::params![project_id, group_id],
            |row| row.get(0),
        )?;
        if valid_lab == 0 {
            return Err(AppError::Validation("所选实验室未关联当前项目".into()));
        }
    }
    Ok(())
}

pub fn list(
    pool: &DbPool,
    project_id: Option<i64>,
    group_id: Option<i64>,
    subject_user_id: Option<i64>,
    user_name: Option<&str>,
    division_id: Option<i64>,
    start: Option<&str>,
    end: Option<&str>,
    page: i64,
    page_size: i64,
    include_deleted: bool,
) -> Result<(Vec<RecordResponse>, i64)> {
    let conn = pool.get()?;
    let mut clauses = vec![];
    let mut params: Vec<Box<dyn postgres_compat::types::ToSql>> = vec![];
    if !include_deleted {
        clauses.push("wr.deleted_at IS NULL".to_string());
    }
    if let Some(value) = project_id {
        clauses.push(format!("wr.project_id={value}"));
    }
    if let Some(value) = group_id {
        clauses.push(format!("wr.group_id={value}"));
    }
    if let Some(value) = division_id {
        clauses.push(format!("wr.division_id={value}"));
    }
    if let Some(value) = subject_user_id {
        clauses.push(format!("wr.subject_user_id={value}"));
    }
    if let Some(value) = user_name {
        let i = params.len() + 1;
        clauses.push(format!("wr.user_name=?{i}"));
        params.push(Box::new(value.to_string()));
    }
    if let Some(value) = start {
        let i = params.len() + 1;
        clauses.push(format!("wr.recorded_at>=?{i}"));
        params.push(Box::new(value.to_string()));
    }
    if let Some(value) = end {
        let i = params.len() + 1;
        clauses.push(format!("wr.recorded_at<=?{i}"));
        params.push(Box::new(format!("{value}T23:59:59")));
    }
    let where_sql = if clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", clauses.join(" AND "))
    };
    let sql = format!(
        "{} {} ORDER BY wr.recorded_at DESC LIMIT {} OFFSET {}",
        SELECT_RECORD,
        where_sql,
        page_size,
        (page - 1) * page_size
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        postgres_compat::params_from_iter(params.iter().map(|p| p.as_ref())),
        map_record,
    )?;
    let items = rows.collect::<std::result::Result<Vec<_>, _>>()?;
    let count: i64 = conn.query_row(
        &format!(
            "SELECT COUNT(*) FROM work_records wr JOIN projects p ON p.id=wr.project_id {}",
            where_sql
        ),
        postgres_compat::params_from_iter(params.iter().map(|p| p.as_ref())),
        |row| row.get(0),
    )?;
    Ok((items, count))
}

fn get_by_id_on_conn(conn: &postgres_compat::Connection, id: i64) -> Result<RecordResponse> {
    conn.query_row(
        &format!("{} WHERE wr.id=?1", SELECT_RECORD),
        [id],
        map_record,
    )
    .map_err(|error| match error {
        postgres_compat::Error::QueryReturnedNoRows => AppError::NotFound("记录不存在".into()),
        _ => error.into(),
    })
}

pub fn get_by_id(pool: &DbPool, id: i64) -> Result<RecordResponse> {
    let conn = pool.get()?;
    get_by_id_on_conn(&conn, id)
}

pub fn create(pool: &DbPool, body: &RecordCreate, operator: &str) -> Result<RecordResponse> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    validate_record_bindings(&tx, body.project_id, body.method_id, body.group_id)?;
    let inserted=tx.execute(
        "INSERT INTO work_records
         (project_id,method_id,user_name,quantity,recorded_at,group_id,division_id,multiplier,high_item,
           project_name_snapshot,lab_name_snapshot,method_name_snapshot,instrument_code_snapshot,instrument_type_snapshot,high_item_snapshot,coefficient_snapshot,
           subject_user_id,created_by_user_id,created_by_username_snapshot,instrument_id_snapshot)
         SELECT ?1,?2,?3,?4,?5,?6,?7,
                COALESCE(?8,(SELECT multiplier FROM methods WHERE id=?2),1.0),
                COALESCE(?9,p.high_item),p.name,COALESCE(pg.name,''),
                COALESCE((SELECT COALESCE(NULLIF(full_name,''),name) FROM methods WHERE id=?2),''),
                COALESCE((SELECT i.code FROM methods m JOIN instruments i ON i.id=m.instrument_id WHERE m.id=?2),''),
                COALESCE((SELECT i.instrument_type FROM methods m JOIN instruments i ON i.id=m.instrument_id WHERE m.id=?2),''),
                 COALESCE(?9,p.high_item,''),COALESCE(p.coefficient,1.0),
                 (SELECT id FROM users WHERE username=?3 ORDER BY id LIMIT 1),
                 (SELECT id FROM users WHERE username=?10 ORDER BY id LIMIT 1),?10,
                 (SELECT instrument_id FROM methods WHERE id=?2)
         FROM projects p LEFT JOIN project_groups pg ON pg.id=?6 WHERE p.id=?1",
        postgres_compat::params![body.project_id,body.method_id,&body.user_name,body.quantity,&body.recorded_at,
            body.group_id,body.division_id,body.multiplier,body.high_item,operator],
    )?;
    if inserted == 0 {
        return Err(AppError::Validation("项目不存在".into()));
    }
    let id = tx.last_insert_rowid();
    let business_no = trace_repo::make_business_no("WK", &body.recorded_at, id);
    tx.execute(
        "UPDATE work_records SET business_no=?1 WHERE id=?2",
        postgres_compat::params![business_no, id],
    )?;
    let record = get_by_id_on_conn(&tx, id)?;
    let after = snapshot(&record);
    let detail = format!(
        "创建分析检测记录 {}：项目「{}」/ 方法「{}」，数量 {}",
        record.business_no,
        record.project_name,
        record.method_name.as_deref().unwrap_or("未知"),
        record.quantity
    );
    structured_log(
        &tx,
        "create",
        &record,
        operator,
        &detail,
        None,
        Some(&after),
    )?;
    tx.commit()?;
    get_by_id_on_conn(&conn, id)
}

pub fn update(
    pool: &DbPool,
    id: i64,
    body: &RecordUpdate,
    operator: &str,
) -> Result<RecordResponse> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let existing = get_by_id_on_conn(&tx, id)?;
    if existing.deleted_at.is_some() {
        return Err(AppError::Validation("记录已被删除，无法编辑".into()));
    }
    let before = snapshot(&existing);
    let next_project_id = body.project_id.unwrap_or(existing.project_id);
    let next_method_id = body.method_id.or(existing.method_id);
    let next_group_id = body.group_id.or_else(|| {
        tx.query_row(
            "SELECT group_id FROM work_records WHERE id=?1",
            [id],
            |row| row.get(0),
        )
        .ok()
        .flatten()
    });
    validate_record_bindings(&tx, next_project_id, next_method_id, next_group_id)?;
    let mut changes = Vec::new();
    if let Some(value) = &body.user_name {
        if value != &existing.user_name {
            changes.push(format!("人员 {} → {}", existing.user_name, value));
            tx.execute(
                "UPDATE work_records SET user_name=?1,
                 subject_user_id=(SELECT id FROM users WHERE username=?1 ORDER BY id LIMIT 1)
                 WHERE id=?2",
                postgres_compat::params![value, id],
            )?;
        }
    }
    if let Some(value) = body.quantity {
        if value != existing.quantity {
            changes.push(format!("数量 {} → {}", existing.quantity, value));
            tx.execute(
                "UPDATE work_records SET quantity=?1 WHERE id=?2",
                postgres_compat::params![value, id],
            )?;
        }
    }
    if let Some(value) = &body.recorded_at {
        if value != &existing.recorded_at {
            changes.push(format!("日期 {} → {}", existing.recorded_at, value));
            tx.execute(
                "UPDATE work_records SET recorded_at=?1 WHERE id=?2",
                postgres_compat::params![value, id],
            )?;
        }
    }
    if let Some(value) = body.multiplier {
        if (value - existing.multiplier).abs() > f64::EPSILON {
            changes.push(format!(
                "单价倍率 {:.2} → {:.2}",
                existing.multiplier, value
            ));
            tx.execute(
                "UPDATE work_records SET multiplier=?1 WHERE id=?2",
                postgres_compat::params![value, id],
            )?;
        }
    }
    if let Some(value) = body.project_id {
        if value != existing.project_id {
            changes.push(format!("项目ID {} → {}", existing.project_id, value));
            tx.execute("UPDATE work_records SET project_id=?1,project_name_snapshot=(SELECT name FROM projects WHERE id=?1),high_item_snapshot=COALESCE((SELECT high_item FROM projects WHERE id=?1),''),coefficient_snapshot=COALESCE((SELECT coefficient FROM projects WHERE id=?1),1.0) WHERE id=?2",postgres_compat::params![value,id])?;
        }
    }
    if let Some(value) = body.method_id {
        if Some(value) != existing.method_id {
            changes.push(format!("方法ID {:?} → {}", existing.method_id, value));
            tx.execute("UPDATE work_records SET method_id=?1,method_name_snapshot=COALESCE((SELECT COALESCE(NULLIF(full_name,''),name) FROM methods WHERE id=?1),''),instrument_id_snapshot=(SELECT instrument_id FROM methods WHERE id=?1),instrument_code_snapshot=COALESCE((SELECT i.code FROM methods m JOIN instruments i ON i.id=m.instrument_id WHERE m.id=?1),''),instrument_type_snapshot=COALESCE((SELECT i.instrument_type FROM methods m JOIN instruments i ON i.id=m.instrument_id WHERE m.id=?1),'') WHERE id=?2",postgres_compat::params![value,id])?;
        }
    }
    if let Some(value) = body.group_id {
        changes.push(format!("实验室更新为ID {}", value));
        tx.execute("UPDATE work_records SET group_id=?1,lab_name_snapshot=COALESCE((SELECT name FROM project_groups WHERE id=?1),'') WHERE id=?2",postgres_compat::params![value,id])?;
    }
    if let Some(value) = body.division_id {
        changes.push(format!("部门更新为ID {}", value));
        tx.execute(
            "UPDATE work_records SET division_id=?1 WHERE id=?2",
            postgres_compat::params![value, id],
        )?;
    }
    if let Some(value) = &body.high_item {
        let normalized = if value.trim().is_empty() {
            None
        } else {
            Some(value.trim())
        };
        if normalized != existing.high_item.as_deref() {
            changes.push(format!("高项 {:?} → {:?}", existing.high_item, normalized));
            tx.execute("UPDATE work_records SET high_item=?1,high_item_snapshot=COALESCE(?1,'') WHERE id=?2",postgres_compat::params![normalized,id])?;
        }
    }
    if changes.is_empty() {
        return Err(AppError::Validation("没有需要更新的字段".into()));
    }
    tx.execute(
        "UPDATE work_records SET updated_at=datetime('now','localtime') WHERE id=?1",
        [id],
    )?;
    let updated = get_by_id_on_conn(&tx, id)?;
    let after = snapshot(&updated);
    let detail = format!("修改 {}：{}", updated.business_no, changes.join("，"));
    structured_log(
        &tx,
        "update",
        &updated,
        operator,
        &detail,
        Some(&before),
        Some(&after),
    )?;
    tx.commit()?;
    get_by_id_on_conn(&conn, id)
}

pub fn soft_delete(pool: &DbPool, id: i64, operator: &str, reason: &str) -> Result<()> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let existing = get_by_id_on_conn(&tx, id)?;
    if existing.deleted_at.is_some() {
        return Err(AppError::Validation("记录已被删除".into()));
    }
    let before = snapshot(&existing);
    tx.execute("UPDATE work_records SET deleted_at=datetime('now','localtime'),updated_at=datetime('now','localtime') WHERE id=?1",[id])?;
    let updated = get_by_id_on_conn(&tx, id)?;
    let after = snapshot(&updated);
    let owner_group_id: Option<i64> = tx
        .query_row(
            "SELECT group_id FROM work_records WHERE id=?1",
            [id],
            |row| row.get(0),
        )
        .ok()
        .flatten();
    trash_repo::move_to_trash_on_conn(
        &tx,
        "分析检测记录",
        "work_records",
        id,
        "records",
        "work",
        &updated.business_no,
        &updated.business_no,
        &before,
        reason,
        operator,
        updated.subject_user_id.or(updated.created_by_user_id),
        owner_group_id,
        "",
        true,
    )?;
    let detail = format!("删除分析检测记录 {}", updated.business_no);
    structured_log(
        &tx,
        "delete",
        &updated,
        operator,
        &detail,
        Some(&before),
        Some(&after),
    )?;
    tx.commit()?;
    Ok(())
}

pub fn restore(pool: &DbPool, id: i64, operator: &str) -> Result<RecordResponse> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let existing = get_by_id_on_conn(&tx, id)?;
    if existing.deleted_at.is_none() {
        return Err(AppError::Validation("记录未被删除，无需恢复".into()));
    }
    let before = snapshot(&existing);
    tx.execute("UPDATE work_records SET deleted_at=NULL,updated_at=datetime('now','localtime') WHERE id=?1",[id])?;
    trash_repo::mark_restored_on_conn(&tx, "work_records", id, operator)?;
    let updated = get_by_id_on_conn(&tx, id)?;
    let after = snapshot(&updated);
    let detail = format!("恢复分析检测记录 {}", updated.business_no);
    structured_log(
        &tx,
        "restore",
        &updated,
        operator,
        &detail,
        Some(&before),
        Some(&after),
    )?;
    tx.commit()?;
    get_by_id_on_conn(&conn, id)
}

pub fn delete_by_user(
    pool: &DbPool,
    user_name: &str,
    start: Option<&str>,
    end: Option<&str>,
    operator: &str,
    reason: &str,
) -> Result<i64> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let mut select =
        "SELECT id FROM work_records WHERE user_name=?1 AND deleted_at IS NULL".to_string();
    let mut params: Vec<Box<dyn postgres_compat::types::ToSql>> =
        vec![Box::new(user_name.to_string())];
    if let Some(value) = start {
        let i = params.len() + 1;
        select.push_str(&format!(" AND recorded_at>=?{i}"));
        params.push(Box::new(value.to_string()));
    }
    if let Some(value) = end {
        let i = params.len() + 1;
        select.push_str(&format!(" AND recorded_at<=?{i}"));
        params.push(Box::new(format!("{value}T23:59:59")));
    }
    let ids: Vec<i64> = {
        let mut stmt = tx.prepare(&select)?;
        let rows = stmt.query_map(
            postgres_compat::params_from_iter(params.iter().map(|p| p.as_ref())),
            |r| r.get(0),
        )?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };
    for id in &ids {
        let before_rec = get_by_id_on_conn(&tx, *id)?;
        let before = snapshot(&before_rec);
        tx.execute("UPDATE work_records SET deleted_at=datetime('now','localtime'),updated_at=datetime('now','localtime') WHERE id=?1",[*id])?;
        let after_rec = get_by_id_on_conn(&tx, *id)?;
        let after = snapshot(&after_rec);
        let owner_group_id: Option<i64> = tx
            .query_row(
                "SELECT group_id FROM work_records WHERE id=?1",
                [*id],
                |row| row.get(0),
            )
            .ok()
            .flatten();
        trash_repo::move_to_trash_on_conn(
            &tx,
            "分析检测记录",
            "work_records",
            *id,
            "records",
            "work",
            &after_rec.business_no,
            &after_rec.business_no,
            &before,
            reason,
            operator,
            after_rec.subject_user_id.or(after_rec.created_by_user_id),
            owner_group_id,
            "",
            true,
        )?;
        structured_log(
            &tx,
            "delete",
            &after_rec,
            operator,
            "批量删除分析检测记录",
            Some(&before),
            Some(&after),
        )?;
    }
    tx.commit()?;
    Ok(ids.len() as i64)
}

pub fn soft_delete_range(
    pool: &DbPool,
    start: &str,
    end: &str,
    operator: &str,
    reason: &str,
) -> Result<i64> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let ids: Vec<i64> = {
        let mut stmt = tx.prepare(
            "SELECT id FROM work_records
             WHERE deleted_at IS NULL AND recorded_at>=?1 AND recorded_at<=?2
             ORDER BY id",
        )?;
        let rows = stmt.query_map(postgres_compat::params![start, end], |row| row.get(0))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };
    for id in &ids {
        let before_record = get_by_id_on_conn(&tx, *id)?;
        let before = snapshot(&before_record);
        tx.execute(
            "UPDATE work_records SET deleted_at=datetime('now','localtime'),updated_at=datetime('now','localtime') WHERE id=?1",
            [*id],
        )?;
        let after_record = get_by_id_on_conn(&tx, *id)?;
        let after = snapshot(&after_record);
        let owner_group_id: Option<i64> = tx
            .query_row(
                "SELECT group_id FROM work_records WHERE id=?1",
                [*id],
                |row| row.get(0),
            )
            .ok()
            .flatten();
        trash_repo::move_to_trash_on_conn(
            &tx,
            "分析检测记录",
            "work_records",
            *id,
            "records",
            "work",
            &after_record.business_no,
            &after_record.business_no,
            &before,
            reason,
            operator,
            after_record
                .subject_user_id
                .or(after_record.created_by_user_id),
            owner_group_id,
            "",
            true,
        )?;
        structured_log(
            &tx,
            "delete",
            &after_record,
            operator,
            &format!("数据治理批量移入回收站：{}", after_record.business_no),
            Some(&before),
            Some(&after),
        )?;
    }
    tx.commit()?;
    Ok(ids.len() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> (DbPool, i64, i64, i64) {
        let pool = crate::db::init_pool("postgres-test");
        crate::db::test_migrations::run(&pool.get().unwrap()).unwrap();
        let conn = pool.get().unwrap();
        conn.execute("INSERT INTO project_groups(name) VALUES('Lab01')", [])
            .unwrap();
        let group = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO projects(group_id,name,coefficient) VALUES(?1,'Project01',2.0)",
            [group],
        )
        .unwrap();
        let project = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO instruments(code,name,instrument_type) VALUES('LC-01','Liquid01','液相')",
            [],
        )
        .unwrap();
        let instrument = conn.last_insert_rowid();
        conn.execute("INSERT INTO methods(method_code,name,full_name,coefficient,instrument_id) VALUES('M-LC01-001','Method01','Method01 Full',1.5,?1)",[instrument]).unwrap();
        let method = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO project_lab_links(project_id,group_id) VALUES(?1,?2)",
            postgres_compat::params![project, group],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO project_method_links(project_id,method_id) VALUES(?1,?2)",
            postgres_compat::params![project, method],
        )
        .unwrap();
        drop(conn);
        (pool, group, project, method)
    }
    #[test]
    fn score_snapshot_and_trace_survive_master_data_change() {
        let (pool, group, project, method) = setup();
        let record = create(
            &pool,
            &RecordCreate {
                project_id: project,
                method_id: Some(method),
                user_name: "detector01".into(),
                quantity: 3,
                recorded_at: "2026-07-18T09:00:00".into(),
                group_id: Some(group),
                multiplier: None,
                high_item: Some("High01".into()),
                division_id: None,
            },
            "admin",
        )
        .unwrap();
        assert!(record.business_no.starts_with("WK-20260718-"));
        assert_eq!(record.coefficient_snapshot, 2.0);
        assert_eq!(record.instrument_code, "LC-01");
        assert_eq!(record.instrument_type, "液相");
        pool.get()
            .unwrap()
            .execute(
                "UPDATE projects SET coefficient=9.0,name='Changed' WHERE id=?1",
                [project],
            )
            .unwrap();
        pool.get().unwrap().execute("UPDATE instruments SET code='LC-RENAMED',instrument_type='其他' WHERE code='LC-01'",[]).unwrap();
        let reread = get_by_id(&pool, record.id).unwrap();
        assert_eq!(reread.coefficient_snapshot, 2.0);
        assert_eq!(reread.project_name, "Project01");
        assert_eq!(reread.instrument_code, "LC-01");
        assert_eq!(reread.instrument_type, "液相");
        let events = crate::repo::trace_repo::list(&pool, "work_records", record.id).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "create");
    }

    #[test]
    fn update_delete_restore_builds_structured_audit_timeline() {
        let (pool, group, project, method) = setup();
        let record = create(
            &pool,
            &RecordCreate {
                project_id: project,
                method_id: Some(method),
                user_name: "detector01".into(),
                quantity: 3,
                recorded_at: "2026-07-18T09:00:00".into(),
                group_id: Some(group),
                multiplier: None,
                high_item: None,
                division_id: None,
            },
            "creator01",
        )
        .unwrap();
        update(
            &pool,
            record.id,
            &RecordUpdate {
                user_name: None,
                quantity: Some(5),
                recorded_at: None,
                multiplier: None,
                project_id: None,
                method_id: None,
                group_id: None,
                division_id: None,
                batch_no: None,
                notes: None,
                high_item: None,
            },
            "editor01",
        )
        .unwrap();
        soft_delete(&pool, record.id, "deleter01", "测试删除").unwrap();
        restore(&pool, record.id, "restorer01").unwrap();

        let events = crate::repo::trace_repo::list(&pool, "work_records", record.id).unwrap();
        assert_eq!(
            events
                .iter()
                .map(|event| event.event_type.as_str())
                .collect::<Vec<_>>(),
            vec!["create", "update", "delete", "restore"]
        );
        assert_eq!(events[1].operator, "editor01");
        assert_eq!(
            events[1]
                .before_data
                .as_ref()
                .and_then(|v| v.get("quantity"))
                .and_then(|v| v.as_i64()),
            Some(3)
        );
        assert_eq!(
            events[1]
                .after_data
                .as_ref()
                .and_then(|v| v.get("quantity"))
                .and_then(|v| v.as_i64()),
            Some(5)
        );

        let (audits, total) = crate::repo::audit_repo::list(
            &pool,
            1,
            20,
            Some("work"),
            None,
            None,
            Some(&record.business_no),
        )
        .unwrap();
        assert_eq!(total, 4);
        assert!(audits
            .iter()
            .all(|audit| audit.before_data.is_some() || audit.action == "create"));

        crate::db::test_migrations::run(&pool.get().unwrap()).unwrap();
        let events_after_rerun =
            crate::repo::trace_repo::list(&pool, "work_records", record.id).unwrap();
        assert_eq!(events_after_rerun.len(), 4);
    }

    #[test]
    fn record_multiplier_uses_method_default_and_preserves_zero() {
        let (pool, group, project, method) = setup();
        pool.get()
            .unwrap()
            .execute("UPDATE methods SET multiplier=1.6 WHERE id=?1", [method])
            .unwrap();
        let default_multiplier = create(
            &pool,
            &RecordCreate {
                project_id: project,
                method_id: Some(method),
                user_name: "detector01".into(),
                quantity: 1,
                recorded_at: "2026-07-18T09:00:00".into(),
                group_id: Some(group),
                multiplier: None,
                high_item: None,
                division_id: None,
            },
            "creator01",
        )
        .unwrap();
        let zero_multiplier = create(
            &pool,
            &RecordCreate {
                project_id: project,
                method_id: Some(method),
                user_name: "detector01".into(),
                quantity: 1,
                recorded_at: "2026-07-18T10:00:00".into(),
                group_id: Some(group),
                multiplier: Some(0.0),
                high_item: None,
                division_id: None,
            },
            "creator01",
        )
        .unwrap();
        assert!((default_multiplier.multiplier - 1.6).abs() < f64::EPSILON);
        assert_eq!(zero_multiplier.multiplier, 0.0);
        let stored_method_multiplier: f64 = pool
            .get()
            .unwrap()
            .query_row(
                "SELECT multiplier FROM methods WHERE id=?1",
                [method],
                |row| row.get(0),
            )
            .unwrap();
        assert!((stored_method_multiplier - 1.6).abs() < f64::EPSILON);
    }
}
