use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::settings::SystemSetting;

/// 获取单个系统设置
pub fn get(pool: &DbPool, key: &str) -> Result<Option<SystemSetting>> {
    let conn = pool.get().map_err(AppError::Pool)?;
    get_on_conn(&conn, key)
}

pub fn upsert(pool: &DbPool, key: &str, value_json_str: &str) -> Result<()> {
    let conn = pool.get().map_err(AppError::Pool)?;
    upsert_on_conn(&conn, key, value_json_str)
}

/// 在指定连接上读取设置。保存设置时必须使用事务连接，避免审计和设置读写分散到不同连接。
pub fn get_on_conn(conn: &postgres_compat::Connection, key: &str) -> Result<Option<SystemSetting>> {
    let mut stmt = conn
        .prepare("SELECT key, value, updated_at FROM system_settings WHERE key = ?1")
        .map_err(|e| AppError::Database(e))?;
    let mut rows = stmt
        .query_map(postgres_compat::params![key], |row| {
            Ok(SystemSetting {
                key: row.get(0)?,
                value: row.get(1)?,
                updated_at: row.get(2)?,
            })
        })
        .map_err(|e| AppError::Database(e))?;
    match rows.next() {
        Some(Ok(setting)) => Ok(Some(setting)),
        Some(Err(e)) => Err(AppError::Database(e)),
        None => Ok(None),
    }
}

/// 获取所有系统设置
pub fn get_all(pool: &DbPool) -> Result<Vec<SystemSetting>> {
    let conn = pool.get().map_err(AppError::Pool)?;
    let mut stmt = conn
        .prepare("SELECT key, value, updated_at FROM system_settings ORDER BY key")
        .map_err(|e| AppError::Database(e))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(SystemSetting {
                key: row.get(0)?,
                value: row.get(1)?,
                updated_at: row.get(2)?,
            })
        })
        .map_err(|e| AppError::Database(e))?;
    let mut settings = Vec::new();
    for row in rows {
        settings.push(row.map_err(|e| AppError::Database(e))?);
    }
    Ok(settings)
}

/// 在指定连接上插入或更新设置。使用 PostgreSQL 原生时间表达式，不依赖 SQLite 兼容转换。
pub fn upsert_on_conn(
    conn: &postgres_compat::Connection,
    key: &str,
    value_json_str: &str,
) -> Result<()> {
    let sql = "INSERT INTO system_settings (key, value, updated_at)
               VALUES (?1, ?2, to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS'))
               ON CONFLICT (key) DO UPDATE
               SET value=EXCLUDED.value, updated_at=EXCLUDED.updated_at";
    conn.execute(sql, postgres_compat::params![key, value_json_str])
        .map_err(AppError::Database)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setting_and_audit_write_commit_in_one_postgres_transaction() {
        let mut conn = postgres_compat::Connection::open_test_database().expect("test database");
        crate::db::test_migrations::run(&conn).expect("migrations");
        let test_key = format!("test_form_sample_entry_{}", uuid::Uuid::new_v4());
        let tx = conn.transaction().expect("transaction");
        assert!(get_on_conn(&tx, &test_key).expect("read before").is_none());
        upsert_on_conn(&tx, &test_key, r#"{"fields":[]}"#).expect("write setting");
        crate::repo::audit_repo::log_structured_actor_on_conn(
            &tx,
            "update",
            "system_settings",
            Some(0),
            1,
            "admin",
            "test setting save",
            "shared",
            &test_key,
            None,
            None,
            "test",
        )
        .expect("write audit");
        tx.commit().expect("commit");

        let saved = get_on_conn(&conn, &test_key)
            .expect("read saved")
            .expect("setting exists");
        assert_eq!(saved.value, r#"{"fields":[]}"#);
    }
}
