use crate::error::Result;

pub fn run(connection: &postgres_compat::Connection, initial_admin_password: &str) -> Result<()> {
    let initialized: bool = connection.query_row(
        "SELECT to_regclass('schema_migrations') IS NOT NULL",
        [],
        |row| row.get(0),
    )?;
    if !initialized {
        connection.execute_batch(include_str!("postgres_schema.sql"))?;
    }
    let seeded: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.0-beta.2-server-seed')",
        [],
        |row| row.get(0),
    )?;
    if !seeded {
        connection.execute_batch(include_str!("postgres_seed.sql"))?;
        connection.execute(
            "INSERT INTO schema_migrations(version) VALUES ('1.1.0-beta.2-server-seed') ON CONFLICT DO NOTHING",
            [],
        )?;
    }
    let notifications_migrated: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='1.1.0-beta.10-notifications')",
        [],
        |row| row.get(0),
    )?;
    if !notifications_migrated {
        connection.execute_batch(include_str!("postgres_notification_migration.sql"))?;
    }
    connection.execute_batch("INSERT INTO role_permissions(role_id,permission_key) SELECT r.id,'manage:notifications' FROM roles r WHERE r.name IN ('系统管理员','分析检测组长') AND NOT EXISTS(SELECT 1 FROM role_permissions rp WHERE rp.role_id=r.id AND rp.permission_key='manage:notifications');")?;
    if !initialized {
        let password_hash = bcrypt::hash(initial_admin_password, bcrypt::DEFAULT_COST)
            .map_err(|error| crate::error::AppError::Internal(error.to_string()))?;
        connection.execute(
            "UPDATE users SET password=?1, updated_at=datetime('now','localtime') WHERE username='admin'",
            postgres_compat::params![password_hash],
        )?;
    }
    Ok(())
}
