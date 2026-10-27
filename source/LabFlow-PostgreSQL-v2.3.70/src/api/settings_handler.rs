use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::settings::{SettingUpdate, SystemSetting};
use crate::models::ApiResponse;
use crate::repo::settings_repo;
use crate::service::authz_service;
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json, Router,
};

fn setting_save_error(stage: &str, error: impl std::fmt::Display) -> AppError {
    tracing::error!(stage, detail = %error, "system setting save failed");
    AppError::Internal(format!(
        "保存系统配置失败（{}），请重试；如持续出现请联系管理员。",
        stage
    ))
}

fn requires_system_admin(key: &str) -> bool {
    key.starts_with("record_table_style_")
        || matches!(
            key,
            "record_actions_sample_info" | "record_actions_rd" | "record_actions_auxiliary"
        )
        || key == super::workload_category_handler::SETTING_KEY
        || key == crate::service::sample_workload_mapping::SETTING_KEY
}

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/settings", axum::routing::get(list))
        .route(
            "/api/settings/:key",
            axum::routing::get(get_by_key).put(upsert),
        )
        .with_state(pool)
}

/// 登录页需要在登录前读取主题配色，只放行这一组键；其余系统设置必须登录后读取。
/// v2.3.19：此前 list 与 get_by_key 完全没有鉴权，任何人都能读取全部系统设置。
const PUBLIC_SETTING_KEYS: &[&str] = &["theme"];

/// 从 HeaderMap 中提取 JWT claims
/// GET /api/settings — 获取所有系统设置
async fn list(
    State(pool): State<DbPool>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<SystemSetting>>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    let mut settings = settings_repo::get_all(&pool)?;
    if !ctx.user.is_admin {
        settings.retain(|setting| {
            setting.key != super::workload_category_handler::SETTING_KEY
                && setting.key != crate::service::sample_workload_mapping::SETTING_KEY
        });
    }
    Ok(Json(ApiResponse::ok(settings)))
}

/// GET /api/settings/:key — 获取单个系统设置
async fn get_by_key(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(key): Path<String>,
) -> Result<Json<ApiResponse<SystemSetting>>> {
    if !PUBLIC_SETTING_KEYS.contains(&key.as_str()) {
        let ctx = authz_service::authenticate(&pool, &headers)?;
        if key == super::workload_category_handler::SETTING_KEY && !ctx.user.is_admin {
            return Err(AppError::Forbidden(
                "请通过工作量统计接口读取授权范围内的辅助分类".into(),
            ));
        }
        if key == crate::service::sample_workload_mapping::SETTING_KEY && !ctx.user.is_admin {
            return Err(AppError::Forbidden(
                "仅系统管理员可读取完整的样品工作量映射配置".into(),
            ));
        }
    }
    let setting = settings_repo::get(&pool, &key)?
        .ok_or_else(|| AppError::NotFound(format!("设置 '{}' 不存在", key)))?;
    Ok(Json(ApiResponse::ok(setting)))
}

/// PUT /api/settings/:key — 更新系统设置（需管理员权限）
async fn upsert(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Path(key): Path<String>,
    Json(body): Json<SettingUpdate>,
) -> Result<Json<ApiResponse<SystemSetting>>> {
    // 鉴权：检查管理员权限
    let ctx = authz_service::authenticate(&pool, &headers)?;
    if key == super::workload_category_handler::SETTING_KEY {
        return Err(AppError::Forbidden(
            "辅助工作分类请通过专用配置接口保存".into(),
        ));
    }
    authz_service::require_permission(&ctx, "manage:settings")?;
    if requires_system_admin(&key) && !ctx.user.is_admin {
        return Err(AppError::Forbidden("仅系统管理员可修改该配置".into()));
    }

    let value_str = serde_json::to_string(&body.value)
        .map_err(|e| AppError::Internal(format!("JSON 序列化失败: {}", e)))?;

    let mut conn = pool
        .get()
        .map_err(|error| setting_save_error("获取数据库连接", error))?;
    let tx = conn
        .transaction()
        .map_err(|error| setting_save_error("开启事务", error))?;
    if key == crate::service::sample_workload_mapping::SETTING_KEY {
        let mapping = serde_json::from_value(body.value).map_err(|_| {
            AppError::Validation("样品工作量映射配置格式无效，请检查类型、字段和项目别名".into())
        })?;
        crate::service::sample_workload_mapping::validate(&tx, &mapping)?;
    }
    let before = settings_repo::get_on_conn(&tx, &key)
        .map_err(|error| setting_save_error("读取当前配置", error))?
        .map(|setting| serde_json::json!({"key":setting.key,"value":setting.value}));
    settings_repo::upsert_on_conn(&tx, &key, &value_str)
        .map_err(|error| setting_save_error("写入配置", error))?;
    let after = serde_json::json!({"key":key,"value":value_str});
    crate::repo::audit_repo::log_structured_actor_on_conn(
        &tx,
        "update",
        "system_settings",
        Some(0),
        ctx.user.id,
        &ctx.user.username,
        &format!("更新系统设置: {}", key),
        "shared",
        &key,
        before.as_ref(),
        Some(&after),
        "management",
    )
    .map_err(|error| setting_save_error("写入审计日志", error))?;
    tx.commit()
        .map_err(|error| setting_save_error("提交事务", error))?;

    let setting = settings_repo::get(&pool, &key)
        .map_err(|error| setting_save_error("读取保存结果", error))?
        .ok_or_else(|| AppError::Internal("保存系统配置失败（读取保存结果）。".into()))?;

    Ok(Json(ApiResponse::ok(setting)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_layout_settings_require_system_admin() {
        for key in [
            "record_table_style_rd",
            "record_table_style_sample_info",
            "record_actions_rd",
            "record_actions_sample_info",
            "record_actions_auxiliary",
            "workload_auxiliary_categories",
            "sample_info_workload_mapping",
        ] {
            assert!(requires_system_admin(key));
        }
        assert!(!requires_system_admin("theme"));
    }

    #[tokio::test]
    async fn sample_mapping_handlers_enforce_admin_and_reject_invalid_saves_atomically() {
        use crate::service::sample_workload_mapping::SETTING_KEY;

        // The test manager creates one dedicated schema shared by this pool's connections.
        let pool = crate::db::init_pool("mapping-settings-boundary-test");
        crate::db::test_migrations::run(&pool.get().unwrap()).unwrap();
        let user_ids = {
            let conn = pool.get().unwrap();
            conn.execute(
                "INSERT INTO roles(name,is_system) VALUES('mapping-settings-manager',0)",
                [],
            )
            .unwrap();
            let role_id = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO role_permissions(role_id,permission_key) VALUES(?1,'manage:settings')",
                [role_id],
            )
            .unwrap();
            let mut ids = Vec::new();
            for (username, is_admin) in [("mapping-manager", 0_i64), ("mapping-admin", 1_i64)] {
                conn.execute(
                    "INSERT INTO users(username,password,is_admin,is_active,role_id) VALUES(?1,'unused',?2,1,?3)",
                    postgres_compat::params![username, is_admin, role_id],
                )
                .unwrap();
                let user_id = conn.last_insert_rowid();
                conn.execute(
                    "INSERT INTO user_roles(user_id,role_id) VALUES(?1,?2)",
                    postgres_compat::params![user_id, role_id],
                )
                .unwrap();
                ids.push(user_id);
            }
            ids
        };
        let mut authenticated_headers = Vec::new();
        for user_id in &user_ids {
            let user = crate::repo::user_repo::find_by_id(&pool, *user_id)
                .unwrap()
                .unwrap();
            let login = crate::service::auth_service::create_login_response(
                &pool,
                user,
                false,
                Some("mapping-settings-test-device"),
                Some("test"),
            )
            .unwrap();
            let mut headers = HeaderMap::new();
            headers.insert(
                axum::http::header::AUTHORIZATION,
                format!("Bearer {}", login.token).parse().unwrap(),
            );
            authenticated_headers.push(headers);
        }
        let ordinary_headers = &authenticated_headers[0];
        let admin_headers = &authenticated_headers[1];
        let ordinary_ctx = authz_service::authenticate(&pool, ordinary_headers).unwrap();
        assert!(!ordinary_ctx.user.is_admin);
        assert!(ordinary_ctx.has_permission("manage:settings"));

        let empty = serde_json::json!({});
        let saved = upsert(
            State(pool.clone()),
            admin_headers.clone(),
            Path(SETTING_KEY.into()),
            Json(SettingUpdate {
                value: empty.clone(),
            }),
        )
        .await
        .unwrap()
        .0
        .data
        .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&saved.value).unwrap(),
            empty
        );
        let admin_setting = get_by_key(
            State(pool.clone()),
            admin_headers.clone(),
            Path(SETTING_KEY.into()),
        )
        .await
        .unwrap()
        .0
        .data
        .unwrap();
        assert_eq!(admin_setting.value, saved.value);
        assert!(list(State(pool.clone()), admin_headers.clone())
            .await
            .unwrap()
            .0
            .data
            .unwrap()
            .iter()
            .any(|setting| setting.key == SETTING_KEY));

        let snapshot = || {
            pool.get()
                .unwrap()
                .query_row(
                    "SELECT value,updated_at,(SELECT COUNT(*) FROM audit_log WHERE table_name='system_settings' AND business_no=?1) FROM system_settings WHERE key=?1",
                    [SETTING_KEY],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?, row.get::<_, i64>(2)?)),
                )
                .unwrap()
        };
        let before = snapshot();
        assert_eq!(before.2, 1);
        let audit_actor: i64 = pool
            .get()
            .unwrap()
            .query_row(
                "SELECT operator_user_id FROM audit_log WHERE table_name='system_settings' AND business_no=?1",
                [SETTING_KEY],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(audit_actor, user_ids[1]);
        assert!(matches!(
            get_by_key(
                State(pool.clone()),
                ordinary_headers.clone(),
                Path(SETTING_KEY.into())
            )
            .await,
            Err(AppError::Forbidden(_))
        ));
        assert!(matches!(
            upsert(
                State(pool.clone()),
                ordinary_headers.clone(),
                Path(SETTING_KEY.into()),
                Json(SettingUpdate { value: empty })
            )
            .await,
            Err(AppError::Forbidden(_))
        ));
        assert!(!list(State(pool.clone()), ordinary_headers.clone())
            .await
            .unwrap()
            .0
            .data
            .unwrap()
            .iter()
            .any(|setting| setting.key == SETTING_KEY));
        assert_eq!(snapshot(), before);

        for invalid in [
            serde_json::json!("wrong JSON shape"),
            serde_json::json!({"unknown_mapping_key": []}),
            serde_json::json!({"project_aliases": [{"alias": "missing-project", "project_id": i64::MAX}]}),
            serde_json::json!({"type_mappings": [{"type_key": "missing-sample-type", "method_type_ids": [i64::MAX]}]}),
        ] {
            assert!(matches!(
                upsert(
                    State(pool.clone()),
                    admin_headers.clone(),
                    Path(SETTING_KEY.into()),
                    Json(SettingUpdate { value: invalid })
                )
                .await,
                Err(AppError::Validation(_))
            ));
            assert_eq!(snapshot(), before);
        }
    }
}
