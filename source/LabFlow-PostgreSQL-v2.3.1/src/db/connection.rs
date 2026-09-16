use postgres_compat::ConnectionManager;
use r2d2::Pool;

pub type DbPool = Pool<ConnectionManager>;

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
            .connection_timeout(std::time::Duration::from_secs(10))
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
        .connection_timeout(std::time::Duration::from_secs(10))
        .build(manager)
        .expect("Failed to create PostgreSQL connection pool")
}
