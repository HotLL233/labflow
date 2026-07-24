use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::audit::AuditLogResponse;

/// Records an automatic backup in the same PostgreSQL database as all business data.
pub fn log_for_backup(pool: &DbPool, action: &str, detail: &str) -> Result<()> {
    let conn = pool.get()?;
    log_on_conn(&conn, action, "backups", None, "system", detail)
}

pub fn log(
    pool: &DbPool,
    action: &str,
    table: &str,
    record_id: Option<i64>,
    user_name: &str,
    detail: &str,
) -> Result<()> {
    pool.get()?.execute(
        "INSERT INTO audit_log (action,table_name,record_id,user_id,user_name,detail,created_at)
         VALUES (?1,?2,?3,(SELECT id FROM users WHERE username=?4 ORDER BY id LIMIT 1),?4,?5,datetime('now','localtime'))",
        postgres_compat::params!(action, table, record_id, user_name, detail),
    )?;
    Ok(())
}

pub fn log_actor(
    pool: &DbPool,
    action: &str,
    table: &str,
    record_id: Option<i64>,
    user_id: i64,
    user_name: &str,
    detail: &str,
    module: &str,
) -> Result<()> {
    let conn = pool.get()?;
    log_actor_on_conn(
        &conn, action, table, record_id, user_id, user_name, detail, module,
    )
}

/// Write audit log on an existing connection (same-connection, avoids pool contention)
pub fn log_on_conn(
    conn: &postgres_compat::Connection,
    action: &str,
    table: &str,
    record_id: Option<i64>,
    user_name: &str,
    detail: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO audit_log (action,table_name,record_id,user_id,user_name,detail,created_at)
         VALUES (?1,?2,?3,(SELECT id FROM users WHERE username=?4 ORDER BY id LIMIT 1),?4,?5,datetime('now','localtime'))",
        postgres_compat::params!(action, table, record_id, user_name, detail),
    )?;
    Ok(())
}

pub fn log_actor_on_conn(
    conn: &postgres_compat::Connection,
    action: &str,
    table: &str,
    record_id: Option<i64>,
    user_id: i64,
    user_name: &str,
    detail: &str,
    module: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO audit_log
         (action,table_name,record_id,user_id,user_name,detail,module,created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,datetime('now','localtime'))",
        postgres_compat::params![action, table, record_id, user_id, user_name, detail, module],
    )?;
    Ok(())
}

/// 模块感知审计 — 追加 module 列（work=分析检测, rd=研发送样, shared=主数据）
pub fn log_with_module(
    pool: &DbPool,
    action: &str,
    table: &str,
    record_id: Option<i64>,
    user_name: &str,
    detail: &str,
    module: &str,
) -> Result<()> {
    pool.get()?.execute(
        "INSERT INTO audit_log (action,table_name,record_id,user_id,user_name,detail,module,created_at)
         VALUES (?1,?2,?3,(SELECT id FROM users WHERE username=?4 ORDER BY id LIMIT 1),?4,?5,?6,datetime('now','localtime'))",
        postgres_compat::params!(action, table, record_id, user_name, detail, module),
    )?;
    Ok(())
}

/// 模块感知审计 — 在已有连接上写入（同连接，避免连接池争用）
pub fn log_on_conn_with_module(
    conn: &postgres_compat::Connection,
    action: &str,
    table: &str,
    record_id: Option<i64>,
    user_name: &str,
    detail: &str,
    module: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO audit_log (action,table_name,record_id,user_id,user_name,detail,module,created_at)
         VALUES (?1,?2,?3,(SELECT id FROM users WHERE username=?4 ORDER BY id LIMIT 1),?4,?5,?6,datetime('now','localtime'))",
        postgres_compat::params!(action, table, record_id, user_name, detail, module),
    )?;
    Ok(())
}

pub fn log_structured_on_conn(
    conn: &postgres_compat::Connection,
    action: &str,
    table: &str,
    record_id: Option<i64>,
    user_name: &str,
    detail: &str,
    module: &str,
    business_no: &str,
    before_data: Option<&serde_json::Value>,
    after_data: Option<&serde_json::Value>,
    source: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO audit_log
         (action,table_name,record_id,user_id,user_name,detail,module,business_no,before_json,after_json,source,created_at)
         VALUES (?1,?2,?3,(SELECT id FROM users WHERE username=?4 ORDER BY id LIMIT 1),?4,?5,?6,?7,?8,?9,?10,datetime('now','localtime'))",
        postgres_compat::params![
            action, table, record_id, user_name, detail, module, business_no,
            before_data.map(serde_json::Value::to_string),
            after_data.map(serde_json::Value::to_string),
            source,
        ],
    )?;
    Ok(())
}

pub fn log_structured_actor_on_conn(
    conn: &postgres_compat::Connection,
    action: &str,
    table: &str,
    record_id: Option<i64>,
    user_id: i64,
    user_name: &str,
    detail: &str,
    module: &str,
    business_no: &str,
    before_data: Option<&serde_json::Value>,
    after_data: Option<&serde_json::Value>,
    source: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO audit_log
         (action,table_name,record_id,user_id,user_name,detail,module,business_no,before_json,after_json,source,created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,datetime('now','localtime'))",
        postgres_compat::params![
            action, table, record_id, user_id, user_name, detail, module, business_no,
            before_data.map(serde_json::Value::to_string),
            after_data.map(serde_json::Value::to_string),
            source,
        ],
    )?;
    Ok(())
}

/// 审计日志列表，支持按 module 过滤：
/// - work → 分析检测录入 + 共享主数据（module IN ('work','shared')）
/// - rd  → 研发送样录入 + 共享主数据（module IN ('rd','shared')）
/// - shared → 仅主数据
/// - None → 全部
pub fn list(
    pool: &DbPool,
    page: i64,
    page_size: i64,
    module: Option<&str>,
    action: Option<&str>,
    user_name: Option<&str>,
    business_no: Option<&str>,
) -> Result<(Vec<AuditLogResponse>, i64)> {
    let conn = pool.get()?;
    let mut clauses: Vec<String> = Vec::new();
    let mut params: Vec<String> = Vec::new();
    match module {
        Some("work") => clauses.push("module='work'".into()),
        Some("rd") => clauses.push("module='rd'".into()),
        Some("sample_info") => clauses.push("module='sample_info'".into()),
        Some("shared") => clauses.push("module='shared'".into()),
        Some(_) => return Err(AppError::Validation("无效的审计模块".into())),
        None => {}
    }
    if let Some(value) = action.filter(|value| !value.is_empty()) {
        params.push(value.to_string());
        clauses.push(format!("action=?{}", params.len()));
    }
    if let Some(value) = user_name.filter(|value| !value.is_empty()) {
        params.push(value.to_string());
        clauses.push(format!("user_name=?{}", params.len()));
    }
    if let Some(value) = business_no.filter(|value| !value.is_empty()) {
        params.push(format!("%{}%", value));
        clauses.push(format!("business_no LIKE ?{}", params.len()));
    }
    let where_clause = if clauses.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", clauses.join(" AND "))
    };
    let count: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM audit_log{}", where_clause),
        postgres_compat::params_from_iter(params.iter()),
        |row| row.get(0),
    )?;
    let offset = (page - 1) * page_size;
    let base = "SELECT id,action,table_name,record_id,user_id,user_name,detail,module,created_at,
                       business_no,before_json,after_json,source FROM audit_log";
    let sql = format!(
        "{}{} ORDER BY created_at DESC,id DESC LIMIT {} OFFSET {}",
        base, where_clause, page_size, offset
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(postgres_compat::params_from_iter(params.iter()), |row| {
        let before: Option<String> = row.get(10)?;
        let after: Option<String> = row.get(11)?;
        Ok(AuditLogResponse {
            id: row.get(0)?,
            action: row.get(1)?,
            table_name: row.get(2)?,
            record_id: row.get(3)?,
            user_id: row.get(4)?,
            user_name: row.get::<_, String>(5).unwrap_or_default(),
            detail: row.get::<_, String>(6).unwrap_or_default(),
            module: row
                .get::<_, String>(7)
                .unwrap_or_else(|_| "shared".to_string()),
            created_at: row.get(8)?,
            business_no: row.get::<_, String>(9).unwrap_or_default(),
            before_data: before.and_then(|value| serde_json::from_str(&value).ok()),
            after_data: after.and_then(|value| serde_json::from_str(&value).ok()),
            source: row
                .get::<_, String>(12)
                .unwrap_or_else(|_| "application".into()),
        })
    })?;
    let items: Vec<AuditLogResponse> = rows.collect::<std::result::Result<Vec<_>, _>>()?;
    Ok((items, count))
}
