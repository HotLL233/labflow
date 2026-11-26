use crate::db::DbPool;
use crate::error::AppError;
use postgres_compat::Transaction;

/// Audit action types
#[derive(Debug, Clone, Copy)]
pub enum AuditAction {
    Create,
    Update,
    Delete,
}

impl AuditAction {
    pub fn to_str(&self) -> &'static str {
        match self {
            AuditAction::Create => "create",
            AuditAction::Update => "update",
            AuditAction::Delete => "delete",
        }
    }
}

/// Execute business logic in a transaction with automatic audit logging.
/// If the closure returns Err, the transaction is rolled back and no audit log is written.
pub fn execute_with_audit<F, T>(
    pool: &DbPool,
    action: AuditAction,
    table_name: &str,
    description: &str,
    user_name: Option<&str>,
    f: F,
) -> Result<T, AppError>
where
    F: FnOnce(&Transaction) -> Result<T, AppError>,
{
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;

    let result = f(&tx)?;

    // Write audit log in same transaction
    tx.execute(
        "INSERT INTO audit_log (action, table_name, user_name, detail, created_at) VALUES (?1, ?2, ?3, ?4, datetime('now','localtime'))",
        postgres_compat::params![action.to_str(), table_name, user_name, description],
    )?;

    tx.commit()?;
    Ok(result)
}
