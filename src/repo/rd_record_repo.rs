use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::rd_record::RdRecordResponse;
use crate::models::record::{RecordCreate, RecordUpdate};
use crate::repo::{audit_repo, trace_repo, trash_repo};

const SELECT_RECORD:&str="SELECT wr.id,wr.business_no,wr.project_id,wr.method_id,
 COALESCE(NULLIF(wr.project_name_snapshot,''),p.name),COALESCE(NULLIF(wr.lab_name_snapshot,''),pg.name,'未知'),
 wr.user_name,wr.quantity,wr.recorded_at,wr.batch_no,wr.notes,wr.created_at,wr.deleted_at,wr.status,wr.sampler,wr.sampled_at,
 COALESCE(NULLIF(wr.method_name_snapshot,''),NULLIF(m.full_name,''),NULLIF(m.name,'')),
 (SELECT string_agg(DISTINCT mt.name, ',') FROM method_type_links mtl JOIN method_types mt ON mt.id=mtl.method_type_id WHERE mtl.method_id=wr.method_id),
 COALESCE(NULLIF(wr.instrument_code_snapshot,''),i.code,''),COALESCE(NULLIF(wr.instrument_type_snapshot,''),i.instrument_type,''),
 wr.division_id,wr.group_id,NULLIF(COALESCE(NULLIF(wr.high_item_snapshot,''),wr.high_item),''),COALESCE(wr.coefficient_snapshot,1.0),
 wr.detected_by,wr.detected_at,wr.subject_user_id,wr.created_by_user_id
 FROM rd_work_records wr JOIN projects p ON p.id=wr.project_id LEFT JOIN methods m ON m.id=wr.method_id LEFT JOIN instruments i ON i.id=m.instrument_id LEFT JOIN project_groups pg ON pg.id=wr.group_id";

fn map_record(row: &postgres_compat::Row<'_>) -> postgres_compat::Result<RdRecordResponse> {
    Ok(RdRecordResponse {
        id: row.get(0)?,
        business_no: row.get::<_, String>(1).unwrap_or_default(),
        project_id: row.get(2)?,
        method_id: row.get(3)?,
        project_name: row.get(4)?,
        group_name: row.get(5)?,
        user_name: row.get(6)?,
        quantity: row.get(7)?,
        recorded_at: row.get(8)?,
        batch_no: row.get(9)?,
        notes: row.get(10)?,
        created_at: row.get(11)?,
        deleted_at: row.get(12)?,
        status: row.get(13)?,
        sampler: row.get(14)?,
        sampled_at: row.get(15)?,
        method_name: row.get(16)?,
        method_type: row.get(17)?,
        instrument_code: row.get(18)?,
        instrument_type: row.get(19)?,
        division_id: row.get(20)?,
        group_id: row.get(21)?,
        high_item: row.get(22)?,
        coefficient_snapshot: row.get::<_, f64>(23).unwrap_or(1.0),
        detected_by: row.get(24)?,
        detected_at: row.get(25)?,
        subject_user_id: row.get(26)?,
        created_by_user_id: row.get(27)?,
    })
}

fn get_by_id_on_conn(conn: &postgres_compat::Connection, id: i64) -> Result<RdRecordResponse> {
    conn.query_row(
        &format!("{} WHERE wr.id=?1", SELECT_RECORD),
        [id],
        map_record,
    )
    .map_err(|e| match e {
        postgres_compat::Error::QueryReturnedNoRows => AppError::NotFound("记录不存在".into()),
        _ => e.into(),
    })
}
pub fn get_by_id(pool: &DbPool, id: i64) -> Result<RdRecordResponse> {
    let conn = pool.get()?;
    get_by_id_on_conn(&conn, id)
}
fn snap(r: &RdRecordResponse) -> serde_json::Value {
    serde_json::json!({"business_no":r.business_no,"project_id":r.project_id,"project_name":r.project_name,"method_id":r.method_id,"method_name":r.method_name,"instrument_code":r.instrument_code,"instrument_type":r.instrument_type,"lab_name":r.group_name,"sender":r.user_name,"quantity":r.quantity,"recorded_at":r.recorded_at,"batch_no":r.batch_no,"notes":r.notes,"status":r.status,"sampler":r.sampler,"sampled_at":r.sampled_at,"high_item":r.high_item,"coefficient_snapshot":r.coefficient_snapshot,"deleted_at":r.deleted_at})
}
fn log(
    conn: &postgres_compat::Connection,
    action: &str,
    r: &RdRecordResponse,
    operator: &str,
    detail: &str,
    from: Option<&str>,
    to: Option<&str>,
    before: Option<&serde_json::Value>,
    after: Option<&serde_json::Value>,
) -> Result<()> {
    audit_repo::log_structured_on_conn(
        conn,
        action,
        "rd_work_records",
        Some(r.id),
        operator,
        detail,
        "rd",
        &r.business_no,
        before,
        after,
        "record",
    )?;
    trace_repo::log_event_on_conn(
        conn,
        "rd",
        "rd_work_records",
        r.id,
        &r.business_no,
        action,
        from,
        to,
        operator,
        detail,
        before,
        after,
    )
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
) -> Result<(Vec<RdRecordResponse>, i64)> {
    let conn = pool.get()?;
    let mut clauses = Vec::new();
    let mut params: Vec<Box<dyn postgres_compat::types::ToSql>> = Vec::new();
    if !include_deleted {
        clauses.push("wr.deleted_at IS NULL".into());
    }
    if let Some(v) = project_id {
        clauses.push(format!("wr.project_id={v}"));
    }
    if let Some(v) = group_id {
        clauses.push(format!("wr.group_id={v}"));
    }
    if let Some(v) = division_id {
        clauses.push(format!("wr.division_id={v}"));
    }
    if let Some(v) = subject_user_id {
        clauses.push(format!("wr.subject_user_id={v}"));
    }
    if let Some(v) = user_name {
        let i = params.len() + 1;
        clauses.push(format!("wr.user_name=?{i}"));
        params.push(Box::new(v.to_string()));
    }
    if let Some(v) = start {
        let i = params.len() + 1;
        clauses.push(format!("wr.recorded_at>=?{i}"));
        params.push(Box::new(v.to_string()));
    }
    if let Some(v) = end {
        let i = params.len() + 1;
        clauses.push(format!("wr.recorded_at<=?{i}"));
        params.push(Box::new(format!("{v}T23:59:59")));
    }
    let wc = if clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", clauses.join(" AND "))
    };
    let sql = format!(
        "{} {} ORDER BY wr.recorded_at DESC LIMIT {} OFFSET {}",
        SELECT_RECORD,
        wc,
        page_size,
        (page - 1) * page_size
    );
    let mut stmt = conn.prepare(&sql)?;
    let items = stmt
        .query_map(
            postgres_compat::params_from_iter(params.iter().map(|p| p.as_ref())),
            map_record,
        )?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let count = conn.query_row(
        &format!(
            "SELECT COUNT(*) FROM rd_work_records wr JOIN projects p ON p.id=wr.project_id {}",
            wc
        ),
        postgres_compat::params_from_iter(params.iter().map(|p| p.as_ref())),
        |r| r.get(0),
    )?;
    Ok((items, count))
}

pub fn create(
    pool: &DbPool,
    body: &RecordCreate,
    batch_no: Option<String>,
    notes: Option<String>,
    operator: &str,
) -> Result<RdRecordResponse> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    super::record_repo::validate_record_bindings(
        &tx,
        body.project_id,
        body.method_id,
        body.group_id,
    )?;
    let inserted=tx.execute("INSERT INTO rd_work_records(project_id,method_id,user_name,quantity,recorded_at,group_id,division_id,batch_no,notes,status,project_name_snapshot,lab_name_snapshot,method_name_snapshot,high_item_snapshot,coefficient_snapshot,subject_user_id,created_by_user_id,created_by_username_snapshot,instrument_id_snapshot,instrument_code_snapshot,instrument_type_snapshot) SELECT ?1,?2,?3,?4,?5,?6,?7,?8,?9,'待取样',p.name,COALESCE(pg.name,''),COALESCE((SELECT COALESCE(NULLIF(full_name,''),name) FROM methods WHERE id=?2),''),COALESCE(p.high_item,''),COALESCE(p.coefficient,1.0),(SELECT id FROM users WHERE username=?3 ORDER BY id LIMIT 1),(SELECT id FROM users WHERE username=?10 ORDER BY id LIMIT 1),?10,(SELECT instrument_id FROM methods WHERE id=?2),COALESCE((SELECT i.code FROM methods m JOIN instruments i ON i.id=m.instrument_id WHERE m.id=?2),''),COALESCE((SELECT i.instrument_type FROM methods m JOIN instruments i ON i.id=m.instrument_id WHERE m.id=?2),'') FROM projects p LEFT JOIN project_groups pg ON pg.id=?6 WHERE p.id=?1",postgres_compat::params![body.project_id,body.method_id,&body.user_name,body.quantity,&body.recorded_at,body.group_id,body.division_id,batch_no,notes,operator])?;
    if inserted == 0 {
        return Err(AppError::Validation("项目不存在".into()));
    }
    let id = tx.last_insert_rowid();
    let business = trace_repo::make_business_no("RD", &body.recorded_at, id);
    tx.execute(
        "UPDATE rd_work_records SET business_no=?1 WHERE id=?2",
        postgres_compat::params![business, id],
    )?;
    let r = get_by_id_on_conn(&tx, id)?;
    let after = snap(&r);
    let detail = format!(
        "创建研发送样记录 {}：项目「{}」，数量 {}",
        r.business_no, r.project_name, r.quantity
    );
    log(
        &tx,
        "create",
        &r,
        operator,
        &detail,
        None,
        Some("待取样"),
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
) -> Result<RdRecordResponse> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let old = get_by_id_on_conn(&tx, id)?;
    if old.deleted_at.is_some() {
        return Err(AppError::Validation("记录已被删除，无法编辑".into()));
    }
    if old.sampled_at.is_some() {
        return Err(AppError::Forbidden("该记录已取样，不可修改".into()));
    }
    if body.multiplier.is_some() {
        return Err(AppError::Validation("研发送样记录不使用单价倍率".into()));
    }
    let next_project_id = body.project_id.unwrap_or(old.project_id);
    let next_method_id = body.method_id.or(old.method_id);
    let next_group_id = body.group_id.or(old.group_id);
    super::record_repo::validate_record_bindings(
        &tx,
        next_project_id,
        next_method_id,
        next_group_id,
    )?;
    let before = snap(&old);
    let mut changes = Vec::new();
    if let Some(v) = &body.user_name {
        if v != &old.user_name {
            changes.push(format!("送样人 {} → {}", old.user_name, v));
            tx.execute(
                "UPDATE rd_work_records SET user_name=?1,
                 subject_user_id=(SELECT id FROM users WHERE username=?1 ORDER BY id LIMIT 1)
                 WHERE id=?2",
                postgres_compat::params![v, id],
            )?;
        }
    }
    if let Some(v) = body.quantity {
        if v != old.quantity {
            changes.push(format!("数量 {} → {}", old.quantity, v));
            tx.execute(
                "UPDATE rd_work_records SET quantity=?1 WHERE id=?2",
                postgres_compat::params![v, id],
            )?;
        }
    }
    if let Some(v) = &body.recorded_at {
        if v != &old.recorded_at {
            changes.push(format!("时间 {} → {}", old.recorded_at, v));
            tx.execute(
                "UPDATE rd_work_records SET recorded_at=?1 WHERE id=?2",
                postgres_compat::params![v, id],
            )?;
        }
    }
    if let Some(v) = &body.batch_no {
        if Some(v.as_str()) != old.batch_no.as_deref() {
            changes.push("批号已修改".into());
            tx.execute(
                "UPDATE rd_work_records SET batch_no=?1 WHERE id=?2",
                postgres_compat::params![v, id],
            )?;
        }
    }
    if let Some(v) = &body.notes {
        if Some(v.as_str()) != old.notes.as_deref() {
            changes.push("备注已修改".into());
            tx.execute(
                "UPDATE rd_work_records SET notes=?1 WHERE id=?2",
                postgres_compat::params![v, id],
            )?;
        }
    }
    if let Some(v) = body.project_id {
        if v != old.project_id {
            changes.push(format!("项目ID {} → {}", old.project_id, v));
            tx.execute("UPDATE rd_work_records SET project_id=?1,project_name_snapshot=(SELECT name FROM projects WHERE id=?1),high_item_snapshot=COALESCE((SELECT high_item FROM projects WHERE id=?1),''),coefficient_snapshot=COALESCE((SELECT coefficient FROM projects WHERE id=?1),1.0) WHERE id=?2",postgres_compat::params![v,id])?;
        }
    }
    if let Some(v) = body.method_id {
        if Some(v) != old.method_id {
            changes.push("方法已修改".into());
            tx.execute("UPDATE rd_work_records SET method_id=?1,method_name_snapshot=COALESCE((SELECT COALESCE(NULLIF(full_name,''),name) FROM methods WHERE id=?1),''),instrument_id_snapshot=(SELECT instrument_id FROM methods WHERE id=?1),instrument_code_snapshot=COALESCE((SELECT i.code FROM methods m JOIN instruments i ON i.id=m.instrument_id WHERE m.id=?1),''),instrument_type_snapshot=COALESCE((SELECT i.instrument_type FROM methods m JOIN instruments i ON i.id=m.instrument_id WHERE m.id=?1),'') WHERE id=?2",postgres_compat::params![v,id])?;
        }
    }
    if let Some(v) = body.group_id {
        changes.push("实验室已修改".into());
        tx.execute("UPDATE rd_work_records SET group_id=?1,lab_name_snapshot=COALESCE((SELECT name FROM project_groups WHERE id=?1),'') WHERE id=?2",postgres_compat::params![v,id])?;
    }
    if let Some(v) = body.division_id {
        changes.push("部门已修改".into());
        tx.execute(
            "UPDATE rd_work_records SET division_id=?1 WHERE id=?2",
            postgres_compat::params![v, id],
        )?;
    }
    if let Some(v) = &body.high_item {
        changes.push("高项已修改".into());
        tx.execute("UPDATE rd_work_records SET high_item=?1,high_item_snapshot=COALESCE(?1,'') WHERE id=?2",postgres_compat::params![if v.trim().is_empty(){None}else{Some(v.trim())},id])?;
    }
    if changes.is_empty() {
        return Err(AppError::Validation("没有需要更新的字段".into()));
    }
    tx.execute(
        "UPDATE rd_work_records SET updated_at=datetime('now','localtime') WHERE id=?1",
        [id],
    )?;
    let r = get_by_id_on_conn(&tx, id)?;
    let after = snap(&r);
    let detail = format!("修改 {}：{}", r.business_no, changes.join("，"));
    log(
        &tx,
        "update",
        &r,
        operator,
        &detail,
        None,
        None,
        Some(&before),
        Some(&after),
    )?;
    tx.commit()?;
    get_by_id_on_conn(&conn, id)
}

pub fn soft_delete(pool: &DbPool, id: i64, operator: &str, reason: &str) -> Result<()> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let old = get_by_id_on_conn(&tx, id)?;
    if old.deleted_at.is_some() {
        return Err(AppError::Validation("记录已被删除".into()));
    }
    let before = snap(&old);
    tx.execute("UPDATE rd_work_records SET deleted_at=datetime('now','localtime'),updated_at=datetime('now','localtime') WHERE id=?1",[id])?;
    let r = get_by_id_on_conn(&tx, id)?;
    let after = snap(&r);
    trash_repo::move_to_trash_on_conn(
        &tx,
        "研发送样记录",
        "rd_work_records",
        id,
        "records",
        "rd",
        &r.business_no,
        &r.business_no,
        &before,
        reason,
        operator,
        r.subject_user_id.or(r.created_by_user_id),
        r.group_id,
        "",
        true,
    )?;
    log(
        &tx,
        "delete",
        &r,
        operator,
        &format!("删除研发送样记录 {}", r.business_no),
        None,
        None,
        Some(&before),
        Some(&after),
    )?;
    tx.commit()?;
    Ok(())
}
pub fn restore(pool: &DbPool, id: i64, operator: &str) -> Result<RdRecordResponse> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let old = get_by_id_on_conn(&tx, id)?;
    if old.deleted_at.is_none() {
        return Err(AppError::Validation("记录未被删除，无需恢复".into()));
    }
    let before = snap(&old);
    tx.execute("UPDATE rd_work_records SET deleted_at=NULL,updated_at=datetime('now','localtime') WHERE id=?1",[id])?;
    trash_repo::mark_restored_on_conn(&tx, "rd_work_records", id, operator)?;
    let r = get_by_id_on_conn(&tx, id)?;
    let after = snap(&r);
    log(
        &tx,
        "restore",
        &r,
        operator,
        &format!("恢复研发送样记录 {}", r.business_no),
        None,
        None,
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
    let mut sql =
        "SELECT id FROM rd_work_records WHERE user_name=?1 AND deleted_at IS NULL".to_string();
    let mut params: Vec<Box<dyn postgres_compat::types::ToSql>> =
        vec![Box::new(user_name.to_string())];
    if let Some(v) = start {
        let i = params.len() + 1;
        sql.push_str(&format!(" AND recorded_at>=?{i}"));
        params.push(Box::new(v.to_string()));
    }
    if let Some(v) = end {
        let i = params.len() + 1;
        sql.push_str(&format!(" AND recorded_at<=?{i}"));
        params.push(Box::new(format!("{v}T23:59:59")));
    }
    let ids: Vec<i64> = {
        let mut stmt = tx.prepare(&sql)?;
        let rows = stmt.query_map(
            postgres_compat::params_from_iter(params.iter().map(|p| p.as_ref())),
            |r| r.get(0),
        )?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };
    for id in &ids {
        let old = get_by_id_on_conn(&tx, *id)?;
        let before = snap(&old);
        tx.execute(
            "UPDATE rd_work_records SET deleted_at=datetime('now','localtime') WHERE id=?1",
            [*id],
        )?;
        let r = get_by_id_on_conn(&tx, *id)?;
        let after = snap(&r);
        trash_repo::move_to_trash_on_conn(
            &tx,
            "研发送样记录",
            "rd_work_records",
            *id,
            "records",
            "rd",
            &r.business_no,
            &r.business_no,
            &before,
            reason,
            operator,
            r.subject_user_id.or(r.created_by_user_id),
            r.group_id,
            "",
            true,
        )?;
        log(
            &tx,
            "delete",
            &r,
            operator,
            "批量删除研发送样记录",
            None,
            None,
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
            "SELECT id FROM rd_work_records
             WHERE deleted_at IS NULL AND recorded_at>=?1 AND recorded_at<=?2
             ORDER BY id",
        )?;
        let rows = stmt.query_map(postgres_compat::params![start, end], |row| row.get(0))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };
    for id in &ids {
        let before_record = get_by_id_on_conn(&tx, *id)?;
        let before = snap(&before_record);
        tx.execute(
            "UPDATE rd_work_records SET deleted_at=datetime('now','localtime'),updated_at=datetime('now','localtime') WHERE id=?1",
            [*id],
        )?;
        let after_record = get_by_id_on_conn(&tx, *id)?;
        let after = snap(&after_record);
        trash_repo::move_to_trash_on_conn(
            &tx,
            "研发送样记录",
            "rd_work_records",
            *id,
            "records",
            "rd",
            &after_record.business_no,
            &after_record.business_no,
            &before,
            reason,
            operator,
            after_record
                .subject_user_id
                .or(after_record.created_by_user_id),
            after_record.group_id,
            "",
            true,
        )?;
        log(
            &tx,
            "delete",
            &after_record,
            operator,
            &format!("数据治理批量移入回收站：{}", after_record.business_no),
            None,
            None,
            Some(&before),
            Some(&after),
        )?;
    }
    tx.commit()?;
    Ok(ids.len() as i64)
}

pub fn sample(pool: &DbPool, id: i64, sampler: &str) -> Result<RdRecordResponse> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let old = get_by_id_on_conn(&tx, id)?;
    if old.deleted_at.is_some() {
        return Err(AppError::Validation("记录已被删除".into()));
    }
    if old.sampled_at.is_some() {
        return Err(AppError::Validation("记录已取样".into()));
    }
    let before = snap(&old);
    tx.execute("UPDATE rd_work_records SET sampler=?1,sampled_at=datetime('now','localtime'),status='已取样',updated_at=datetime('now','localtime') WHERE id=?2",postgres_compat::params![sampler,id])?;
    let r = get_by_id_on_conn(&tx, id)?;
    let after = snap(&r);
    log(
        &tx,
        "sample",
        &r,
        sampler,
        &format!("研发送样记录 {} 已取样", r.business_no),
        Some(&old.status),
        Some(&r.status),
        Some(&before),
        Some(&after),
    )?;
    tx.commit()?;
    get_by_id_on_conn(&conn, id)
}
pub fn is_sampled(pool: &DbPool, id: i64) -> Result<bool> {
    Ok(get_by_id(pool, id)?.sampled_at.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rd_snapshot_and_sample_event_are_independent_from_work_records() {
        let pool = crate::db::init_pool("postgres-test");
        crate::db::test_migrations::run(&pool.get().unwrap()).unwrap();
        let conn = pool.get().unwrap();
        conn.execute("INSERT INTO project_groups(name) VALUES('RdLab01')", [])
            .unwrap();
        let group = conn.last_insert_rowid();
        conn.execute("INSERT INTO projects(group_id,name,coefficient,high_item) VALUES(?1,'RdProject01',2.5,'HighItem01')",[group]).unwrap();
        let project = conn.last_insert_rowid();
        conn.execute("INSERT INTO instruments(code,name,instrument_type) VALUES('LC-RD01','RdLiquid01','液相')",[]).unwrap();
        let instrument = conn.last_insert_rowid();
        conn.execute("INSERT INTO methods(method_code,name,full_name,instrument_id) VALUES('M-RD-001','RdMethod01','RdMethod01 Full',?1)",[instrument]).unwrap();
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
        let record = create(
            &pool,
            &RecordCreate {
                project_id: project,
                method_id: Some(method),
                user_name: "sender01".into(),
                quantity: 2,
                recorded_at: "2026-07-18T10:00:00".into(),
                group_id: Some(group),
                multiplier: None,
                high_item: None,
                division_id: None,
            },
            Some("B001".into()),
            None,
            "sender01",
        )
        .unwrap();
        assert!(record.business_no.starts_with("RD-20260718-"));
        assert_eq!(record.coefficient_snapshot, 2.5);
        assert_eq!(record.high_item.as_deref(), Some("HighItem01"));
        pool.get().unwrap().execute("UPDATE projects SET name='Renamed',coefficient=9.0,high_item='ChangedHighItem' WHERE id=?1",[project]).unwrap();
        let reread = get_by_id(&pool, record.id).unwrap();
        assert_eq!(reread.project_name, "RdProject01");
        assert_eq!(reread.coefficient_snapshot, 2.5);
        assert_eq!(reread.high_item.as_deref(), Some("HighItem01"));
        let sampled = sample(&pool, record.id, "sampler01").unwrap();
        assert_eq!(sampled.status, "已取样");
        assert_eq!(sampled.sampler.as_deref(), Some("sampler01"));
        let events = crate::repo::trace_repo::list(&pool, "rd_work_records", record.id).unwrap();
        assert_eq!(
            events
                .iter()
                .map(|event| event.event_type.as_str())
                .collect::<Vec<_>>(),
            vec!["create", "sample"]
        );
        let work_events: i64 = pool
            .get()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM record_events WHERE table_name='work_records'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(work_events, 0);
    }
}
