use super::export_data;
use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::ApiResponse;
use crate::repo::settings_repo;
use crate::service::authz_service::{self, AuthContext};
use axum::{extract::State, http::HeaderMap, routing::get, Json, Router};
use postgres_compat::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const SETTING_KEY: &str = settings_repo::WORKLOAD_AUXILIARY_CATEGORIES_KEY;

#[derive(Serialize)]
struct AuxiliaryOption {
    key: String,
    name: String,
    kind: String,
    active: bool,
}

#[derive(Serialize)]
struct Configuration {
    options: Vec<AuxiliaryOption>,
    mapping: BTreeMap<String, String>,
    revision: String,
}

#[derive(Deserialize)]
struct ConfigurationUpdate {
    mapping: BTreeMap<String, String>,
    expected_revision: Option<String>,
}

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route(
            "/api/stats/workload-auxiliary-config",
            get(get_configuration).put(save_configuration),
        )
        .with_state(pool)
}

fn require_admin(ctx: &AuthContext) -> Result<()> {
    if ctx.user.is_admin {
        Ok(())
    } else {
        Err(AppError::Forbidden("仅系统管理员可修改辅助工作分类".into()))
    }
}

fn mapping(conn: &Connection) -> Result<BTreeMap<String, String>> {
    settings_repo::workload_auxiliary_categories_on_conn(conn)
}

fn options(
    conn: &Connection,
    work_visibility: &str,
    auxiliary_visibility: &str,
) -> Result<Vec<AuxiliaryOption>> {
    let auxiliary_method = format!(
        "(i.instrument_type='辅助工作' OR EXISTS(SELECT 1 FROM method_type_links ml JOIN method_types mt ON mt.id=ml.method_type_id WHERE ml.method_id=m.id AND mt.name='辅助工作')
         OR EXISTS(SELECT 1 FROM work_records historical WHERE historical.method_id=m.id AND {}))",
        export_data::legacy_auxiliary_record_sql("historical")
    );
    let sql = format!(
        "SELECT 'method:' || m.id,COALESCE(NULLIF(m.full_name,''),m.name) ||
          CASE WHEN NULLIF(i.code,'') IS NOT NULL THEN ' [' || i.code || ']'
               WHEN NULLIF(m.method_code,'') IS NOT NULL THEN ' [' || m.method_code || ']' ELSE '' END, 'method',
          m.deleted_at IS NULL AND m.is_active=1
         FROM methods m LEFT JOIN instruments i ON i.id=m.instrument_id
         WHERE {auxiliary_method}{work_visibility}
         UNION ALL
         SELECT 'item:' || ai.id, ai.name || ' [' || COALESCE(g.name,'公共事项') || ']', 'item', ai.deleted_at IS NULL AND ai.enabled
         FROM auxiliary_work_items ai LEFT JOIN project_groups g ON g.id=ai.group_id WHERE TRUE{auxiliary_visibility}
         ORDER BY 3,2,1"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| {
        Ok(AuxiliaryOption {
            key: row.get(0)?,
            name: row.get(1)?,
            kind: row.get(2)?,
            active: row.get(3)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

fn configuration(pool: &DbPool, ctx: &AuthContext) -> Result<Configuration> {
    authz_service::require_permission(ctx, "stats:portal:workload")?;
    let (work_visibility, auxiliary_visibility) = if ctx.user.is_admin {
        (String::new(), String::new())
    } else {
        let divisions = authz_service::work_allowed_division_ids(pool, ctx)?;
        let department = authz_service::work_record_authorization_division_sql("wr");
        let department_scope = match divisions.as_deref() {
            None => String::new(),
            Some([]) => " AND FALSE".into(),
            Some(ids) => format!(
                " AND {department} IN ({})",
                ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
            ),
        };
        let subject = if ctx.can_view_workload_scope() {
            String::new()
        } else {
            format!(" AND wr.subject_user_id={}", ctx.user.id)
        };
        let auxiliary_subject = if ctx.can_view_workload_scope() {
            String::new()
        } else {
            format!(" AND awr.user_id={}", ctx.user.id)
        };
        let auxiliary_scope =
            export_data::auxiliary_scope_sql("awr", divisions.as_deref(), Some(ctx.user.id));
        (
            format!(" AND EXISTS(SELECT 1 FROM work_records wr WHERE wr.method_id=m.id AND wr.deleted_at IS NULL AND {}{department_scope}{subject})", export_data::legacy_auxiliary_record_sql("wr")),
            format!(" AND (EXISTS(SELECT 1 FROM auxiliary_work_records awr WHERE awr.auxiliary_work_item_id=ai.id AND awr.deleted_at IS NULL AND (
              (TRUE{auxiliary_scope}{auxiliary_subject}) OR EXISTS(SELECT 1 FROM work_records wr WHERE wr.source_type='auxiliary_work' AND wr.source_record_id=awr.id AND wr.deleted_at IS NULL{department_scope}{subject})))
              OR EXISTS(SELECT 1 FROM legacy_auxiliary_links mapped JOIN work_records wr ON wr.id=mapped.source_record_id
                WHERE mapped.source_table='work_records' AND mapped.auxiliary_work_item_id=ai.id AND mapped.revoked_at IS NULL
                  AND wr.deleted_at IS NULL{department_scope}{subject}))"),
        )
    };
    let conn = pool.get()?;
    let options = options(&conn, &work_visibility, &auxiliary_visibility)?;
    let mut mapping = mapping(&conn)?;
    let revision = mapping_revision(&mapping)?;
    mapping.retain(|key, _| options.iter().any(|option| &option.key == key));
    Ok(Configuration {
        options,
        mapping,
        revision,
    })
}

fn mapping_revision(values: &BTreeMap<String, String>) -> Result<String> {
    let bytes = serde_json::to_vec(values).map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn validate_mapping(
    conn: &Connection,
    values: BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>> {
    if values.len() > 5000 {
        return Err(AppError::Validation("辅助工作分类配置过多".into()));
    }
    let allowed = options(conn, "", "")?
        .into_iter()
        .map(|option| option.key)
        .collect::<BTreeSet<_>>();
    let mut normalized = BTreeMap::new();
    for (key, value) in values {
        if !allowed.contains(&key) {
            return Err(AppError::Validation(
                "辅助分类仅可绑定现有辅助方法或辅助事项".into(),
            ));
        }
        let value = value.trim();
        if value.chars().count() > 64 || value.chars().any(char::is_control) {
            return Err(AppError::Validation(
                "辅助子类型不能超过64个字符或包含控制字符".into(),
            ));
        }
        if !value.is_empty() {
            normalized.insert(key, value.to_string());
        }
    }
    Ok(normalized)
}

async fn get_configuration(
    State(pool): State<DbPool>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Configuration>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(configuration(&pool, &ctx)?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn purged_auxiliary_method_does_not_block_saving_remaining_configuration() {
        let pool = crate::db::init_pool("postgres-test");
        let conn = pool.get().unwrap();
        crate::db::test_migrations::run(&conn).unwrap();
        let ctx = AuthContext {
            user: crate::repo::user_repo::find_by_username(&pool, "admin")
                .unwrap()
                .unwrap(),
            role_names: vec![],
        };
        conn.execute(
            "INSERT INTO instruments(code,instrument_type) VALUES('CONFIG48-PURGE','辅助工作')",
            [],
        )
        .unwrap();
        let instrument = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO methods(name,instrument_id) VALUES('purge auxiliary method',?1)",
            [instrument],
        )
        .unwrap();
        let method = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO auxiliary_work_items(name,scope_type) VALUES('retained item','global')",
            [],
        )
        .unwrap();
        let item = conn.last_insert_rowid();
        let method_key = format!("method:{method}");
        let item_key = format!("item:{item}");
        let original = BTreeMap::from([
            (method_key.clone(), "维护".into()),
            (item_key.clone(), "前处理".into()),
        ]);
        save_for_user(
            &pool,
            &ctx,
            original.clone(),
            Some(&configuration(&pool, &ctx).unwrap().revision),
        )
        .unwrap();
        crate::repo::method_repo::delete(&pool, method, &ctx.user.username, "cleanup").unwrap();
        assert!(configuration(&pool, &ctx)
            .unwrap()
            .mapping
            .contains_key(&method_key));
        let trash_id = conn.query_row(
            "SELECT id FROM trash_entries WHERE table_name='methods' AND record_id=?1 AND purged_at IS NULL",
            [method],
            |row| row.get::<_, i64>(0),
        ).unwrap();
        crate::repo::trash_repo::purge(&pool, trash_id, ctx.user.id, &ctx.user.username).unwrap();
        assert_eq!(mapping(&conn).unwrap(), original);
        let mut configuration = configuration(&pool, &ctx).unwrap();
        assert!(!configuration
            .options
            .iter()
            .any(|option| option.key == method_key));
        assert_eq!(
            configuration.mapping,
            BTreeMap::from([(item_key.clone(), "前处理".into())])
        );
        configuration
            .mapping
            .insert(item_key.clone(), "仪器维护".into());
        let saved = save_for_user(
            &pool,
            &ctx,
            configuration.mapping,
            Some(&configuration.revision),
        )
        .unwrap();
        assert_eq!(
            saved.mapping,
            BTreeMap::from([(item_key, "仪器维护".into())])
        );
        let raw_before: String = conn.query_row(
            "SELECT before_json FROM audit_log WHERE table_name='system_settings' AND business_no=?1 ORDER BY id DESC LIMIT 1",
            [SETTING_KEY],
            |row| row.get(0),
        ).unwrap();
        let before: serde_json::Value = serde_json::from_str(&raw_before).unwrap();
        assert_eq!(
            before["value"].as_str(),
            Some(serde_json::to_string(&original).unwrap().as_str())
        );
    }

    #[test]
    fn auxiliary_configuration_validates_ids_and_filters_person_and_department_scope() {
        let pool = crate::db::init_pool("postgres-test");
        let conn = pool.get().unwrap();
        crate::db::test_migrations::run(&conn).unwrap();
        let admin = crate::repo::user_repo::find_by_username(&pool, "admin")
            .unwrap()
            .unwrap();
        let mut ctx = AuthContext {
            user: admin,
            role_names: vec![],
        };
        assert!(require_admin(&ctx).is_ok());
        conn.execute("INSERT INTO divisions(name) VALUES('aux_config_A')", [])
            .unwrap();
        let a = conn.last_insert_rowid();
        conn.execute("INSERT INTO divisions(name) VALUES('aux_config_B')", [])
            .unwrap();
        let b = conn.last_insert_rowid();
        conn.execute("INSERT INTO roles(name) VALUES('aux_config_role')", [])
            .unwrap();
        let role = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO role_work_division_scopes(role_id,division_id) VALUES(?1,?2)",
            postgres_compat::params![role, a],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO project_groups(name,division_id) VALUES('aux_config_lab',?1)",
            [a],
        )
        .unwrap();
        let lab = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO projects(name,group_id) VALUES('aux_config_project',?1)",
            [lab],
        )
        .unwrap();
        let project = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO users(username,password) VALUES('aux_config_other','hash')",
            [],
        )
        .unwrap();
        let other = conn.last_insert_rowid();
        let mut keys = Vec::new();
        for (owner, division, name) in [
            (ctx.user.id, a, "own"),
            (other, a, "other"),
            (ctx.user.id, b, "foreign"),
        ] {
            conn.execute("INSERT INTO methods(name) VALUES(?1)", [name])
                .unwrap();
            let method = conn.last_insert_rowid();
            conn.execute("INSERT INTO work_records(project_id,method_id,user_name,quantity,recorded_at,subject_user_id,detection_division_id,detection_type_snapshot,business_no)
                VALUES(?1,?2,'owner',1,'2026-10-12T10:00:00',?3,?4,'辅助工作',?5)", postgres_compat::params![project,method,owner,division,format!("CONFIG48-{method}")]).unwrap();
            keys.push(format!("method:{method}"));
        }
        conn.execute("INSERT INTO auxiliary_work_items(name,scope_type) VALUES('own historical item','global')", []).unwrap();
        let own_item = conn.last_insert_rowid();
        conn.execute("INSERT INTO auxiliary_work_records(auxiliary_work_item_id,user_id,user_name_snapshot,auxiliary_work_name_snapshot,quantity,recorded_at)
            VALUES(?1,?2,'owner','own historical item',1,'2026-10-12T10:00:00')", postgres_compat::params![own_item,ctx.user.id]).unwrap();
        keys.push(format!("item:{own_item}"));
        conn.execute("INSERT INTO auxiliary_work_items(name,scope_type) VALUES('foreign historical item','global')", []).unwrap();
        let foreign_item = conn.last_insert_rowid();
        conn.execute("INSERT INTO auxiliary_work_records(auxiliary_work_item_id,user_id,user_name_snapshot,auxiliary_work_name_snapshot,quantity,recorded_at,detection_division_id)
            VALUES(?1,?2,'other','foreign historical item',1,'2026-10-12T10:00:00',?3)", postgres_compat::params![foreign_item,other,b]).unwrap();
        keys.push(format!("item:{foreign_item}"));
        let values = keys
            .iter()
            .map(|key| (key.clone(), " 前处理 ".to_string()))
            .collect::<BTreeMap<_, _>>();
        let normalized = validate_mapping(&conn, values).unwrap();
        assert!(normalized.values().all(|value| value == "前处理"));
        assert!(validate_mapping(
            &conn,
            BTreeMap::from([("method:0".into(), "前处理".into())])
        )
        .is_err());
        assert!(
            validate_mapping(&conn, BTreeMap::from([(keys[0].clone(), "x".repeat(65))])).is_err()
        );
        settings_repo::upsert_on_conn(
            &conn,
            SETTING_KEY,
            &serde_json::to_string(&normalized).unwrap(),
        )
        .unwrap();
        assert_eq!(configuration(&pool, &ctx).unwrap().mapping.len(), 5);
        for damaged in ["not-json", "[]", "{\"method:0\":12}"] {
            settings_repo::upsert_on_conn(&conn, SETTING_KEY, damaged).unwrap();
            assert!(configuration(&pool, &ctx).unwrap().mapping.is_empty());
            let repaired = save_for_user(
                &pool,
                &ctx,
                normalized.clone(),
                Some(&configuration(&pool, &ctx).unwrap().revision),
            )
            .unwrap();
            assert_eq!(repaired.mapping, normalized);
            let raw_before: String = conn.query_row("SELECT before_json FROM audit_log WHERE table_name='system_settings' AND business_no=?1 ORDER BY id DESC LIMIT 1",[SETTING_KEY],|row| row.get(0)).unwrap();
            let before: serde_json::Value = serde_json::from_str(&raw_before).unwrap();
            assert_eq!(before["value"].as_str(), Some(damaged));
        }
        ctx.user.is_admin = false;
        ctx.user.role_keys.clear();
        ctx.user.role_ids = vec![role];
        ctx.user.permissions = vec!["stats:portal:workload".into(), "manage:settings".into()];
        assert!(require_admin(&ctx).is_err());
        let personal = configuration(&pool, &ctx).unwrap();
        assert_eq!(
            personal
                .options
                .iter()
                .map(|option| option.key.clone())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([keys[0].clone(), keys[3].clone()])
        );
        assert_eq!(personal.mapping.len(), 2);
        ctx.user.permissions.push("stats:workload:view-all".into());
        let scoped = configuration(&pool, &ctx).unwrap();
        assert!(scoped.mapping.contains_key(&keys[1]));
        assert!(!scoped.mapping.contains_key(&keys[2]));
        assert!(!scoped.mapping.contains_key(&keys[4]));
        ctx.user.permissions = vec!["*".into()];
        assert!(require_admin(&ctx).is_err());
    }

    #[test]
    fn auxiliary_configuration_rejects_stale_updates_and_invalidates_item_preview_tokens() {
        let pool = crate::db::init_pool("postgres-test");
        let conn = pool.get().unwrap();
        crate::db::test_migrations::run(&conn).unwrap();
        let ctx = AuthContext {
            user: crate::repo::user_repo::find_by_username(&pool, "admin")
                .unwrap()
                .unwrap(),
            role_names: vec![],
        };
        conn.execute(
            "INSERT INTO auxiliary_work_items(name,scope_type) VALUES('concurrent item','public')",
            [],
        )
        .unwrap();
        let item = conn.last_insert_rowid();
        let before: String = conn
            .query_row(
                "SELECT updated_at::text FROM auxiliary_work_items WHERE id=?1",
                [item],
                |r| r.get(0),
            )
            .unwrap();
        let config = configuration(&pool, &ctx).unwrap();
        let key = format!("item:{item}");
        let first = save_for_user(
            &pool,
            &ctx,
            BTreeMap::from([(key.clone(), "维护".into())]),
            Some(&config.revision),
        )
        .unwrap();
        assert_ne!(config.revision, first.revision);
        let after: String = conn
            .query_row(
                "SELECT updated_at::text FROM auxiliary_work_items WHERE id=?1",
                [item],
                |r| r.get(0),
            )
            .unwrap();
        assert_ne!(before, after);
        assert!(matches!(
            save_for_user(&pool, &ctx, BTreeMap::new(), Some(&config.revision)),
            Err(AppError::Conflict(_))
        ));
        assert!(save_for_user(&pool, &ctx, BTreeMap::new(), None).is_err());
        assert_eq!(
            mapping(&conn).unwrap().get(&key).map(String::as_str),
            Some("维护")
        );
    }
}

async fn save_configuration(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(body): Json<ConfigurationUpdate>,
) -> Result<Json<ApiResponse<Configuration>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    Ok(Json(ApiResponse::ok(save_for_user(
        &pool,
        &ctx,
        body.mapping,
        body.expected_revision.as_deref(),
    )?)))
}

fn save_for_user(
    pool: &DbPool,
    ctx: &AuthContext,
    requested: BTreeMap<String, String>,
    expected_revision: Option<&str>,
) -> Result<Configuration> {
    require_admin(ctx)?;
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    super::auxiliary_handler::lock_categories(&tx)?;
    let current = mapping(&tx)?;
    if expected_revision != Some(mapping_revision(&current)?.as_str()) {
        return Err(AppError::Conflict(
            "辅助分类配置已变化，请关闭并重新打开配置后保存".into(),
        ));
    }
    let values = validate_mapping(&tx, requested)?;
    for key in current.keys().chain(values.keys()).collect::<BTreeSet<_>>() {
        if current.get(key) != values.get(key) {
            if let Some(id) = key
                .strip_prefix("item:")
                .and_then(|value| value.parse::<i64>().ok())
            {
                tx.execute("UPDATE auxiliary_work_items SET updated_at=GREATEST(clock_timestamp(),updated_at+interval '1 microsecond') WHERE id=?1",[id])?;
            }
        }
    }
    let before = settings_repo::get_on_conn(&tx, SETTING_KEY)?
        .map(|setting| serde_json::json!({"key":setting.key,"value":setting.value}));
    let after = serde_json::to_value(&values).map_err(|e| AppError::Internal(e.to_string()))?;
    settings_repo::upsert_on_conn(&tx, SETTING_KEY, &after.to_string())?;
    crate::repo::audit_repo::log_structured_actor_on_conn(
        &tx,
        "update",
        "system_settings",
        Some(0),
        ctx.user.id,
        &ctx.user.username,
        "更新辅助工作子类型配置",
        "shared",
        SETTING_KEY,
        before.as_ref(),
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    drop(conn);
    configuration(pool, ctx)
}
