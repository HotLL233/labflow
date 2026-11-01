use super::*;

const STATUSES: &[(&str, &str)] = &[
    ("difference", "可修正差异"),
    ("equal", "系数一致"),
    ("method_deleted", "原方法已删除"),
    ("method_missing", "原方法不存在"),
    ("unbound", "历史方法未关联"),
    ("custom", "明确自定义方法"),
    ("auxiliary", "辅助工作，单独核对"),
    ("record_deleted", "记录已删除"),
    ("invalid", "日期或数值异常"),
];
const MAX_FLOAT: &str = "1.7976931348623157e308::DOUBLE PRECISION";

pub(super) fn valid_record_numbers_sql() -> String {
    format!(
        "wr.quantity>0 AND wr.coefficient_snapshot>=0
         AND wr.coefficient_snapshot<={MAX_FLOAT}/GREATEST(ABS(wr.quantity::DOUBLE PRECISION),1)
         AND wr.multiplier BETWEEN -{MAX_FLOAT} AND {MAX_FLOAT}"
    )
}

pub(super) fn valid_method_coefficient_sql() -> String {
    format!(
        "m.coefficient>=0 AND m.coefficient<={MAX_FLOAT}/GREATEST(ABS(wr.quantity::DOUBLE PRECISION),1)"
    )
}

#[derive(Debug, Deserialize)]
pub(super) struct DiagnosticQuery {
    start: String,
    end: String,
    keyword: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
    status: Option<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct DiagnosticRow {
    id: i64,
    business_no: String,
    user_name: String,
    recorded_at: String,
    project_name: String,
    lab_name: String,
    source_type: String,
    source_record_id: Option<i64>,
    method_id: Option<i64>,
    method_name: String,
    current_method_name: String,
    instrument_name: String,
    quantity: i64,
    coefficient_snapshot: Option<f64>,
    method_coefficient: Option<f64>,
    method_active: Option<bool>,
    status: String,
    status_label: String,
    can_reassociate: bool,
    before_workload: Option<f64>,
    origin: String,
}

#[derive(Debug, Serialize)]
struct DiagnosticCount {
    status: String,
    label: String,
    count: i64,
}

#[derive(Debug, Serialize)]
pub(super) struct DiagnosticResult {
    rows: Vec<DiagnosticRow>,
    total: i64,
    page: i64,
    page_size: i64,
    counts: Vec<DiagnosticCount>,
}

fn status_label(status: &str) -> &str {
    STATUSES
        .iter()
        .find(|(key, _)| *key == status)
        .map_or("未知状态", |(_, label)| *label)
}

fn diagnostic_select() -> String {
    let auxiliary = super::super::export_data::legacy_auxiliary_record_sql("wr");
    format!(
        "SELECT wr.id,wr.business_no,wr.user_name,wr.recorded_at,
          COALESCE(NULLIF(wr.project_name_snapshot,''),p.name,'') AS project_name,
          COALESCE(NULLIF(wr.lab_name_snapshot,''),pg.name,'') AS lab_name,
          COALESCE(NULLIF(wr.source_type,''),'analysis') AS source_type,wr.source_record_id,
          wr.method_id,COALESCE(NULLIF(wr.method_name_snapshot,''),m.name,'') AS method_name,
          COALESCE(NULLIF(m.full_name,''),m.name,'') AS current_method_name,
          COALESCE(NULLIF(wr.instrument_code_snapshot,''),i.name,'') AS instrument_name,
          wr.quantity,wr.coefficient_snapshot,m.coefficient AS method_coefficient,
          (m.is_active<>0) AS method_active,
          CASE WHEN wr.deleted_at IS NOT NULL THEN 'record_deleted'
            WHEN NOT COALESCE(isfinite(labflow_try_timestamptz(wr.recorded_at)),FALSE)
              OR labflow_try_jsonb(COALESCE(NULLIF(wr.extra_fields,''),'{{}}')) IS NULL
              OR NOT COALESCE(({valid_record}),FALSE) THEN 'invalid'
            WHEN {auxiliary} THEN 'auxiliary'
            WHEN COALESCE(labflow_try_jsonb(wr.extra_fields)->'source_custom_method','false'::jsonb)='true'::jsonb THEN 'custom'
            WHEN wr.method_id IS NULL THEN 'unbound'
            WHEN m.id IS NULL THEN 'method_missing'
            WHEN m.deleted_at IS NOT NULL THEN 'method_deleted'
            WHEN NOT COALESCE(({valid_method}),FALSE) THEN 'invalid'
            WHEN wr.coefficient_snapshot=m.coefficient THEN 'equal'
            ELSE 'difference' END AS status,
          to_jsonb(wr)::text AS snapshot,'work_record' AS origin
         FROM work_records wr LEFT JOIN methods m ON m.id=wr.method_id
          LEFT JOIN projects p ON p.id=wr.project_id
          LEFT JOIN project_groups pg ON pg.id=wr.group_id
          LEFT JOIN instruments i ON i.id=m.instrument_id",
        valid_record = valid_record_numbers_sql(),
        valid_method = valid_method_coefficient_sql(),
    )
}

fn diagnostic_sources() -> String {
    format!(
        "{} UNION ALL
         SELECT awr.id,'' AS business_no,awr.user_name_snapshot,awr.recorded_at::text,
          '' AS project_name,'' AS lab_name,'auxiliary_work',awr.id,NULL::BIGINT,
          awr.auxiliary_work_name_snapshot,'' AS current_method_name,'' AS instrument_name,
          awr.quantity,awr.coefficient_snapshot,NULL::DOUBLE PRECISION,NULL::BOOLEAN,
          CASE WHEN awr.deleted_at IS NOT NULL OR COALESCE(awr.status,'有效')='作废' THEN 'record_deleted'
            WHEN NOT isfinite(awr.recorded_at) OR awr.quantity<=0 OR NOT COALESCE(awr.coefficient_snapshot>=0
              AND awr.coefficient_snapshot<={MAX_FLOAT}/GREATEST(ABS(awr.quantity::DOUBLE PRECISION),1),FALSE) THEN 'invalid'
            ELSE 'auxiliary' END AS status,
          to_jsonb(awr)::text,'auxiliary_record'
         FROM auxiliary_work_records awr
         WHERE NOT EXISTS(SELECT 1 FROM work_records mirror
           WHERE mirror.source_type='auxiliary_work' AND mirror.source_record_id=awr.id)",
        diagnostic_select(),
    )
}

fn map_diagnostic(row: &postgres_compat::Row<'_>) -> postgres_compat::Result<DiagnosticRow> {
    let quantity: i64 = row.get(12)?;
    let coefficient_snapshot = row.get::<_, Option<f64>>(13)?.filter(|v| v.is_finite());
    let status: String = row.get(16)?;
    let origin: String = row.get(18)?;
    Ok(DiagnosticRow {
        id: row.get(0)?,
        business_no: row.get(1)?,
        user_name: row.get(2)?,
        recorded_at: row.get(3)?,
        project_name: row.get(4)?,
        lab_name: row.get(5)?,
        source_type: row.get(6)?,
        source_record_id: row.get(7)?,
        method_id: row.get(8)?,
        method_name: row.get(9)?,
        current_method_name: row.get(10)?,
        instrument_name: row.get(11)?,
        quantity,
        coefficient_snapshot,
        method_coefficient: row.get::<_, Option<f64>>(14)?.filter(|v| v.is_finite()),
        method_active: row.get(15)?,
        status_label: status_label(&status).into(),
        can_reassociate: origin == "work_record"
            && matches!(
                status.as_str(),
                "difference" | "equal" | "method_deleted" | "method_missing" | "unbound"
            ),
        before_workload: coefficient_snapshot
            .map(|value| value * quantity as f64)
            .filter(|v| v.is_finite()),
        status,
        origin,
    })
}

fn diagnose_records(conn: &Connection, query: &DiagnosticQuery) -> Result<DiagnosticResult> {
    let filters = query;
    let parse = |value: &str| {
        NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .ok()
            .filter(|date| date.format("%Y-%m-%d").to_string() == value)
    };
    let (Some(start), Some(end)) = (parse(&filters.start), parse(&filters.end)) else {
        return Err(AppError::Validation(
            "请使用 YYYY-MM-DD 格式的有效日期".into(),
        ));
    };
    if start > end {
        return Err(AppError::Validation("开始日期不能晚于结束日期".into()));
    }
    let next = end
        .succ_opt()
        .ok_or_else(|| AppError::Validation("结束日期超出可用范围".into()))?;
    let page = filters.page.unwrap_or(1);
    let page_size = filters.page_size.unwrap_or(50);
    if page < 1 || !(1..=MAX_COEFFICIENT_PREVIEW_ROWS as i64).contains(&page_size) {
        return Err(AppError::Validation(
            "每页数量须为1至500，页码须大于0".into(),
        ));
    }
    let offset = (page - 1)
        .checked_mul(page_size)
        .ok_or_else(|| AppError::Validation("页码超出范围".into()))?;
    let keyword = filters.keyword.as_deref().unwrap_or("").trim();
    let status = query.status.as_deref().unwrap_or("");
    if keyword.chars().count() > 100
        || (!status.is_empty() && !STATUSES.iter().any(|(key, _)| *key == status))
    {
        return Err(AppError::Validation("搜索关键词过长或诊断状态无效".into()));
    }
    let scoped = format!(
        "WITH diagnostic_records AS ({}), scoped AS
         (SELECT * FROM diagnostic_records
          WHERE (NOT COALESCE(isfinite(labflow_try_timestamptz(recorded_at)),FALSE)
            OR (labflow_try_timestamptz(recorded_at)>=CAST(CAST(?1 AS TEXT) AS TIMESTAMPTZ)
             AND labflow_try_timestamptz(recorded_at)<CAST(CAST(?2 AS TEXT) AS TIMESTAMPTZ)))
          AND CONCAT_WS(' ',business_no,user_name,project_name,lab_name,method_name,
              current_method_name,instrument_name) ILIKE ?3)",
        diagnostic_sources(),
    );
    let params =
        postgres_compat::params![start.to_string(), next.to_string(), format!("%{keyword}%")];
    let mut count_stmt = conn.prepare(&format!(
        "{scoped} SELECT status,COUNT(*)::BIGINT FROM scoped GROUP BY status"
    ))?;
    let grouped = count_stmt
        .query_map(&params, |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?
        .collect::<postgres_compat::Result<BTreeMap<_, _>>>()?;
    let counts = STATUSES
        .iter()
        .map(|(key, label)| DiagnosticCount {
            status: (*key).into(),
            label: (*label).into(),
            count: *grouped.get(*key).unwrap_or(&0),
        })
        .collect::<Vec<_>>();
    let total = counts
        .iter()
        .filter(|row| status.is_empty() || row.status == status)
        .map(|row| row.count)
        .sum();
    let mut stmt = conn.prepare(&format!(
        "{scoped} SELECT * FROM scoped WHERE (?4='' OR status=?4)
         ORDER BY labflow_try_timestamptz(recorded_at) DESC NULLS LAST,origin,id DESC LIMIT {page_size} OFFSET {offset}"
    ))?;
    let rows = stmt
        .query_map(
            postgres_compat::params![
                start.to_string(),
                next.to_string(),
                format!("%{keyword}%"),
                status
            ],
            map_diagnostic,
        )?
        .collect::<postgres_compat::Result<Vec<_>>>()?;
    Ok(DiagnosticResult {
        rows,
        total,
        page,
        page_size,
        counts,
    })
}

pub(super) async fn diagnose(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(query): Query<DiagnosticQuery>,
) -> Result<Json<ApiResponse<DiagnosticResult>>> {
    tokio::task::spawn_blocking(move || {
        let ctx = authz_service::authenticate(&pool, &headers)?;
        require_coefficient_admin(&ctx)?;
        let conn = pool.get()?;
        Ok(Json(ApiResponse::ok(diagnose_records(&conn, &query)?)))
    })
    .await
    .map_err(|error| AppError::Internal(error.to_string()))?
}

#[derive(Debug, Deserialize)]
pub(super) struct AssociationRequest {
    record_id: i64,
    method_id: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct AssociationClaims {
    purpose: String,
    sub: i64,
    exp: usize,
    record_id: i64,
    method_id: i64,
    old_method_id: Option<i64>,
    record_fingerprint: String,
    method_fingerprint: String,
    old_method_fingerprint: String,
}

#[derive(Debug, Serialize)]
pub(super) struct AssociationPreview {
    record_id: i64,
    business_no: String,
    old_method_id: Option<i64>,
    new_method_id: i64,
    old_method_name: String,
    new_method_name: String,
    coefficient_snapshot: f64,
    method_coefficient: f64,
    quantity: i64,
    before_workload: f64,
    after_workload: f64,
    old_unit_price: Option<f64>,
    new_unit_price: Option<f64>,
    multiplier: f64,
    before_amount: Option<f64>,
    after_amount: Option<f64>,
    amount_difference: Option<f64>,
    confirmation_token: String,
    expires_at: String,
}

fn record_for_association(
    conn: &Connection,
    id: i64,
) -> Result<(DiagnosticRow, serde_json::Value)> {
    let mut stmt = conn.prepare(&format!("{} WHERE wr.id=?1", diagnostic_select()))?;
    let value = stmt
        .query_row([id], |row| {
            Ok((map_diagnostic(row)?, row.get::<_, String>(17)?))
        })
        .optional()?
        .ok_or_else(|| AppError::Validation("原工作量记录不存在".into()))?;
    if !value.0.can_reassociate {
        return Err(AppError::Validation(
            "该记录为辅助、自定义、已删除或异常状态，不能修正普通方法关联".into(),
        ));
    }
    let snapshot = serde_json::from_str(&value.1)
        .map_err(|_| AppError::Internal("读取原记录快照失败".into()))?;
    Ok((value.0, snapshot))
}

struct MethodIdentity {
    name: String,
    coefficient: f64,
    price: Option<f64>,
    snapshot: String,
    deleted: bool,
    auxiliary: bool,
}

fn method_identity(conn: &Connection, id: Option<i64>) -> Result<Option<MethodIdentity>> {
    let Some(id) = id else {
        return Ok(None);
    };
    conn.query_row(
        "SELECT COALESCE(NULLIF(m.full_name,''),m.name,''),m.coefficient,m.amount,
          jsonb_build_object('method',to_jsonb(m),'instrument',to_jsonb(i),'types',
           (SELECT jsonb_agg(to_jsonb(mt) ORDER BY mt.id) FROM method_type_links ml
             JOIN method_types mt ON mt.id=ml.method_type_id WHERE ml.method_id=m.id))::text,
          m.deleted_at IS NOT NULL,COALESCE(i.instrument_type,'')='辅助工作'
           OR EXISTS(SELECT 1 FROM method_type_links ml JOIN method_types mt
             ON mt.id=ml.method_type_id WHERE ml.method_id=m.id AND mt.name='辅助工作')
         FROM methods m LEFT JOIN instruments i ON i.id=m.instrument_id
         WHERE m.id=?1",
        [id],
        |row| {
            Ok(MethodIdentity {
                name: row.get(0)?,
                coefficient: row.get(1)?,
                price: row.get(2)?,
                snapshot: row.get(3)?,
                deleted: row.get(4)?,
                auxiliary: row.get(5)?,
            })
        },
    )
    .optional()
    .map_err(AppError::from)
}

fn target_method(conn: &Connection, id: i64) -> Result<MethodIdentity> {
    method_identity(conn, Some(id))?
        .filter(|value| !value.deleted && !value.auxiliary)
        .ok_or_else(|| {
            AppError::Validation("请选择仍存在的普通标准方法；辅助事项不能用于此关联".into())
        })
}

fn method_fingerprint(method: Option<&MethodIdentity>) -> String {
    hex_sha256(
        method
            .map_or("null", |value| value.snapshot.as_str())
            .as_bytes(),
    )
}

fn amount(price: Option<f64>, quantity: i64, multiplier: f64) -> Result<Option<f64>> {
    match price {
        None => Ok(None),
        Some(price) => {
            let value = price * quantity as f64 * multiplier;
            if !price.is_finite() || !value.is_finite() {
                return Err(AppError::Validation(
                    "方法单价或预计金额异常，请核对配置后再关联".into(),
                ));
            }
            Ok(Some(value))
        }
    }
}

fn preview_association(
    conn: &Connection,
    request: &AssociationRequest,
    actor_id: i64,
) -> Result<AssociationPreview> {
    if request.record_id <= 0 || request.method_id <= 0 {
        return Err(AppError::Validation("请选择有效记录和方法".into()));
    }
    let (record, snapshot) = record_for_association(conn, request.record_id)?;
    if record.method_id == Some(request.method_id) {
        return Err(AppError::Validation(
            "所选方法与原关联一致，无需修改".into(),
        ));
    }
    let target = target_method(conn, request.method_id)?;
    let original = method_identity(conn, record.method_id)?;
    record_repo::validate_workload_coefficient(target.coefficient)?;
    if !(target.coefficient * record.quantity as f64).is_finite() {
        return Err(AppError::Validation(
            "目标方法系数与原数量的乘积超出范围".into(),
        ));
    }
    let multiplier = snapshot["multiplier"]
        .as_f64()
        .ok_or_else(|| AppError::Validation("原金额倍率异常".into()))?;
    let old_unit_price = original.as_ref().and_then(|value| value.price);
    let before_amount = amount(old_unit_price, record.quantity, multiplier)?;
    let after_amount = amount(target.price, record.quantity, multiplier)?;
    let amount_difference = before_amount
        .zip(after_amount)
        .map(|(before, after)| after - before);
    if amount_difference.is_some_and(|value| !value.is_finite()) {
        return Err(AppError::Validation("预计金额差额超出范围".into()));
    }
    let before_workload = record
        .before_workload
        .ok_or_else(|| AppError::Validation("历史工作量数值异常".into()))?;
    let expires_at = Utc::now() + chrono::Duration::minutes(10);
    let token = encode(
        &Header::default(),
        &AssociationClaims {
            purpose: "work_method_association_v1".into(),
            sub: actor_id,
            exp: expires_at.timestamp() as usize,
            record_id: record.id,
            method_id: request.method_id,
            old_method_id: record.method_id,
            record_fingerprint: hex_sha256(snapshot.to_string().as_bytes()),
            method_fingerprint: method_fingerprint(Some(&target)),
            old_method_fingerprint: method_fingerprint(original.as_ref()),
        },
        &EncodingKey::from_secret(coefficient_confirmation_key().as_bytes()),
    )
    .map_err(|error| AppError::Internal(format!("生成关联确认失败：{error}")))?;
    Ok(AssociationPreview {
        record_id: record.id,
        business_no: record.business_no,
        old_method_id: record.method_id,
        new_method_id: request.method_id,
        old_method_name: record.method_name,
        new_method_name: target.name,
        coefficient_snapshot: record.coefficient_snapshot.unwrap(),
        method_coefficient: target.coefficient,
        quantity: record.quantity,
        before_workload,
        after_workload: before_workload,
        old_unit_price,
        new_unit_price: target.price,
        multiplier,
        before_amount,
        after_amount,
        amount_difference,
        confirmation_token: token,
        expires_at: expires_at.to_rfc3339(),
    })
}

pub(super) async fn association_preview(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(request): Json<AssociationRequest>,
) -> Result<Json<ApiResponse<AssociationPreview>>> {
    tokio::task::spawn_blocking(move || {
        let ctx = authz_service::authenticate(&pool, &headers)?;
        require_coefficient_admin(&ctx)?;
        let conn = pool.get()?;
        Ok(Json(ApiResponse::ok(preview_association(
            &conn,
            &request,
            ctx.user.id,
        )?)))
    })
    .await
    .map_err(|error| AppError::Internal(error.to_string()))?
}

#[derive(Debug, Deserialize)]
pub(super) struct AssociationApply {
    confirmation_token: String,
    reason: String,
}

fn apply_association(
    conn: &mut Connection,
    request: &AssociationApply,
    ctx: &authz_service::AuthContext,
) -> Result<serde_json::Value> {
    require_coefficient_admin(ctx)?;
    let reason = request.reason.trim();
    if reason.is_empty()
        || reason.chars().count() > 500
        || request.confirmation_token.len() > MAX_COEFFICIENT_CONFIRMATION_BYTES
    {
        return Err(AppError::Validation(
            "请填写1至500字的关联修正原因并重新预览".into(),
        ));
    }
    let mut validation = Validation::default();
    validation.leeway = 0;
    let claims = decode::<AssociationClaims>(
        &request.confirmation_token,
        &DecodingKey::from_secret(coefficient_confirmation_key().as_bytes()),
        &validation,
    )
    .map_err(|_| AppError::Conflict("关联确认已过期或无效，请重新预览".into()))?
    .claims;
    if claims.purpose != "work_method_association_v1" || claims.sub != ctx.user.id {
        return Err(AppError::Forbidden("关联确认不属于当前管理员".into()));
    }
    let tx = conn.transaction()?;
    // Method primary-key locks gate new type links; row locks also freeze existing classification and prices.
    let method_ids = [Some(claims.method_id), claims.old_method_id]
        .into_iter()
        .flatten()
        .collect::<BTreeSet<_>>();
    for id in &method_ids {
        let found = tx
            .query_row(
                "SELECT id FROM methods WHERE id=?1 FOR UPDATE",
                [*id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        if *id == claims.method_id && found.is_none() {
            return Err(AppError::Conflict("目标方法已删除，请重新预览".into()));
        }
    }
    let ids = method_ids
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",");
    for lock_sql in [
        format!("SELECT i.id FROM instruments i WHERE EXISTS(SELECT 1 FROM methods m WHERE m.id IN ({ids}) AND m.instrument_id=i.id) ORDER BY i.id FOR SHARE OF i"),
        format!("SELECT ml.method_id FROM method_type_links ml WHERE ml.method_id IN ({ids}) ORDER BY ml.method_id,ml.method_type_id FOR SHARE OF ml"),
        format!("SELECT mt.id FROM method_types mt WHERE EXISTS(SELECT 1 FROM method_type_links ml WHERE ml.method_id IN ({ids}) AND ml.method_type_id=mt.id) ORDER BY mt.id FOR SHARE OF mt"),
    ] {
        let mut locks=tx.prepare(&lock_sql)?;
        locks.query_map([],|row|row.get::<_,i64>(0))?.collect::<postgres_compat::Result<Vec<_>>>()?;
    }
    tx.query_row(
        "SELECT id FROM work_records WHERE id=?1 FOR UPDATE",
        [claims.record_id],
        |row| row.get::<_, i64>(0),
    )
    .optional()?
    .ok_or_else(|| AppError::Conflict("原记录已删除，请重新预览".into()))?;
    let (record, before) = record_for_association(&tx, claims.record_id)?;
    let target = target_method(&tx, claims.method_id)?;
    let original = method_identity(&tx, claims.old_method_id)?;
    record_repo::validate_workload_coefficient(target.coefficient)?;
    if !(target.coefficient * record.quantity as f64).is_finite() {
        return Err(AppError::Validation(
            "目标方法系数与原数量的乘积超出范围".into(),
        ));
    }
    if hex_sha256(before.to_string().as_bytes()) != claims.record_fingerprint
        || method_fingerprint(Some(&target)) != claims.method_fingerprint
        || method_fingerprint(original.as_ref()) != claims.old_method_fingerprint
    {
        return Err(AppError::Conflict(
            "原记录或目标方法已变化，本次未关联，请重新预览".into(),
        ));
    }
    let changed = tx.execute(
        "UPDATE work_records SET method_id=?1,updated_at=datetime('now','localtime') WHERE id=?2 AND deleted_at IS NULL",
        postgres_compat::params![claims.method_id, claims.record_id],
    )?;
    if changed != 1 {
        return Err(AppError::Conflict("原记录已变化，请重新预览".into()));
    }
    let after_json: String = tx.query_row(
        "SELECT to_jsonb(wr)::text FROM work_records wr WHERE wr.id=?1",
        [record.id],
        |row| row.get(0),
    )?;
    let after: serde_json::Value = serde_json::from_str(&after_json)
        .map_err(|_| AppError::Internal("读取关联结果失败".into()))?;
    let detail = format!("历史方法关联修正：{reason}；历史名称、数量、系数、日期、人员及来源快照保留；方法维度和按当前单价计算的金额报表可能改变，系数需另行预览确认");
    audit_repo::log_structured_actor_on_conn(
        &tx,
        "update",
        "work_records",
        Some(record.id),
        ctx.user.id,
        &ctx.user.username,
        &detail,
        "work",
        &record.business_no,
        Some(&before),
        Some(&after),
        "governance_method_association",
    )?;
    trace_repo::log_event_on_conn(
        &tx,
        "work",
        "work_records",
        record.id,
        &record.business_no,
        "update",
        None,
        None,
        &ctx.user.username,
        &detail,
        Some(&before),
        Some(&after),
    )?;
    tx.commit()?;
    Ok(serde_json::json!({"updated_count": 1}))
}

pub(super) async fn association_apply(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(request): Json<AssociationApply>,
) -> Result<Json<ApiResponse<serde_json::Value>>> {
    tokio::task::spawn_blocking(move || {
        let ctx = authz_service::authenticate(&pool, &headers)?;
        require_coefficient_admin(&ctx)?;
        let mut conn = pool.get()?;
        Ok(Json(ApiResponse::ok(apply_association(
            &mut conn, &request, &ctx,
        )?)))
    })
    .await
    .map_err(|error| AppError::Internal(error.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::super::tests::{
        coefficient_query, coefficient_record, coefficient_setup, record_json,
    };
    use super::*;

    fn query() -> DiagnosticQuery {
        let filters = coefficient_query();
        DiagnosticQuery {
            start: filters.start,
            end: filters.end,
            keyword: filters.keyword,
            page: filters.page,
            page_size: filters.page_size,
            status: None,
        }
    }

    #[test]
    fn diagnostic_query_extracts_numeric_http_pagination_without_flattening() {
        let uri = "/api/data-governance/work/coefficient/diagnose?start=2026-07-20&end=2026-07-21&page=2&page_size=50&keyword=water%20analysis&status=method_deleted"
            .parse().unwrap();
        let Query(query) = Query::<DiagnosticQuery>::try_from_uri(&uri).unwrap();
        assert_eq!((query.page, query.page_size), (Some(2), Some(50)));
        assert_eq!(query.keyword.as_deref(), Some("water analysis"));
        assert_eq!(query.status.as_deref(), Some("method_deleted"));
        let invalid = "/api/data-governance/work/coefficient/diagnose?start=2026-07-20&end=2026-07-21&page=unknown"
            .parse().unwrap();
        assert!(Query::<DiagnosticQuery>::try_from_uri(&invalid).is_err());
    }

    #[test]
    fn diagnoses_all_reasons_and_keeps_invalid_history_visible_without_recalculating() {
        let (pool, ctx, project, method) = coefficient_setup();
        let conn = pool.get().unwrap();
        let relative_date_rejected: bool = conn
            .query_row(
                "SELECT labflow_try_timestamptz('today') IS NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(relative_date_rejected);
        let difference =
            coefficient_record(&conn, project, Some(method), 1.0, 2, "2026-07-20 10:00:00");
        let equal =
            coefficient_record(&conn, project, Some(method), 20.0, 2, "2026-07-20 10:00:00");
        let unbound = coefficient_record(&conn, project, None, 20.0, 2, "2026-07-20 10:00:00");
        let custom = coefficient_record(&conn, project, None, 20.0, 2, "2026-07-20 10:00:00");
        conn.execute(
            "UPDATE work_records SET extra_fields=?1 WHERE id=?2",
            postgres_compat::params![r#"{"source_custom_method":true}"#, custom],
        )
        .unwrap();
        let malformed = coefficient_record(&conn, project, Some(method), 20.0, 2, "bad date");
        conn.execute(
            "UPDATE work_records SET extra_fields='{' WHERE id=?1",
            [malformed],
        )
        .unwrap();
        let nonfinite =
            coefficient_record(&conn, project, Some(method), 1.0, 2, "2026-07-20 10:00:00");
        conn.execute(
            "UPDATE work_records SET coefficient_snapshot='NaN'::DOUBLE PRECISION WHERE id=?1",
            [nonfinite],
        )
        .unwrap();
        let auxiliary =
            coefficient_record(&conn, project, Some(method), 1.0, 2, "2026-07-20 10:00:00");
        conn.execute(
            "UPDATE work_records SET instrument_type_snapshot='辅助工作' WHERE id=?1",
            [auxiliary],
        )
        .unwrap();
        let deleted =
            coefficient_record(&conn, project, Some(method), 1.0, 2, "2026-07-20 10:00:00");
        conn.execute(
            "UPDATE work_records SET deleted_at='2026-07-21' WHERE id=?1",
            [deleted],
        )
        .unwrap();
        let other: i64 = conn
            .query_row("SELECT MAX(id) FROM methods", [], |row| row.get(0))
            .unwrap();
        let lost = coefficient_record(&conn, project, Some(other), 20.0, 2, "2026-07-20 10:00:00");
        conn.execute(
            "UPDATE methods SET deleted_at='2026-07-21' WHERE id=?1",
            [other],
        )
        .unwrap();
        conn.execute("UPDATE methods SET is_active=0 WHERE id=?1", [method])
            .unwrap();
        conn.execute(
            "INSERT INTO auxiliary_work_items(name,coefficient,scope_type) VALUES('独立维护',3,'public')",[]
        ).unwrap();
        let item = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO auxiliary_work_records(auxiliary_work_item_id,user_name_snapshot,
            auxiliary_work_name_snapshot,coefficient_snapshot,quantity,recorded_at)
            VALUES(?1,'历史人员','独立维护',3,4,'2026-07-20 10:00:00')",
            [item],
        )
        .unwrap();
        let before = record_json(&conn, unbound);
        let result = diagnose_records(&conn, &query()).unwrap();
        assert_eq!(result.total, 10);
        let states = result
            .rows
            .iter()
            .filter(|row| row.origin == "work_record")
            .map(|row| (row.id, row.status.as_str()))
            .collect::<BTreeMap<_, _>>();
        for (id, status) in [
            (difference, "difference"),
            (equal, "equal"),
            (unbound, "unbound"),
            (custom, "custom"),
            (malformed, "invalid"),
            (nonfinite, "invalid"),
            (auxiliary, "auxiliary"),
            (deleted, "record_deleted"),
            (lost, "method_deleted"),
        ] {
            assert_eq!(states[&id], status);
        }
        assert_eq!(
            result
                .rows
                .iter()
                .find(|row| row.id == difference && row.origin == "work_record")
                .unwrap()
                .method_active,
            Some(false)
        );
        assert_eq!(
            result
                .counts
                .iter()
                .find(|row| row.status == "auxiliary")
                .unwrap()
                .count,
            2
        );
        assert_eq!(
            result.rows.iter().filter(|row| row.can_reassociate).count(),
            4
        );
        assert!(serde_json::to_value(&result).is_ok());
        assert_eq!(record_json(&conn, unbound), before);
        assert!(matches!(
            preview_coefficients(&conn, &coefficient_query(), ctx.user.id),
            Err(AppError::Validation(_))
        ));
        conn.execute(
            "UPDATE work_records SET deleted_at='2026-07-21' WHERE id=?1",
            [nonfinite],
        )
        .unwrap();
        let correction = preview_coefficients(&conn, &coefficient_query(), ctx.user.id).unwrap();
        assert_eq!(correction.rows.len(), 1);
        assert_eq!(correction.rows[0].id, difference);
        let mut filtered = query();
        filtered.status = Some("unbound".into());
        filtered.page_size = Some(1);
        assert_eq!(
            diagnose_records(&conn, &filtered).unwrap().rows[0].id,
            unbound
        );
        filtered.page_size = Some(501);
        assert!(diagnose_records(&conn, &filtered).is_err());
        filtered.page_size = Some(1);
        filtered.status = Some("invalid-code".into());
        assert!(diagnose_records(&conn, &filtered).is_err());
    }

    #[test]
    fn reassociation_preserves_all_snapshots_and_requires_separate_coefficient_confirmation() {
        let (pool, ctx, project, method) = coefficient_setup();
        let mut conn = pool.get().unwrap();
        let id = coefficient_record(&conn, project, None, 20.0, 3, "2026-07-20 10:00:00");
        let before = record_json(&conn, id);
        let preview = preview_association(
            &conn,
            &AssociationRequest {
                record_id: id,
                method_id: method,
            },
            ctx.user.id,
        )
        .unwrap();
        assert_eq!(preview.before_workload, 60.0);
        assert_eq!(preview.before_workload, preview.after_workload);
        let result = apply_association(
            &mut conn,
            &AssociationApply {
                confirmation_token: preview.confirmation_token,
                reason: "核实原始单据后明确方法".into(),
            },
            &ctx,
        )
        .unwrap();
        assert_eq!(result["updated_count"], 1);
        let mut after = record_json(&conn, id);
        assert_eq!(after["method_id"], method);
        after["method_id"] = before["method_id"].clone();
        after["updated_at"] = before["updated_at"].clone();
        assert_eq!(before, after);
        conn.execute("UPDATE methods SET coefficient=28 WHERE id=?1", [method])
            .unwrap();
        let corrections = preview_coefficients(&conn, &coefficient_query(), ctx.user.id).unwrap();
        assert_eq!(corrections.total, 1);
        assert_eq!(corrections.rows[0].coefficient_snapshot, 20.0);
        assert_eq!(corrections.rows[0].method_coefficient, 28.0);
        let audits: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM audit_log
            WHERE source='governance_method_association' AND record_id=?1",
                [id],
                |row| row.get(0),
            )
            .unwrap();
        let traces: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM record_events WHERE table_name='work_records'
            AND record_id=?1 AND reason LIKE '历史方法关联修正%'",
                [id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(traces, 1);
        assert_eq!(audits, 1);
    }

    #[test]
    fn reassociation_rejects_changed_inputs_wrong_admin_expiry_and_rolls_back_failed_audit() {
        let (pool, ctx, project, method) = coefficient_setup();
        let mut conn = pool.get().unwrap();
        let id = coefficient_record(&conn, project, None, 20.0, 3, "2026-07-20 10:00:00");
        let preview = preview_association(
            &conn,
            &AssociationRequest {
                record_id: id,
                method_id: method,
            },
            ctx.user.id,
        )
        .unwrap();
        let request = AssociationApply {
            confirmation_token: preview.confirmation_token.clone(),
            reason: "复核".into(),
        };
        let mut other = ctx.clone();
        other.user.id += 999;
        assert!(matches!(
            apply_association(&mut conn, &request, &other),
            Err(AppError::Forbidden(_))
        ));
        conn.execute("UPDATE methods SET coefficient=28 WHERE id=?1", [method])
            .unwrap();
        assert!(matches!(
            apply_association(&mut conn, &request, &ctx),
            Err(AppError::Conflict(_))
        ));
        assert_eq!(record_json(&conn, id)["method_id"], serde_json::Value::Null);
        let fresh = preview_association(
            &conn,
            &AssociationRequest {
                record_id: id,
                method_id: method,
            },
            ctx.user.id,
        )
        .unwrap();
        let mut claims = decode::<AssociationClaims>(
            &fresh.confirmation_token,
            &DecodingKey::from_secret(coefficient_confirmation_key().as_bytes()),
            &Validation::default(),
        )
        .unwrap()
        .claims;
        claims.exp = (Utc::now() - chrono::Duration::minutes(1)).timestamp() as usize;
        let expired = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(coefficient_confirmation_key().as_bytes()),
        )
        .unwrap();
        assert!(apply_association(
            &mut conn,
            &AssociationApply {
                confirmation_token: expired,
                reason: "复核".into()
            },
            &ctx
        )
        .is_err());
        let before = record_json(&conn, id);
        conn.execute_batch(
            "ALTER TABLE audit_log ADD CONSTRAINT reject_association_audit
             CHECK(source<>'governance_method_association')",
        )
        .unwrap();
        assert!(apply_association(
            &mut conn,
            &AssociationApply {
                confirmation_token: fresh.confirmation_token,
                reason: "复核".into()
            },
            &ctx
        )
        .is_err());
        assert_eq!(record_json(&conn, id), before);
        conn.execute(
            "UPDATE work_records SET extra_fields=?1 WHERE id=?2",
            postgres_compat::params![r#"{"source_custom_method":true}"#, id],
        )
        .unwrap();
        assert!(preview_association(
            &conn,
            &AssociationRequest {
                record_id: id,
                method_id: method
            },
            ctx.user.id
        )
        .is_err());
    }

    #[test]
    fn diagnostic_and_correction_use_identical_timezone_day_boundaries() {
        let (pool, ctx, project, method) = coefficient_setup();
        let conn = pool.get().unwrap();
        conn.execute_batch("SET TIME ZONE 'Asia/Shanghai';")
            .unwrap();
        let first =
            coefficient_record(&conn, project, Some(method), 1.0, 1, "2026-07-19T17:30:00Z");
        let last = coefficient_record(
            &conn,
            project,
            Some(method),
            1.0,
            1,
            "2026-07-21T00:30:00+09:00",
        );
        coefficient_record(&conn, project, Some(method), 1.0, 1, "2026-07-20T23:30:00Z");
        let diagnosis = diagnose_records(&conn, &query()).unwrap();
        let preview = preview_coefficients(&conn, &coefficient_query(), ctx.user.id).unwrap();
        assert_eq!(
            diagnosis.rows.iter().map(|row| row.id).collect::<Vec<_>>(),
            vec![last, first]
        );
        assert_eq!(
            preview.rows.iter().map(|row| row.id).collect::<Vec<_>>(),
            vec![last, first]
        );
    }

    #[test]
    fn reassociation_previews_price_impact_and_rejects_changed_original_prices() {
        let (pool, ctx, project, method) = coefficient_setup();
        let mut conn = pool.get().unwrap();
        let target: i64 = conn
            .query_row("SELECT MAX(id) FROM methods", [], |row| row.get(0))
            .unwrap();
        conn.execute("UPDATE methods SET amount=12 WHERE id=?1", [method])
            .unwrap();
        conn.execute("UPDATE methods SET amount=25 WHERE id=?1", [target])
            .unwrap();
        let id = coefficient_record(&conn, project, Some(method), 20.0, 3, "2026-07-20 10:00:00");
        let before = record_json(&conn, id);
        let preview = preview_association(
            &conn,
            &AssociationRequest {
                record_id: id,
                method_id: target,
            },
            ctx.user.id,
        )
        .unwrap();
        assert_eq!(preview.old_unit_price, Some(12.0));
        assert_eq!(preview.new_unit_price, Some(25.0));
        assert_eq!(preview.before_amount, Some(126.0));
        assert_eq!(preview.after_amount, Some(262.5));
        assert_eq!(preview.amount_difference, Some(136.5));
        assert_eq!(preview.before_workload, preview.after_workload);
        conn.execute("UPDATE methods SET amount=13 WHERE id=?1", [method])
            .unwrap();
        assert!(matches!(
            apply_association(
                &mut conn,
                &AssociationApply {
                    confirmation_token: preview.confirmation_token,
                    reason: "明确实际方法".into()
                },
                &ctx
            ),
            Err(AppError::Conflict(_))
        ));
        assert_eq!(record_json(&conn, id), before);
    }
}
