use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::settings::SystemSetting;

/// 获取单个系统设置
pub fn get(pool: &DbPool, key: &str) -> Result<Option<SystemSetting>> {
    let conn = pool.get().map_err(AppError::Pool)?;
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

/// 插入或替换系统设置
/// 如果提供了 conn，则在给定连接上执行（用于事务）；否则从 pool 获取连接
pub fn upsert(
    pool: &DbPool,
    key: &str,
    value_json_str: &str,
    conn: Option<&postgres_compat::Connection>,
) -> Result<()> {
    let sql = "INSERT INTO system_settings (key, value, updated_at)
               VALUES (?1, ?2, datetime('now','localtime'))
               ON CONFLICT (key) DO UPDATE
               SET value=EXCLUDED.value, updated_at=EXCLUDED.updated_at";
    if let Some(c) = conn {
        c.execute(sql, postgres_compat::params![key, value_json_str])
            .map_err(|e| AppError::Database(e))?;
    } else {
        let conn = pool.get().map_err(AppError::Pool)?;
        conn.execute(sql, postgres_compat::params![key, value_json_str])
            .map_err(|e| AppError::Database(e))?;
    }
    Ok(())
}
