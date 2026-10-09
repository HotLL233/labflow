use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::rd_record::RdRecordResponse;
use crate::models::record::{RecordCreate, RecordUpdate};
use crate::repo::{audit_repo, trace_repo, trash_repo};

const SELECT_RECORD:&str="SELECT wr.id,wr.business_no,wr.project_id,wr.method_id,
 COALESCE(NULLIF(wr.project_name_snapshot,''),p.name),COALESCE(NULLIF(wr.lab_name_snapshot,''),pg.name,'未知'),
 wr.user_name,wr.quantity,wr.recorded_at,wr.last_activity_at,wr.batch_no,wr.notes,wr.created_at,wr.deleted_at,wr.status,wr.sampler,wr.sampled_at,
 COALESCE(NULLIF(wr.method_name_snapshot,''),NULLIF(m.full_name,''),NULLIF(m.name,'')),
 (SELECT string_agg(DISTINCT mt.name, ',') FROM method_type_links mtl JOIN method_types mt ON mt.id=mtl.method_type_id WHERE mtl.method_id=wr.method_id),
 COALESCE(NULLIF(wr.instrument_code_snapshot,''),i.code,''),COALESCE(NULLIF(wr.instrument_type_snapshot,''),i.instrument_type,''),
 wr.division_id,wr.group_id,NULLIF(COALESCE(NULLIF(wr.high_item_snapshot,''),wr.high_item),''),COALESCE(wr.coefficient_snapshot,1.0),
 wr.detected_by,wr.detected_at,wr.subject_user_id,wr.created_by_user_id,wr.extra_fields,
 COALESCE(wr.return_reason,''),COALESCE(wr.returned_by,''),wr.returned_at,wr.return_confirmed_at,COALESCE(wr.return_confirmed_by,''),
 wr.voided_at,COALESCE(wr.voided_by,''),COALESCE(wr.void_reason,''),
 wr.project_division_id,wr.execution_division_id,wr.execution_group_id,
 COALESCE((SELECT SUM(workload.quantity) FROM work_records workload WHERE workload.source_type='rd_sample' AND workload.source_record_id=wr.id AND workload.deleted_at IS NULL),0) >= wr.quantity
 FROM rd_work_records wr JOIN projects p ON p.id=wr.project_id LEFT JOIN methods m ON m.id=wr.method_id LEFT JOIN instruments i ON i.id=m.instrument_id LEFT JOIN project_groups pg ON pg.id=wr.group_id";

fn map_record(row: &postgres_compat::Row<'_>) -> postgres_compat::Result<RdRecordResponse> {
    let raw_extra_fields: Option<String> = row.get(29)?;
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
        last_activity_at: row.get(9)?,
        batch_no: row.get(10)?,
        notes: row.get(11)?,
        created_at: row.get(12)?,
        deleted_at: row.get(13)?,
        status: row.get(14)?,
        sampler: row.get(15)?,
        sampled_at: row.get(16)?,
        method_name: row.get(17)?,
        method_type: row.get(18)?,
        instrument_code: row.get(19)?,
        instrument_type: row.get(20)?,
        division_id: row.get(21)?,
        group_id: row.get(22)?,
        high_item: row.get(23)?,
        coefficient_snapshot: row.get::<_, f64>(24).unwrap_or(1.0),
        detected_by: row.get(25)?,
        detected_at: row.get(26)?,
        subject_user_id: row.get(27)?,
        created_by_user_id: row.get(28)?,
        extra_fields: raw_extra_fields.and_then(|value| serde_json::from_str(&value).ok()),
        // List queries append the immutable submission rank; detail queries
        // do not, so keep a neutral value for non-list responses.
        sequence_no: row.get(42).unwrap_or(0),
        return_reason: row.get::<_, String>(30).unwrap_or_default(),
        returned_by: row.get::<_, String>(31).unwrap_or_default(),
        returned_at: row.get(32)?,
        return_confirmed_at: row.get(33)?,
        return_confirmed_by: row.get::<_, String>(34).unwrap_or_default(),
        voided_at: row.get(35)?,
        voided_by: row.get::<_, String>(36).unwrap_or_default(),
        void_reason: row.get::<_, String>(37).unwrap_or_default(),
        project_division_id: row.get(38)?,
        execution_division_id: row.get(39)?,
        execution_group_id: row.get(40)?,
        workload_recorded: row.get(41).unwrap_or(false),
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

/// The RD entry page requires the explicit project -> type -> method selection.
/// Keep this validation at the API boundary so a direct request cannot bypass it.
pub(crate) fn validate_submission_selection(
    conn: &postgres_compat::Connection,
    project_id: i64,
    method_id: i64,
    detection_type: &str,
) -> Result<()> {
    let valid_selection: i64 = conn.query_row(
        "SELECT COUNT(*)
         FROM project_method_links pml
         JOIN methods m ON m.id=pml.method_id
         JOIN instruments i ON i.id=m.instrument_id
         JOIN method_type_links mtl ON mtl.method_id=m.id
         JOIN method_types mt ON mt.id=mtl.method_type_id
         WHERE pml.project_id=?1 AND m.id=?2 AND mt.name=?3
           AND m.is_active=1 AND i.is_active=1",
        postgres_compat::params![project_id, method_id, detection_type],
        |row| row.get(0),
    )?;
    if valid_selection == 0 {
        return Err(AppError::Validation(
            "所选方法未关联当前项目，或不属于所选检测类型".into(),
        ));
    }
    Ok(())
}
pub fn get_by_id(pool: &DbPool, id: i64) -> Result<RdRecordResponse> {
    let conn = pool.get()?;
    get_by_id_on_conn(&conn, id)
}
fn snap(r: &RdRecordResponse) -> serde_json::Value {
    serde_json::json!({"business_no":r.business_no,"project_id":r.project_id,"project_name":r.project_name,"method_id":r.method_id,"method_name":r.method_name,"instrument_code":r.instrument_code,"instrument_type":r.instrument_type,"lab_name":r.group_name,"sender":r.user_name,"subject_user_id":r.subject_user_id,"created_by_user_id":r.created_by_user_id,"quantity":r.quantity,"recorded_at":r.recorded_at,"last_activity_at":r.last_activity_at,"batch_no":r.batch_no,"notes":r.notes,"extra_fields":r.extra_fields,"status":r.status,"return_reason":r.return_reason,"returned_by":r.returned_by,"returned_at":r.returned_at,"return_confirmed_at":r.return_confirmed_at,"return_confirmed_by":r.return_confirmed_by,"voided_at":r.voided_at,"voided_by":r.voided_by,"void_reason":r.void_reason,"sampler":r.sampler,"sampled_at":r.sampled_at,"high_item":r.high_item,"coefficient_snapshot":r.coefficient_snapshot,"deleted_at":r.deleted_at})
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
    related_user_id: Option<i64>,
    user_name: Option<&str>,
    division_id: Option<i64>,
    allowed_division_ids: Option<&[i64]>,
    start: Option<&str>,
    end: Option<&str>,
    page: i64,
    page_size: i64,
    include_deleted: bool,
    sort_by: Option<&str>,
    sort_dir: Option<&str>,
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
        clauses.push(format!(
            "COALESCE(wr.execution_division_id,wr.division_id,(SELECT division_id FROM project_groups WHERE id=wr.group_id))={v}"
        ));
    }
    if let Some(ids) = allowed_division_ids {
        if ids.is_empty() {
            clauses.push("1=0".into());
        } else {
            let values = ids
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",");
            clauses.push(format!(
                "COALESCE(wr.execution_division_id,wr.division_id,(SELECT division_id FROM project_groups WHERE id=wr.group_id)) IN ({values})"
            ));
        }
    }
    if let Some(v) = related_user_id {
        clauses.push(format!(
            "(wr.subject_user_id={v} OR wr.created_by_user_id={v})"
        ));
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
    // Sorting is deliberately request-scoped. The client sends the current
    // user's table preference; no shared/default business data is rewritten.
    let sort_expression = match sort_by.unwrap_or("submitted_at") {
        "submitted_at" | "recorded_at" | "seq_no" => "wr.recorded_at",
        "created_at" => "wr.created_at",
        "status" => "COALESCE(wr.status,'')",
        "business_no" => "COALESCE(wr.business_no,'')",
        "batch_no" => "COALESCE(wr.batch_no,'')",
        "user_name" => "COALESCE(wr.user_name,'')",
        "division_id" => "COALESCE(wr.division_id,0)",
        "lab_name" => "COALESCE(NULLIF(wr.lab_name_snapshot,''),pg.name,'')",
        "project_name" => "COALESCE(NULLIF(wr.project_name_snapshot,''),p.name,'')",
        "method_name" => "COALESCE(NULLIF(wr.method_name_snapshot,''),NULLIF(m.full_name,''),NULLIF(m.name,''),'')",
        "detection_type" => "COALESCE((SELECT string_agg(DISTINCT mt.name, ',') FROM method_type_links mtl JOIN method_types mt ON mt.id=mtl.method_type_id WHERE mtl.method_id=wr.method_id),'')",
        "instrument_code" => "COALESCE(NULLIF(wr.instrument_code_snapshot,''),i.code,'')",
        "high_item" => "COALESCE(NULLIF(wr.high_item_snapshot,''),wr.high_item,'')",
        "quantity" => "COALESCE(wr.quantity,0)",
        _ => "wr.recorded_at",
    };
    let descending = sort_dir.unwrap_or("desc").eq_ignore_ascii_case("desc");
    let direction = if descending { "DESC" } else { "ASC" };
    let select_with_sequence = SELECT_RECORD.replacen(
        " FROM rd_work_records wr",
        ", ROW_NUMBER() OVER (ORDER BY wr.recorded_at ASC, wr.id ASC) AS sequence_no FROM rd_work_records wr",
        1,
    );
    let sql = format!(
        "{} {} ORDER BY {} {}, wr.id {} LIMIT {} OFFSET {}",
        select_with_sequence,
        wc,
        sort_expression,
        direction,
        direction,
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
    operator_user_id: i64,
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
    let extra_fields = body
        .extra_fields
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|e| AppError::Validation(format!("自定义字段格式错误: {e}")))?;
    // Resolve this before binding SQL parameters. PostgreSQL cannot infer the intended
    // BIGINT type safely from COALESCE parameters in this INSERT ... SELECT expression.
    let subject_user_id = body.sender_user_id.unwrap_or(operator_user_id);
    let inserted=tx.execute("INSERT INTO rd_work_records(project_id,method_id,user_name,quantity,recorded_at,last_activity_at,group_id,division_id,project_division_id,project_division_name_snapshot,execution_division_id,execution_division_name_snapshot,execution_group_id,execution_group_name_snapshot,batch_no,notes,extra_fields,status,project_name_snapshot,lab_name_snapshot,method_name_snapshot,high_item_snapshot,coefficient_snapshot,subject_user_id,created_by_user_id,created_by_username_snapshot,instrument_id_snapshot,instrument_code_snapshot,instrument_type_snapshot) SELECT ?1,?2,?3,?4,?5,?5,?6,?7,p.project_division_id,COALESCE(p.project_division_name_snapshot,''),COALESCE(?7,pg.division_id),COALESCE(d.name,''),?6,COALESCE(pg.name,''),?8,?9,COALESCE(?10,'{}'),'待取样',p.name,COALESCE(pg.name,''),COALESCE((SELECT COALESCE(NULLIF(full_name,''),name) FROM methods WHERE id=?2),''),COALESCE(p.high_item,''),COALESCE(p.coefficient,1.0),?11,?12,?13,(SELECT instrument_id FROM methods WHERE id=?2),COALESCE((SELECT i.code FROM methods m JOIN instruments i ON i.id=m.instrument_id WHERE m.id=?2),''),COALESCE((SELECT i.instrument_type FROM methods m JOIN instruments i ON i.id=m.instrument_id WHERE m.id=?2),'') FROM projects p LEFT JOIN project_groups pg ON pg.id=?6 LEFT JOIN divisions d ON d.id=COALESCE(?7,pg.division_id) WHERE p.id=?1",postgres_compat::params![body.project_id,body.method_id,&body.user_name,body.quantity,&body.recorded_at,body.group_id,body.division_id,batch_no,notes,extra_fields,subject_user_id,operator_user_id,operator])?;
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
    if old.status == "已退回" || old.status == "已退回已确认" {
        return Err(AppError::Forbidden(
            "该记录已退回，请先确认退回原因后再修改".into(),
        ));
    }
    if old.status == "已作废" {
        return Err(AppError::Forbidden("已作废记录不可再次编辑".into()));
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
                "UPDATE rd_work_records SET user_name=?1 WHERE id=?2",
                postgres_compat::params![v, id],
            )?;
        }
    }
    if let Some(sender_user_id) = body.sender_user_id {
        if Some(sender_user_id) != old.subject_user_id {
            changes.push("实际送样人已修改".into());
            tx.execute(
                "UPDATE rd_work_records SET subject_user_id=?1 WHERE id=?2",
                postgres_compat::params![sender_user_id, id],
            )?;
        }
    }
    if let Some(v) = body.quantity {
        if v < 0 {
            return Err(AppError::Validation("数量不能小于 0".into()));
        }
        if v == 0 && old.status != "退回待修改" {
            return Err(AppError::Validation(
                "仅驳回后的待修改记录可将数量设置为 0 作废".into(),
            ));
        }
        if v != old.quantity {
            changes.push(format!("数量 {} → {}", old.quantity, v));
            tx.execute(
                "UPDATE rd_work_records SET quantity=?1 WHERE id=?2",
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
    if let Some(v) = &body.extra_fields {
        let encoded = serde_json::to_string(v)
            .map_err(|e| AppError::Validation(format!("自定义字段格式错误: {e}")))?;
        changes.push("自定义字段已修改".into());
        tx.execute(
            "UPDATE rd_work_records SET extra_fields=?1 WHERE id=?2",
            postgres_compat::params![encoded, id],
        )?;
    }
    if let Some(v) = body.project_id {
        if v != old.project_id {
            changes.push(format!("项目ID {} → {}", old.project_id, v));
            tx.execute("UPDATE rd_work_records SET project_id=?1,project_name_snapshot=(SELECT name FROM projects WHERE id=?1),project_division_id=(SELECT project_division_id FROM projects WHERE id=?1),project_division_name_snapshot=COALESCE((SELECT project_division_name_snapshot FROM projects WHERE id=?1),''),high_item_snapshot=COALESCE((SELECT high_item FROM projects WHERE id=?1),''),coefficient_snapshot=COALESCE((SELECT coefficient FROM projects WHERE id=?1),1.0) WHERE id=?2",postgres_compat::params![v,id])?;
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
        tx.execute("UPDATE rd_work_records SET group_id=?1,division_id=COALESCE((SELECT division_id FROM project_groups WHERE id=?1),division_id),lab_name_snapshot=COALESCE((SELECT name FROM project_groups WHERE id=?1),''),execution_group_id=?1,execution_group_name_snapshot=COALESCE((SELECT name FROM project_groups WHERE id=?1),''),execution_division_id=COALESCE((SELECT division_id FROM project_groups WHERE id=?1),division_id),execution_division_name_snapshot=COALESCE((SELECT d.name FROM divisions d WHERE d.id=COALESCE((SELECT division_id FROM project_groups WHERE id=?1),division_id)), '') WHERE id=?2",postgres_compat::params![v,id])?;
    }
    if let Some(v) = body.division_id {
        changes.push("部门已修改".into());
        tx.execute(
            "UPDATE rd_work_records SET division_id=?1,execution_division_id=COALESCE(?1,(SELECT division_id FROM project_groups WHERE id=group_id)),execution_division_name_snapshot=COALESCE((SELECT d.name FROM divisions d WHERE d.id=COALESCE(?1,(SELECT division_id FROM project_groups WHERE id=group_id))), '') WHERE id=?2",
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
    let void_return_draft = old.status == "退回待修改" && body.quantity == Some(0);
    if void_return_draft {
        changes.push("驳回后数量调整为 0，记录已作废".into());
        tx.execute(
            "UPDATE rd_work_records SET status='已作废',voided_at=to_char(CURRENT_TIMESTAMP,'YYYY-MM-DD\"T\"HH24:MI:SS'),voided_by=?1,void_reason='驳回后数量调整为 0',last_activity_at=to_char(CURRENT_TIMESTAMP,'YYYY-MM-DD\"T\"HH24:MI:SS'),updated_at=datetime('now','localtime') WHERE id=?2",
            postgres_compat::params![operator, id],
        )?;
    } else if old.status == "退回待修改" {
        changes.push("退回修改后重新提交".into());
        tx.execute(
            "UPDATE rd_work_records SET status='待取样',recorded_at=to_char(CURRENT_TIMESTAMP,'YYYY-MM-DD\"T\"HH24:MI:SS'),last_activity_at=to_char(CURRENT_TIMESTAMP,'YYYY-MM-DD\"T\"HH24:MI:SS'),updated_at=datetime('now','localtime') WHERE id=?1",
            [id],
        )?;
    } else {
        tx.execute(
            "UPDATE rd_work_records SET updated_at=datetime('now','localtime') WHERE id=?1",
            [id],
        )?;
    }
    let r = get_by_id_on_conn(&tx, id)?;
    let after = snap(&r);
    let detail = format!("修改 {}：{}", r.business_no, changes.join("，"));
    log(
        &tx,
        if void_return_draft { "void" } else { "update" },
        &r,
        operator,
        &detail,
        if old.status == "退回待修改" {
            Some(&old.status)
        } else {
            None
        },
        if old.status == "退回待修改" {
            Some(&r.status)
        } else {
            None
        },
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

pub fn sample(pool: &DbPool, id: i64, sampler: &str, operator: &str) -> Result<RdRecordResponse> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let old = get_by_id_on_conn(&tx, id)?;
    if old.deleted_at.is_some() {
        return Err(AppError::Validation("记录已被删除".into()));
    }
    if old.sampled_at.is_some() {
        return Err(AppError::Validation("记录已取样".into()));
    }
    if old.status != "待取样" {
        return Err(AppError::Validation("仅待取样记录可以取样".into()));
    }
    let before = snap(&old);
    tx.execute("UPDATE rd_work_records SET sampler=?1,sampled_at=datetime('now','localtime'),status='已取样',last_activity_at=to_char(CURRENT_TIMESTAMP,'YYYY-MM-DD\"T\"HH24:MI:SS'),updated_at=datetime('now','localtime') WHERE id=?2",postgres_compat::params![sampler,id])?;
    let r = get_by_id_on_conn(&tx, id)?;
    let after = snap(&r);
    log(
        &tx,
        "sample",
        &r,
        operator,
        &format!(
            "研发送样记录 {} 已取样（取样归属：{}）",
            r.business_no, sampler
        ),
        Some(&old.status),
        Some(&r.status),
        Some(&before),
        Some(&after),
    )?;
    tx.commit()?;
    get_by_id_on_conn(&conn, id)
}

pub fn withdraw_sample(
    pool: &DbPool,
    id: i64,
    reason: &str,
    operator: &str,
) -> Result<RdRecordResponse> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(AppError::Validation("请填写撤回取样原因".into()));
    }
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let old = get_by_id_on_conn(&tx, id)?;
    if old.deleted_at.is_some() {
        return Err(AppError::Validation("记录已被删除".into()));
    }
    if old.status != "已取样" || old.sampled_at.is_none() {
        return Err(AppError::Validation("仅已取样记录可以撤回取样".into()));
    }
    if old.detected_at.is_some()
        || old
            .detected_by
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
    {
        return Err(AppError::Validation(
            "记录已进入检测完成流程，不能直接撤回取样".into(),
        ));
    }
    let before = snap(&old);
    let original_sampler = old.sampler.as_deref().unwrap_or("未记录取样人");
    tx.execute(
        "UPDATE rd_work_records
         SET sampler=NULL,sampled_at=NULL,status='待取样',
             last_activity_at=to_char(CURRENT_TIMESTAMP,'YYYY-MM-DD\"T\"HH24:MI:SS'),
             updated_at=datetime('now','localtime')
         WHERE id=?1",
        [id],
    )?;
    let record = get_by_id_on_conn(&tx, id)?;
    let after = snap(&record);
    log(
        &tx,
        "sample_withdraw",
        &record,
        operator,
        &format!(
            "研发送样记录 {} 已撤回取样（原取样人：{}）：{}",
            record.business_no, original_sampler, reason
        ),
        Some(&old.status),
        Some(&record.status),
        Some(&before),
        Some(&after),
    )?;
    tx.commit()?;
    get_by_id_on_conn(&conn, id)
}

pub fn return_record(
    pool: &DbPool,
    id: i64,
    reason: &str,
    returned_by: &str,
) -> Result<RdRecordResponse> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(AppError::Validation("请填写退回原因".into()));
    }
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let old = get_by_id_on_conn(&tx, id)?;
    if old.deleted_at.is_some() {
        return Err(AppError::Validation("记录已被删除".into()));
    }
    if old.sampled_at.is_some() || old.status == "已取样" {
        return Err(AppError::Validation("已取样记录不能退回".into()));
    }
    if old.status == "已退回"
        || old.status == "已退回已确认"
        || old.status == "退回待修改"
        || old.status == "已作废"
    {
        return Err(AppError::Validation("该记录不允许再次退回".into()));
    }
    let before = snap(&old);
    tx.execute(
        "UPDATE rd_work_records
         SET status='已退回',return_reason=?1,returned_by=?2,
              returned_at=datetime('now','localtime'),last_activity_at=to_char(CURRENT_TIMESTAMP,'YYYY-MM-DD\"T\"HH24:MI:SS'),updated_at=datetime('now','localtime')
         WHERE id=?3",
        postgres_compat::params![reason, returned_by, id],
    )?;
    let record = get_by_id_on_conn(&tx, id)?;
    let after = snap(&record);
    log(
        &tx,
        "return",
        &record,
        returned_by,
        &format!("研发送样记录 {} 已退回：{}", record.business_no, reason),
        Some(&old.status),
        Some(&record.status),
        Some(&before),
        Some(&after),
    )?;
    tx.commit()?;
    get_by_id_on_conn(&conn, id)
}

pub fn confirm_return(pool: &DbPool, id: i64, confirmed_by: &str) -> Result<RdRecordResponse> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let old = get_by_id_on_conn(&tx, id)?;
    if old.deleted_at.is_some() {
        return Err(AppError::Validation("记录已被删除".into()));
    }
    if old.status != "已退回" {
        return Err(AppError::Validation("仅已退回记录可以确认修改".into()));
    }
    let before = snap(&old);
    tx.execute(
        "UPDATE rd_work_records
         SET status='已退回已确认',return_confirmed_by=?1,
              return_confirmed_at=datetime('now','localtime'),last_activity_at=to_char(CURRENT_TIMESTAMP,'YYYY-MM-DD\"T\"HH24:MI:SS'),updated_at=datetime('now','localtime')
         WHERE id=?2",
        postgres_compat::params![confirmed_by, id],
    )?;
    let record = get_by_id_on_conn(&tx, id)?;
    let after = snap(&record);
    log(
        &tx,
        "return_confirm",
        &record,
        confirmed_by,
        &format!(
            "研发送样记录 {} 已确认退回，原记录保留不再编辑",
            record.business_no
        ),
        Some(&old.status),
        Some(&record.status),
        Some(&before),
        Some(&after),
    )?;

    // Keep the returned record immutable for traceability. The sender edits a
    // separate copy, which becomes a normal pending-sampling record only after save.
    tx.execute(
        "INSERT INTO rd_work_records(
            project_id,method_id,user_name,quantity,recorded_at,last_activity_at,group_id,division_id,project_division_id,project_division_name_snapshot,execution_division_id,execution_division_name_snapshot,execution_group_id,execution_group_name_snapshot,batch_no,notes,
            extra_fields,status,project_name_snapshot,lab_name_snapshot,method_name_snapshot,
            high_item_snapshot,coefficient_snapshot,subject_user_id,created_by_user_id,
            created_by_username_snapshot,instrument_id_snapshot,instrument_code_snapshot,
            instrument_type_snapshot
         )
          SELECT project_id,method_id,user_name,quantity,to_char(CURRENT_TIMESTAMP,'YYYY-MM-DD\"T\"HH24:MI:SS'),to_char(CURRENT_TIMESTAMP,'YYYY-MM-DD\"T\"HH24:MI:SS'),group_id,division_id,project_division_id,project_division_name_snapshot,execution_division_id,execution_division_name_snapshot,execution_group_id,execution_group_name_snapshot,batch_no,notes,
            extra_fields,'退回待修改',project_name_snapshot,lab_name_snapshot,method_name_snapshot,
            high_item_snapshot,coefficient_snapshot,subject_user_id,created_by_user_id,
            created_by_username_snapshot,instrument_id_snapshot,instrument_code_snapshot,
            instrument_type_snapshot
         FROM rd_work_records WHERE id=?1",
        [id],
    )?;
    let draft_id = tx.last_insert_rowid();
    let mut draft = get_by_id_on_conn(&tx, draft_id)?;
    let draft_business_no = trace_repo::make_business_no("RD", &draft.recorded_at, draft_id);
    tx.execute(
        "UPDATE rd_work_records SET business_no=?1 WHERE id=?2",
        postgres_compat::params![draft_business_no, draft_id],
    )?;
    draft = get_by_id_on_conn(&tx, draft_id)?;
    let draft_after = snap(&draft);
    log(
        &tx,
        "return_resubmit_draft",
        &draft,
        confirmed_by,
        &format!(
            "由退回记录 {} 复制创建，修改保存后重新提交",
            record.business_no
        ),
        None,
        Some(&draft.status),
        None,
        Some(&draft_after),
    )?;
    tx.commit()?;
    get_by_id_on_conn(&conn, draft_id)
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
        conn.execute(
            "INSERT INTO users(username,password,is_active) VALUES('sender01','test-password',1)",
            [],
        )
        .unwrap();
        let sender_user_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO users(username,password,is_active) VALUES('public01','test-password',1)",
            [],
        )
        .unwrap();
        let public_user_id = conn.last_insert_rowid();
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
            "INSERT INTO method_types(name,sort_order) VALUES('Type-RD',1)",
            [],
        )
        .unwrap();
        let method_type = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO method_type_links(method_id,method_type_id) VALUES(?1,?2)",
            postgres_compat::params![method, method_type],
        )
        .unwrap();
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
        assert!(validate_submission_selection(&conn, project, method, "Type-RD").is_ok());
        assert!(validate_submission_selection(&conn, project, method, "Other-Type").is_err());
        drop(conn);
        let record = create(
            &pool,
            &RecordCreate {
                project_id: project,
                method_id: Some(method),
                user_name: "sender01".into(),
                sender_user_id: Some(sender_user_id),
                quantity: 2,
                recorded_at: "2000-01-01T10:00:00".into(),
                group_id: Some(group),
                multiplier: None,
                high_item: None,
                division_id: None,
                extra_fields: None,
                source_type: None,
                source_record_id: None,
            },
            Some("B001".into()),
            None,
            public_user_id,
            "public01",
        )
        .unwrap();
        assert!(record.business_no.starts_with("RD-20000101-"));
        assert_eq!(record.user_name, "sender01");
        assert_eq!(record.subject_user_id, Some(sender_user_id));
        assert_eq!(record.created_by_user_id, Some(public_user_id));
        assert_eq!(record.coefficient_snapshot, 2.5);
        assert_eq!(record.high_item.as_deref(), Some("HighItem01"));
        let later_unprocessed = create(
            &pool,
            &RecordCreate {
                project_id: project,
                method_id: Some(method),
                user_name: "sender02".into(),
                sender_user_id: Some(sender_user_id),
                quantity: 1,
                recorded_at: "2001-01-01T10:00:00".into(),
                group_id: Some(group),
                multiplier: None,
                high_item: None,
                division_id: None,
                extra_fields: None,
                source_type: None,
                source_record_id: None,
            },
            None,
            None,
            sender_user_id,
            "sender01",
        )
        .unwrap();
        let older_pending = create(
            &pool,
            &RecordCreate {
                project_id: project,
                method_id: Some(method),
                user_name: "sender03".into(),
                sender_user_id: Some(sender_user_id),
                quantity: 1,
                recorded_at: "1999-01-01T10:00:00".into(),
                group_id: Some(group),
                multiplier: None,
                high_item: None,
                division_id: None,
                extra_fields: None,
                source_type: None,
                source_record_id: None,
            },
            None,
            None,
            sender_user_id,
            "sender01",
        )
        .unwrap();
        let sampled = sample(&pool, older_pending.id, "sampler01", "sampler01").unwrap();
        assert_eq!(sampled.status, "已取样");
        assert_eq!(sampled.sampler.as_deref(), Some("sampler01"));
        let withdrawn = withdraw_sample(
            &pool,
            older_pending.id,
            "误点取样，重新核对样品",
            "sampler01",
        )
        .unwrap();
        assert_eq!(withdrawn.status, "待取样");
        assert!(withdrawn.sampler.is_none());
        assert!(withdrawn.sampled_at.is_none());
        let trace =
            crate::repo::trace_repo::list(&pool, "rd_work_records", older_pending.id).unwrap();
        assert!(trace
            .iter()
            .any(|event| event.event_type == "sample_withdraw"));
        sample(&pool, older_pending.id, "sampler01", "sampler01").unwrap();
        let (after_sampling_older_record, _) = list(
            &pool, None, None, None, None, None, None, None, None, 1, 20, false, None, None,
        )
        .unwrap();
        let newer_position = after_sampling_older_record
            .iter()
            .position(|item| item.id == later_unprocessed.id)
            .unwrap();
        let sampled_older_position = after_sampling_older_record
            .iter()
            .position(|item| item.id == older_pending.id)
            .unwrap();
        assert_eq!(after_sampling_older_record[0].id, later_unprocessed.id);
        assert_eq!(after_sampling_older_record[1].id, record.id);
        assert_eq!(after_sampling_older_record[2].id, older_pending.id);
        assert_eq!(after_sampling_older_record[0].sequence_no, 3);
        assert_eq!(after_sampling_older_record[1].sequence_no, 2);
        assert_eq!(after_sampling_older_record[2].sequence_no, 1);
        assert!(sampled_older_position > newer_position);
        let (ascending_records, _) = list(
            &pool,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            1,
            20,
            false,
            Some("submitted_at"),
            Some("asc"),
        )
        .unwrap();
        assert_eq!(ascending_records[0].id, older_pending.id);
        assert_eq!(ascending_records[2].id, later_unprocessed.id);
        assert_eq!(ascending_records[0].sequence_no, 1);
        assert_eq!(ascending_records[1].sequence_no, 2);
        assert_eq!(ascending_records[2].sequence_no, 3);
        pool.get().unwrap().execute("UPDATE projects SET name='Renamed',coefficient=9.0,high_item='ChangedHighItem' WHERE id=?1",[project]).unwrap();
        let reread = get_by_id(&pool, record.id).unwrap();
        assert_eq!(reread.project_name, "RdProject01");
        assert_eq!(reread.coefficient_snapshot, 2.5);
        assert_eq!(reread.high_item.as_deref(), Some("HighItem01"));
        let returned = return_record(&pool, record.id, "批号缺失", "analyst01").unwrap();
        assert_eq!(returned.status, "已退回");
        assert_eq!(returned.return_reason, "批号缺失");
        assert_eq!(returned.returned_by, "analyst01");
        assert!(returned.last_activity_at > returned.recorded_at);
        let (after_return, _) = list(
            &pool, None, None, None, None, None, None, None, None, 1, 20, false, None, None,
        )
        .unwrap();
        assert_eq!(after_return[0].id, later_unprocessed.id);
        assert_eq!(after_return[1].id, record.id);
        assert_eq!(after_return[2].id, older_pending.id);
        assert!(after_return
            .iter()
            .any(|item| item.id == later_unprocessed.id));
        let group_with_return = crate::repo::group_repo::get_by_id(&pool, group).unwrap();
        assert_eq!(
            group_with_return.returned_sender_names.as_deref(),
            Some("sender01")
        );
        assert!(sample(&pool, record.id, "sampler01", "sampler01").is_err());
        assert!(update(
            &pool,
            record.id,
            &RecordUpdate {
                user_name: Some("sender01-updated".into()),
                sender_user_id: None,
                quantity: Some(3),
                recorded_at: None,
                multiplier: None,
                project_id: None,
                method_id: None,
                group_id: None,
                division_id: None,
                batch_no: None,
                notes: None,
                high_item: None,
                extra_fields: None,
                source_type: None,
                source_record_id: None,
            },
            "sender01",
        )
        .is_err());
        let draft = confirm_return(&pool, record.id, "sender01").unwrap();
        assert_ne!(draft.id, record.id);
        assert_eq!(draft.status, "退回待修改");
        let confirmed_original = get_by_id(&pool, record.id).unwrap();
        assert_eq!(confirmed_original.status, "已退回已确认");
        assert_eq!(confirmed_original.return_confirmed_by, "sender01");
        let group_after_confirmation = crate::repo::group_repo::get_by_id(&pool, group).unwrap();
        assert_eq!(group_after_confirmation.returned_sender_names, None);
        let resubmitted = update(
            &pool,
            draft.id,
            &RecordUpdate {
                user_name: Some("sender01-updated".into()),
                sender_user_id: None,
                quantity: Some(3),
                recorded_at: None,
                multiplier: None,
                project_id: None,
                method_id: None,
                group_id: None,
                division_id: None,
                batch_no: None,
                notes: None,
                high_item: None,
                extra_fields: None,
                source_type: None,
                source_record_id: None,
            },
            "sender01",
        )
        .unwrap();
        assert_eq!(resubmitted.status, "待取样");
        assert_eq!(resubmitted.quantity, 3);
        assert_eq!(resubmitted.user_name, "sender01-updated");
        assert_eq!(resubmitted.subject_user_id, Some(sender_user_id));
        assert!(resubmitted.last_activity_at >= draft.last_activity_at);
        let (after_confirmation, _) = list(
            &pool, None, None, None, None, None, None, None, None, 1, 20, false, None, None,
        )
        .unwrap();
        assert_eq!(after_confirmation[0].id, draft.id);
        let sampled = sample(&pool, draft.id, "sampler01", "sampler01").unwrap();
        assert_eq!(sampled.status, "已取样");
        assert_eq!(sampled.sampler.as_deref(), Some("sampler01"));
        let events = crate::repo::trace_repo::list(&pool, "rd_work_records", record.id).unwrap();
        assert_eq!(
            events
                .iter()
                .map(|event| event.event_type.as_str())
                .collect::<Vec<_>>(),
            vec!["create", "return", "return_confirm"]
        );
        let draft_events =
            crate::repo::trace_repo::list(&pool, "rd_work_records", draft.id).unwrap();
        assert_eq!(
            draft_events
                .iter()
                .map(|event| event.event_type.as_str())
                .collect::<Vec<_>>(),
            vec!["return_resubmit_draft", "update", "sample"]
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
