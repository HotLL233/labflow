use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use chrono::{NaiveDate, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use postgres_compat::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, sync::OnceLock};
use uuid::Uuid;

use crate::{
    db::DbPool,
    error::{AppError, Result},
    models::ApiResponse,
    repo::{audit_repo, settings_repo},
    service::authz_service::{self, AuthContext},
};

const MAX_BATCH: usize = 100;
const MAX_TOKEN: usize = 256 * 1024;

#[derive(Deserialize)]
struct RecordQuery {
    start: String,
    end: String,
    keyword: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
}

#[derive(Default, Deserialize)]
struct PageQuery {
    page: Option<i64>,
    page_size: Option<i64>,
}

#[derive(Serialize)]
struct CandidateRow {
    id: i64,
    business_no: String,
    user_name: String,
    recorded_at: String,
    method_name: String,
    source_type: String,
    source_record_id: Option<i64>,
    quantity: i64,
    coefficient: f64,
    workload: f64,
    linked_item_id: Option<i64>,
    linked_item_name: String,
    linked_batch_id: Option<String>,
    evidence: String,
}

struct Candidate {
    row: CandidateRow,
    snapshot: String,
}

#[derive(Serialize)]
struct Page<T> {
    rows: Vec<T>,
    total: i64,
    page: i64,
    page_size: i64,
}

#[derive(Deserialize)]
struct Assignment {
    record_id: i64,
    item_id: i64,
}

#[derive(Deserialize)]
struct PreviewRequest {
    assignments: Vec<Assignment>,
}

#[derive(Serialize)]
struct PreviewRow {
    #[serde(flatten)]
    record: CandidateRow,
    item_id: i64,
    item_name: String,
    area_name: String,
    category: String,
}

#[derive(Serialize)]
struct Preview {
    rows: Vec<PreviewRow>,
    record_count: usize,
    total_quantity: i64,
    total_workload: f64,
    confirmation_token: String,
    expires_at: String,
}

#[derive(Serialize, Deserialize)]
struct ConfirmationRow {
    record_id: i64,
    item_id: i64,
    source_fingerprint: String,
    item_fingerprint: String,
    link_id: Option<i64>,
    link_fingerprint: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Confirmation {
    purpose: String,
    sub: i64,
    exp: usize,
    batch_id: String,
    rows: Vec<ConfirmationRow>,
}

#[derive(Deserialize)]
struct ApplyRequest {
    confirmation_token: String,
    reason: String,
}

#[derive(Serialize)]
struct ApplyResult {
    batch_id: String,
    record_count: i64,
    total_quantity: i64,
    total_workload: f64,
    replayed: bool,
}

#[derive(Serialize)]
struct BatchRow {
    id: String,
    reason: String,
    created_by_name: String,
    created_at: String,
    record_count: i64,
    total_quantity: i64,
    total_workload: f64,
    revoked_at: Option<String>,
}

struct Item {
    id: i64,
    name: String,
    area_id: Option<i64>,
    area_name: String,
    category: String,
    enabled: bool,
    deleted: bool,
    area_enabled: bool,
    snapshot: String,
}

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route(
            "/api/data-governance/legacy-auxiliary/records",
            get(records),
        )
        .route(
            "/api/data-governance/legacy-auxiliary/preview",
            post(preview),
        )
        .route("/api/data-governance/legacy-auxiliary/apply", post(apply))
        .route(
            "/api/data-governance/legacy-auxiliary/batches",
            get(batches),
        )
        .route(
            "/api/data-governance/legacy-auxiliary/batches/:id/undo-preview",
            get(undo_preview),
        )
        .route(
            "/api/data-governance/legacy-auxiliary/batches/:id/undo",
            post(undo),
        )
        .with_state(pool)
}

fn require_admin(ctx: &AuthContext, writing: bool) -> Result<()> {
    if !ctx.is_system_admin() {
        return Err(AppError::Forbidden("仅系统管理员可关联历史辅助工作".into()));
    }
    authz_service::require_permission(ctx, "manage:auxiliary")?;
    authz_service::require_permission(ctx, "manage:data-governance:view")?;
    if writing {
        authz_service::require_permission(ctx, "manage:data-governance:import")?;
    }
    Ok(())
}

fn key() -> &'static str {
    static KEY: OnceLock<String> = OnceLock::new();
    KEY.get_or_init(|| format!("{}{}", Uuid::new_v4(), Uuid::new_v4()))
}

fn fingerprint(snapshot: &str) -> Result<String> {
    let canonical: serde_json::Value =
        serde_json::from_str(snapshot).map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(format!(
        "{:x}",
        Sha256::digest(canonical.to_string().as_bytes())
    ))
}

fn read_confirmation(token: &str, user_id: i64, purpose: &str) -> Result<Confirmation> {
    if token.len() > MAX_TOKEN {
        return Err(AppError::Validation("关联确认无效，请重新预览".into()));
    }
    let mut validation = Validation::default();
    validation.leeway = 0;
    let claims = decode::<Confirmation>(
        token,
        &DecodingKey::from_secret(key().as_bytes()),
        &validation,
    )
    .map_err(|_| AppError::Conflict("关联确认已过期或失效，请重新预览".into()))?
    .claims;
    if claims.purpose != purpose || claims.sub != user_id {
        return Err(AppError::Forbidden("关联确认不属于当前管理员或操作".into()));
    }
    let ids = claims
        .rows
        .iter()
        .map(|row| row.record_id)
        .collect::<BTreeSet<_>>();
    if claims.rows.is_empty()
        || claims.rows.len() > MAX_BATCH
        || ids.len() != claims.rows.len()
        || claims
            .rows
            .iter()
            .any(|row| row.record_id <= 0 || row.item_id <= 0)
        || Uuid::parse_str(&claims.batch_id).is_err()
    {
        return Err(AppError::Validation("关联确认范围无效".into()));
    }
    Ok(claims)
}

fn page(query: &PageQuery) -> Result<(i64, i64, i64)> {
    let page = query.page.unwrap_or(1);
    let size = query.page_size.unwrap_or(50);
    if page < 1 || !(1..=200).contains(&size) {
        return Err(AppError::Validation(
            "页码须大于0，每页数量须为1至200".into(),
        ));
    }
    let offset = (page - 1)
        .checked_mul(size)
        .ok_or_else(|| AppError::Validation("页码超出范围".into()))?;
    Ok((page, size, offset))
}

fn source_sql() -> String {
    format!(
        "SELECT wr.id,COALESCE(wr.business_no,''),wr.user_name,wr.recorded_at,
         COALESCE(NULLIF(wr.method_name_snapshot,''),m.name,'未记录方法'),
         COALESCE(wr.source_type,''),wr.source_record_id,wr.quantity,wr.coefficient_snapshot,
         ll.auxiliary_work_item_id,COALESCE(ai.name,''),ll.batch_id,
         CASE WHEN ll.id IS NOT NULL THEN '已确认历史辅助关联'
           WHEN wr.instrument_type_snapshot='辅助工作' THEN '原仪器类型快照为辅助工作'
           WHEN NULLIF(wr.detection_type_snapshot,'') IS NOT NULL THEN '原检测类型快照为辅助工作'
           ELSE '原方法关联辅助工作类型' END,
         to_jsonb(wr)::text
         FROM work_records wr LEFT JOIN methods m ON m.id=wr.method_id
         LEFT JOIN legacy_auxiliary_links ll ON ll.source_table='work_records'
           AND ll.source_record_id=wr.id AND ll.revoked_at IS NULL
         LEFT JOIN auxiliary_work_items ai ON ai.id=ll.auxiliary_work_item_id
         WHERE wr.deleted_at IS NULL AND COALESCE(wr.source_type,'')<>'auxiliary_work'
           AND isfinite(labflow_try_timestamptz(wr.recorded_at))
           AND {}",
        super::export_data::legacy_auxiliary_record_sql("wr")
    )
}

fn map_candidate(row: &postgres_compat::Row<'_>) -> postgres_compat::Result<Candidate> {
    let quantity: i64 = row.get(7)?;
    let coefficient: f64 = row.get(8)?;
    Ok(Candidate {
        row: CandidateRow {
            id: row.get(0)?,
            business_no: row.get(1)?,
            user_name: row.get(2)?,
            recorded_at: row.get(3)?,
            method_name: row.get(4)?,
            source_type: row.get(5)?,
            source_record_id: row.get(6)?,
            quantity,
            coefficient,
            workload: quantity as f64 * coefficient,
            linked_item_id: row.get(9)?,
            linked_item_name: row.get(10)?,
            linked_batch_id: row.get(11)?,
            evidence: row.get(12)?,
        },
        snapshot: row.get(13)?,
    })
}

fn validate_candidate(record: &Candidate) -> Result<()> {
    let row = &record.row;
    if row.quantity <= 0
        || row.coefficient < 0.0
        || !row.coefficient.is_finite()
        || !row.workload.is_finite()
    {
        return Err(AppError::Validation(
            "历史记录数量或系数异常，请先在数据诊断中核对".into(),
        ));
    }
    Ok(())
}

fn read_record(conn: &Connection, id: i64, lock: bool) -> Result<Candidate> {
    let record = conn
        .query_row(
            &format!(
                "{} AND wr.id=?1 {}",
                source_sql(),
                if lock { "FOR UPDATE OF wr" } else { "" }
            ),
            [id],
            map_candidate,
        )
        .optional()?
        .ok_or_else(|| {
            AppError::Conflict("原记录已变化、删除或不再属于可关联旧辅助，请重新预览".into())
        })?;
    validate_candidate(&record)?;
    Ok(record)
}

fn read_item(conn: &Connection, id: i64, lock: bool) -> Result<Item> {
    let category = settings_repo::workload_auxiliary_categories_on_conn(conn)?
        .get(&format!("item:{id}"))
        .cloned()
        .unwrap_or_default();
    let mut item = conn
        .query_row(
            &format!(
                "SELECT ai.id,ai.name,ai.area_type_id,COALESCE(aa.name,''),
          ai.enabled,ai.deleted_at IS NOT NULL,COALESCE(aa.enabled,TRUE),
          jsonb_build_object('item',to_jsonb(ai),'area',to_jsonb(aa))::text
          FROM auxiliary_work_items ai LEFT JOIN auxiliary_work_areas aa ON aa.id=ai.area_type_id
          WHERE ai.id=?1 {}",
                if lock { "FOR UPDATE OF ai" } else { "" }
            ),
            [id],
            |row| {
                Ok(Item {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    area_id: row.get(2)?,
                    area_name: row.get(3)?,
                    category: String::new(),
                    enabled: row.get(4)?,
                    deleted: row.get(5)?,
                    area_enabled: row.get(6)?,
                    snapshot: row.get(7)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::Conflict("辅助事项已不存在，请重新选择".into()))?;
    let mut snapshot: serde_json::Value =
        serde_json::from_str(&item.snapshot).map_err(|e| AppError::Internal(e.to_string()))?;
    snapshot["category"] = serde_json::Value::String(category.clone());
    item.category = category;
    item.snapshot = snapshot.to_string();
    Ok(item)
}

fn validate_item(item: &Item) -> Result<()> {
    if item.deleted || !item.enabled || !item.area_enabled {
        return Err(AppError::Conflict(
            "辅助事项或区域已停用，请重新选择".into(),
        ));
    }
    Ok(())
}

fn list_records(conn: &Connection, query: &RecordQuery) -> Result<Page<CandidateRow>> {
    let parse = |text: &str| {
        NaiveDate::parse_from_str(text, "%Y-%m-%d")
            .ok()
            .filter(|date| date.to_string() == text)
    };
    let (Some(start), Some(end)) = (parse(&query.start), parse(&query.end)) else {
        return Err(AppError::Validation(
            "请使用YYYY-MM-DD格式的有效日期".into(),
        ));
    };
    if start > end {
        return Err(AppError::Validation("开始日期不能晚于结束日期".into()));
    }
    let next = end
        .succ_opt()
        .ok_or_else(|| AppError::Validation("结束日期超出范围".into()))?;
    let keyword = query.keyword.as_deref().unwrap_or("").trim();
    if keyword.chars().count() > 100 {
        return Err(AppError::Validation("关键词不能超过100字符".into()));
    }
    let (page, size, offset) = page(&PageQuery {
        page: query.page,
        page_size: query.page_size,
    })?;
    let sql = format!(
        "{} AND labflow_try_timestamptz(wr.recorded_at)>=CAST(CAST(?1 AS TEXT) AS TIMESTAMPTZ)
         AND labflow_try_timestamptz(wr.recorded_at)<CAST(CAST(?2 AS TEXT) AS TIMESTAMPTZ)
         AND CONCAT_WS(' ',wr.business_no,wr.user_name,wr.method_name_snapshot,m.name,ai.name) ILIKE ?3",
        source_sql()
    );
    let params = params![start.to_string(), next.to_string(), format!("%{keyword}%")];
    let total = conn.query_row(
        &format!("SELECT COUNT(*) FROM ({sql}) candidates"),
        &params,
        |row| row.get(0),
    )?;
    let mut stmt = conn.prepare(&format!(
        "{sql} ORDER BY wr.recorded_at DESC,wr.id DESC LIMIT {size} OFFSET {offset}"
    ))?;
    let records = stmt
        .query_map(&params, map_candidate)?
        .collect::<postgres_compat::Result<Vec<_>>>()?;
    let mut rows = Vec::with_capacity(records.len());
    for record in records {
        validate_candidate(&record)?;
        rows.push(record.row);
    }
    Ok(Page {
        rows,
        total,
        page,
        page_size: size,
    })
}

fn finish_preview(
    rows: Vec<PreviewRow>,
    confirmation_rows: Vec<ConfirmationRow>,
    user_id: i64,
    batch_id: String,
    purpose: &str,
) -> Result<Preview> {
    let mut quantity = 0_i64;
    let mut workload = 0.0;
    for row in &rows {
        quantity = quantity
            .checked_add(row.record.quantity)
            .ok_or_else(|| AppError::Validation("数量合计超出范围".into()))?;
        workload += row.record.workload;
    }
    if !workload.is_finite() {
        return Err(AppError::Validation("工作量合计超出范围".into()));
    }
    let expires = Utc::now() + chrono::Duration::minutes(10);
    let token = encode(
        &Header::default(),
        &Confirmation {
            purpose: purpose.into(),
            sub: user_id,
            exp: expires.timestamp() as usize,
            batch_id,
            rows: confirmation_rows,
        },
        &EncodingKey::from_secret(key().as_bytes()),
    )
    .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Preview {
        record_count: rows.len(),
        rows,
        total_quantity: quantity,
        total_workload: workload,
        confirmation_token: token,
        expires_at: expires.to_rfc3339(),
    })
}

fn preview_assignments(
    conn: &Connection,
    request: &PreviewRequest,
    ctx: &AuthContext,
) -> Result<Preview> {
    require_admin(ctx, true)?;
    let ids = request
        .assignments
        .iter()
        .map(|r| r.record_id)
        .collect::<BTreeSet<_>>();
    if request.assignments.is_empty()
        || request.assignments.len() > MAX_BATCH
        || ids.len() != request.assignments.len()
        || request
            .assignments
            .iter()
            .any(|r| r.record_id <= 0 || r.item_id <= 0)
    {
        return Err(AppError::Validation(
            "每批选择1至100条不重复记录及有效事项".into(),
        ));
    }
    let mut rows = Vec::new();
    let mut signed = Vec::new();
    for id in ids {
        let choice = request
            .assignments
            .iter()
            .find(|row| row.record_id == id)
            .unwrap();
        let record = read_record(conn, id, false)?;
        if record.row.linked_item_id.is_some() {
            return Err(AppError::Conflict("记录已关联事项，请勿重复迁移".into()));
        }
        let item = read_item(conn, choice.item_id, false)?;
        validate_item(&item)?;
        signed.push(ConfirmationRow {
            record_id: id,
            item_id: item.id,
            source_fingerprint: fingerprint(&record.snapshot)?,
            item_fingerprint: fingerprint(&item.snapshot)?,
            link_id: None,
            link_fingerprint: None,
        });
        rows.push(PreviewRow {
            record: record.row,
            item_id: item.id,
            item_name: item.name,
            area_name: item.area_name,
            category: item.category,
        });
    }
    finish_preview(
        rows,
        signed,
        ctx.user.id,
        Uuid::new_v4().to_string(),
        "legacy_auxiliary_apply",
    )
}

fn reason(value: &str) -> Result<&str> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 500 || value.contains('\0') {
        return Err(AppError::Validation("请填写1至500字符的操作原因".into()));
    }
    Ok(value)
}

fn read_batch(conn: &Connection, id: &str) -> Result<BatchRow> {
    conn.query_row(
        "SELECT id,reason,created_by_name,created_at::text,record_count,total_quantity,total_workload,revoked_at::text
         FROM legacy_auxiliary_batches WHERE id=?1",
        [id],
        map_batch,
    ).optional()?.ok_or_else(|| AppError::NotFound("关联批次不存在".into()))
}

fn map_batch(row: &postgres_compat::Row<'_>) -> postgres_compat::Result<BatchRow> {
    Ok(BatchRow {
        id: row.get(0)?,
        reason: row.get(1)?,
        created_by_name: row.get(2)?,
        created_at: row.get(3)?,
        record_count: row.get(4)?,
        total_quantity: row.get(5)?,
        total_workload: row.get(6)?,
        revoked_at: row.get(7)?,
    })
}

fn result(batch: BatchRow, replayed: bool) -> ApplyResult {
    ApplyResult {
        batch_id: batch.id,
        record_count: batch.record_count,
        total_quantity: batch.total_quantity,
        total_workload: batch.total_workload,
        replayed,
    }
}

fn apply_assignments(
    conn: &mut Connection,
    request: &ApplyRequest,
    ctx: &AuthContext,
) -> Result<ApplyResult> {
    require_admin(ctx, true)?;
    let reason = reason(&request.reason)?;
    let claims = read_confirmation(
        &request.confirmation_token,
        ctx.user.id,
        "legacy_auxiliary_apply",
    )?;
    let request_fingerprint = format!(
        "{:x}",
        Sha256::digest(format!("{}\n{reason}", request.confirmation_token).as_bytes())
    );
    let tx = conn.transaction()?;
    // Serialize overlapping association and undo batches before their row locks.
    tx.execute("SELECT pg_advisory_xact_lock(2377,1)", [])?;
    let existing: Option<(String,i64,Option<String>)> = tx.query_row(
        "SELECT request_fingerprint,created_by,revoked_at::text FROM legacy_auxiliary_batches WHERE id=?1",
        [claims.batch_id.as_str()],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
    ).optional()?;
    if let Some((hash, user, revoked)) = existing {
        if hash != request_fingerprint || user != ctx.user.id || revoked.is_some() {
            return Err(AppError::Conflict(
                "关联批次已存在或已撤销，请重新预览".into(),
            ));
        }
        return Ok(result(read_batch(&tx, &claims.batch_id)?, true));
    }
    super::auxiliary_handler::lock_categories(&tx)?;
    let mut targets = std::collections::BTreeMap::new();
    for item_id in claims
        .rows
        .iter()
        .map(|r| r.item_id)
        .collect::<BTreeSet<_>>()
    {
        let item = read_item(&tx, item_id, true)?;
        if let Some(area) = item.area_id {
            tx.query_row(
                "SELECT id FROM auxiliary_work_areas WHERE id=?1 FOR SHARE",
                [area],
                |row| row.get::<_, i64>(0),
            )?;
        }
        targets.insert(item_id, read_item(&tx, item_id, false)?);
    }
    let mut verified = Vec::new();
    let mut total_quantity = 0_i64;
    let mut total_workload = 0.0;
    for expected in &claims.rows {
        let record = read_record(&tx, expected.record_id, true)?;
        let item = &targets[&expected.item_id];
        validate_item(item)?;
        if record.row.linked_item_id.is_some()
            || fingerprint(&record.snapshot)? != expected.source_fingerprint
            || fingerprint(&item.snapshot)? != expected.item_fingerprint
        {
            return Err(AppError::Conflict(
                "记录或事项配置已变化，本批未关联，请重新预览".into(),
            ));
        }
        total_quantity = total_quantity
            .checked_add(record.row.quantity)
            .ok_or_else(|| AppError::Validation("数量合计超出范围".into()))?;
        total_workload += record.row.workload;
        verified.push(record);
    }
    if !total_workload.is_finite() {
        return Err(AppError::Validation("工作量合计超出范围".into()));
    }
    tx.execute(
        "INSERT INTO legacy_auxiliary_batches(id,reason,request_fingerprint,created_by,created_by_name,record_count,total_quantity,total_workload)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
        params![claims.batch_id,reason,request_fingerprint,ctx.user.id,ctx.user.username,verified.len() as i64,total_quantity,total_workload],
    )?;
    for (record, expected) in verified.iter().zip(&claims.rows) {
        let item = &targets[&expected.item_id];
        tx.execute(
            "INSERT INTO legacy_auxiliary_links(source_record_id,auxiliary_work_item_id,area_type_id_snapshot,area_name_snapshot,category_snapshot,batch_id,source_fingerprint,source_snapshot,created_by)
             VALUES(?1,?2,?3,?4,?5,?6,?7,CAST(CAST(?8 AS TEXT) AS JSONB),?9)",
            params![record.row.id,item.id,item.area_id,item.area_name,item.category,claims.batch_id,expected.source_fingerprint,record.snapshot,ctx.user.id],
        )?;
        let before: serde_json::Value = serde_json::from_str(&record.snapshot)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let mut after = before.clone();
        after["_legacy_auxiliary_link"] = serde_json::json!({"batch_id":claims.batch_id,"item_id":item.id,"item_name":item.name,"area_name":item.area_name,"category":item.category});
        audit_repo::log_structured_actor_on_conn(
            &tx,
            "update",
            "work_records",
            Some(record.row.id),
            ctx.user.id,
            &ctx.user.username,
            &format!("历史辅助关联：{reason}；仅新增事项关联，保留原始记录及全部计量快照"),
            "work",
            &record.row.business_no,
            Some(&before),
            Some(&after),
            "legacy_auxiliary",
        )?;
    }
    let batch = read_batch(&tx, &claims.batch_id)?;
    tx.commit()?;
    Ok(result(batch, false))
}

fn preview_undo(conn: &Connection, id: &str, ctx: &AuthContext) -> Result<Preview> {
    require_admin(ctx, true)?;
    let batch = read_batch(conn, id)?;
    if batch.revoked_at.is_some() {
        return Err(AppError::Conflict("该批次已撤销".into()));
    }
    let mut stmt=conn.prepare("SELECT id,source_record_id,auxiliary_work_item_id,source_fingerprint,to_jsonb(ll)::text,area_name_snapshot,category_snapshot FROM legacy_auxiliary_links ll WHERE batch_id=?1 AND revoked_at IS NULL ORDER BY source_record_id")?;
    let links = stmt
        .query_map([id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
            ))
        })?
        .collect::<postgres_compat::Result<Vec<_>>>()?;
    if links.len() != batch.record_count as usize || links.is_empty() || links.len() > MAX_BATCH {
        return Err(AppError::Conflict("批次关联已变化，不能整体撤销".into()));
    }
    let mut rows = Vec::new();
    let mut signed = Vec::new();
    for (link_id, record_id, item_id, source_fingerprint, link_snapshot, area_name, category) in
        links
    {
        let record = read_record(conn, record_id, false)?;
        if fingerprint(&record.snapshot)? != source_fingerprint
            || record.row.linked_batch_id.as_deref() != Some(id)
        {
            return Err(AppError::Conflict(
                "关联后原记录已被维护，请先核对后续修改，不能直接撤销该批次".into(),
            ));
        }
        let item = read_item(conn, item_id, false)?;
        signed.push(ConfirmationRow {
            record_id,
            item_id,
            source_fingerprint,
            item_fingerprint: String::new(),
            link_id: Some(link_id),
            link_fingerprint: Some(fingerprint(&link_snapshot)?),
        });
        rows.push(PreviewRow {
            record: record.row,
            item_id,
            item_name: item.name,
            area_name,
            category,
        });
    }
    finish_preview(
        rows,
        signed,
        ctx.user.id,
        id.to_owned(),
        "legacy_auxiliary_undo",
    )
}

fn undo_assignments(
    conn: &mut Connection,
    id: &str,
    request: &ApplyRequest,
    ctx: &AuthContext,
) -> Result<ApplyResult> {
    require_admin(ctx, true)?;
    let reason = reason(&request.reason)?;
    let claims = read_confirmation(
        &request.confirmation_token,
        ctx.user.id,
        "legacy_auxiliary_undo",
    )?;
    if claims.batch_id != id {
        return Err(AppError::Validation("撤销确认与批次不符".into()));
    }
    let tx = conn.transaction()?;
    tx.execute("SELECT pg_advisory_xact_lock(2377,1)", [])?;
    let batch = read_batch(&tx, id)?;
    if batch.revoked_at.is_some() {
        return Err(AppError::Conflict(
            "批次已撤销，请刷新结果，系统不会重复撤销".into(),
        ));
    }
    let active_count: i64 = tx.query_row(
        "SELECT COUNT(*) FROM legacy_auxiliary_links WHERE batch_id=?1 AND revoked_at IS NULL",
        [id],
        |row| row.get(0),
    )?;
    if active_count != claims.rows.len() as i64 || active_count != batch.record_count {
        return Err(AppError::Conflict("批次关联数量已变化".into()));
    }
    let mut verified = Vec::new();
    for expected in &claims.rows {
        let record = read_record(&tx, expected.record_id, true)?;
        let link:Option<(i64,String)>=tx.query_row(
            "SELECT id,to_jsonb(ll)::text FROM legacy_auxiliary_links ll WHERE id=?1 AND batch_id=?2 AND revoked_at IS NULL FOR UPDATE",
            params![expected.link_id,id],|row|Ok((row.get(0)?,row.get(1)?)),
        ).optional()?;
        let link = link.ok_or_else(|| AppError::Conflict("关联已被撤销或更换".into()))?;
        if fingerprint(&record.snapshot)? != expected.source_fingerprint
            || Some(fingerprint(&link.1)?) != expected.link_fingerprint
            || record.row.linked_item_id != Some(expected.item_id)
            || record.row.linked_batch_id.as_deref() != Some(id)
        {
            return Err(AppError::Conflict(
                "预览后记录或关联已变化，本批未撤销".into(),
            ));
        }
        verified.push((record, link.0));
    }
    for (record, link_id) in verified {
        let before: serde_json::Value = serde_json::from_str(&record.snapshot)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let mut linked = before.clone();
        linked["_legacy_auxiliary_link"] =
            serde_json::json!({"batch_id":id,"item_id":record.row.linked_item_id});
        tx.execute(
            "UPDATE legacy_auxiliary_links SET revoked_at=clock_timestamp() WHERE id=?1",
            [link_id],
        )?;
        audit_repo::log_structured_actor_on_conn(
            &tx,
            "update",
            "work_records",
            Some(record.row.id),
            ctx.user.id,
            &ctx.user.username,
            &format!("撤销历史辅助关联：{reason}；仅撤销本批关联，原始记录及计量快照保持"),
            "work",
            &record.row.business_no,
            Some(&linked),
            Some(&before),
            "legacy_auxiliary_undo",
        )?;
    }
    tx.execute("UPDATE legacy_auxiliary_batches SET revoked_at=clock_timestamp(),revoked_by=?1,revoke_reason=?2 WHERE id=?3",params![ctx.user.id,reason,id])?;
    tx.commit()?;
    Ok(result(batch, false))
}

async fn records(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(query): Query<RecordQuery>,
) -> Result<Json<ApiResponse<Page<CandidateRow>>>> {
    tokio::task::spawn_blocking(move || {
        let ctx = authz_service::authenticate(&pool, &headers)?;
        require_admin(&ctx, false)?;
        let conn = pool.get()?;
        Ok(Json(ApiResponse::ok(list_records(&conn, &query)?)))
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}
async fn preview(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(body): Json<PreviewRequest>,
) -> Result<Json<ApiResponse<Preview>>> {
    tokio::task::spawn_blocking(move || {
        let ctx = authz_service::authenticate(&pool, &headers)?;
        let conn = pool.get()?;
        Ok(Json(ApiResponse::ok(preview_assignments(
            &conn, &body, &ctx,
        )?)))
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}
async fn apply(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(body): Json<ApplyRequest>,
) -> Result<Json<ApiResponse<ApplyResult>>> {
    tokio::task::spawn_blocking(move || {
        let ctx = authz_service::authenticate(&pool, &headers)?;
        let mut conn = pool.get()?;
        Ok(Json(ApiResponse::ok(apply_assignments(
            &mut conn, &body, &ctx,
        )?)))
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}
async fn batches(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(query): Query<PageQuery>,
) -> Result<Json<ApiResponse<Page<BatchRow>>>> {
    tokio::task::spawn_blocking(move||{
        let ctx=authz_service::authenticate(&pool,&headers)?;
        require_admin(&ctx,false)?;
        let (page,size,offset)=page(&query)?;
        let conn=pool.get()?;
        let total=conn.query_row("SELECT COUNT(*) FROM legacy_auxiliary_batches",[],|row|row.get(0))?;
        let mut stmt=conn.prepare(&format!("SELECT id,reason,created_by_name,created_at::text,record_count,total_quantity,total_workload,revoked_at::text FROM legacy_auxiliary_batches ORDER BY created_at DESC,id DESC LIMIT {size} OFFSET {offset}"))?;
        let rows=stmt.query_map([],map_batch)?.collect::<postgres_compat::Result<Vec<_>>>()?;
        Ok(Json(ApiResponse::ok(Page{rows,total,page,page_size:size})))
    }).await.map_err(|e|AppError::Internal(e.to_string()))?
}
async fn undo_preview(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Preview>>> {
    tokio::task::spawn_blocking(move || {
        let ctx = authz_service::authenticate(&pool, &headers)?;
        let conn = pool.get()?;
        Ok(Json(ApiResponse::ok(preview_undo(&conn, &id, &ctx)?)))
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}
async fn undo(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<ApplyRequest>,
) -> Result<Json<ApiResponse<ApplyResult>>> {
    tokio::task::spawn_blocking(move || {
        let ctx = authz_service::authenticate(&pool, &headers)?;
        let mut conn = pool.get()?;
        Ok(Json(ApiResponse::ok(undo_assignments(
            &mut conn, &id, &body, &ctx,
        )?)))
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (DbPool, AuthContext, i64, i64) {
        let pool = crate::db::init_pool(":memory:");
        let conn = pool.get().unwrap();
        crate::db::test_migrations::run(&conn).unwrap();
        conn.execute("INSERT INTO users(username,password,is_admin,is_active) VALUES('关联管理员','hash',1,1)",[]).unwrap();
        let admin = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO project_groups(name) VALUES('旧辅助实验室')",
            [],
        )
        .unwrap();
        let group = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO projects(group_id,name) VALUES(?1,'旧辅助项目')",
            [group],
        )
        .unwrap();
        let project = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO auxiliary_work_areas(name) VALUES('独立液相区域')",
            [],
        )
        .unwrap();
        let area = conn.last_insert_rowid();
        conn.execute("INSERT INTO auxiliary_work_items(name,coefficient,area_type_id,scope_type) VALUES('新辅助事项',99,?1,'public')",[area]).unwrap();
        let item = conn.last_insert_rowid();
        settings_repo::upsert_on_conn(
            &conn,
            settings_repo::WORKLOAD_AUXILIARY_CATEGORIES_KEY,
            &serde_json::json!({format!("item:{item}"):"仪器维护"}).to_string(),
        )
        .unwrap();
        drop(conn);
        let user = crate::repo::user_repo::find_by_id(&pool, admin)
            .unwrap()
            .unwrap();
        let ctx = AuthContext {
            role_names: user.role_names.clone(),
            user,
        };
        (pool, ctx, project, item)
    }

    fn record(
        conn: &Connection,
        project: i64,
        quantity: i64,
        coefficient: f64,
        auxiliary: bool,
    ) -> i64 {
        conn.execute(
            "INSERT INTO work_records(project_id,user_name,quantity,recorded_at,coefficient_snapshot,multiplier,business_no,method_name_snapshot,instrument_type_snapshot,source_type,source_record_id)
             VALUES(?1,'旧人员快照',?2,'2026-10-08 08:00:00',?3,3,?4,'旧辅助方法',?5,'rd_sample',98765)",
            params![project,quantity,coefficient,Uuid::new_v4().to_string(),if auxiliary {"辅助工作"}else{"液相"}],
        ).unwrap();
        conn.last_insert_rowid()
    }

    fn snapshot(conn: &Connection, id: i64) -> serde_json::Value {
        let raw: String = conn
            .query_row(
                "SELECT to_jsonb(wr)::text FROM work_records wr WHERE id=?1",
                [id],
                |row| row.get(0),
            )
            .unwrap();
        serde_json::from_str(&raw).unwrap()
    }

    fn choices(ids: &[i64], item: i64) -> PreviewRequest {
        PreviewRequest {
            assignments: ids
                .iter()
                .map(|id| Assignment {
                    record_id: *id,
                    item_id: item,
                })
                .collect(),
        }
    }

    fn request(preview: Preview) -> ApplyRequest {
        ApplyRequest {
            confirmation_token: preview.confirmation_token,
            reason: "确认旧辅助归类，保留原始计量".into(),
        }
    }

    #[test]
    fn legacy_association_is_lossless_idempotent_and_undoable_without_recounting_rd_parts() {
        let (pool, ctx, project, item) = setup();
        let mut conn = pool.get().unwrap();
        let first = record(&conn, project, 2, 0.0, true);
        let second = record(&conn, project, 3, 2.5, true);
        let before = [snapshot(&conn, first), snapshot(&conn, second)];
        let candidates = list_records(
            &conn,
            &RecordQuery {
                start: "2026-10-08".into(),
                end: "2026-10-08".into(),
                keyword: Some("旧人员".into()),
                page: Some(1),
                page_size: Some(1),
            },
        )
        .unwrap();
        assert_eq!(candidates.total, 2);
        assert_eq!(candidates.rows.len(), 1);
        let preview = preview_assignments(&conn, &choices(&[first, second], item), &ctx).unwrap();
        assert_eq!(preview.record_count, 2);
        assert_eq!(preview.total_quantity, 5);
        assert_eq!(preview.total_workload, 7.5);
        assert!(preview
            .rows
            .iter()
            .all(|row| row.category == "仪器维护" && row.area_name == "独立液相区域"));
        let apply = request(preview);
        let completed = apply_assignments(&mut conn, &apply, &ctx).unwrap();
        assert_eq!(completed.record_count, 2);
        assert!(!completed.replayed);
        assert!(apply_assignments(&mut conn, &apply, &ctx).unwrap().replayed);
        assert_eq!([snapshot(&conn, first), snapshot(&conn, second)], before);
        let (links,copies):(i64,i64)=conn.query_row("SELECT (SELECT COUNT(*) FROM legacy_auxiliary_links WHERE revoked_at IS NULL),(SELECT COUNT(*) FROM auxiliary_work_records)",[],|row|Ok((row.get(0)?,row.get(1)?))).unwrap();
        assert_eq!((links, copies), (2, 0));
        let snapshots:i64=conn.query_row("SELECT COUNT(*) FROM legacy_auxiliary_links WHERE category_snapshot='仪器维护' AND area_name_snapshot='独立液相区域' AND source_snapshot->>'source_type'='rd_sample' AND source_snapshot->>'source_record_id'='98765'",[],|row|row.get(0)).unwrap();
        assert_eq!(snapshots, 2);
        assert!(preview_assignments(&conn, &choices(&[first], item), &ctx).is_err());
        conn.execute(
            "UPDATE auxiliary_work_items SET coefficient=333,name='改名后的事项' WHERE id=?1",
            [item],
        )
        .unwrap();
        let undo = request(preview_undo(&conn, &completed.batch_id, &ctx).unwrap());
        undo_assignments(&mut conn, &completed.batch_id, &undo, &ctx).unwrap();
        assert_eq!([snapshot(&conn, first), snapshot(&conn, second)], before);
        let active: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM legacy_auxiliary_links WHERE revoked_at IS NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(active, 0);
        assert_eq!(
            read_batch(&conn, &completed.batch_id).unwrap().record_count,
            2
        );
        assert!(read_batch(&conn, &completed.batch_id)
            .unwrap()
            .revoked_at
            .is_some());
        assert!(preview_undo(&conn, &completed.batch_id, &ctx).is_err());
        let again = request(preview_assignments(&conn, &choices(&[first], item), &ctx).unwrap());
        assert_eq!(
            apply_assignments(&mut conn, &again, &ctx)
                .unwrap()
                .record_count,
            1
        );
        assert!(
            !crate::repo::trash_repo::precheck(&pool, "work_records", second)
                .unwrap()
                .can_purge
        );
        conn.execute(
            "UPDATE work_records SET deleted_at=now() WHERE id=?1",
            [second],
        )
        .unwrap();
        let trash = crate::repo::trash_repo::move_to_trash_on_conn(
            &conn,
            "分析检测记录",
            "work_records",
            second,
            "records",
            "work",
            "迁移审计历史",
            "",
            &before[1],
            "测试旧清理标记",
            &ctx.user.username,
            None,
            None,
            "",
            true,
        )
        .unwrap();
        assert!(
            !crate::repo::trash_repo::get_active(&pool, trash)
                .unwrap()
                .can_purge
        );
        assert!(matches!(
            crate::repo::trash_repo::purge(&pool, trash, ctx.user.id, &ctx.user.username),
            Err(AppError::Conflict(_))
        ));
        assert!(crate::repo::trash_repo::list(
            &pool,
            crate::repo::trash_repo::TrashScope::Global,
            None,
            None,
            None,
            1,
            50
        )
        .unwrap()
        .0
        .iter()
        .any(|row| row.record_id == second && !row.can_purge));
    }

    #[test]
    fn legacy_association_rejects_stale_ambiguous_native_and_unauthorized_changes_atomically() {
        let (pool, ctx, project, item) = setup();
        let mut conn = pool.get().unwrap();
        let first = record(&conn, project, 2, 2.0, true);
        let second = record(&conn, project, 3, 4.0, true);
        let standard = record(&conn, project, 1, 1.0, false);
        assert!(preview_assignments(&conn, &choices(&[standard], item), &ctx).is_err());
        let native = record(&conn, project, 1, 1.0, true);
        conn.execute(
            "UPDATE work_records SET source_type='auxiliary_work' WHERE id=?1",
            [native],
        )
        .unwrap();
        assert!(preview_assignments(&conn, &choices(&[native], item), &ctx).is_err());
        assert!(preview_assignments(&conn, &choices(&[first, first], item), &ctx).is_err());
        assert!(preview_assignments(&conn, &choices(&vec![first; 101], item), &ctx).is_err());
        let mut normal = ctx.clone();
        normal.user.is_admin = false;
        normal.user.role_keys.clear();
        assert!(preview_assignments(&conn, &choices(&[first], item), &normal).is_err());
        let stale =
            request(preview_assignments(&conn, &choices(&[first, second], item), &ctx).unwrap());
        conn.execute("UPDATE work_records SET quantity=4 WHERE id=?1", [second])
            .unwrap();
        assert!(apply_assignments(&mut conn, &stale, &ctx).is_err());
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM legacy_auxiliary_batches", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
        let stale_item =
            request(preview_assignments(&conn, &choices(&[first], item), &ctx).unwrap());
        conn.execute(
            "UPDATE auxiliary_work_items SET coefficient=101 WHERE id=?1",
            [item],
        )
        .unwrap();
        assert!(apply_assignments(&mut conn, &stale_item, &ctx).is_err());
        let stale_category =
            request(preview_assignments(&conn, &choices(&[first], item), &ctx).unwrap());
        settings_repo::upsert_on_conn(
            &conn,
            settings_repo::WORKLOAD_AUXILIARY_CATEGORIES_KEY,
            &serde_json::json!({format!("item:{item}"):"修改后的分类"}).to_string(),
        )
        .unwrap();
        assert!(apply_assignments(&mut conn, &stale_category, &ctx).is_err());
        let audit_failed =
            request(preview_assignments(&conn, &choices(&[first, second], item), &ctx).unwrap());
        conn.execute_batch("ALTER TABLE audit_log ADD CONSTRAINT reject_legacy_association_audit CHECK(source<>'legacy_auxiliary')").unwrap();
        assert!(apply_assignments(&mut conn, &audit_failed, &ctx).is_err());
        let (batch_count,link_count):(i64,i64)=conn.query_row("SELECT (SELECT COUNT(*) FROM legacy_auxiliary_batches),(SELECT COUNT(*) FROM legacy_auxiliary_links)",[],|row|Ok((row.get(0)?,row.get(1)?))).unwrap();
        assert_eq!((batch_count, link_count), (0, 0));
    }

    #[test]
    fn legacy_undo_requires_fresh_owned_preview_and_preserves_later_record_edits() {
        let (pool, ctx, project, item) = setup();
        let mut conn = pool.get().unwrap();
        let first = record(&conn, project, 1, 2.0, true);
        let apply = request(preview_assignments(&conn, &choices(&[first], item), &ctx).unwrap());
        let completed = apply_assignments(&mut conn, &apply, &ctx).unwrap();
        let undo = request(preview_undo(&conn, &completed.batch_id, &ctx).unwrap());
        assert!(read_confirmation(
            &undo.confirmation_token,
            ctx.user.id + 1,
            "legacy_auxiliary_undo"
        )
        .is_err());
        assert!(read_confirmation(
            &undo.confirmation_token,
            ctx.user.id,
            "legacy_auxiliary_apply"
        )
        .is_err());
        let mut expired = read_confirmation(
            &undo.confirmation_token,
            ctx.user.id,
            "legacy_auxiliary_undo",
        )
        .unwrap();
        expired.exp = 1;
        let expired = encode(
            &Header::default(),
            &expired,
            &EncodingKey::from_secret(key().as_bytes()),
        )
        .unwrap();
        assert!(read_confirmation(&expired, ctx.user.id, "legacy_auxiliary_undo").is_err());
        conn.execute(
            "UPDATE work_records SET extra_info='关联后人工维护' WHERE id=?1",
            [first],
        )
        .unwrap();
        assert!(undo_assignments(&mut conn, &completed.batch_id, &undo, &ctx).is_err());
        assert!(preview_undo(&conn, &completed.batch_id, &ctx).is_err());
        assert!(read_batch(&conn, &completed.batch_id)
            .unwrap()
            .revoked_at
            .is_none());
        let active: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM legacy_auxiliary_links WHERE revoked_at IS NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(active, 1);
        let (safe_json,safe_date):(bool,bool)=conn.query_row("SELECT labflow_try_jsonb('broken') IS NULL,labflow_try_timestamptz('broken') IS NULL",[],|row|Ok((row.get(0)?,row.get(1)?))).unwrap();
        assert!(safe_json && safe_date);
    }
}
