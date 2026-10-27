use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::{ApiResponse, PaginatedResponse};
use crate::repo::{audit_repo, division_repo, settings_repo, user_repo};
use crate::service::authz_service::{self, AuthContext};
use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use postgres_compat::{params, Connection, OptionalExtension};
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, Serialize)]
struct NamedOption {
    id: i64,
    name: String,
}
#[derive(Debug, Clone, Serialize)]
struct PersonOption {
    id: i64,
    username: String,
}
#[derive(Serialize)]
struct Context {
    people: Vec<PersonOption>,
    divisions: Vec<NamedOption>,
    groups: Vec<NamedOption>,
    areas: Vec<NamedOption>,
    can_create: bool,
    can_manage_items: bool,
    can_manage_records: bool,
    can_edit_own: bool,
}
#[derive(Debug, Clone, Serialize)]
struct Item {
    id: i64,
    name: String,
    coefficient: f64,
    category: String,
    area_type_id: Option<i64>,
    area_name: String,
    scope_type: String,
    group_id: Option<i64>,
    group_name: String,
    enabled: bool,
    sort_order: i64,
    notes: String,
    updated_at: String,
    deleted_at: Option<String>,
}
#[derive(Deserialize)]
struct ItemWrite {
    name: String,
    coefficient: f64,
    #[serde(default)]
    category: String,
    #[serde(default, deserialize_with = "deserialize_area_update")]
    area_type_id: Option<Option<i64>>,
    enabled: bool,
    #[serde(default)]
    sort_order: i64,
    #[serde(default)]
    notes: String,
    expected_updated_at: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
struct AuxiliaryRecord {
    id: i64,
    origin: &'static str,
    auxiliary_work_item_id: Option<i64>,
    user_id: Option<i64>,
    user_name: String,
    auxiliary_work_name: String,
    coefficient: f64,
    category: String,
    area_type_id: Option<i64>,
    area_name: String,
    quantity: i64,
    recorded_at: String,
    notes: String,
    status: String,
    detection_division_id: Option<i64>,
    detection_division_name: String,
    group_id: Option<i64>,
    group_name: String,
    created_by: Option<i64>,
    created_by_name: String,
    updated_at: String,
    deleted_at: Option<String>,
    linked_work_record_id: Option<i64>,
    can_edit: bool,
    can_delete: bool,
    can_restore: bool,
}
#[derive(Deserialize)]
struct RecordCreate {
    auxiliary_work_item_id: i64,
    expected_item_updated_at: Option<String>,
    user_id: Option<i64>,
    detection_division_id: i64,
    group_id: Option<i64>,
    quantity: i64,
    recorded_at: String,
    #[serde(default)]
    notes: String,
}
#[derive(Deserialize)]
struct RecordUpdate {
    auxiliary_work_item_id: Option<i64>,
    expected_item_updated_at: Option<String>,
    quantity: i64,
    recorded_at: String,
    #[serde(default)]
    notes: String,
    expected_updated_at: String,
}
#[derive(Deserialize)]
struct Maintenance {
    reason: String,
    expected_updated_at: String,
}
#[derive(Default, Deserialize)]
struct ListQuery {
    start: Option<String>,
    end: Option<String>,
    user_id: Option<i64>,
    item_id: Option<i64>,
    area_type_id: Option<i64>,
    category: Option<String>,
    keyword: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
    include_deleted: Option<bool>,
}
#[derive(Default, Deserialize)]
struct ItemQuery {
    include_deleted: Option<bool>,
}

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/auxiliary/context", get(context))
        .route("/api/auxiliary/items", get(list_items).post(create_item))
        .route(
            "/api/auxiliary/items/:id",
            axum::routing::put(update_item).delete(delete_item),
        )
        .route("/api/auxiliary/items/:id/restore", post(restore_item))
        .route(
            "/api/auxiliary/records",
            get(list_records).post(create_record),
        )
        .route(
            "/api/auxiliary/records/:id",
            axum::routing::put(update_record).delete(delete_record),
        )
        .route("/api/auxiliary/records/:id/restore", post(restore_record))
        .with_state(pool)
}

fn require_entry(ctx: &AuthContext) -> Result<()> {
    authz_service::require_permission(ctx, "entry:auxiliary")?;
    if ctx.is_analysis_public_account() {
        authz_service::require_permission(ctx, "records:work:portal-scoped")?;
    }
    Ok(())
}
fn manages_items(ctx: &AuthContext) -> bool {
    ctx.user.is_admin && ctx.has_permission("manage:auxiliary")
}
fn require_item_manager(ctx: &AuthContext) -> Result<()> {
    if manages_items(ctx) {
        Ok(())
    } else {
        Err(AppError::Forbidden("仅管理员可维护辅助工作事项".into()))
    }
}
// Missing preserves an existing assignment; explicit null clears it for updated clients.
fn deserialize_area_update<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<Option<i64>>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<i64>::deserialize(deserializer).map(Some)
}
fn validate_text(value: &str, limit: usize, label: &str, required: bool) -> Result<String> {
    let text = value.trim();
    if (required && text.is_empty()) || text.chars().count() > limit || text.contains('\0') {
        return Err(AppError::Validation(format!(
            "{label}不能为空或超过{limit}个字符"
        )));
    }
    Ok(text.to_owned())
}
fn validate_item(body: &ItemWrite) -> Result<()> {
    validate_text(&body.name, 120, "事项名称", true)?;
    validate_text(&body.category, 64, "辅助分类", false)?;
    validate_text(&body.notes, 4000, "备注", false)?;
    if !body.coefficient.is_finite() || body.coefficient < 0.0 || body.coefficient > 1e9 {
        return Err(AppError::Validation("系数须为0至10亿的有限数字".into()));
    }
    if body.category.chars().any(char::is_control) {
        return Err(AppError::Validation("辅助分类不能包含控制字符".into()));
    }
    if body.area_type_id.flatten().is_some_and(|id| id <= 0) {
        return Err(AppError::Validation("辅助区域标识无效".into()));
    }
    Ok(())
}
fn record_values(quantity: i64, recorded_at: &str, notes: &str) -> Result<(String, String)> {
    if quantity <= 0 || quantity > 1_000_000_000 {
        return Err(AppError::Validation("数量须为1至10亿的整数".into()));
    }
    let native = chrono::DateTime::parse_from_rfc3339(recorded_at.trim())
        .ok()
        .map(|date| {
            date.with_timezone(&chrono::FixedOffset::east_opt(8 * 60 * 60).expect("Beijing offset"))
                .format("%Y-%m-%dT%H:%M:%S")
                .to_string()
        });
    let date = crate::service::record_service::sample_workload_recorded_at(Some(
        native.as_deref().unwrap_or(recorded_at),
    ))?;
    Ok((date, validate_text(notes, 4000, "备注", false)?))
}
fn check_version(actual: &str, expected: Option<&str>) -> Result<()> {
    if expected != Some(actual) {
        return Err(AppError::Conflict("记录已变化，请刷新后重新操作".into()));
    }
    Ok(())
}
pub(super) fn lock_categories(conn: &Connection) -> Result<()> {
    conn.execute("SELECT pg_advisory_xact_lock(2368,1)", [])?;
    Ok(())
}
fn save_category(conn: &Connection, id: i64, category: &str) -> Result<()> {
    let mut categories = settings_repo::workload_auxiliary_categories_on_conn(conn)?;
    let key = format!("item:{id}");
    if category.trim().is_empty() {
        categories.remove(&key);
    } else {
        categories.insert(key, category.trim().to_owned());
    }
    settings_repo::upsert_on_conn(
        conn,
        settings_repo::WORKLOAD_AUXILIARY_CATEGORIES_KEY,
        &serde_json::to_string(&categories).map_err(|e| AppError::Internal(e.to_string()))?,
    )
}
fn read_item(conn: &Connection, id: i64, lock: bool) -> Result<Item> {
    if lock {
        conn.query_row(
            "SELECT id FROM auxiliary_work_items WHERE id=?1 FOR UPDATE",
            [id],
            |r| r.get::<_, i64>(0),
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound("辅助事项不存在".into()))?;
    }
    let mut item = conn.query_row(
        "SELECT ai.id,ai.name,ai.coefficient,ai.scope_type,ai.group_id,COALESCE(g.name,''),ai.enabled,ai.sort_order,ai.notes,ai.updated_at::text,ai.deleted_at::text,ai.area_type_id,COALESCE(mt.name,'')
         FROM auxiliary_work_items ai LEFT JOIN project_groups g ON g.id=ai.group_id LEFT JOIN method_types mt ON mt.id=ai.area_type_id WHERE ai.id=?1", [id],
        |r| Ok(Item { id:r.get(0)?,name:r.get(1)?,coefficient:r.get(2)?,category:String::new(),scope_type:r.get(3)?,group_id:r.get(4)?,group_name:r.get(5)?,enabled:r.get(6)?,sort_order:r.get(7)?,notes:r.get(8)?,updated_at:r.get(9)?,deleted_at:r.get(10)?,area_type_id:r.get(11)?,area_name:r.get(12)? }))
        .optional()?.ok_or_else(|| AppError::NotFound("辅助事项不存在".into()))?;
    item.category = settings_repo::workload_auxiliary_categories_on_conn(conn)?
        .get(&format!("item:{id}"))
        .cloned()
        .unwrap_or_default();
    Ok(item)
}
fn audit_item(
    conn: &Connection,
    ctx: &AuthContext,
    action: &str,
    before: Option<&Item>,
    after: &Item,
    reason: &str,
) -> Result<()> {
    audit_repo::log_structured_actor_on_conn(
        conn,
        action,
        "auxiliary_work_items",
        Some(after.id),
        ctx.user.id,
        &ctx.user.username,
        reason,
        "work",
        &format!("aux-item:{}", after.id),
        before
            .map(serde_json::to_value)
            .transpose()
            .map_err(|e| AppError::Internal(e.to_string()))?
            .as_ref(),
        Some(&serde_json::to_value(after).map_err(|e| AppError::Internal(e.to_string()))?),
        "auxiliary",
    )
}
fn allowed_divisions(pool: &DbPool, ctx: &AuthContext) -> Result<Vec<NamedOption>> {
    let allowed = authz_service::work_allowed_division_ids(pool, ctx)?;
    Ok(division_repo::list_for_portal(pool, Some("work"))?
        .into_iter()
        .filter(|d| d.is_active && allowed.as_ref().is_none_or(|ids| ids.contains(&d.id)))
        .map(|d| NamedOption {
            id: d.id,
            name: d.name,
        })
        .collect())
}
fn available_people(pool: &DbPool, ctx: &AuthContext) -> Result<Vec<PersonOption>> {
    let own = || {
        vec![PersonOption {
            id: ctx.user.id,
            username: ctx.user.username.clone(),
        }]
    };
    if ctx.is_analysis_public_account() {
        if !ctx.has_permission("records:work:select-detector") {
            return Ok(vec![]);
        }
        return Ok(user_repo::analysis_public_account_scope(pool, ctx.user.id)?
            .accounts
            .into_iter()
            .map(|u| PersonOption {
                id: u.id,
                username: u.username,
            })
            .collect());
    }
    if !ctx.has_permission("records:work:select-detector") {
        return Ok(own());
    }
    let actor_divisions = allowed_divisions(pool, ctx)?;
    let mut people = own();
    let mut candidates = user_repo::list_work_detectors(pool)?;
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT u.id FROM users u WHERE u.is_active=1 AND u.deleted_at IS NULL AND u.is_admin=0
         AND EXISTS(SELECT 1 FROM user_roles ur JOIN role_permissions rp ON rp.role_id=ur.role_id WHERE ur.user_id=u.id AND rp.permission_key='entry:auxiliary')
         AND NOT EXISTS(SELECT 1 FROM user_roles ur JOIN roles r ON r.id=ur.role_id WHERE ur.user_id=u.id AND r.system_key IN ('public_account','rd_public_account','analysis_public_account','system_admin')) ORDER BY u.username,u.id",
    )?;
    let ids = stmt
        .query_map([], |r| r.get::<_, i64>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(stmt);
    drop(conn);
    for id in ids {
        if !candidates.iter().any(|u| u.id == id) {
            if let Some(user) = user_repo::find_by_id(pool, id)? {
                candidates.push(user);
            }
        }
    }
    for user in candidates {
        let person_ctx = AuthContext {
            role_names: user.role_names.clone(),
            user,
        };
        let allowed = authz_service::work_allowed_division_ids(pool, &person_ctx)?;
        if ctx.user.is_admin
            || allowed
                .as_ref()
                .is_some_and(|ids| actor_divisions.iter().any(|d| ids.contains(&d.id)))
        {
            if !people.iter().any(|p| p.id == person_ctx.user.id) {
                people.push(PersonOption {
                    id: person_ctx.user.id,
                    username: person_ctx.user.username,
                });
            }
        }
    }
    Ok(people)
}
fn context_for(pool: &DbPool, ctx: &AuthContext) -> Result<Context> {
    require_entry(ctx)?;
    let divisions = allowed_divisions(pool, ctx)?;
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT id,name,division_id FROM project_groups WHERE deleted_at IS NULL ORDER BY name,id",
    )?;
    let groups = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<i64>>(2)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|g| g.2.is_some_and(|id| divisions.iter().any(|d| d.id == id)))
        .map(|(id, name, _)| NamedOption { id, name })
        .collect();
    drop(stmt);
    let mut areas_stmt = conn.prepare(
        "SELECT id,name FROM method_types WHERE deleted_at IS NULL ORDER BY sort_order,id",
    )?;
    let areas = areas_stmt
        .query_map([], |r| {
            Ok(NamedOption {
                id: r.get(0)?,
                name: r.get(1)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(areas_stmt);
    drop(conn);
    Ok(Context {
        people: available_people(pool, ctx)?,
        divisions,
        groups,
        areas,
        can_create: ctx.has_permission("entry:auxiliary"),
        can_manage_items: manages_items(ctx),
        can_manage_records: ctx.has_permission("records:auxiliary:manage"),
        can_edit_own: ctx.has_permission("records:auxiliary:edit-own"),
    })
}
async fn context(
    State(pool): State<DbPool>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Context>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(context_for(&pool, &ctx)?)))
}
fn items_for(pool: &DbPool, ctx: &AuthContext, include_deleted: bool) -> Result<Vec<Item>> {
    require_entry(ctx)?;
    if include_deleted {
        require_item_manager(ctx)?;
    }
    let groups = authz_service::work_allowed_group_ids(pool, ctx)?;
    let conn = pool.get()?;
    let mut stmt=conn.prepare("SELECT id FROM auxiliary_work_items WHERE (?1 OR deleted_at IS NULL) ORDER BY sort_order,name,id")?;
    let ids = stmt
        .query_map([include_deleted], |r| r.get::<_, i64>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut items = vec![];
    for id in ids {
        let item = read_item(&conn, id, false)?;
        if manages_items(ctx)
            || item.scope_type == "public" && item.group_id.is_none()
            || item.scope_type == "laboratory"
                && item
                    .group_id
                    .is_some_and(|id| groups.as_ref().is_none_or(|gs| gs.contains(&id)))
        {
            items.push(item);
        }
    }
    Ok(items)
}
async fn list_items(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<ItemQuery>,
) -> Result<Json<ApiResponse<Vec<Item>>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(items_for(
        &pool,
        &ctx,
        q.include_deleted.unwrap_or(false),
    )?)))
}
fn write_item(pool: &DbPool, ctx: &AuthContext, id: Option<i64>, body: &ItemWrite) -> Result<Item> {
    require_item_manager(ctx)?;
    validate_item(body)?;
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    lock_categories(&tx)?;
    let before = id.map(|id| read_item(&tx, id, true)).transpose()?;
    if let Some(item) = &before {
        check_version(&item.updated_at, body.expected_updated_at.as_deref())?;
        if item.deleted_at.is_some() {
            return Err(AppError::Validation("请先恢复已删除事项".into()));
        }
    }
    let duplicate:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM auxiliary_work_items WHERE name=?1 AND COALESCE(group_id,0)=?2 AND deleted_at IS NULL AND id<>?3)",params![body.name.trim(),before.as_ref().and_then(|i|i.group_id).unwrap_or(0),id.unwrap_or(0)],|r|r.get(0))?;
    if duplicate {
        return Err(AppError::Validation("同一范围内辅助事项名称已存在".into()));
    }
    let area_type_id = body
        .area_type_id
        .unwrap_or_else(|| before.as_ref().and_then(|item| item.area_type_id));
    if area_type_id.is_some() && area_type_id != before.as_ref().and_then(|item| item.area_type_id)
    {
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM method_types WHERE id=?1 AND deleted_at IS NULL)",
            [area_type_id.unwrap()],
            |r| r.get(0),
        )?;
        if !exists {
            return Err(AppError::Validation(
                "辅助区域不存在或已删除，请选择现有检测类型".into(),
            ));
        }
    }
    let id = if let Some(id) = id {
        tx.execute("UPDATE auxiliary_work_items SET name=?1,coefficient=?2,enabled=?3,sort_order=?4,notes=?5,area_type_id=?6,updated_at=GREATEST(clock_timestamp(),updated_at+interval '1 microsecond') WHERE id=?7",params![body.name.trim(),body.coefficient,body.enabled,body.sort_order,body.notes.trim(),area_type_id,id])?;
        id
    } else {
        tx.query_row("INSERT INTO auxiliary_work_items(name,coefficient,scope_type,enabled,sort_order,notes,created_by,area_type_id) VALUES(?1,?2,'public',?3,?4,?5,?6,?7) RETURNING id",params![body.name.trim(),body.coefficient,body.enabled,body.sort_order,body.notes.trim(),ctx.user.id,area_type_id],|r|r.get(0))?
    };
    save_category(&tx, id, &body.category)?;
    let after = read_item(&tx, id, false)?;
    audit_item(
        &tx,
        ctx,
        if before.is_some() { "update" } else { "create" },
        before.as_ref(),
        &after,
        "维护辅助事项（历史快照不重算）",
    )?;
    tx.commit()?;
    Ok(after)
}
async fn create_item(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(body): Json<ItemWrite>,
) -> Result<Json<ApiResponse<Item>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(write_item(&pool, &ctx, None, &body)?)))
}
async fn update_item(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<ItemWrite>,
) -> Result<Json<ApiResponse<Item>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(write_item(
        &pool,
        &ctx,
        Some(id),
        &body,
    )?)))
}
fn item_deleted(
    pool: &DbPool,
    ctx: &AuthContext,
    id: i64,
    body: &Maintenance,
    restore: bool,
) -> Result<Item> {
    require_item_manager(ctx)?;
    let reason = validate_text(&body.reason, 500, "操作原因", true)?;
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    lock_categories(&tx)?;
    let before = read_item(&tx, id, true)?;
    check_version(&before.updated_at, Some(&body.expected_updated_at))?;
    if restore != before.deleted_at.is_some() {
        return Err(AppError::Validation("事项状态已变化，请刷新".into()));
    }
    if restore {
        let duplicate:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM auxiliary_work_items WHERE name=?1 AND COALESCE(group_id,0)=?2 AND deleted_at IS NULL AND id<>?3)",params![&before.name,before.group_id.unwrap_or(0),id],|r|r.get(0))?;
        if duplicate {
            return Err(AppError::Validation("已有同名事项，请先调整名称".into()));
        }
    }
    tx.execute("UPDATE auxiliary_work_items SET deleted_at=CASE WHEN ?1 THEN NULL ELSE clock_timestamp() END,updated_at=GREATEST(clock_timestamp(),updated_at+interval '1 microsecond') WHERE id=?2",params![restore,id])?;
    let after = read_item(&tx, id, false)?;
    audit_item(
        &tx,
        ctx,
        if restore { "restore" } else { "delete" },
        Some(&before),
        &after,
        &reason,
    )?;
    tx.commit()?;
    Ok(after)
}
async fn delete_item(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<Maintenance>,
) -> Result<Json<ApiResponse<Item>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(item_deleted(
        &pool, &ctx, id, &body, false,
    )?)))
}
async fn restore_item(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<Maintenance>,
) -> Result<Json<ApiResponse<Item>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(item_deleted(
        &pool, &ctx, id, &body, true,
    )?)))
}

const RECORD_SELECT:&str="SELECT ar.id,ar.auxiliary_work_item_id,ar.user_id,ar.user_name_snapshot,ar.auxiliary_work_name_snapshot,ar.coefficient_snapshot,ar.auxiliary_type_snapshot,ar.quantity,to_char(ar.recorded_at AT TIME ZONE 'Asia/Shanghai','YYYY-MM-DD\"T\"HH24:MI:SS'),ar.notes,ar.status,ar.detection_division_id,ar.detection_division_name_snapshot,ar.group_id,ar.group_name_snapshot,ar.created_by,COALESCE(u.username,''),ar.updated_at::text,ar.deleted_at::text,(SELECT min(w.id) FROM work_records w WHERE w.source_type='auxiliary_work' AND w.source_record_id=ar.id),ar.area_type_id,ar.area_name_snapshot FROM auxiliary_work_records ar LEFT JOIN users u ON u.id=ar.created_by";
fn map_record(r: &postgres_compat::Row<'_>) -> postgres_compat::Result<AuxiliaryRecord> {
    Ok(AuxiliaryRecord {
        id: r.get(0)?,
        origin: "auxiliary_record",
        auxiliary_work_item_id: r.get(1)?,
        user_id: r.get(2)?,
        user_name: r.get(3)?,
        auxiliary_work_name: r.get(4)?,
        coefficient: r.get(5)?,
        category: r.get(6)?,
        area_type_id: r.get(20)?,
        area_name: r.get(21)?,
        quantity: r.get(7)?,
        recorded_at: r.get(8)?,
        notes: r.get(9)?,
        status: r.get(10)?,
        detection_division_id: r.get(11)?,
        detection_division_name: r.get(12)?,
        group_id: r.get(13)?,
        group_name: r.get(14)?,
        created_by: r.get(15)?,
        created_by_name: r.get(16)?,
        updated_at: r.get(17)?,
        deleted_at: r.get(18)?,
        linked_work_record_id: r.get(19)?,
        can_edit: false,
        can_delete: false,
        can_restore: false,
    })
}
fn read_record(conn: &Connection, id: i64, lock: bool) -> Result<AuxiliaryRecord> {
    if lock {
        conn.query_row(
            "SELECT id FROM auxiliary_work_records WHERE id=?1 FOR UPDATE",
            [id],
            |r| r.get::<_, i64>(0),
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound("辅助工作记录不存在".into()))?;
    }
    conn.query_row(&format!("{RECORD_SELECT} WHERE ar.id=?1"), [id], map_record)
        .optional()?
        .ok_or_else(|| AppError::NotFound("辅助工作记录不存在".into()))
}
fn department_allowed(
    record: &AuxiliaryRecord,
    ctx: &AuthContext,
    divisions: Option<&[i64]>,
) -> bool {
    ctx.user.is_admin
        || match record.detection_division_id {
            Some(id) => divisions.is_none_or(|ids| ids.contains(&id)),
            None => record.user_id == Some(ctx.user.id),
        }
}
fn record_visible(record: &AuxiliaryRecord, ctx: &AuthContext, divisions: Option<&[i64]>) -> bool {
    department_allowed(record, ctx, divisions)
        && ((ctx.can_view_workload_scope() || ctx.has_permission("records:auxiliary:manage"))
            || record.user_id == Some(ctx.user.id)
            || ctx.is_analysis_public_account() && record.created_by == Some(ctx.user.id))
}
fn can_mutate(record: &AuxiliaryRecord, ctx: &AuthContext, divisions: Option<&[i64]>) -> bool {
    department_allowed(record, ctx, divisions)
        && record.linked_work_record_id.is_none()
        && (ctx.user.is_admin
            || ctx.has_permission("records:auxiliary:manage")
            || ctx.has_permission("records:auxiliary:edit-own")
                && (record.user_id == Some(ctx.user.id)
                    || ctx.is_analysis_public_account() && record.created_by == Some(ctx.user.id)))
}
fn apply_actions(record: &mut AuxiliaryRecord, ctx: &AuthContext, divisions: Option<&[i64]>) {
    let allowed = can_mutate(record, ctx, divisions);
    record.can_edit = allowed && record.deleted_at.is_none() && record.status == "有效";
    record.can_delete = allowed && record.deleted_at.is_none();
    record.can_restore = allowed && record.deleted_at.is_some();
}
fn records_for(
    pool: &DbPool,
    ctx: &AuthContext,
    q: &ListQuery,
) -> Result<PaginatedResponse<AuxiliaryRecord>> {
    require_entry(ctx)?;
    if q.user_id.is_some_and(|id| id <= 0)
        || q.item_id.is_some_and(|id| id <= 0)
        || q.area_type_id.is_some_and(|id| id == 0 || id < -1)
    {
        return Err(AppError::Validation("筛选标识无效".into()));
    }
    let allowed = authz_service::work_allowed_division_ids(pool, ctx)?;
    let mut where_sql = String::from(" WHERE TRUE");
    if !q.include_deleted.unwrap_or(false) {
        where_sql.push_str(" AND ar.deleted_at IS NULL");
    }
    if !ctx.user.is_admin {
        let ids = allowed
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(i64::to_string)
            .collect::<Vec<_>>()
            .join(",");
        where_sql.push_str(&format!(
            " AND ({} OR (ar.detection_division_id IS NULL AND ar.user_id={}))",
            if ids.is_empty() {
                "FALSE".into()
            } else {
                format!("ar.detection_division_id IN ({ids})")
            },
            ctx.user.id
        ));
        if !(ctx.can_view_workload_scope() || ctx.has_permission("records:auxiliary:manage")) {
            if ctx.is_analysis_public_account() {
                where_sql.push_str(&format!(
                    " AND (ar.user_id={} OR ar.created_by={})",
                    ctx.user.id, ctx.user.id
                ));
            } else {
                where_sql.push_str(&format!(" AND ar.user_id={}", ctx.user.id));
            }
        }
    }
    let (start, end) =
        crate::repo::record_repo::work_record_date_bounds(q.start.as_deref(), q.end.as_deref())?;
    let keyword = validate_text(q.keyword.as_deref().unwrap_or(""), 200, "关键词", false)?;
    let mut parameters: Vec<Box<dyn postgres_compat::types::ToSql>> = vec![];
    for (condition, value) in [
        (
            "ar.recorded_at>=CAST(CAST(? AS TEXT) AS timestamptz)",
            start,
        ),
        ("ar.recorded_at<=CAST(CAST(? AS TEXT) AS timestamptz)", end),
    ] {
        if let Some(value) = value {
            parameters.push(Box::new(value));
            where_sql.push_str(&format!(
                " AND {}",
                condition.replace('?', &format!("?{}", parameters.len()))
            ));
        }
    }
    for (field, value) in [
        ("ar.user_id", q.user_id),
        ("ar.auxiliary_work_item_id", q.item_id),
    ] {
        if let Some(value) = value {
            parameters.push(Box::new(value));
            where_sql.push_str(&format!(" AND {field}=?{}", parameters.len()));
        }
    }
    if let Some(area) = q.area_type_id {
        if area == -1 {
            where_sql.push_str(" AND ar.area_type_id IS NULL");
        } else {
            parameters.push(Box::new(area));
            where_sql.push_str(&format!(" AND ar.area_type_id=?{}", parameters.len()));
        }
    }
    if let Some(category) = &q.category {
        let category = validate_text(category, 64, "辅助分类", false)?;
        if category.chars().any(char::is_control) {
            return Err(AppError::Validation("辅助分类不能包含控制字符".into()));
        }
        if !category.is_empty() {
            parameters.push(Box::new(category));
            where_sql.push_str(&format!(
                " AND COALESCE(NULLIF(ar.auxiliary_type_snapshot,''),'未分类')=?{}",
                parameters.len()
            ));
        }
    }
    if !keyword.is_empty() {
        parameters.push(Box::new(format!(
            "%{}%",
            keyword
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        )));
        let n = parameters.len();
        where_sql.push_str(&format!(" AND (ar.user_name_snapshot ILIKE ?{n} OR ar.auxiliary_work_name_snapshot ILIKE ?{n} OR ar.auxiliary_type_snapshot ILIKE ?{n} OR ar.area_name_snapshot ILIKE ?{n} OR ar.notes ILIKE ?{n})"));
    }
    let refs = parameters
        .iter()
        .map(|p| p.as_ref() as &dyn postgres_compat::types::ToSql)
        .collect::<Vec<_>>();
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    tx.execute(
        "SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY",
        [],
    )?;
    let total = tx.query_row(
        &format!("SELECT COUNT(*) FROM auxiliary_work_records ar{where_sql}"),
        refs.as_slice(),
        |r| r.get(0),
    )?;
    let page = q.page.unwrap_or(1).clamp(1, 1_000_000);
    let page_size = q.page_size.unwrap_or(50).clamp(1, 500);
    let mut stmt=tx.prepare(&format!("{RECORD_SELECT}{where_sql} ORDER BY ar.recorded_at DESC,ar.id DESC LIMIT {page_size} OFFSET {}",(page-1)*page_size))?;
    let mut items = stmt
        .query_map(refs.as_slice(), map_record)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for item in &mut items {
        apply_actions(item, ctx, allowed.as_deref());
    }
    tx.commit()?;
    Ok(PaginatedResponse {
        items,
        total,
        page,
        page_size,
    })
}
async fn list_records(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(q): Query<ListQuery>,
) -> Result<Json<ApiResponse<PaginatedResponse<AuxiliaryRecord>>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(records_for(&pool, &ctx, &q)?)))
}
fn validate_available_item(
    conn: &Connection,
    item: &Item,
    group_id: Option<i64>,
    division_id: i64,
) -> Result<()> {
    if item.deleted_at.is_some() || !item.enabled {
        return Err(AppError::Validation("所选辅助事项已停用或删除".into()));
    }
    if item.scope_type == "public" && item.group_id.is_none() {
        return Ok(());
    }
    if item.scope_type != "laboratory" || item.group_id.is_none() || item.group_id != group_id {
        return Err(AppError::Validation(
            "旧实验室事项必须选择原实验室；请管理员创建公共事项".into(),
        ));
    }
    let group_division: Option<i64> = conn
        .query_row(
            "SELECT division_id FROM project_groups WHERE id=?1 AND deleted_at IS NULL",
            [group_id.unwrap()],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
    if group_division != Some(division_id) {
        return Err(AppError::Validation("实验室与检测部门不一致".into()));
    }
    Ok(())
}
fn audit_record(
    conn: &Connection,
    ctx: &AuthContext,
    action: &str,
    before: Option<&AuxiliaryRecord>,
    after: &AuxiliaryRecord,
    reason: &str,
) -> Result<()> {
    let before_json = before
        .map(serde_json::to_string)
        .transpose()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let after_json = serde_json::to_string(after).map_err(|e| AppError::Internal(e.to_string()))?;
    conn.execute("INSERT INTO audit_log(action,table_name,record_id,user_id,user_name,detail,module,business_no,before_json,after_json,source,operator_user_id,operator_username_snapshot,business_user_id,business_username_snapshot,business_division_id,business_division_name_snapshot,created_at) VALUES(?1,'auxiliary_work_records',?2,?3,?4,?5,'work',?6,?7,?8,'auxiliary',?3,?4,?9,?10,?11,?12,to_char(CURRENT_TIMESTAMP,'YYYY-MM-DD HH24:MI:SS'))",params![action,after.id,ctx.user.id,&ctx.user.username,reason,format!("auxiliary:{}",after.id),before_json,after_json,after.user_id,&after.user_name,after.detection_division_id,&after.detection_division_name])?;
    Ok(())
}
fn create_for(pool: &DbPool, ctx: &AuthContext, body: &RecordCreate) -> Result<AuxiliaryRecord> {
    require_entry(ctx)?;
    let (recorded_at, notes) = record_values(body.quantity, &body.recorded_at, &body.notes)?;
    let owner = body.user_id.unwrap_or(ctx.user.id);
    let people = available_people(pool, ctx)?;
    if !people.iter().any(|p| p.id == owner) {
        return Err(AppError::Forbidden("无权为所选人员录入辅助工作".into()));
    }
    let owner_user = user_repo::find_by_id(pool, owner)?
        .ok_or_else(|| AppError::Validation("所选人员不存在".into()))?;
    if owner_user.is_analysis_public_account
        || owner_user.is_rd_public_account
        || !owner_user.is_active
    {
        return Err(AppError::Validation("请选择有效的实际工作人员".into()));
    }
    if owner != ctx.user.id {
        let owner_ctx = AuthContext {
            role_names: owner_user.role_names.clone(),
            user: owner_user.clone(),
        };
        if !authz_service::work_division_allowed(
            pool,
            &owner_ctx,
            Some(body.detection_division_id),
        )? {
            return Err(AppError::Forbidden("所选人员无权在该部门录入工作量".into()));
        }
    }
    let division = allowed_divisions(pool, ctx)?
        .into_iter()
        .find(|d| d.id == body.detection_division_id)
        .ok_or_else(|| AppError::Forbidden("请选择授权检测部门".into()))?;
    let allowed = authz_service::work_allowed_division_ids(pool, ctx)?;
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    lock_categories(&tx)?;
    let item = read_item(&tx, body.auxiliary_work_item_id, true)?;
    check_version(&item.updated_at, body.expected_item_updated_at.as_deref())?;
    validate_available_item(&tx, &item, body.group_id, division.id)?;
    let category = if item.category.is_empty() {
        "未分类"
    } else {
        &item.category
    };
    let group_id = if item.scope_type == "laboratory" {
        item.group_id
    } else {
        None
    };
    let group_name = if group_id.is_some() {
        item.group_name.clone()
    } else {
        String::new()
    };
    let id:i64=tx.query_row("INSERT INTO auxiliary_work_records(auxiliary_work_item_id,user_id,user_name_snapshot,auxiliary_work_name_snapshot,coefficient_snapshot,auxiliary_type_snapshot,quantity,recorded_at,notes,status,created_by,detection_division_id,detection_division_name_snapshot,group_id,group_name_snapshot,area_type_id,area_name_snapshot) VALUES(?1,?2,?3,?4,?5,?6,?7,CAST(CAST(?8 AS TEXT) AS timestamp) AT TIME ZONE 'Asia/Shanghai',?9,'有效',?10,?11,?12,?13,?14,?15,?16) RETURNING id",params![item.id,owner,&owner_user.username,&item.name,item.coefficient,category,body.quantity,recorded_at,notes,ctx.user.id,division.id,&division.name,group_id,group_name,item.area_type_id,item.area_name],|r|r.get(0))?;
    let mut after = read_record(&tx, id, false)?;
    audit_record(&tx, ctx, "create", None, &after, "独立录入辅助工作")?;
    tx.commit()?;
    apply_actions(&mut after, ctx, allowed.as_deref());
    Ok(after)
}
async fn create_record(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(body): Json<RecordCreate>,
) -> Result<Json<ApiResponse<AuxiliaryRecord>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(create_for(&pool, &ctx, &body)?)))
}
fn require_mutation(
    record: &AuxiliaryRecord,
    ctx: &AuthContext,
    allowed: Option<&[i64]>,
    expected: &str,
) -> Result<()> {
    if !record_visible(record, ctx, allowed) {
        return Err(AppError::Forbidden("无权访问该辅助工作记录".into()));
    }
    if record.linked_work_record_id.is_some() {
        return Err(AppError::Validation(
            "该历史记录已关联原工作量记录，请在原工作量入口维护".into(),
        ));
    }
    if !can_mutate(record, ctx, allowed) {
        return Err(AppError::Forbidden("缺少该辅助工作记录维护权限".into()));
    }
    check_version(&record.updated_at, Some(expected))
}
fn update_for(
    pool: &DbPool,
    ctx: &AuthContext,
    id: i64,
    body: &RecordUpdate,
) -> Result<AuxiliaryRecord> {
    require_entry(ctx)?;
    let (recorded_at, notes) = record_values(body.quantity, &body.recorded_at, &body.notes)?;
    let allowed = authz_service::work_allowed_division_ids(pool, ctx)?;
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    lock_categories(&tx)?;
    let before = read_record(&tx, id, true)?;
    require_mutation(&before, ctx, allowed.as_deref(), &body.expected_updated_at)?;
    if before.deleted_at.is_some() || before.status != "有效" {
        return Err(AppError::Validation("仅有效未删除记录可编辑".into()));
    }
    if body.auxiliary_work_item_id.is_some_and(|id| id <= 0) {
        return Err(AppError::Validation("辅助事项标识无效".into()));
    }
    if let Some(item_id) = body
        .auxiliary_work_item_id
        .filter(|id| before.auxiliary_work_item_id != Some(*id))
    {
        let division = before.detection_division_id.ok_or_else(|| {
            AppError::Validation("无部门历史只可修改数量、时间或备注，请勿更换事项".into())
        })?;
        let item = read_item(&tx, item_id, true)?;
        check_version(&item.updated_at, body.expected_item_updated_at.as_deref())?;
        validate_available_item(&tx, &item, before.group_id, division)?;
        let category = if item.category.is_empty() {
            "未分类"
        } else {
            &item.category
        };
        let group_id = if item.scope_type == "laboratory" {
            item.group_id
        } else {
            None
        };
        let group_name = if group_id.is_some() {
            item.group_name.clone()
        } else {
            String::new()
        };
        tx.execute("UPDATE auxiliary_work_records SET auxiliary_work_item_id=?1,auxiliary_work_name_snapshot=?2,coefficient_snapshot=?3,auxiliary_type_snapshot=?4,group_id=?5,group_name_snapshot=?6,area_type_id=?7,area_name_snapshot=?8 WHERE id=?9",params![item.id,item.name,item.coefficient,category,group_id,group_name,item.area_type_id,item.area_name,id])?;
    }
    tx.execute("UPDATE auxiliary_work_records SET quantity=?1,recorded_at=CAST(CAST(?2 AS TEXT) AS timestamp) AT TIME ZONE 'Asia/Shanghai',notes=?3,updated_at=GREATEST(clock_timestamp(),updated_at+interval '1 microsecond') WHERE id=?4",params![body.quantity,recorded_at,notes,id])?;
    let mut after = read_record(&tx, id, false)?;
    audit_record(
        &tx,
        ctx,
        "update",
        Some(&before),
        &after,
        "修改辅助工作；同事项保持历史系数、分类与区域快照",
    )?;
    tx.commit()?;
    apply_actions(&mut after, ctx, allowed.as_deref());
    Ok(after)
}
async fn update_record(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<RecordUpdate>,
) -> Result<Json<ApiResponse<AuxiliaryRecord>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(update_for(&pool, &ctx, id, &body)?)))
}
fn record_deleted(
    pool: &DbPool,
    ctx: &AuthContext,
    id: i64,
    body: &Maintenance,
    restore: bool,
) -> Result<AuxiliaryRecord> {
    require_entry(ctx)?;
    let reason = validate_text(&body.reason, 500, "操作原因", true)?;
    let allowed = authz_service::work_allowed_division_ids(pool, ctx)?;
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let before = read_record(&tx, id, true)?;
    require_mutation(&before, ctx, allowed.as_deref(), &body.expected_updated_at)?;
    if restore != before.deleted_at.is_some() {
        return Err(AppError::Validation("记录状态已变化，请刷新".into()));
    }
    tx.execute("UPDATE auxiliary_work_records SET deleted_at=CASE WHEN ?1 THEN NULL ELSE clock_timestamp() END,status=CASE WHEN ?1 THEN '有效' ELSE '作废' END,updated_at=GREATEST(clock_timestamp(),updated_at+interval '1 microsecond') WHERE id=?2",params![restore,id])?;
    let mut after = read_record(&tx, id, false)?;
    audit_record(
        &tx,
        ctx,
        if restore { "restore" } else { "delete" },
        Some(&before),
        &after,
        &reason,
    )?;
    tx.commit()?;
    apply_actions(&mut after, ctx, allowed.as_deref());
    Ok(after)
}
async fn delete_record(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<Maintenance>,
) -> Result<Json<ApiResponse<AuxiliaryRecord>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(record_deleted(
        &pool, &ctx, id, &body, false,
    )?)))
}
async fn restore_record(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<Maintenance>,
) -> Result<Json<ApiResponse<AuxiliaryRecord>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(record_deleted(
        &pool, &ctx, id, &body, true,
    )?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (DbPool, AuthContext, AuthContext, i64, i64) {
        let pool = crate::db::init_pool("postgres-test");
        fixture_on(pool)
    }
    fn fixture_on(pool: DbPool) -> (DbPool, AuthContext, AuthContext, i64, i64) {
        let conn = pool.get().unwrap();
        crate::db::test_migrations::run(&conn).unwrap();
        drop(conn);
        let admin = AuthContext {
            user: user_repo::find_by_username(&pool, "admin")
                .unwrap()
                .unwrap(),
            role_names: vec![],
        };
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO divisions(name,is_active) VALUES('aux68-a',1)",
            [],
        )
        .unwrap();
        let department = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO divisions(name,is_active) VALUES('aux68-b',1)",
            [],
        )
        .unwrap();
        let foreign = conn.last_insert_rowid();
        conn.execute("INSERT INTO roles(name) VALUES('aux68-role')", [])
            .unwrap();
        let role = conn.last_insert_rowid();
        for permission in ["entry:auxiliary", "records:auxiliary:edit-own"] {
            conn.execute(
                "INSERT INTO role_permissions(role_id,permission_key) VALUES(?1,?2)",
                params![role, permission],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO role_work_division_scopes(role_id,division_id) VALUES(?1,?2)",
            params![role, department],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO users(username,password,division_id) VALUES('aux68-person','hash',?1)",
            [department],
        )
        .unwrap();
        let person = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO user_roles(user_id,role_id) VALUES(?1,?2)",
            params![person, role],
        )
        .unwrap();
        drop(conn);
        let user = user_repo::find_by_id(&pool, person).unwrap().unwrap();
        let ctx = AuthContext {
            role_names: user.role_names.clone(),
            user,
        };
        (pool, admin, ctx, department, foreign)
    }
    fn item_body(coefficient: f64) -> ItemWrite {
        ItemWrite {
            name: "aux68-public".into(),
            coefficient,
            category: "前处理".into(),
            area_type_id: None,
            enabled: true,
            sort_order: 1,
            notes: "note".into(),
            expected_updated_at: None,
        }
    }
    fn entry(item: &Item, department: i64) -> RecordCreate {
        RecordCreate {
            auxiliary_work_item_id: item.id,
            expected_item_updated_at: Some(item.updated_at.clone()),
            user_id: None,
            detection_division_id: department,
            group_id: None,
            quantity: 3,
            recorded_at: "2026-10-25T12:30".into(),
            notes: "original".into(),
        }
    }
    fn edit(record: &AuxiliaryRecord) -> RecordUpdate {
        RecordUpdate {
            auxiliary_work_item_id: record.auxiliary_work_item_id,
            expected_item_updated_at: None,
            quantity: 4,
            recorded_at: "2026-10-26T10:00".into(),
            notes: "edited".into(),
            expected_updated_at: record.updated_at.clone(),
        }
    }
    #[test]
    fn auxiliary_area_update_distinguishes_missing_null_and_assignment() {
        let body = serde_json::json!({"name":"事项","coefficient":1,"enabled":true});
        let absent: ItemWrite = serde_json::from_value(body.clone()).unwrap();
        assert_eq!(absent.area_type_id, None);
        let mut cleared = body.clone();
        cleared["area_type_id"] = serde_json::Value::Null;
        assert_eq!(
            serde_json::from_value::<ItemWrite>(cleared)
                .unwrap()
                .area_type_id,
            Some(None)
        );
        let mut assigned = body;
        assigned["area_type_id"] = serde_json::json!(17);
        assert_eq!(
            serde_json::from_value::<ItemWrite>(assigned.clone())
                .unwrap()
                .area_type_id,
            Some(Some(17))
        );
        assigned["area_type_id"] = serde_json::json!(-1);
        assert!(validate_item(&serde_json::from_value::<ItemWrite>(assigned).unwrap()).is_err());
    }

    #[test]
    fn auxiliary_area_and_category_snapshots_filters_and_per_item_coefficients_stay_independent() {
        let (pool, admin, person, department, _) = fixture();
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO method_types(name) VALUES('aux70液相'),('aux70气相')",
            [],
        )
        .unwrap();
        let liquid: i64 = conn
            .query_row(
                "SELECT id FROM method_types WHERE name='aux70液相'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let gas: i64 = conn
            .query_row(
                "SELECT id FROM method_types WHERE name='aux70气相'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        drop(conn);
        let item = write_item(&pool, &admin, None, &item_body(1.0)).unwrap();
        let legacy = create_for(&pool, &person, &entry(&item, department)).unwrap();
        assert_eq!((legacy.area_type_id, legacy.area_name.as_str()), (None, ""));
        let mut body = item_body(1.5);
        body.category = "辅助1".into();
        body.area_type_id = Some(Some(liquid));
        body.expected_updated_at = Some(item.updated_at.clone());
        let liquid_item = write_item(&pool, &admin, Some(item.id), &body).unwrap();
        let first = create_for(&pool, &person, &entry(&liquid_item, department)).unwrap();
        assert_eq!(
            (
                first.area_type_id,
                first.area_name.as_str(),
                first.category.as_str(),
                first.coefficient
            ),
            (Some(liquid), "aux70液相", "辅助1", 1.5)
        );
        body.name = "aux70气相辅助事项".into();
        body.coefficient = 4.0;
        body.area_type_id = Some(Some(gas));
        body.expected_updated_at = None;
        let second_item = write_item(&pool, &admin, None, &body).unwrap();
        let second = create_for(&pool, &person, &entry(&second_item, department)).unwrap();
        assert_eq!(
            (second.category.as_str(), second.coefficient),
            ("辅助1", 4.0)
        );
        let conn = pool.get().unwrap();
        conn.execute(
            "UPDATE method_types SET name='aux70液相新名称' WHERE id=?1",
            [liquid],
        )
        .unwrap();
        drop(conn);
        body.name = liquid_item.name.clone();
        body.category = "辅助2".into();
        body.coefficient = 2.5;
        body.expected_updated_at = Some(liquid_item.updated_at.clone());
        let reassigned = write_item(&pool, &admin, Some(liquid_item.id), &body).unwrap();
        let edited = update_for(&pool, &person, first.id, &edit(&first)).unwrap();
        assert_eq!(
            (
                edited.area_type_id,
                edited.area_name.as_str(),
                edited.category.as_str(),
                edited.coefficient
            ),
            (Some(liquid), "aux70液相", "辅助1", 1.5)
        );
        let current = create_for(&pool, &person, &entry(&reassigned, department)).unwrap();
        assert_eq!(
            (
                current.area_type_id,
                current.category.as_str(),
                current.coefficient
            ),
            (Some(gas), "辅助2", 2.5)
        );
        let filtered = records_for(
            &pool,
            &person,
            &ListQuery {
                area_type_id: Some(liquid),
                category: Some("辅助1".into()),
                page_size: Some(1),
                ..ListQuery::default()
            },
        )
        .unwrap();
        assert_eq!((filtered.total, filtered.items[0].id), (1, first.id));
        let unassigned = records_for(
            &pool,
            &person,
            &ListQuery {
                area_type_id: Some(-1),
                ..ListQuery::default()
            },
        )
        .unwrap();
        assert_eq!((unassigned.total, unassigned.items[0].id), (1, legacy.id));
        assert_eq!(
            records_for(
                &pool,
                &person,
                &ListQuery {
                    category: Some("辅助1".into()),
                    ..ListQuery::default()
                }
            )
            .unwrap()
            .total,
            2
        );
        let mut replacement = edit(&edited);
        replacement.auxiliary_work_item_id = Some(second_item.id);
        replacement.expected_item_updated_at = Some(second_item.updated_at.clone());
        let replaced = update_for(&pool, &person, edited.id, &replacement).unwrap();
        assert_eq!(
            (
                replaced.area_type_id,
                replaced.area_name.as_str(),
                replaced.category.as_str(),
                replaced.coefficient
            ),
            (Some(gas), "aux70气相", "辅助1", 4.0)
        );
        body.area_type_id = None;
        body.expected_updated_at = Some(reassigned.updated_at);
        let preserved = write_item(&pool, &admin, Some(item.id), &body).unwrap();
        assert_eq!(preserved.area_type_id, Some(gas));
        body.area_type_id = Some(None);
        body.expected_updated_at = Some(preserved.updated_at);
        let cleared = write_item(&pool, &admin, Some(item.id), &body).unwrap();
        assert_eq!(
            (cleared.area_type_id, cleared.area_name.as_str()),
            (None, "")
        );
        let conn = pool.get().unwrap();
        assert_eq!(
            read_record(&conn, current.id, false).unwrap().area_type_id,
            Some(gas)
        );
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM work_records WHERE source_type='auxiliary_work'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        drop(conn);
        assert!(context_for(&pool, &person)
            .unwrap()
            .areas
            .iter()
            .any(|a| a.id == liquid && a.name == "aux70液相新名称"));
        body.area_type_id = Some(Some(i64::MAX));
        body.expected_updated_at = Some(cleared.updated_at);
        assert!(matches!(
            write_item(&pool, &admin, Some(item.id), &body),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn auxiliary_snapshots_lifecycle_zero_and_audit_are_atomic() {
        let (pool, admin, person, department, _) = fixture();
        let item = write_item(&pool, &admin, None, &item_body(0.0)).unwrap();
        assert_eq!(item.scope_type, "public");
        assert!(item.group_id.is_none());
        let record = create_for(&pool, &person, &entry(&item, department)).unwrap();
        assert_eq!(record.user_id, Some(person.user.id));
        assert_eq!(record.created_by, Some(person.user.id));
        assert_eq!(record.coefficient, 0.0);
        assert_eq!(record.category, "前处理");
        assert_eq!(record.detection_division_id, Some(department));
        assert!(record.can_edit);
        let mut changed = item_body(2.5);
        changed.category = "仪器维护".into();
        changed.expected_updated_at = Some(item.updated_at.clone());
        write_item(&pool, &admin, Some(item.id), &changed).unwrap();
        let edited = update_for(&pool, &person, record.id, &edit(&record)).unwrap();
        assert_eq!(edited.coefficient, 0.0);
        assert_eq!(edited.category, "前处理");
        assert!(matches!(
            update_for(&pool, &person, record.id, &edit(&record)),
            Err(AppError::Conflict(_))
        ));
        let reason = Maintenance {
            reason: "mistake".into(),
            expected_updated_at: edited.updated_at.clone(),
        };
        let deleted = record_deleted(&pool, &person, record.id, &reason, false).unwrap();
        assert_eq!(deleted.status, "作废");
        assert!(deleted.can_restore);
        assert_eq!(
            records_for(&pool, &person, &ListQuery::default())
                .unwrap()
                .total,
            0
        );
        let restored = record_deleted(
            &pool,
            &person,
            record.id,
            &Maintenance {
                reason: "restore".into(),
                expected_updated_at: deleted.updated_at,
            },
            true,
        )
        .unwrap();
        assert_eq!(restored.status, "有效");
        assert_eq!(restored.coefficient, 0.0);
        let conn = pool.get().unwrap();
        let (audit_count,owner_count):(i64,i64)=conn.query_row("SELECT COUNT(*),COUNT(*) FILTER(WHERE business_user_id=?1 AND operator_user_id=?1 AND business_division_id=?2) FROM audit_log WHERE table_name='auxiliary_work_records'",params![person.user.id,department],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
        assert_eq!((audit_count, owner_count), (4, 4));
        let mirror_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM work_records WHERE source_type='auxiliary_work'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(mirror_count, 0);
        let item = read_item(&conn, item.id, false).unwrap();
        let fresh = create_for(&pool, &person, &entry(&item, department)).unwrap();
        assert_eq!(fresh.coefficient, 2.5);
        assert_eq!(fresh.category, "仪器维护");
        assert!(write_item(&pool, &admin, Some(item.id), &changed).is_err());
    }
    #[test]
    fn auxiliary_authorization_blocks_impersonation_foreign_department_and_statistics_writes() {
        let (pool, admin, person, department, foreign) = fixture();
        let item = write_item(&pool, &admin, None, &item_body(1.5)).unwrap();
        let mut forged = entry(&item, department);
        forged.user_id = Some(admin.user.id);
        assert!(matches!(
            create_for(&pool, &person, &forged),
            Err(AppError::Forbidden(_))
        ));
        assert!(matches!(
            create_for(&pool, &person, &entry(&item, foreign)),
            Err(AppError::Forbidden(_))
        ));
        assert!(write_item(&pool, &person, None, &item_body(1.0)).is_err());
        let own = create_for(&pool, &person, &entry(&item, department)).unwrap();
        let foreign_record = create_for(&pool, &admin, &entry(&item, foreign)).unwrap();
        let mut read_only = person.clone();
        read_only.user.permissions =
            vec!["entry:auxiliary".into(), "stats:workload:view-all".into()];
        let in_scope_admin = create_for(&pool, &admin, &entry(&item, department)).unwrap();
        assert_eq!(
            records_for(&pool, &read_only, &ListQuery::default())
                .unwrap()
                .total,
            2
        );
        assert!(update_for(&pool, &read_only, in_scope_admin.id, &edit(&in_scope_admin)).is_err());
        assert!(update_for(&pool, &read_only, own.id, &edit(&own)).is_err());
        read_only.user.permissions =
            vec!["entry:auxiliary".into(), "records:auxiliary:manage".into()];
        assert_eq!(
            records_for(&pool, &read_only, &ListQuery::default())
                .unwrap()
                .total,
            2
        );
        assert!(update_for(&pool, &read_only, in_scope_admin.id, &edit(&in_scope_admin)).is_ok());
        assert!(update_for(&pool, &read_only, foreign_record.id, &edit(&foreign_record)).is_err());
        let options = available_people(&pool, &admin).unwrap();
        assert!(
            options.iter().any(|u| u.id == person.user.id),
            "auxiliary-only account is selectable by administrator"
        );
        let mut proxy = entry(&item, department);
        proxy.user_id = Some(person.user.id);
        let proxy_record = create_for(&pool, &admin, &proxy).unwrap();
        assert_eq!(proxy_record.user_id, Some(person.user.id));
        assert_eq!(proxy_record.created_by, Some(admin.user.id));
        let conn = pool.get().unwrap();
        conn.execute(
            "UPDATE auxiliary_work_records SET detection_division_id=NULL WHERE id=?1",
            [own.id],
        )
        .unwrap();
        assert!(records_for(&pool, &person, &ListQuery::default())
            .unwrap()
            .items
            .iter()
            .any(|r| r.id == own.id));
        assert!(!records_for(&pool, &read_only, &ListQuery::default())
            .unwrap()
            .items
            .iter()
            .any(|r| r.id == foreign_record.id));
        assert!(create_for(
            &pool,
            &person,
            &RecordCreate {
                quantity: 0,
                ..entry(&item, department)
            }
        )
        .is_err());
        assert!(create_for(
            &pool,
            &person,
            &RecordCreate {
                recorded_at: "bad".into(),
                ..entry(&item, department)
            }
        )
        .is_err());
    }
    #[test]
    fn auxiliary_filters_apply_before_paging_and_linked_history_remains_read_only() {
        let (pool, admin, person, department, _) = fixture();
        let item = write_item(&pool, &admin, None, &item_body(1.0)).unwrap();
        let mut records = vec![];
        for i in 0..5 {
            let mut body = entry(&item, department);
            body.notes = format!("batch-{i}");
            records.push(create_for(&pool, &person, &body).unwrap());
        }
        let page = records_for(
            &pool,
            &person,
            &ListQuery {
                page_size: Some(2),
                ..ListQuery::default()
            },
        )
        .unwrap();
        assert_eq!(page.total, 5);
        assert_eq!(page.items.len(), 2);
        let found = records_for(
            &pool,
            &person,
            &ListQuery {
                keyword: Some("batch-0".into()),
                page_size: Some(2),
                ..ListQuery::default()
            },
        )
        .unwrap();
        assert_eq!(found.total, 1);
        assert_eq!(found.items[0].id, records[0].id);
        assert_eq!(
            records_for(
                &pool,
                &person,
                &ListQuery {
                    start: Some("2026-10-26".into()),
                    ..ListQuery::default()
                }
            )
            .unwrap()
            .total,
            0
        );
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO project_groups(name,division_id) VALUES('aux68-lab',?1)",
            [department],
        )
        .unwrap();
        let group = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO projects(name,group_id) VALUES('aux68-project',?1)",
            [group],
        )
        .unwrap();
        let project = conn.last_insert_rowid();
        conn.execute("INSERT INTO work_records(project_id,user_name,quantity,recorded_at,source_type,source_record_id) VALUES(?1,'old',3,'2026-10-25T12:30:00','auxiliary_work',?2)",params![project,records[0].id]).unwrap();
        let main = conn.last_insert_rowid();
        let linked = records_for(&pool, &person, &ListQuery::default())
            .unwrap()
            .items
            .into_iter()
            .find(|r| r.id == records[0].id)
            .unwrap();
        assert_eq!(linked.linked_work_record_id, Some(main));
        assert!(!linked.can_edit);
        assert!(update_for(&pool, &admin, linked.id, &edit(&linked)).is_err());
        assert!(record_deleted(
            &pool,
            &person,
            linked.id,
            &Maintenance {
                reason: "delete".into(),
                expected_updated_at: linked.updated_at.clone()
            },
            false
        )
        .is_err());
        conn.execute(
            "UPDATE work_records SET deleted_at='2026-10-27T00:00:00' WHERE id=?1",
            [main],
        )
        .unwrap();
        assert!(
            update_for(&pool, &admin, linked.id, &edit(&linked)).is_err(),
            "deleted linked main remains authoritative for maintenance"
        );
        conn.execute("INSERT INTO auxiliary_work_items(name,group_id,scope_type) VALUES('legacy lab item',?1,'laboratory')",[group]).unwrap();
        let legacy = conn.last_insert_rowid();
        assert!(create_for(
            &pool,
            &person,
            &entry(&read_item(&conn, legacy, false).unwrap(), department)
        )
        .is_err());
        let legacy_record = create_for(
            &pool,
            &person,
            &RecordCreate {
                group_id: Some(group),
                ..entry(&read_item(&conn, legacy, false).unwrap(), department)
            },
        )
        .unwrap();
        assert_eq!(legacy_record.group_id, Some(group));
        assert!(items_for(&pool, &person, false)
            .unwrap()
            .iter()
            .any(|i| i.id == legacy));
    }
    #[test]
    fn auxiliary_permissions_are_granted_once_and_remain_revoked_on_restart() {
        let (pool, _, person, _, _) = fixture();
        let conn = pool.get().unwrap();
        for p in [
            "entry:auxiliary",
            "records:auxiliary:edit-own",
            "records:auxiliary:manage",
            "manage:auxiliary",
        ] {
            assert!(crate::models::role::is_valid_permission(p));
        }
        let role = person.user.role_ids[0];
        conn.execute("INSERT INTO role_permissions(role_id,permission_key) VALUES(?1,'entry:workload') ON CONFLICT DO NOTHING",[role]).unwrap();
        conn.execute("DELETE FROM role_permissions WHERE role_id=?1 AND permission_key IN ('entry:auxiliary','records:auxiliary:edit-own')",[role]).unwrap();
        conn.execute(
            "DELETE FROM schema_migrations WHERE version='2.3.68-auxiliary-maintenance'",
            [],
        )
        .unwrap();
        crate::db::test_migrations::run(&conn).unwrap();
        let granted:i64=conn.query_row("SELECT COUNT(*) FROM role_permissions WHERE role_id=?1 AND permission_key IN ('entry:auxiliary','records:auxiliary:edit-own')",[role],|r|r.get(0)).unwrap();
        assert_eq!(granted, 2);
        let manager:i64=conn.query_row("SELECT COUNT(*) FROM role_permissions WHERE role_id=?1 AND permission_key IN ('records:auxiliary:manage','manage:auxiliary')",[role],|r|r.get(0)).unwrap();
        assert_eq!(manager, 0);
        conn.execute("DELETE FROM role_permissions WHERE role_id=?1 AND permission_key IN ('entry:auxiliary','records:auxiliary:edit-own')",[role]).unwrap();
        crate::db::test_migrations::run(&conn).unwrap();
        let granted:i64=conn.query_row("SELECT COUNT(*) FROM role_permissions WHERE role_id=?1 AND permission_key='entry:auxiliary'",[role],|r|r.get(0)).unwrap();
        assert_eq!(granted, 0);
    }

    #[test]
    fn auxiliary_paths_release_connections_before_reusing_a_single_connection_pool() {
        let pool = r2d2::Pool::builder()
            .max_size(1)
            .connection_timeout(std::time::Duration::from_secs(2))
            .build(postgres_compat::ConnectionManager::test_database().unwrap())
            .unwrap();
        let (pool, admin, person, department, _) = fixture_on(pool);
        assert!(context_for(&pool, &admin)
            .unwrap()
            .people
            .iter()
            .any(|p| p.id == person.user.id));
        let item = write_item(&pool, &admin, None, &item_body(1.0)).unwrap();
        assert_eq!(items_for(&pool, &person, false).unwrap().len(), 1);
        let record = create_for(&pool, &person, &entry(&item, department)).unwrap();
        let edited = update_for(&pool, &person, record.id, &edit(&record)).unwrap();
        assert_eq!(
            records_for(&pool, &person, &ListQuery::default())
                .unwrap()
                .total,
            1
        );
        let deleted = record_deleted(
            &pool,
            &person,
            record.id,
            &Maintenance {
                reason: "single-pool-delete".into(),
                expected_updated_at: edited.updated_at,
            },
            false,
        )
        .unwrap();
        record_deleted(
            &pool,
            &person,
            record.id,
            &Maintenance {
                reason: "single-pool-restore".into(),
                expected_updated_at: deleted.updated_at,
            },
            true,
        )
        .unwrap();
    }

    #[test]
    fn auxiliary_linked_main_delete_restore_bulk_and_purge_keep_one_authoritative_source() {
        let (pool, admin, person, department, _) = fixture();
        let item = write_item(&pool, &admin, None, &item_body(1.5)).unwrap();
        let mirror = create_for(&pool, &person, &entry(&item, department)).unwrap();
        let independent = create_for(&pool, &person, &entry(&item, department)).unwrap();
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO project_groups(name,division_id) VALUES('aux68-authority-lab',?1)",
            [department],
        )
        .unwrap();
        let lab = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO projects(name,group_id) VALUES('aux68-authority-project',?1)",
            [lab],
        )
        .unwrap();
        let project = conn.last_insert_rowid();
        conn.execute("INSERT INTO work_records(project_id,group_id,subject_user_id,user_name,quantity,coefficient_snapshot,recorded_at,detection_division_id,source_type,source_record_id) VALUES(?1,?2,?3,?4,3,1.5,'2026-10-25T12:30:00',?5,'auxiliary_work',?6)",params![project,lab,person.user.id,&person.user.username,department,mirror.id]).unwrap();
        let main = conn.last_insert_rowid();
        drop(conn);
        let total = || {
            let conn = pool.get().unwrap();
            super::super::export_data::configure_dimension_filters(
                &conn, None, None, None, false, None,
            )
            .unwrap();
            super::super::export_data::query_auxiliary_work_details(
                &conn,
                "2026-10-01",
                "2026-10-31",
                Some(person.user.id),
                None,
            )
            .unwrap()
            .into_iter()
            .map(|r| r.quantity)
            .sum::<i64>()
        };
        let trash_id = |id| {
            pool.get().unwrap().query_row("SELECT id FROM trash_entries WHERE table_name='work_records' AND record_id=?1 AND restored_at IS NULL AND purged_at IS NULL ORDER BY id DESC LIMIT 1",[id],|r|r.get::<_,i64>(0)).unwrap()
        };
        assert_eq!(
            total(),
            6,
            "same name/date/quantity without a source link stays independent"
        );
        assert!(
            !crate::repo::trash_repo::precheck(&pool, "work_records", main)
                .unwrap()
                .can_purge
        );
        crate::repo::record_repo::soft_delete(&pool, main, &admin.user.username, "single delete")
            .unwrap();
        assert_eq!(total(), 3, "deleted main must not resurrect its mirror");
        assert!(matches!(
            crate::repo::trash_repo::purge(
                &pool,
                trash_id(main),
                admin.user.id,
                &admin.user.username
            ),
            Err(AppError::Conflict(_))
        ));
        crate::repo::trash_repo::restore(
            &pool,
            trash_id(main),
            admin.user.id,
            &admin.user.username,
        )
        .unwrap();
        assert_eq!(total(), 6);
        assert_eq!(
            crate::repo::record_repo::delete_by_user(
                &pool,
                &person.user.username,
                None,
                None,
                None,
                &admin.user.username,
                "bulk person delete"
            )
            .unwrap(),
            1
        );
        assert_eq!(total(), 3);
        crate::repo::record_repo::restore(&pool, main, &admin.user.username).unwrap();
        assert_eq!(total(), 6);
        assert_eq!(
            crate::repo::record_repo::soft_delete_range(
                &pool,
                "2026-10-25",
                "2026-10-25T23:59:59",
                &admin.user.username,
                "bulk date delete"
            )
            .unwrap(),
            1
        );
        assert_eq!(total(), 3);
        crate::repo::trash_repo::restore(
            &pool,
            trash_id(main),
            admin.user.id,
            &admin.user.username,
        )
        .unwrap();
        assert_eq!(total(), 6);
        let conn = pool.get().unwrap();
        conn.execute("INSERT INTO work_records(project_id,user_name,quantity,recorded_at,business_no) VALUES(?1,'unlinked-normal',2,'2026-10-25T12:30:00','AUX68-UNLINKED')",[project]).unwrap();
        let unrelated = conn.last_insert_rowid();
        drop(conn);
        assert!(
            crate::repo::trash_repo::precheck(&pool, "work_records", unrelated)
                .unwrap()
                .can_purge
        );
        crate::repo::record_repo::soft_delete(
            &pool,
            unrelated,
            &admin.user.username,
            "normal delete",
        )
        .unwrap();
        crate::repo::trash_repo::purge(
            &pool,
            trash_id(unrelated),
            admin.user.id,
            &admin.user.username,
        )
        .unwrap();
        assert_eq!(total(), 6);
        let conn = pool.get().unwrap();
        let intact:(Option<String>,String,i64,f64)=conn.query_row("SELECT deleted_at::text,status,quantity,coefficient_snapshot FROM auxiliary_work_records WHERE id=?1",[independent.id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap();
        assert_eq!(intact, (None, "有效".into(), 3, 1.5));
    }
}
