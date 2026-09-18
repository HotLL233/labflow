use crate::config::AppConfig;
use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::sample_info::{
    SampleInfoCreate, SampleInfoQuery, SampleInfoRecord, SampleInfoResponse, SampleInfoUpdate,
};
use crate::repo::{audit_repo, trace_repo, trash_repo};
use std::path::PathBuf;
use uuid::Uuid;

fn beijing_timestamp() -> String {
    let offset = chrono::FixedOffset::east_opt(8 * 60 * 60).expect("valid Beijing offset");
    chrono::Utc::now()
        .with_timezone(&offset)
        .format("%Y-%m-%dT%H:%M:%S")
        .to_string()
}

fn has_required_value(data: &SampleInfoCreate, field_key: &str) -> bool {
    match field_key {
        "batch_no" => !data.batch_no.trim().is_empty(),
        "user_name" => !data.user_name.trim().is_empty(),
        "lab_name" => !data.lab_name.trim().is_empty(),
        "project_name" => !data.project_name.trim().is_empty(),
        "main_components" => !data.main_components.trim().is_empty(),
        "detection_type" => !data.detection_type.trim().is_empty(),
        "type_key" => !data.type_key.trim().is_empty(),
        "division_id" => data.division_id.is_some(),
        "quantity" => data.quantity > 0,
        "notes" => data.notes.as_deref().is_some_and(|v| !v.trim().is_empty()),
        "detection_date" => data
            .detection_date
            .as_deref()
            .is_some_and(|v| !v.trim().is_empty()),
        key => data
            .extra_fields
            .as_ref()
            .and_then(|v| v.get(key))
            .is_some_and(|v| match v {
                serde_json::Value::String(s) => !s.trim().is_empty(),
                serde_json::Value::Null => false,
                _ => true,
            }),
    }
}

fn validate_type_required_fields(
    conn: &postgres_compat::Connection,
    data: &SampleInfoCreate,
) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT c.field_key,c.label,c.data_type FROM sample_info_columns c
         JOIN sample_info_column_visibility v ON v.column_id=c.id
         WHERE v.type_key=?1 AND v.is_visible=1 AND v.is_required=1
           AND c.is_active=1 AND c.deleted_at IS NULL",
    )?;
    let rules = stmt.query_map([&data.type_key], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    for rule in rules {
        let (field_key, label, data_type) = rule?;
        if data_type != "attachment" && !has_required_value(data, &field_key) {
            return Err(AppError::Validation(format!("{}为必填项", label)));
        }
    }
    Ok(())
}

const STATUS_ORDER: &[&str] = &["待取样", "待检测", "已检测"];

/// 只返回预先定义的完整 ORDER BY 片段，避免把浏览器传入的字符串直接拼进 SQL。
/// 默认按实际送样时间倒序；状态流转时间不会参与默认排序。
fn order_by(q: &SampleInfoQuery) -> &'static str {
    let ascending = q.sort_dir.as_deref() == Some("asc");
    match q.sort_by.as_deref() {
        Some("created_at") => {
            if ascending {
                "created_at ASC, id ASC"
            } else {
                "created_at DESC, id DESC"
            }
        }
        Some("status") => {
            if ascending {
                "status ASC, id ASC"
            } else {
                "status DESC, id DESC"
            }
        }
        Some("seq_no") => {
            if ascending {
                "seq_no ASC, id ASC"
            } else {
                "seq_no DESC, id DESC"
            }
        }
        Some("batch_no") => {
            if ascending {
                "batch_no ASC, id ASC"
            } else {
                "batch_no DESC, id DESC"
            }
        }
        Some("user_name") => {
            if ascending {
                "user_name ASC, id ASC"
            } else {
                "user_name DESC, id DESC"
            }
        }
        Some("division_id") => {
            if ascending {
                "division_id ASC NULLS LAST, id ASC"
            } else {
                "division_id DESC NULLS LAST, id DESC"
            }
        }
        Some("lab_name") => {
            if ascending {
                "lab_name ASC, id ASC"
            } else {
                "lab_name DESC, id DESC"
            }
        }
        Some("project_name") => {
            if ascending {
                "project_name ASC, id ASC"
            } else {
                "project_name DESC, id DESC"
            }
        }
        Some("detection_type") => {
            if ascending {
                "detection_type ASC, id ASC"
            } else {
                "detection_type DESC, id DESC"
            }
        }
        Some("detection_date") => {
            if ascending {
                "detection_date ASC, id ASC"
            } else {
                "detection_date DESC, id DESC"
            }
        }
        Some("quantity") => {
            if ascending {
                "quantity ASC, id ASC"
            } else {
                "quantity DESC, id DESC"
            }
        }
        Some("sampled_at") => {
            if ascending {
                "sampled_at ASC NULLS LAST, id ASC"
            } else {
                "sampled_at DESC NULLS LAST, id DESC"
            }
        }
        Some("detected_by") => {
            if ascending {
                "detected_by ASC, id ASC"
            } else {
                "detected_by DESC, id DESC"
            }
        }
        Some("notes") => {
            if ascending {
                "notes ASC, id ASC"
            } else {
                "notes DESC, id DESC"
            }
        }
        Some("main_components") => {
            if ascending {
                "main_components ASC, id ASC"
            } else {
                "main_components DESC, id DESC"
            }
        }
        Some("business_no") => {
            if ascending {
                "business_no ASC, id ASC"
            } else {
                "business_no DESC, id DESC"
            }
        }
        Some("submitted_at") | None => {
            if ascending {
                "submitted_at ASC, id ASC"
            } else {
                "submitted_at DESC, id DESC"
            }
        }
        _ => "submitted_at DESC, id DESC",
    }
}

/// 构建通用 WHERE 子句（不含分页），所有筛选维度：
/// 时间(submitted_at) / 送样人 / 实验室 / 项目 / 类型(type_key) / 状态
fn ownership_division_column(q: &SampleInfoQuery) -> Result<&'static str> {
    match q.ownership_basis.as_deref().unwrap_or("submitted") {
        "submitted" => Ok("submitted_division_id"),
        "execution" => Ok("execution_division_id"),
        "project" => Ok("project_division_id"),
        _ => Err(AppError::Validation(
            "样品信息查询口径只能是 submitted、execution 或 project".into(),
        )),
    }
}

fn build_where(q: &SampleInfoQuery) -> Result<(String, Vec<String>)> {
    let mut clauses: Vec<String> = if q.include_deleted.unwrap_or(false) {
        vec![]
    } else {
        vec!["deleted_at IS NULL".to_string()]
    };
    let mut params: Vec<String> = vec![];
    // Pending ownership is a reporting-governance state, not a visibility
    // state for ordinary business records. Submitters must still be able to
    // find and correct a newly created record before its ownership is fixed.
    // Statistics and exports apply their own default exclusion.
    if q.include_pending_ownership == Some(false) {
        clauses.push("COALESCE(ownership_status,'confirmed')<>'pending_confirmation'".to_string());
    }
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
        clauses.push(format!("{}=?{}", ownership_division_column(q)?, i));
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
    if !q.scope_filters.is_empty() {
        let mut scope_clauses = Vec::new();
        for scope in &q.scope_filters {
            let mut parts = Vec::new();
            match (scope.created_by_user_id, scope.business_user_id) {
                (Some(created_by), Some(business_user)) => {
                    let created_by_index = params.len() + 1;
                    params.push(created_by.to_string());
                    let business_user_index = params.len() + 1;
                    params.push(business_user.to_string());
                    parts.push(format!(
                        "(created_by_user_id=?{} OR business_user_id=?{})",
                        created_by_index, business_user_index
                    ));
                }
                (Some(user_id), None) => {
                    let i = params.len() + 1;
                    parts.push(format!("created_by_user_id=?{}", i));
                    params.push(user_id.to_string());
                }
                (None, Some(user_id)) => {
                    let i = params.len() + 1;
                    parts.push(format!("business_user_id=?{}", i));
                    params.push(user_id.to_string());
                }
                (None, None) => {}
            }
            if !scope.division_ids.is_empty() {
                let mut placeholders = Vec::new();
                for division_id in &scope.division_ids {
                    let i = params.len() + 1;
                    placeholders.push(format!("?{}", i));
                    params.push(division_id.to_string());
                }
                parts.push(format!("division_id IN ({})", placeholders.join(",")));
            }
            if !scope.type_keys.is_empty() {
                let mut placeholders = Vec::new();
                for type_key in &scope.type_keys {
                    let i = params.len() + 1;
                    placeholders.push(format!("?{}", i));
                    params.push(type_key.clone());
                }
                parts.push(format!("type_key IN ({})", placeholders.join(",")));
            }
            if !parts.is_empty() {
                scope_clauses.push(format!("({})", parts.join(" AND ")));
            }
        }
        if !scope_clauses.is_empty() {
            clauses.push(format!("({})", scope_clauses.join(" OR ")));
        }
    }
    let where_sql = if clauses.is_empty() {
        "1=1".into()
    } else {
        clauses.join(" AND ")
    };
    Ok((where_sql, params))
}

/// 分页查询，支持全部维度筛选，未软删除
pub fn list(pool: &DbPool, q: &SampleInfoQuery) -> Result<(Vec<SampleInfoResponse>, i64)> {
    let conn = pool.get()?;
    let page = q.page.unwrap_or(1).max(1);
    let page_size = q.page_size.unwrap_or(20).min(500);

    let (where_sql, params) = build_where(q)?;

    let sql = format!(
        "SELECT id, business_no, status, seq_no, batch_no, user_name, lab_name, project_name, \
         submitted_at, detection_date, sampled_by, sampled_at, detected_by, \
         main_components, detection_type, type_key, division_id, quantity, notes, extra_fields, \
         created_at, updated_at, deleted_at, \
         (SELECT name FROM divisions d WHERE d.id=sample_info_records.division_id), \
         group_id, created_by_user_id, return_reason, returned_by, returned_at, \
         return_confirmed_by, return_confirmed_at, source_record_id, \
         project_division_id, project_division_name_snapshot, \
         execution_division_id, execution_division_name_snapshot, \
         execution_group_id, execution_group_name_snapshot, \
         submitted_division_id, submitted_division_name_snapshot, \
         business_user_id, business_username_snapshot, ownership_status, \
         EXISTS(SELECT 1 FROM work_records wr WHERE wr.source_type='sample_info_sample' AND wr.source_record_id=sample_info_records.id) \
         FROM sample_info_records WHERE {} ORDER BY {} \
         LIMIT {} OFFSET {}",
        where_sql,
        order_by(q),
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
                return_reason: row.get::<_, String>(26).unwrap_or_default(),
                returned_by: row.get::<_, String>(27).unwrap_or_default(),
                returned_at: row.get(28)?,
                return_confirmed_by: row.get::<_, String>(29).unwrap_or_default(),
                return_confirmed_at: row.get(30)?,
                source_record_id: row.get(31)?,
                project_division_id: row.get(32)?,
                project_division_name_snapshot: row.get::<_, String>(33).unwrap_or_default(),
                execution_division_id: row.get(34)?,
                execution_division_name_snapshot: row.get::<_, String>(35).unwrap_or_default(),
                execution_group_id: row.get(36)?,
                execution_group_name_snapshot: row.get::<_, String>(37).unwrap_or_default(),
                submitted_division_id: row.get(38)?,
                submitted_division_name_snapshot: row.get::<_, String>(39).unwrap_or_default(),
                business_user_id: row.get(40)?,
                business_username_snapshot: row.get::<_, String>(41).unwrap_or_default(),
                ownership_status: row
                    .get::<_, String>(42)
                    .unwrap_or_else(|_| "confirmed".into()),
                workload_recorded: row.get(43).unwrap_or(false),
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
         group_id, created_by_user_id, return_reason, returned_by, returned_at, \
         return_confirmed_by, return_confirmed_at, source_record_id, \
         project_division_id, project_division_name_snapshot, \
         execution_division_id, execution_division_name_snapshot, \
         execution_group_id, execution_group_name_snapshot, \
         submitted_division_id, submitted_division_name_snapshot, \
         business_user_id, business_username_snapshot, ownership_status, \
         EXISTS(SELECT 1 FROM work_records wr WHERE wr.source_type='sample_info_sample' AND wr.source_record_id=sample_info_records.id) \
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
                return_reason: row.get::<_, String>(26).unwrap_or_default(),
                returned_by: row.get::<_, String>(27).unwrap_or_default(),
                returned_at: row.get(28)?,
                return_confirmed_by: row.get::<_, String>(29).unwrap_or_default(),
                return_confirmed_at: row.get(30)?,
                source_record_id: row.get(31)?,
                project_division_id: row.get(32)?,
                project_division_name_snapshot: row.get::<_, String>(33).unwrap_or_default(),
                execution_division_id: row.get(34)?,
                execution_division_name_snapshot: row.get::<_, String>(35).unwrap_or_default(),
                execution_group_id: row.get(36)?,
                execution_group_name_snapshot: row.get::<_, String>(37).unwrap_or_default(),
                submitted_division_id: row.get(38)?,
                submitted_division_name_snapshot: row.get::<_, String>(39).unwrap_or_default(),
                business_user_id: row.get(40)?,
                business_username_snapshot: row.get::<_, String>(41).unwrap_or_default(),
                ownership_status: row
                    .get::<_, String>(42)
                    .unwrap_or_else(|_| "confirmed".into()),
                workload_recorded: row.get(43).unwrap_or(false),
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

/// Freeze the ownership dimensions used by permissions, audit and statistics.
/// Names remain snapshots so a later rename or organization adjustment cannot
/// rewrite the meaning of an already submitted record.
fn refresh_ownership_snapshot_on_conn(conn: &postgres_compat::Connection, id: i64) -> Result<()> {
    conn.execute(
        "UPDATE sample_info_records s
         SET
           project_division_id=(SELECT p.project_division_id FROM projects p WHERE p.name=s.project_name AND p.deleted_at IS NULL ORDER BY p.id LIMIT 1),
           project_division_name_snapshot=COALESCE((SELECT p.project_division_name_snapshot FROM projects p WHERE p.name=s.project_name AND p.deleted_at IS NULL ORDER BY p.id LIMIT 1),''),
           execution_group_id=s.group_id,
           execution_group_name_snapshot=COALESCE((SELECT g.name FROM project_groups g WHERE g.id=s.group_id),NULLIF(s.lab_name,''),''),
           execution_division_id=COALESCE((SELECT g.division_id FROM project_groups g WHERE g.id=s.group_id),s.division_id),
           execution_division_name_snapshot=COALESCE((SELECT d.name FROM divisions d WHERE d.id=COALESCE((SELECT g.division_id FROM project_groups g WHERE g.id=s.group_id),s.division_id)),''),
           submitted_division_id=s.division_id,
           submitted_division_name_snapshot=COALESCE((SELECT d.name FROM divisions d WHERE d.id=s.division_id),''),
           business_user_id=COALESCE((SELECT u.id FROM users u WHERE u.username=s.user_name AND u.deleted_at IS NULL ORDER BY u.id LIMIT 1),(SELECT u.id FROM users u WHERE u.username=s.created_by_username_snapshot AND u.deleted_at IS NULL ORDER BY u.id LIMIT 1)),
           business_username_snapshot=COALESCE(NULLIF(s.user_name,''),s.created_by_username_snapshot,''),
           ownership_status=CASE
             WHEN COALESCE((SELECT g.division_id FROM project_groups g WHERE g.id=s.group_id),s.division_id) IS NULL
             THEN 'pending_confirmation'
             ELSE 'confirmed'
           END
         WHERE s.id=?1",
        [id],
    )?;
    Ok(())
}

/// 创建记录，分配全局、持久化的提交序号。
pub fn create(
    pool: &DbPool,
    data: &SampleInfoCreate,
    operator: &str,
) -> Result<SampleInfoResponse> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;

    // 如果未提供 submitted_at，使用当前时间
    let submitted_at = beijing_timestamp();
    validate_type_required_fields(&tx, data)?;

    // 由 PostgreSQL 序列分配，避免按类型/日期重置，也避免多人并发提交时重复取号。
    let seq_no: i64 = tx.query_row(
        "SELECT nextval('sample_info_records_seq_no_seq')",
        [],
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
    refresh_ownership_snapshot_on_conn(&tx, id)?;
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
    if existing.status == "已退回" || existing.status == "已退回已确认" {
        return Err(AppError::Forbidden(
            "该记录已退回，请先确认退回原因后再修改".into(),
        ));
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
    // 送样时间由服务器在创建记录时写入北京时间。保留请求字段仅为兼容旧客户端，
    // 更新接口始终忽略它，避免客户端时间覆盖审计时间。
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
    refresh_ownership_snapshot_on_conn(&tx, id)?;
    if existing.status == "退回待修改" {
        changes.push("退回修改后重新提交".into());
        tx.execute(
            "UPDATE sample_info_records
             SET status='待取样',submitted_at=to_char(CURRENT_TIMESTAMP,'YYYY-MM-DD\"T\"HH24:MI:SS'),
                 updated_at=datetime('now','localtime') WHERE id=?1",
            [id],
        )?;
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

/// 将待取样记录退回，原记录保留为只读历史记录。
pub fn return_record(
    pool: &DbPool,
    id: i64,
    reason: &str,
    returned_by: &str,
) -> Result<SampleInfoResponse> {
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
    if old.sampled_at.is_some() || old.status != "待取样" {
        return Err(AppError::Validation(
            "仅待取样记录可以退回，已取样记录不能退回".into(),
        ));
    }
    let before = serde_json::to_value(SampleInfoResponse::from(old)).ok();
    tx.execute(
        "UPDATE sample_info_records
         SET status='已退回',return_reason=?1,returned_by=?2,
             returned_at=datetime('now','localtime'),updated_at=datetime('now','localtime')
         WHERE id=?3",
        postgres_compat::params![reason, returned_by, id],
    )?;
    let updated = SampleInfoResponse::from(get_by_id_on_conn(&tx, id)?);
    let after = serde_json::to_value(&updated).ok();
    let detail = format!("样品信息{}已退回：{}", updated.business_no, reason);
    audit_repo::log_structured_on_conn(
        &tx,
        "return",
        "sample_info_records",
        Some(id),
        returned_by,
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
        "return",
        Some("待取样"),
        Some("已退回"),
        returned_by,
        &detail,
        before.as_ref(),
        after.as_ref(),
    )?;
    tx.commit()?;
    Ok(SampleInfoResponse::from(get_by_id_on_conn(&conn, id)?))
}

/// 确认退回并创建独立的可编辑草稿；原记录保持已退回已确认。
pub fn confirm_return(pool: &DbPool, id: i64, confirmed_by: &str) -> Result<SampleInfoResponse> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let old = get_by_id_on_conn(&tx, id)?;
    if old.deleted_at.is_some() {
        return Err(AppError::Validation("记录已被删除".into()));
    }
    if old.status != "已退回" {
        return Err(AppError::Validation("仅已退回记录可以确认修改".into()));
    }
    let before = serde_json::to_value(SampleInfoResponse::from(old)).ok();
    tx.execute(
        "UPDATE sample_info_records
         SET status='已退回已确认',return_confirmed_by=?1,
             return_confirmed_at=datetime('now','localtime'),updated_at=datetime('now','localtime')
         WHERE id=?2",
        postgres_compat::params![confirmed_by, id],
    )?;
    let confirmed = SampleInfoResponse::from(get_by_id_on_conn(&tx, id)?);
    let confirmed_after = serde_json::to_value(&confirmed).ok();
    let detail = format!(
        "样品信息{}已确认退回，原记录保留不再编辑",
        confirmed.business_no
    );
    audit_repo::log_structured_on_conn(
        &tx,
        "return_confirm",
        "sample_info_records",
        Some(id),
        confirmed_by,
        &detail,
        "sample_info",
        &confirmed.business_no,
        before.as_ref(),
        confirmed_after.as_ref(),
        "record",
    )?;
    trace_repo::log_event_on_conn(
        &tx,
        "sample_info",
        "sample_info_records",
        id,
        &confirmed.business_no,
        "return_confirm",
        Some("已退回"),
        Some("已退回已确认"),
        confirmed_by,
        &detail,
        before.as_ref(),
        confirmed_after.as_ref(),
    )?;

    tx.execute(
        "INSERT INTO sample_info_records(
            status,seq_no,batch_no,user_name,lab_name,project_name,submitted_at,detection_date,
            main_components,detection_type,type_key,division_id,quantity,notes,extra_fields,
            sampled_by,sampled_at,detected_by,group_id,created_by_user_id,created_by_username_snapshot,
            return_reason,returned_by,returned_at,source_record_id,
            project_division_id,project_division_name_snapshot,
            execution_division_id,execution_division_name_snapshot,
            execution_group_id,execution_group_name_snapshot,
            submitted_division_id,submitted_division_name_snapshot,
            business_user_id,business_username_snapshot,ownership_status
         )
         SELECT '退回待修改',nextval('sample_info_records_seq_no_seq'),batch_no,user_name,lab_name,project_name,
            to_char(CURRENT_TIMESTAMP,'YYYY-MM-DD\"T\"HH24:MI:SS'),detection_date,
            main_components,detection_type,type_key,division_id,quantity,notes,extra_fields,
            '',NULL,'',group_id,created_by_user_id,created_by_username_snapshot,
            return_reason,returned_by,returned_at,id,
            project_division_id,project_division_name_snapshot,
            execution_division_id,execution_division_name_snapshot,
            execution_group_id,execution_group_name_snapshot,
            submitted_division_id,submitted_division_name_snapshot,
            business_user_id,business_username_snapshot,ownership_status
         FROM sample_info_records WHERE id=?1",
        [id],
    )?;
    let draft_id = tx.last_insert_rowid();
    let draft = get_by_id_on_conn(&tx, draft_id)?;
    let draft_business_no = trace_repo::make_business_no(
        &format!("SI-{}", draft.type_key.to_ascii_uppercase()),
        &draft.submitted_at,
        draft_id,
    );
    tx.execute(
        "UPDATE sample_info_records SET business_no=?1 WHERE id=?2",
        postgres_compat::params![draft_business_no, draft_id],
    )?;
    let draft = SampleInfoResponse::from(get_by_id_on_conn(&tx, draft_id)?);
    let draft_after = serde_json::to_value(&draft).ok();
    audit_repo::log_structured_on_conn(
        &tx,
        "return_resubmit_draft",
        "sample_info_records",
        Some(draft_id),
        confirmed_by,
        &format!(
            "由退回记录 {} 复制创建，修改保存后重新提交",
            confirmed.business_no
        ),
        "sample_info",
        &draft.business_no,
        None,
        draft_after.as_ref(),
        "record",
    )?;
    trace_repo::log_event_on_conn(
        &tx,
        "sample_info",
        "sample_info_records",
        draft_id,
        &draft.business_no,
        "return_resubmit_draft",
        None,
        Some("退回待修改"),
        confirmed_by,
        &format!("由退回记录 {} 复制创建", confirmed.business_no),
        None,
        draft_after.as_ref(),
    )?;

    let attachment_dir = AppConfig::load().attachments_dir();
    let mut copied_files: Vec<PathBuf> = Vec::new();
    let clone_result: Result<()> = (|| {
        let mut stmt = tx.prepare(
            "SELECT file_name,stored_name,file_size,file_type
             FROM sample_info_attachments WHERE record_id=?1 AND deleted_at IS NULL",
        )?;
        let attachments: Vec<(String, String, i64, String)> = stmt
            .query_map([id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for (file_name, stored_name, file_size, file_type) in attachments {
            let source_path = attachment_dir.join(&stored_name);
            if !source_path.is_file() {
                return Err(AppError::NotFound(format!(
                    "原附件文件不存在：{}",
                    file_name
                )));
            }
            let extension = std::path::Path::new(&file_name)
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("bin");
            let new_name = format!(
                "sample_info_return_{}_{}.{}",
                draft_id,
                Uuid::new_v4(),
                extension
            );
            let target_path = attachment_dir.join(&new_name);
            std::fs::copy(&source_path, &target_path)
                .map_err(|error| AppError::Internal(format!("复制退回附件失败：{error}")))?;
            copied_files.push(target_path);
            tx.execute(
                "INSERT INTO sample_info_attachments(record_id,file_name,stored_name,file_size,file_type)
                 VALUES(?1,?2,?3,?4,?5)",
                postgres_compat::params![draft_id, file_name, new_name, file_size, file_type],
            )?;
        }
        Ok(())
    })();
    if let Err(error) = clone_result {
        for path in copied_files {
            let _ = std::fs::remove_file(path);
        }
        return Err(error);
    }
    tx.commit()?;
    Ok(SampleInfoResponse::from(get_by_id_on_conn(
        &conn, draft_id,
    )?))
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

/// 撤回取样：将当前取样记录恢复为待取样，保留审计轨迹。
pub fn withdraw_sample(pool: &DbPool, id: i64, user_name: &str) -> Result<SampleInfoResponse> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let existing = get_by_id_on_conn(&tx, id)?;
    if existing.deleted_at.is_some() {
        return Err(AppError::Validation("记录已被删除".into()));
    }
    if existing.status != "待检测" || existing.sampled_at.is_none() {
        return Err(AppError::Validation(
            "仅已取样且未完成检测的记录可以撤回取样".into(),
        ));
    }
    let before = serde_json::to_value(SampleInfoResponse::from(existing)).ok();
    tx.execute(
        "UPDATE sample_info_records
         SET status='待取样', sampled_by='', sampled_at=NULL,
             updated_at=datetime('now','localtime') WHERE id=?1",
        [id],
    )?;
    let updated = SampleInfoResponse::from(get_by_id_on_conn(&tx, id)?);
    let after = serde_json::to_value(&updated).ok();
    let detail = format!("样品信息#{} 撤回取样", id);
    audit_repo::log_structured_on_conn(
        &tx,
        "withdraw_sample",
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
        "withdraw_sample",
        Some("待检测"),
        Some("待取样"),
        user_name,
        &detail,
        before.as_ref(),
        after.as_ref(),
    )?;
    tx.commit()?;
    Ok(SampleInfoResponse::from(get_by_id_on_conn(&conn, id)?))
}

pub fn get_by_id(pool: &DbPool, id: i64) -> Result<SampleInfoResponse> {
    let conn = pool.get()?;
    Ok(SampleInfoResponse::from(get_by_id_on_conn(&conn, id)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_info_sorting_has_stable_submission_time_default_and_whitelist() {
        let default_query = SampleInfoQuery::default();
        assert_eq!(order_by(&default_query), "submitted_at DESC, id DESC");

        let mut ascending = SampleInfoQuery::default();
        ascending.sort_by = Some("submitted_at".into());
        ascending.sort_dir = Some("asc".into());
        assert_eq!(order_by(&ascending), "submitted_at ASC, id ASC");

        let mut invalid = SampleInfoQuery::default();
        invalid.sort_by = Some("submitted_at; DROP TABLE sample_info_records".into());
        assert_eq!(order_by(&invalid), "submitted_at DESC, id DESC");
    }

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
        let beijing_date = chrono::Utc::now()
            .with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap())
            .format("%Y%m%d")
            .to_string();
        assert!(created
            .business_no
            .starts_with(&format!("SI-ICP-{beijing_date}-")));
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
                ownership_basis: None,
                include_pending_ownership: None,
                group_id: None,
                created_by_user_id: None,
                start: None,
                end: None,
                page: Some(1),
                page_size: Some(20),
                extra_fields: None,
                include_deleted: None,
                sort_by: None,
                sort_dir: None,
                scope_filters: vec![],
            },
        )
        .unwrap();
        assert_eq!(total, 1);
        assert_eq!(items[0].division_name.as_deref(), Some("DivisionTest01"));
    }

    #[test]
    fn sample_info_sequence_is_global_and_stable_when_sorted() {
        let pool = test_pool();
        let first = create(&pool, &sample_data(), "sender01").unwrap();
        let mut second_data = sample_data();
        second_data.batch_no = "B002".into();
        second_data.detection_type = "热分析".into();
        second_data.type_key = "thermal".into();
        let second = create(&pool, &second_data, "sender01").unwrap();

        assert_eq!(first.seq_no + 1, second.seq_no);

        let mut descending = SampleInfoQuery::default();
        descending.sort_by = Some("submitted_at".into());
        descending.sort_dir = Some("desc".into());
        let (desc_items, _) = list(&pool, &descending).unwrap();
        assert_eq!(
            desc_items
                .iter()
                .map(|item| item.seq_no)
                .collect::<Vec<_>>(),
            vec![second.seq_no, first.seq_no]
        );

        let mut ascending = SampleInfoQuery::default();
        ascending.sort_by = Some("submitted_at".into());
        ascending.sort_dir = Some("asc".into());
        let (asc_items, _) = list(&pool, &ascending).unwrap();
        assert_eq!(
            asc_items.iter().map(|item| item.seq_no).collect::<Vec<_>>(),
            vec![first.seq_no, second.seq_no]
        );
    }

    #[test]
    fn sample_info_return_confirm_creates_editable_draft_and_resubmits() {
        let pool = test_pool();
        let created = create(&pool, &sample_data(), "sender01").unwrap();

        let returned = return_record(&pool, created.id, "主要成分需要补充", "analysis01").unwrap();
        assert_eq!(returned.status, "已退回");
        assert_eq!(returned.return_reason, "主要成分需要补充");
        assert_eq!(returned.returned_by, "analysis01");
        assert!(return_record(&pool, created.id, "重复退回", "analysis01").is_err());

        let draft = confirm_return(&pool, created.id, "sender01").unwrap();
        assert_eq!(draft.status, "退回待修改");
        assert_eq!(draft.source_record_id, Some(created.id));
        assert_eq!(draft.return_reason, "主要成分需要补充");

        let original = get_by_id(&pool, created.id).unwrap();
        assert_eq!(original.status, "已退回已确认");
        assert!(update(
            &pool,
            created.id,
            &SampleInfoUpdate {
                status: None,
                batch_no: Some("should-not-change".into()),
                user_name: None,
                lab_name: None,
                project_name: None,
                submitted_at: None,
                detection_date: None,
                main_components: None,
                division_id: None,
                quantity: None,
                notes: None,
                extra_fields: None,
            },
            "sender01",
        )
        .is_err());

        let resubmitted = update(
            &pool,
            draft.id,
            &SampleInfoUpdate {
                status: None,
                batch_no: Some("B001-fixed".into()),
                user_name: None,
                lab_name: None,
                project_name: None,
                submitted_at: None,
                detection_date: None,
                main_components: Some("component01-fixed".into()),
                division_id: None,
                quantity: None,
                notes: None,
                extra_fields: None,
            },
            "sender01",
        )
        .unwrap();
        assert_eq!(resubmitted.status, "待取样");
        assert_eq!(resubmitted.batch_no, "B001-fixed");
        assert_eq!(resubmitted.source_record_id, Some(created.id));
    }
}
