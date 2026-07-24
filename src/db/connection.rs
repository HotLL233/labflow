use postgres_compat::ConnectionManager;
use r2d2::Pool;

pub type DbPool = Pool<ConnectionManager>;

pub fn init_pool(database_url: &str) -> DbPool {
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
        .max_size(40)
        .min_idle(Some(4))
        .connection_timeout(std::time::Duration::from_secs(10))
        .build(manager)
        .expect("Failed to create PostgreSQL connection pool")
}
