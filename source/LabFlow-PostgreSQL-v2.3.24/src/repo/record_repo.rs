use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::record::{RecordCreate, RecordResponse, RecordUpdate};
use crate::repo::{audit_repo, trace_repo, trash_repo};
use crate::service::authz_service;
use postgres_compat::OptionalExtension;

const SELECT_RECORD: &str = "SELECT wr.id, wr.business_no, wr.project_id, wr.method_id,
            COALESCE(NULLIF(wr.project_name_snapshot,''), p.name),
            COALESCE(NULLIF(wr.lab_name_snapshot,''), pg.name, '未知'),
            wr.user_name, wr.quantity, wr.multiplier, wr.recorded_at, wr.created_at,
            wr.deleted_at,
            COALESCE(NULLIF(wr.method_name_snapshot,''), NULLIF(m.full_name,''), NULLIF(m.name,'')),
            method_types_agg.types,
            COALESCE(NULLIF(wr.instrument_code_snapshot,''), i.code, ''),
            COALESCE(NULLIF(wr.instrument_type_snapshot,''), i.instrument_type, ''),
            COALESCE(NULLIF(wr.high_item_snapshot,''), wr.high_item, p.high_item),
            COALESCE(wr.coefficient_snapshot,1.0), wr.subject_user_id, wr.created_by_user_id,
            wr.project_division_id, wr.execution_division_id, wr.execution_group_id,
            wr.detection_division_id, wr.sending_division_id, wr.sending_group_id
     FROM work_records wr
     JOIN projects p ON wr.project_id=p.id
     LEFT JOIN methods m ON wr.method_id=m.id
     LEFT JOIN instruments i ON i.id=m.instrument_id
     LEFT JOIN project_groups pg ON pg.id=wr.group_id
     LEFT JOIN LATERAL (
         SELECT string_agg(DISTINCT mt.name, ',') AS types
         FROM method_type_links mtl
         JOIN method_types mt ON mtl.method_type_id=mt.id
         WHERE mtl.method_id=wr.method_id
     ) method_types_agg ON true";

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
        project_division_id: row.get(20)?,
        execution_division_id: row.get(21)?,
        execution_group_id: row.get(22)?,
        detection_division_id: row.get(23)?,
        sending_division_id: row.get(24)?,
        sending_group_id: row.get(25)?,
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
        "detection_division_id": record.detection_division_id,
        "sending_division_id": record.sending_division_id,
        "sending_group_id": record.sending_group_id,
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

/// v2.3.13: 样品信息工作量录入的宽松绑定校验。
/// 不强制「项目-方法」关联和「方法-仪器」绑定，避免历史配置无法补录工作量。
fn validate_sample_workload_bindings(
    conn: &postgres_compat::Connection,
    project_id: i64,
    method_id: Option<i64>,
) -> Result<()> {
    let project_exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM projects WHERE id=?1 AND is_active=1 AND deleted_at IS NULL",
        [project_id],
        |row| row.get(0),
    )?;
    if project_exists == 0 {
        return Err(AppError::Validation("项目不存在或已停用".into()));
    }

    if let Some(method_id) = method_id {
        let valid_method: i64 = conn.query_row(
            "SELECT COUNT(*) FROM methods WHERE id=?1 AND is_active=1 AND deleted_at IS NULL",
            [method_id],
            |row| row.get(0),
        )?;
        if valid_method == 0 {
            return Err(AppError::Validation("检测方法不存在或已停用".into()));
        }
    }
    Ok(())
}

pub fn list(
    pool: &DbPool,
    project_id: Option<i64>,
    group_id: Option<i64>,
    subject_user_id: Option<i64>,
    created_by_user_id: Option<i64>,
    user_name: Option<&str>,
    division_id: Option<i64>,
    start: Option<&str>,
    end: Option<&str>,
    page: i64,
    page_size: i64,
    include_deleted: bool,
    allowed_division_ids: Option<&[i64]>,
    detection_division_ids: Option<&str>,
    sending_division_ids: Option<&str>,
    include_pending_ownership: bool,
) -> Result<(Vec<RecordResponse>, i64)> {
    let conn = pool.get()?;
    let mut clauses = vec![];
    let mut params: Vec<Box<dyn postgres_compat::types::ToSql>> = vec![];
    let detection_ids = parse_dimension_ids(detection_division_ids, "检测部门")?;
    let sending_ids = parse_dimension_ids(sending_division_ids, "送样部门")?;
    if !include_deleted {
        clauses.push("wr.deleted_at IS NULL".to_string());
    }
    if !include_pending_ownership {
        clauses
            .push("COALESCE(wr.ownership_status,'confirmed')<>'pending_confirmation'".to_string());
    }
    if let Some(value) = project_id {
        clauses.push(format!("wr.project_id={value}"));
    }
    if let Some(value) = group_id {
        clauses.push(format!("wr.group_id={value}"));
    }
    if let Some(value) = division_id {
        clauses.push(format!(
            "{}={value}",
            authz_service::work_record_authorization_division_sql("wr")
        ));
    }
    if let Some(ids) = allowed_division_ids {
        if ids.is_empty() {
            clauses.push("1=0".to_string());
        } else {
            let values = ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
            clauses.push(format!(
                "{} IN ({values})",
                authz_service::work_record_authorization_division_sql("wr")
            ));
        }
    }
    if let Some(ids) = detection_ids {
        if ids.is_empty() {
            clauses.push("1=0".to_string());
        } else {
            let values = ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
            clauses.push(format!("wr.detection_division_id IN ({values})"));
        }
    }
    if let Some(ids) = sending_ids {
        if ids.is_empty() {
            clauses.push("1=0".to_string());
        } else {
            let values = ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
            clauses.push(format!(
                "{} IN ({values})",
                authz_service::work_record_sending_division_sql("wr")
            ));
        }
    }
    if let Some(value) = subject_user_id {
        clauses.push(format!("wr.subject_user_id={value}"));
    }
    if let Some(value) = created_by_user_id {
        clauses.push(format!("wr.created_by_user_id={value}"));
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

pub fn parse_dimension_ids(raw: Option<&str>, label: &str) -> Result<Option<Vec<i64>>> {
    let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let mut ids = raw
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            value
                .parse::<i64>()
                .map_err(|_| AppError::Validation(format!("{label}筛选值无效")))
        })
        .collect::<Result<Vec<_>>>()?;
    ids.sort_unstable();
    ids.dedup();
    Ok(Some(ids))
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

pub fn source_quantity(pool: &DbPool, source_type: &str, source_record_id: i64) -> Result<i32> {
    let conn = pool.get()?;
    Ok(conn.query_row(
        "SELECT COALESCE(SUM(quantity),0)::BIGINT FROM work_records WHERE source_type=?1 AND source_record_id=?2 AND deleted_at IS NULL",
        postgres_compat::params![source_type, source_record_id],
        |row| row.get::<_, i64>(0),
    )? as i32)
}

pub fn exists_source(pool: &DbPool, source_type: &str, source_record_id: i64) -> Result<bool> {
    Ok(source_quantity(pool, source_type, source_record_id)? > 0)
}

pub fn source_fully_recorded(
    pool: &DbPool,
    source_type: &str,
    source_record_id: i64,
    required_quantity: i32,
) -> Result<bool> {
    Ok(source_quantity(pool, source_type, source_record_id)? >= required_quantity)
}

/// 样品信息登记历史版本曾使用过 `sample_info` 作为来源类型；保留兼容判断，
/// 让升级后的列表仍能识别已经录入过的工作量。
pub fn sample_info_source_quantity(pool: &DbPool, source_record_id: i64) -> Result<i32> {
    let conn = pool.get()?;
    Ok(conn.query_row(
        "SELECT COALESCE(SUM(quantity),0)::BIGINT FROM work_records WHERE source_type IN ('sample_info_sample','sample_info') AND source_record_id=?1 AND deleted_at IS NULL",
        [source_record_id],
        |row| row.get::<_, i64>(0),
    )? as i32)
}

pub fn exists_sample_info_source(pool: &DbPool, source_record_id: i64) -> Result<bool> {
    Ok(sample_info_source_quantity(pool, source_record_id)? > 0)
}

/// v2.3.19：同一取样来源（研发送样 / 样品信息）的累计录入量不得超过来源数量。
/// 校验与插入放在同一事务内，并对来源行加锁，避免并发请求或重复点击把工作量超额累加。
/// 放在 create_inner 是为了同时覆盖分析检测录入与取样工作量录入两条入口。
fn guard_source_quantity(tx: &postgres_compat::Transaction<'_>, body: &RecordCreate) -> Result<()> {
    let (Some(source_type), Some(source_record_id)) =
        (body.source_type.as_deref(), body.source_record_id)
    else {
        return Ok(());
    };
    let source_table = match source_type {
        "rd_sample" => "rd_work_records",
        // 历史数据用 sample_info 作为来源类型，与 sample_info_source_quantity 的兼容口径一致。
        "sample_info_sample" | "sample_info" => "sample_info_records",
        _ => return Ok(()),
    };
    let source_quantity: Option<i64> = tx
        .query_row(
            &format!("SELECT quantity FROM {source_table} WHERE id=?1 FOR UPDATE"),
            [source_record_id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(source_quantity) = source_quantity else {
        return Err(AppError::Validation("取样来源记录不存在".into()));
    };
    let recorded: i64 = if source_type == "rd_sample" {
        tx.query_row(
            "SELECT COALESCE(SUM(quantity),0)::BIGINT FROM work_records
             WHERE source_type='rd_sample' AND source_record_id=?1 AND deleted_at IS NULL",
            [source_record_id],
            |row| row.get(0),
        )?
    } else {
        tx.query_row(
            "SELECT COALESCE(SUM(quantity),0)::BIGINT FROM work_records
             WHERE source_type IN ('sample_info_sample','sample_info')
               AND source_record_id=?1 AND deleted_at IS NULL",
            [source_record_id],
            |row| row.get(0),
        )?
    };
    let remaining = (source_quantity - recorded).max(0);
    if body.quantity as i64 > remaining {
        return Err(AppError::Validation(format!(
            "本次录入数量不能超过剩余数量 {remaining}"
        )));
    }
    Ok(())
}

pub fn create(pool: &DbPool, body: &RecordCreate, operator: &str) -> Result<RecordResponse> {
    create_inner(pool, body, operator, true, None)
}

/// v2.3.13: 样品信息取样工作量录入使用放宽的绑定校验。
/// 启用中的检测方法允许未关联当前项目或未绑定启用仪器，历史配置也能补录工作量。
/// v2.3.14: `custom` 为方法库检索不到候选时手工填写的方法/仪器名称
/// （`(方法名, 可选仪器名)`），仅在 `method_id` 为空时生效。
pub fn create_sample_workload(
    pool: &DbPool,
    body: &RecordCreate,
    operator: &str,
    custom: Option<(&str, Option<&str>)>,
) -> Result<RecordResponse> {
    create_inner(pool, body, operator, false, custom)
}

fn create_inner(
    pool: &DbPool,
    body: &RecordCreate,
    operator: &str,
    strict_bindings: bool,
    custom: Option<(&str, Option<&str>)>,
) -> Result<RecordResponse> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    if strict_bindings {
        validate_record_bindings(&tx, body.project_id, body.method_id, body.group_id)?;
    } else {
        validate_sample_workload_bindings(&tx, body.project_id, body.method_id)?;
    }
    guard_source_quantity(&tx, body)?;
    // 自定义录入时用方法库为空，方法/仪器快照回退到手工填写的名称。
    let custom_method = custom.map(|(name, _)| name).unwrap_or("");
    let custom_instrument = custom.and_then(|(_, instrument)| instrument).unwrap_or("");

    // v0.3.26 性能优化：将子查询改为单个 LEFT JOIN，减少查询次数
    let inserted = match tx.execute(
        "INSERT INTO work_records
         (project_id,method_id,user_name,quantity,recorded_at,group_id,division_id,multiplier,high_item,
           project_division_id,project_division_name_snapshot,execution_division_id,execution_division_name_snapshot,execution_group_id,execution_group_name_snapshot,
           project_name_snapshot,lab_name_snapshot,method_name_snapshot,instrument_code_snapshot,instrument_type_snapshot,high_item_snapshot,coefficient_snapshot,
           subject_user_id,created_by_user_id,created_by_username_snapshot,instrument_id_snapshot,
           detection_division_id,detection_division_name_snapshot,
           sending_division_id,sending_division_name_snapshot,sending_group_id,sending_group_name_snapshot,
           source_type,source_record_id)
         SELECT ?1,?2,?3,?4,?5,?6,?7,
                COALESCE(?8,m.multiplier,1.0),
                COALESCE(?9,p.high_item),p.project_division_id,COALESCE(p.project_division_name_snapshot,''),
                COALESCE(?7,pg.division_id),COALESCE(d.name,''),?6,COALESCE(pg.name,''),
                p.name,COALESCE(pg.name,''),
                COALESCE(NULLIF(m.full_name,''),m.name,NULLIF(?14,''),''),
                COALESCE(i.code,NULLIF(?15,''),''),
                COALESCE(i.instrument_type,''),
                COALESCE(?9,p.high_item,''),COALESCE(p.coefficient,1.0),
                COALESCE(?10,subject.id),
                creator.id,?11,
                m.instrument_id,
                subject.division_id,COALESCE(detection_division.name,''),
                COALESCE(?7,pg.division_id),COALESCE(d.name,''),?6,COALESCE(pg.name,''),?12,?13
         FROM projects p
         LEFT JOIN project_groups pg ON pg.id=?6
         LEFT JOIN divisions d ON d.id=COALESCE(?7,pg.division_id)
         LEFT JOIN methods m ON m.id=?2
         LEFT JOIN instruments i ON i.id=m.instrument_id
         LEFT JOIN users subject ON subject.id=COALESCE(?10,(SELECT id FROM users WHERE username=?3 ORDER BY id LIMIT 1))
         LEFT JOIN divisions detection_division ON detection_division.id=subject.division_id
         LEFT JOIN users creator ON creator.username=?11
         WHERE p.id=?1",
        postgres_compat::params![body.project_id,body.method_id,&body.user_name,body.quantity,&body.recorded_at,
            body.group_id,body.division_id,body.multiplier,body.high_item,body.sender_user_id,operator,
            body.source_type,body.source_record_id,custom_method,custom_instrument],
    ) {
        Ok(inserted) => inserted,
        Err(error) => return Err(error.into()),
    };
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
                 subject_user_id=(SELECT id FROM users WHERE username=?1 ORDER BY id LIMIT 1),
                 detection_division_id=(SELECT division_id FROM users WHERE username=?1 ORDER BY id LIMIT 1),
                 detection_division_name_snapshot=COALESCE((SELECT d.name FROM divisions d WHERE d.id=(SELECT division_id FROM users WHERE username=?1 ORDER BY id LIMIT 1)), '')
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
            tx.execute("UPDATE work_records SET project_id=?1,project_name_snapshot=(SELECT name FROM projects WHERE id=?1),project_division_id=(SELECT project_division_id FROM projects WHERE id=?1),project_division_name_snapshot=COALESCE((SELECT project_division_name_snapshot FROM projects WHERE id=?1),''),high_item_snapshot=COALESCE((SELECT high_item FROM projects WHERE id=?1),''),coefficient_snapshot=COALESCE((SELECT coefficient FROM projects WHERE id=?1),1.0) WHERE id=?2",postgres_compat::params![value,id])?;
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
        tx.execute("UPDATE work_records SET group_id=?1,division_id=COALESCE((SELECT division_id FROM project_groups WHERE id=?1),division_id),lab_name_snapshot=COALESCE((SELECT name FROM project_groups WHERE id=?1),''),execution_group_id=?1,execution_group_name_snapshot=COALESCE((SELECT name FROM project_groups WHERE id=?1),''),execution_division_id=COALESCE((SELECT division_id FROM project_groups WHERE id=?1),division_id),execution_division_name_snapshot=COALESCE((SELECT d.name FROM divisions d WHERE d.id=COALESCE((SELECT division_id FROM project_groups WHERE id=?1),division_id)), ''),sending_group_id=?1,sending_group_name_snapshot=COALESCE((SELECT name FROM project_groups WHERE id=?1),''),sending_division_id=COALESCE((SELECT division_id FROM project_groups WHERE id=?1),division_id),sending_division_name_snapshot=COALESCE((SELECT d.name FROM divisions d WHERE d.id=COALESCE((SELECT division_id FROM project_groups WHERE id=?1),division_id)), '') WHERE id=?2",postgres_compat::params![value,id])?;
    }
    if let Some(value) = body.division_id {
        changes.push(format!("部门更新为ID {}", value));
        tx.execute(
            "UPDATE work_records SET division_id=?1,execution_division_id=COALESCE(?1,(SELECT division_id FROM project_groups WHERE id=group_id)),execution_division_name_snapshot=COALESCE((SELECT d.name FROM divisions d WHERE d.id=COALESCE(?1,(SELECT division_id FROM project_groups WHERE id=group_id))), ''),sending_division_id=COALESCE(?1,(SELECT division_id FROM project_groups WHERE id=group_id)),sending_division_name_snapshot=COALESCE((SELECT d.name FROM divisions d WHERE d.id=COALESCE(?1,(SELECT division_id FROM project_groups WHERE id=group_id))), '') WHERE id=?2",
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
    created_by_user_id: Option<i64>,
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
    if let Some(user_id) = created_by_user_id {
        let i = params.len() + 1;
        select.push_str(&format!(" AND created_by_user_id=?{i}"));
        params.push(Box::new(user_id));
    }
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
                sender_user_id: None,
                quantity: 3,
                recorded_at: "2026-07-18T09:00:00".into(),
                group_id: Some(group),
                multiplier: None,
                high_item: Some("High01".into()),
                division_id: None,
                extra_fields: None,
                source_type: None,
                source_record_id: None,
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
    fn public_account_creation_preserves_actual_detector_identity() {
        let (pool, group, project, method) = setup();
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO users(username,password,is_admin,is_active) VALUES('analysis_public','hash',0,1)",
            [],
        )
        .unwrap();
        let public_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO users(username,password,is_admin,is_active) VALUES('detector02','hash',0,1)",
            [],
        )
        .unwrap();
        let detector_id = conn.last_insert_rowid();
        drop(conn);

        let record = create(
            &pool,
            &RecordCreate {
                project_id: project,
                method_id: Some(method),
                user_name: "detector02".into(),
                sender_user_id: Some(detector_id),
                quantity: 2,
                recorded_at: "2026-08-10T09:00:00".into(),
                group_id: Some(group),
                multiplier: None,
                high_item: None,
                division_id: None,
                extra_fields: None,
                source_type: None,
                source_record_id: None,
            },
            "analysis_public",
        )
        .unwrap();

        assert_eq!(record.user_name, "detector02");
        assert_eq!(record.subject_user_id, Some(detector_id));
        assert_eq!(record.created_by_user_id, Some(public_id));
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
                sender_user_id: None,
                quantity: 3,
                recorded_at: "2026-07-18T09:00:00".into(),
                group_id: Some(group),
                multiplier: None,
                high_item: None,
                division_id: None,
                extra_fields: None,
                source_type: None,
                source_record_id: None,
            },
            "creator01",
        )
        .unwrap();
        update(
            &pool,
            record.id,
            &RecordUpdate {
                user_name: None,
                sender_user_id: None,
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
                extra_fields: None,
                source_type: None,
                source_record_id: None,
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
                sender_user_id: None,
                quantity: 1,
                recorded_at: "2026-07-18T09:00:00".into(),
                group_id: Some(group),
                multiplier: None,
                high_item: None,
                division_id: None,
                extra_fields: None,
                source_type: None,
                source_record_id: None,
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
                sender_user_id: None,
                quantity: 1,
                recorded_at: "2026-07-18T10:00:00".into(),
                group_id: Some(group),
                multiplier: Some(0.0),
                high_item: None,
                division_id: None,
                extra_fields: None,
                source_type: None,
                source_record_id: None,
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
