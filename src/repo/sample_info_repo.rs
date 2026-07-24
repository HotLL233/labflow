use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::sample_info::{
    SampleInfoCreate, SampleInfoQuery, SampleInfoRecord, SampleInfoResponse, SampleInfoUpdate,
};
use crate::repo::{audit_repo, trace_repo, trash_repo};

const STATUS_ORDER: &[&str] = &["待取样", "待检测", "已检测"];

/// 构建通用 WHERE 子句（不含分页），所有筛选维度：
/// 时间(submitted_at) / 送样人 / 实验室 / 项目 / 类型(type_key) / 状态
fn build_where(q: &SampleInfoQuery) -> (String, Vec<String>) {
    let mut clauses: Vec<String> = if q.include_deleted.unwrap_or(false) {
        vec![]
    } else {
        vec!["deleted_at IS NULL".to_string()]
    };
    let mut params: Vec<String> = vec![];
    if let Some(dt) = &q.detection_type {
        if !dt.is_empty() {
            let i = params.len() + 1;
            clauses.push(format!("detection_type=?{}", i));
            params.push(dt.clone());
        }
    }
    if let Some(tk) = &q.type_key {
        if !tk.is_empty() {
            let i = params.len() + 1;
            clauses.push(format!("type_key=?{}", i));
            params.push(tk.clone());
        }
    }
    if let Some(s) = &q.status {
        if !s.is_empty() && s != "全部" {
            let i = params.len() + 1;
            clauses.push(format!("status=?{}", i));
            params.push(s.clone());
        }
    }
    if let Some(u) = &q.user_name {
        if !u.is_empty() {
            let i = params.len() + 1;
            clauses.push(format!("user_name=?{}", i));
            params.push(u.clone());
        }
    }
    if let Some(l) = &q.lab_name {
        if !l.is_empty() {
            let i = params.len() + 1;
            clauses.push(format!("lab_name=?{}", i));
            params.push(l.clone());
        }
    }
    if let Some(p) = &q.project_name {
        if !p.is_empty() {
            let i = params.len() + 1;
            clauses.push(format!("project_name=?{}", i));
            params.push(p.clone());
        }
    }
    if let Some(d) = q.division_id {
        let i = params.len() + 1;
        clauses.push(format!("division_id=?{}", i));
        params.push(d.to_string());
    }
    if let Some(group_id) = q.group_id {
        let i = params.len() + 1;
        clauses.push(format!("group_id=?{}", i));
        params.push(group_id.to_string());
    }
    if let Some(user_id) = q.created_by_user_id {
        let i = params.len() + 1;
        clauses.push(format!("created_by_user_id=?{}", i));
        params.push(user_id.to_string());
    }
    if let Some(s) = &q.start {
        let i = params.len() + 1;
        clauses.push(format!("submitted_at>=?{}", i));
        params.push(s.clone());
    }
    if let Some(e) = &q.end {
        let i = params.len() + 1;
        clauses.push(format!("submitted_at<=?{}", i));
        params.push(format!("{}T23:59:59", e));
    }
    let where_sql = if clauses.is_empty() {
        "1=1".into()
    } else {
        clauses.join(" AND ")
    };
    (where_sql, params)
}

/// 分页查询，支持全部维度筛选，未软删除
pub fn list(pool: &DbPool, q: &SampleInfoQuery) -> Result<(Vec<SampleInfoResponse>, i64)> {
    let conn = pool.get()?;
    let page = q.page.unwrap_or(1).max(1);
    let page_size = q.page_size.unwrap_or(20).min(500);

    let (where_sql, params) = build_where(q);

    let sql = format!(
        "SELECT id, business_no, status, seq_no, batch_no, user_name, lab_name, project_name, \
         submitted_at, detection_date, sampled_by, sampled_at, detected_by, \
         main_components, detection_type, type_key, division_id, quantity, notes, extra_fields, \
         created_at, updated_at, deleted_at, \
         (SELECT name FROM divisions d WHERE d.id=sample_info_records.division_id), \
         group_id, created_by_user_id \
         FROM sample_info_records WHERE {} ORDER BY created_at DESC \
         LIMIT {} OFFSET {}",
        where_sql,
        page_size,
        (page - 1) * page_size
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        postgres_compat::params_from_iter(
            params
                .iter()
                .map(|p| p as &dyn postgres_compat::types::ToSql),
        ),
        |row| {
            Ok(SampleInfoRecord {
                id: row.get(0)?,
                business_no: row.get::<_, String>(1).unwrap_or_default(),
                status: row.get(2)?,
                seq_no: row.get(3)?,
                batch_no: row.get(4)?,
                user_name: row.get(5)?,
                lab_name: row.get(6)?,
                project_name: row.get(7)?,
                submitted_at: row.get(8)?,
                detection_date: row.get(9)?,
                sampled_by: row.get::<_, String>(10).unwrap_or_default(),
                sampled_at: row.get(11)?,
                detected_by: row.get::<_, String>(12).unwrap_or_default(),
                main_components: row.get(13)?,
                detection_type: row.get(14)?,
                type_key: row.get(15)?,
                division_id: row.get(16)?,
                quantity: row.get(17)?,
                notes: row.get::<_, String>(18).unwrap_or_default(),
                extra_fields: row
                    .get::<_, Option<String>>(19)
                    .unwrap_or(Some("{}".into())),
                created_at: row.get(20)?,
                updated_at: row.get(21)?,
                deleted_at: row.get(22)?,
                division_name: row.get(23)?,
                group_id: row.get(24)?,
                created_by_user_id: row.get(25)?,
            })
        },
    )?;
    let items: Vec<SampleInfoResponse> = rows
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .map(SampleInfoResponse::from)
        .collect();

    // Count
    let count_sql = format!(
        "SELECT COUNT(*) FROM sample_info_records WHERE {}",
        where_sql
    );
    let count: i64 = conn.query_row(
        &count_sql,
        postgres_compat::params_from_iter(
            params
                .iter()
                .map(|p| p as &dyn postgres_compat::types::ToSql),
        ),
        |r| r.get(0),
    )?;

    Ok((items, count))
}

/// Internal helper: query a single record on an existing connection
fn get_by_id_on_conn(conn: &postgres_compat::Connection, id: i64) -> Result<SampleInfoRecord> {
    conn.query_row(
        "SELECT id, business_no, status, seq_no, batch_no, user_name, lab_name, project_name, \
         submitted_at, detection_date, sampled_by, sampled_at, detected_by, \
         main_components, detection_type, type_key, division_id, quantity, notes, extra_fields, \
         created_at, updated_at, deleted_at, \
         (SELECT name FROM divisions d WHERE d.id=sample_info_records.division_id), \
         group_id, created_by_user_id \
         FROM sample_info_records WHERE id=?1",
        [id],
        |row| {
            Ok(SampleInfoRecord {
                id: row.get(0)?,
                business_no: row.get::<_, String>(1).unwrap_or_default(),
                status: row.get(2)?,
                seq_no: row.get(3)?,
                batch_no: row.get(4)?,
                user_name: row.get(5)?,
                lab_name: row.get(6)?,
                project_name: row.get(7)?,
                submitted_at: row.get(8)?,
                detection_date: row.get(9)?,
                sampled_by: row.get::<_, String>(10).unwrap_or_default(),
                sampled_at: row.get(11)?,
                detected_by: row.get::<_, String>(12).unwrap_or_default(),
                main_components: row.get(13)?,
                detection_type: row.get(14)?,
                type_key: row.get(15)?,
                division_id: row.get(16)?,
                quantity: row.get(17)?,
                notes: row.get::<_, String>(18).unwrap_or_default(),
                extra_fields: row
                    .get::<_, Option<String>>(19)
                    .unwrap_or(Some("{}".into())),
                created_at: row.get(20)?,
                updated_at: row.get(21)?,
                deleted_at: row.get(22)?,
                division_name: row.get(23)?,
                group_id: row.get(24)?,
                created_by_user_id: row.get(25)?,
            })
        },
    )
    .map_err(|e| match e {
        postgres_compat::Error::QueryReturnedNoRows => {
            crate::error::AppError::NotFound("样品信息记录不存在".into())
        }
        _ => e.into(),
    })
}

/// 创建记录，自动计算 seq_no
pub fn create(
    pool: &DbPool,
    data: &SampleInfoCreate,
    operator: &str,
) -> Result<SampleInfoResponse> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;

    // 如果未提供 submitted_at，使用当前时间
    let submitted_at = data
        .submitted_at
        .clone()
        .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string());

    // 计算 seq_no：同 detection_type + 同天最大 seq_no + 1
    let seq_no: i64 = tx.query_row(
        "SELECT COALESCE(MAX(seq_no), 0) + 1 FROM sample_info_records \
         WHERE detection_type = ?1 AND LEFT(submitted_at, 10) = LEFT(?2, 10) AND deleted_at IS NULL",
        postgres_compat::params![&data.detection_type, &submitted_at],
        |r| r.get(0),
    )?;

    let notes = data.notes.clone().unwrap_or_default();
    let detection_date = data.detection_date.clone().unwrap_or_default();
    let extra_fields = data
        .extra_fields
        .as_ref()
        .map(|v| v.to_string())
        .unwrap_or_else(|| "{}".into());

    tx.execute(
        "INSERT INTO sample_info_records \
         (status, seq_no, batch_no, user_name, lab_name, project_name, submitted_at, \
          detection_date, main_components, detection_type, type_key, \
           division_id, quantity, notes, extra_fields, group_id, created_by_user_id, created_by_username_snapshot) \
         VALUES ('待取样', ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
                 (SELECT id FROM project_groups WHERE name=?4 ORDER BY id LIMIT 1),
                 (SELECT id FROM users WHERE username=?15 ORDER BY id LIMIT 1),?15)",
        postgres_compat::params![
            seq_no,
            &data.batch_no,
            &data.user_name,
            &data.lab_name,
            &data.project_name,
            &submitted_at,
            &detection_date,
            &data.main_components,
            &data.detection_type,
            &data.type_key,
            &data.division_id,
            &data.quantity,
            &notes,
            &extra_fields,
            operator,
        ],
    )?;

    let id = tx.last_insert_rowid();
    let prefix = format!("SI-{}", data.type_key.to_ascii_uppercase());
    let business_no = trace_repo::make_business_no(&prefix, &submitted_at, id);
    tx.execute(
        "UPDATE sample_info_records SET business_no=?1 WHERE id=?2",
        postgres_compat::params![business_no, id],
    )?;
    let created = SampleInfoResponse::from(get_by_id_on_conn(&tx, id)?);
    let after = serde_json::to_value(&created).ok();
    let detail = format!(
        "创建样品信息#{}：检测类型「{}」，批号「{}」，送样人「{}」",
        id, &data.detection_type, &data.batch_no, &data.user_name
    );
    audit_repo::log_structured_on_conn(
        &tx,
        "create",
        "sample_info_records",
        Some(id),
        operator,
        &detail,
        "sample_info",
        &created.business_no,
        None,
        after.as_ref(),
        "record",
    )?;
    trace_repo::log_event_on_conn(
        &tx,
        "sample_info",
        "sample_info_records",
        id,
        &created.business_no,
        "create",
        None,
        Some("待取样"),
        operator,
        &detail,
        None,
        after.as_ref(),
    )?;
    tx.commit()?;

    let record = get_by_id_on_conn(&conn, id)?;
    Ok(SampleInfoResponse::from(record))
}

/// 更新记录，记录审计
pub fn update(
    pool: &DbPool,
    id: i64,
    data: &SampleInfoUpdate,
    user_name: &str,
) -> Result<SampleInfoResponse> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;

    let existing = get_by_id_on_conn(&tx, id)?;
    if existing.deleted_at.is_some() {
        return Err(AppError::Validation("记录已被删除，无法编辑".into()));
    }

    let before = serde_json::to_value(SampleInfoResponse::from(get_by_id_on_conn(&tx, id)?)).ok();
    let mut changed = false;
    let mut changes: Vec<String> = vec![];

    if data.status.is_some() {
        return Err(AppError::Validation(
            "状态只能通过取样或完成检测操作流转".into(),
        ));
    }
    if let Some(ref b) = data.batch_no {
        if b != &existing.batch_no {
            changes.push(format!("批号 {} → {}", existing.batch_no, b));
            tx.execute(
                "UPDATE sample_info_records SET batch_no=?1, updated_at=datetime('now','localtime') WHERE id=?2",
                postgres_compat::params![b, id],
            )?;
            changed = true;
        }
    }
    if let Some(ref u) = data.user_name {
        if u != &existing.user_name {
            changes.push(format!("送样人 {} → {}", existing.user_name, u));
            tx.execute(
                "UPDATE sample_info_records SET user_name=?1, updated_at=datetime('now','localtime') WHERE id=?2",
                postgres_compat::params![u, id],
            )?;
            changed = true;
        }
    }
    if let Some(ref l) = data.lab_name {
        if l != &existing.lab_name {
            changes.push(format!("实验室 {} → {}", existing.lab_name, l));
            tx.execute(
                "UPDATE sample_info_records SET lab_name=?1,
                 group_id=(SELECT id FROM project_groups WHERE name=?1 ORDER BY id LIMIT 1),
                 updated_at=datetime('now','localtime') WHERE id=?2",
                postgres_compat::params![l, id],
            )?;
            changed = true;
        }
    }
    if let Some(ref p) = data.project_name {
        if p != &existing.project_name {
            changes.push(format!("项目 {} → {}", existing.project_name, p));
            tx.execute(
                "UPDATE sample_info_records SET project_name=?1, updated_at=datetime('now','localtime') WHERE id=?2",
                postgres_compat::params![p, id],
            )?;
            changed = true;
        }
    }
    if let Some(ref sa) = data.submitted_at {
        if sa != &existing.submitted_at {
            changes.push(format!("送样时间 {} → {}", existing.submitted_at, sa));
            tx.execute(
                "UPDATE sample_info_records SET submitted_at=?1, updated_at=datetime('now','localtime') WHERE id=?2",
                postgres_compat::params![sa, id],
            )?;
            changed = true;
        }
    }
    if let Some(ref dd) = data.detection_date {
        if dd != &existing.detection_date {
            changes.push(format!("检测时间 {} → {}", existing.detection_date, dd));
            tx.execute(
                "UPDATE sample_info_records SET detection_date=?1, updated_at=datetime('now','localtime') WHERE id=?2",
                postgres_compat::params![dd, id],
            )?;
            changed = true;
        }
    }
    if let Some(ref mc) = data.main_components {
        if mc != &existing.main_components {
            changes.push(format!("主要成分 {} → {}", existing.main_components, mc));
            tx.execute(
                "UPDATE sample_info_records SET main_components=?1, updated_at=datetime('now','localtime') WHERE id=?2",
                postgres_compat::params![mc, id],
            )?;
            changed = true;
        }
    }
    if let Some(ref d) = data.division_id {
        if Some(*d) != existing.division_id {
            changes.push(format!("所属部门 {:?} → {:?}", existing.division_id, d));
            tx.execute(
                "UPDATE sample_info_records SET division_id=?1, updated_at=datetime('now','localtime') WHERE id=?2",
                postgres_compat::params![d, id],
            )?;
            changed = true;
        }
    }
    if let Some(ref q) = data.quantity {
        if *q != existing.quantity {
            changes.push(format!("送样数量 {} → {}", existing.quantity, q));
            tx.execute(
                "UPDATE sample_info_records SET quantity=?1, updated_at=datetime('now','localtime') WHERE id=?2",
                postgres_compat::params![q, id],
            )?;
            changed = true;
        }
    }
    if let Some(ref n) = data.notes {
        if n != &existing.notes {
            changes.push(format!("注意事项 {} → {}", existing.notes, n));
            tx.execute(
                "UPDATE sample_info_records SET notes=?1, updated_at=datetime('now','localtime') WHERE id=?2",
                postgres_compat::params![n, id],
            )?;
            changed = true;
        }
    }
    if let Some(ref ef) = data.extra_fields {
        let ef_str = ef.to_string();
        changes.push(format!("自定义字段已更新"));
        tx.execute(
            "UPDATE sample_info_records SET extra_fields=?1, updated_at=datetime('now','localtime') WHERE id=?2",
            postgres_compat::params![ef_str, id],
        )?;
        changed = true;
    }

    if !changed {
        return Err(AppError::Validation("没有需要更新的字段".into()));
    }
    if changes.is_empty() {
        changes.push("无变化".into());
    }
    let detail = format!("修改样品信息#{}：{}", id, changes.join("，"));
    let updated = SampleInfoResponse::from(get_by_id_on_conn(&tx, id)?);
    let after = serde_json::to_value(&updated).ok();
    audit_repo::log_structured_on_conn(
        &tx,
        "update",
        "sample_info_records",
        Some(id),
        user_name,
        &detail,
        "sample_info",
        &updated.business_no,
        before.as_ref(),
        after.as_ref(),
        "record",
    )?;
    trace_repo::log_event_on_conn(
        &tx,
        "sample_info",
        "sample_info_records",
        id,
        &updated.business_no,
        "update",
        None,
        None,
        user_name,
        &detail,
        before.as_ref(),
        after.as_ref(),
    )?;
    tx.commit()?;

    let record = get_by_id_on_conn(&conn, id)?;
    Ok(SampleInfoResponse::from(record))
}

/// 软删除
pub fn soft_delete(pool: &DbPool, id: i64, user_name: &str, reason: &str) -> Result<()> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;

    let existing_record = get_by_id_on_conn(&tx, id)?;
    let deleted = existing_record.deleted_at.clone();
    let existing = SampleInfoResponse::from(existing_record);

    if deleted.is_some() {
        return Err(AppError::Validation("记录已被删除".into()));
    }

    let rows = tx.execute(
        "UPDATE sample_info_records SET deleted_at=datetime('now','localtime') WHERE id=?1",
        [id],
    )?;
    if rows == 0 {
        return Err(AppError::NotFound("样品信息记录不存在".into()));
    }

    let detail = format!("删除样品信息#{}", id);
    let before = serde_json::to_value(&existing).ok();
    let updated = SampleInfoResponse::from(get_by_id_on_conn(&tx, id)?);
    let after = serde_json::to_value(&updated).ok();
    if let Some(ref snapshot) = before {
        trash_repo::move_to_trash_on_conn(
            &tx,
            "样品信息登记",
            "sample_info_records",
            id,
            "records",
            "sample_info",
            &updated.business_no,
            &updated.business_no,
            snapshot,
            reason,
            user_name,
            updated.created_by_user_id,
            updated.group_id,
            "",
            true,
        )?;
    }
    audit_repo::log_structured_on_conn(
        &tx,
        "delete",
        "sample_info_records",
        Some(id),
        user_name,
        &detail,
        "sample_info",
        &updated.business_no,
        before.as_ref(),
        after.as_ref(),
        "record",
    )?;
    trace_repo::log_event_on_conn(
        &tx,
        "sample_info",
        "sample_info_records",
        id,
        &updated.business_no,
        "delete",
        Some(&existing.status),
        Some(&existing.status),
        user_name,
        &detail,
        before.as_ref(),
        after.as_ref(),
    )?;
    tx.commit()?;
    Ok(())
}

pub fn soft_delete_range(
    pool: &DbPool,
    start: &str,
    end: &str,
    user_name: &str,
    reason: &str,
) -> Result<i64> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let ids: Vec<i64> = {
        let mut stmt = tx.prepare(
            "SELECT id FROM sample_info_records
             WHERE deleted_at IS NULL AND submitted_at>=?1 AND submitted_at<=?2
             ORDER BY id",
        )?;
        let rows = stmt.query_map(postgres_compat::params![start, end], |row| row.get(0))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };
    for id in &ids {
        let existing_record = get_by_id_on_conn(&tx, *id)?;
        let existing = SampleInfoResponse::from(existing_record);
        let before = serde_json::to_value(&existing).ok();
        tx.execute(
            "UPDATE sample_info_records SET deleted_at=datetime('now','localtime'),updated_at=datetime('now','localtime') WHERE id=?1",
            [*id],
        )?;
        let updated = SampleInfoResponse::from(get_by_id_on_conn(&tx, *id)?);
        let after = serde_json::to_value(&updated).ok();
        if let Some(ref snapshot) = before {
            trash_repo::move_to_trash_on_conn(
                &tx,
                "样品信息登记",
                "sample_info_records",
                *id,
                "records",
                "sample_info",
                &updated.business_no,
                &updated.business_no,
                snapshot,
                reason,
                user_name,
                updated.created_by_user_id,
                updated.group_id,
                "附件及历史审计记录保留",
                true,
            )?;
        }
        let detail = format!("数据治理批量移入回收站：{}", updated.business_no);
        audit_repo::log_structured_on_conn(
            &tx,
            "delete",
            "sample_info_records",
            Some(*id),
            user_name,
            &detail,
            "sample_info",
            &updated.business_no,
            before.as_ref(),
            after.as_ref(),
            "governance",
        )?;
        trace_repo::log_event_on_conn(
            &tx,
            "sample_info",
            "sample_info_records",
            *id,
            &updated.business_no,
            "delete",
            Some(&existing.status),
            Some(&existing.status),
            user_name,
            &detail,
            before.as_ref(),
            after.as_ref(),
        )?;
    }
    tx.commit()?;
    Ok(ids.len() as i64)
}

pub fn restore(pool: &DbPool, id: i64, user_name: &str) -> Result<SampleInfoResponse> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let existing_record = get_by_id_on_conn(&tx, id)?;
    if existing_record.deleted_at.is_none() {
        return Err(AppError::Validation("记录未被删除，无需恢复".into()));
    }
    let before = serde_json::to_value(SampleInfoResponse::from(existing_record)).ok();
    tx.execute(
        "UPDATE sample_info_records SET deleted_at=NULL,updated_at=datetime('now','localtime') WHERE id=?1",
        [id],
    )?;
    trash_repo::mark_restored_on_conn(&tx, "sample_info_records", id, user_name)?;
    let updated = SampleInfoResponse::from(get_by_id_on_conn(&tx, id)?);
    let after = serde_json::to_value(&updated).ok();
    let detail = format!("恢复样品信息#{}", id);
    audit_repo::log_structured_on_conn(
        &tx,
        "restore",
        "sample_info_records",
        Some(id),
        user_name,
        &detail,
        "sample_info",
        &updated.business_no,
        before.as_ref(),
        after.as_ref(),
        "record",
    )?;
    trace_repo::log_event_on_conn(
        &tx,
        "sample_info",
        "sample_info_records",
        id,
        &updated.business_no,
        "restore",
        Some(&updated.status),
        Some(&updated.status),
        user_name,
        &detail,
        before.as_ref(),
        after.as_ref(),
    )?;
    tx.commit()?;
    Ok(SampleInfoResponse::from(get_by_id_on_conn(&conn, id)?))
}

/// 状态流转
pub fn update_status(
    pool: &DbPool,
    id: i64,
    new_status: &str,
    user_name: &str,
) -> Result<SampleInfoResponse> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;

    let existing = get_by_id_on_conn(&tx, id)?;
    if existing.deleted_at.is_some() {
        return Err(AppError::Validation("记录已被删除".into()));
    }

    // 验证状态值有效性
    if !STATUS_ORDER.contains(&new_status) {
        return Err(AppError::Validation(format!(
            "无效状态: {}，允许的状态: {}",
            new_status,
            STATUS_ORDER.join(", ")
        )));
    }

    if existing.status == "已检测" {
        return Err(AppError::Validation("已检测状态不可再流转".into()));
    }

    // 只能按顺序流转，不可跳转
    let current_idx = STATUS_ORDER
        .iter()
        .position(|&s| s == existing.status)
        .unwrap_or(0);
    let target_idx = STATUS_ORDER
        .iter()
        .position(|&s| s == new_status)
        .unwrap_or(0);

    if target_idx != current_idx + 1 {
        return Err(AppError::Validation(format!(
            "状态只能按顺序流转：{} → {}，不能直接流转到 {}",
            existing.status,
            STATUS_ORDER.get(current_idx + 1).unwrap_or(&"已检测"),
            new_status
        )));
    }

    if new_status == "待检测" {
        tx.execute(
            "UPDATE sample_info_records
             SET status=?1, sampled_by=?2, sampled_at=datetime('now','localtime'),
                 updated_at=datetime('now','localtime') WHERE id=?3",
            postgres_compat::params![new_status, user_name, id],
        )?;
    } else if new_status == "已检测" {
        tx.execute(
            "UPDATE sample_info_records
             SET status=?1, detected_by=?2, detection_date=datetime('now','localtime'),
                 updated_at=datetime('now','localtime') WHERE id=?3",
            postgres_compat::params![new_status, user_name, id],
        )?;
    }

    let detail = format!(
        "样品信息#{} 状态流转：{} → {}",
        id, existing.status, new_status
    );
    let before = serde_json::to_value(SampleInfoResponse::from(existing)).ok();
    let updated = SampleInfoResponse::from(get_by_id_on_conn(&tx, id)?);
    let after = serde_json::to_value(&updated).ok();
    audit_repo::log_structured_on_conn(
        &tx,
        "status_change",
        "sample_info_records",
        Some(id),
        user_name,
        &detail,
        "sample_info",
        &updated.business_no,
        before.as_ref(),
        after.as_ref(),
        "record",
    )?;
    trace_repo::log_event_on_conn(
        &tx,
        "sample_info",
        "sample_info_records",
        id,
        &updated.business_no,
        "status_change",
        before
            .as_ref()
            .and_then(|v| v.get("status"))
            .and_then(|v| v.as_str()),
        Some(new_status),
        user_name,
        &detail,
        before.as_ref(),
        after.as_ref(),
    )?;
    tx.commit()?;

    let record = get_by_id_on_conn(&conn, id)?;
    Ok(SampleInfoResponse::from(record))
}

pub fn get_by_id(pool: &DbPool, id: i64) -> Result<SampleInfoResponse> {
    let conn = pool.get()?;
    Ok(SampleInfoResponse::from(get_by_id_on_conn(&conn, id)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_pool() -> DbPool {
        let pool = crate::db::init_pool("postgres-test");
        crate::db::test_migrations::run(&pool.get().unwrap()).unwrap();
        pool
    }

    fn sample_data() -> SampleInfoCreate {
        SampleInfoCreate {
            batch_no: "B001".into(),
            user_name: "sender01".into(),
            lab_name: "lab01".into(),
            project_name: "project01".into(),
            submitted_at: Some("2026-07-17T08:30:00".into()),
            detection_date: None,
            main_components: "component01".into(),
            detection_type: "ICP".into(),
            type_key: "icp".into(),
            division_id: None,
            quantity: 1,
            notes: None,
            extra_fields: None,
        }
    }

    #[test]
    fn sample_info_uses_three_step_flow_and_records_operators() {
        let pool = test_pool();
        let created = create(&pool, &sample_data(), "sender01").unwrap();
        assert_eq!(created.status, "待取样");
        assert!(created.business_no.starts_with("SI-ICP-20260717-"));
        assert!(update_status(&pool, created.id, "已检测", "detector02").is_err());

        let sampled = update_status(&pool, created.id, "待检测", "detector01").unwrap();
        assert_eq!(sampled.sampled_by, "detector01");
        assert!(sampled.sampled_at.is_some());

        let completed = update_status(&pool, created.id, "已检测", "detector02").unwrap();
        assert_eq!(completed.detected_by, "detector02");
        assert!(!completed.detection_date.is_empty());
        assert!(update_status(&pool, created.id, "待检测", "detector01").is_err());

        let events = trace_repo::list(&pool, "sample_info_records", created.id).unwrap();
        assert_eq!(
            events
                .iter()
                .map(|event| event.event_type.as_str())
                .collect::<Vec<_>>(),
            vec!["create", "status_change", "status_change"]
        );
        assert_eq!(events[1].operator, "detector01");
        assert_eq!(events[2].operator, "detector02");
    }

    #[test]
    fn sample_info_returns_division_name_for_saved_division_id() {
        let pool = test_pool();
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO divisions(name, sort_order) VALUES('DivisionTest01', 999)",
            [],
        )
        .unwrap();
        let division_id = conn.last_insert_rowid();
        drop(conn);

        let mut data = sample_data();
        data.division_id = Some(division_id);
        let created = create(&pool, &data, "sender01").unwrap();
        assert_eq!(created.division_name.as_deref(), Some("DivisionTest01"));

        let (items, total) = list(
            &pool,
            &SampleInfoQuery {
                detection_type: None,
                type_key: None,
                status: None,
                user_name: None,
                lab_name: None,
                project_name: None,
                division_id: Some(division_id),
                group_id: None,
                created_by_user_id: None,
                start: None,
                end: None,
                page: Some(1),
                page_size: Some(20),
                extra_fields: None,
                include_deleted: None,
            },
        )
        .unwrap();
        assert_eq!(total, 1);
        assert_eq!(items[0].division_name.as_deref(), Some("DivisionTest01"));
    }
}
