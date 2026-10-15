use postgres_compat::ConnectionManager;
use r2d2::Pool;
use std::time::Duration;

pub type DbPool = Pool<ConnectionManager>;

/// 让数据库会话时区与程序所在机器的时区保持一致。
///
/// postgres-compat 把 SQLite 的 `datetime('now','localtime')` 转写成 `CURRENT_TIMESTAMP`，
/// 而 timestamptz 转文本的结果取决于**会话时区**。当 PostgreSQL 的时区与程序所在机器不一致时
/// （例如数据库在另一台机器、或容器里 PostgreSQL 为 UTC），同一列会混入相差数小时的两种时间，
/// 按天/按月的统计与日期范围过滤会随之偏移。
#[derive(Debug)]
struct ProcessTimeZone;

impl r2d2::CustomizeConnection<postgres_compat::Connection, postgres_compat::Error>
    for ProcessTimeZone
{
    fn on_acquire(
        &self,
        conn: &mut postgres_compat::Connection,
    ) -> Result<(), postgres_compat::Error> {
        // 设置失败时只告警，不阻断取连接：此时退回改动前的既有行为。
        if let Err(error) = conn.execute_batch(&session_time_zone_sql()) {
            tracing::warn!("设置数据库会话时区失败，将沿用数据库默认时区: {error}");
        }
        Ok(())
    }
}

fn session_time_zone_sql() -> String {
    let offset = chrono::Local::now().offset().local_minus_utc();
    let sign = if offset < 0 { '-' } else { '+' };
    let abs = offset.abs();
    format!(
        "SET TIME ZONE INTERVAL '{}{:02}:{:02}' HOUR TO MINUTE",
        sign,
        abs / 3600,
        (abs % 3600) / 60
    )
}

pub fn init_pool(database_url: &str) -> DbPool {
    let max_size = std::env::var("WORKLOAD_DB_POOL_MAX_SIZE")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(12);
    let min_idle = std::env::var("WORKLOAD_DB_POOL_MIN_IDLE")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(1);
    init_pool_with_limits(database_url, max_size, min_idle)
}

pub fn init_pool_with_limits(database_url: &str, max_size: u32, min_idle: u32) -> DbPool {
    #[cfg(test)]
    if !database_url.starts_with("postgres://") && !database_url.starts_with("postgresql://") {
        let manager = ConnectionManager::test_database()
            .expect("WORKLOAD_TEST_DATABASE_URL is required for PostgreSQL integration tests");
        return Pool::builder()
            .max_size(4)
            .min_idle(Some(1))
            .connection_timeout(Duration::from_secs(10))
            .build(manager)
            .expect("Failed to create PostgreSQL test connection pool");
    }
    if database_url.trim().is_empty() {
        panic!("PostgreSQL DATABASE_URL is not configured");
    }
    let manager = ConnectionManager::new(database_url);
    Pool::builder()
        .max_size(max_size.max(1))
        .min_idle(Some(min_idle.min(max_size.max(1))))
        .connection_timeout(Duration::from_secs(10))
        .connection_customizer(Box::new(ProcessTimeZone))
        .build(manager)
        .expect("Failed to create PostgreSQL connection pool")
}
