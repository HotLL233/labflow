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
         (action,table_name,record_id,user_id,user_name,detail,module,business_no,before_json,after_json,source,
          operator_user_id,operator_username_snapshot,business_user_id,business_username_snapshot,
          business_division_id,business_division_name_snapshot,created_at)
         VALUES (
          ?1,?2,?3,(SELECT id FROM users WHERE username=?4 ORDER BY id LIMIT 1),?4,?5,?6,?7,?8,?9,?10,
          (SELECT id FROM users WHERE username=?4 ORDER BY id LIMIT 1),?4,
          CASE ?2
            WHEN 'work_records' THEN (SELECT subject_user_id FROM work_records WHERE id=?3)
            WHEN 'rd_work_records' THEN (SELECT subject_user_id FROM rd_work_records WHERE id=?3)
            WHEN 'sample_info_records' THEN (SELECT business_user_id FROM sample_info_records WHERE id=?3)
            ELSE NULL
          END,
          CASE ?2
            WHEN 'work_records' THEN COALESCE((SELECT u.username FROM work_records r LEFT JOIN users u ON u.id=r.subject_user_id WHERE r.id=?3),(SELECT user_name FROM work_records WHERE id=?3),'')
            WHEN 'rd_work_records' THEN COALESCE((SELECT u.username FROM rd_work_records r LEFT JOIN users u ON u.id=r.subject_user_id WHERE r.id=?3),(SELECT user_name FROM rd_work_records WHERE id=?3),'')
            WHEN 'sample_info_records' THEN COALESCE((SELECT business_username_snapshot FROM sample_info_records WHERE id=?3),'')
            ELSE ''
          END,
          CASE ?2
            WHEN 'work_records' THEN (SELECT execution_division_id FROM work_records WHERE id=?3)
            WHEN 'rd_work_records' THEN (SELECT execution_division_id FROM rd_work_records WHERE id=?3)
            WHEN 'sample_info_records' THEN (SELECT submitted_division_id FROM sample_info_records WHERE id=?3)
            ELSE NULL
          END,
          CASE ?2
            WHEN 'work_records' THEN COALESCE((SELECT execution_division_name_snapshot FROM work_records WHERE id=?3),'')
            WHEN 'rd_work_records' THEN COALESCE((SELECT execution_division_name_snapshot FROM rd_work_records WHERE id=?3),'')
            WHEN 'sample_info_records' THEN COALESCE((SELECT submitted_division_name_snapshot FROM sample_info_records WHERE id=?3),'')
            ELSE ''
          END,
          datetime('now','localtime')
         )",
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
         (action,table_name,record_id,user_id,user_name,detail,module,business_no,before_json,after_json,source,
          operator_user_id,operator_username_snapshot,business_user_id,business_username_snapshot,
          business_division_id,business_division_name_snapshot,created_at)
         VALUES (
          ?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?4,?5,
          CASE ?2
            WHEN 'work_records' THEN (SELECT subject_user_id FROM work_records WHERE id=?3)
            WHEN 'rd_work_records' THEN (SELECT subject_user_id FROM rd_work_records WHERE id=?3)
            WHEN 'sample_info_records' THEN (SELECT business_user_id FROM sample_info_records WHERE id=?3)
            ELSE NULL
          END,
          CASE ?2
            WHEN 'work_records' THEN COALESCE((SELECT u.username FROM work_records r LEFT JOIN users u ON u.id=r.subject_user_id WHERE r.id=?3),(SELECT user_name FROM work_records WHERE id=?3),'')
            WHEN 'rd_work_records' THEN COALESCE((SELECT u.username FROM rd_work_records r LEFT JOIN users u ON u.id=r.subject_user_id WHERE r.id=?3),(SELECT user_name FROM rd_work_records WHERE id=?3),'')
            WHEN 'sample_info_records' THEN COALESCE((SELECT business_username_snapshot FROM sample_info_records WHERE id=?3),'')
            ELSE ''
          END,
          CASE ?2
            WHEN 'work_records' THEN (SELECT execution_division_id FROM work_records WHERE id=?3)
            WHEN 'rd_work_records' THEN (SELECT execution_division_id FROM rd_work_records WHERE id=?3)
            WHEN 'sample_info_records' THEN (SELECT submitted_division_id FROM sample_info_records WHERE id=?3)
            ELSE NULL
          END,
          CASE ?2
            WHEN 'work_records' THEN COALESCE((SELECT execution_division_name_snapshot FROM work_records WHERE id=?3),'')
            WHEN 'rd_work_records' THEN COALESCE((SELECT execution_division_name_snapshot FROM rd_work_records WHERE id=?3),'')
            WHEN 'sample_info_records' THEN COALESCE((SELECT submitted_division_name_snapshot FROM sample_info_records WHERE id=?3),'')
            ELSE ''
          END,
          datetime('now','localtime')
         )",
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
/// - work → 分析检测录入
/// - rd  → 研发送样录入
/// - shared → 仅主数据
/// - notifications → 通知配置与投递维护
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
    list_with_dates(
        pool,
        page,
        page_size,
        module,
        action,
        user_name,
        business_no,
        None,
        None,
    )
}

fn audit_date(value: &str) -> Result<chrono::NaiveDate> {
    if value.len() != 10 {
        return Err(AppError::Validation(
            "日志日期必须为有效的 YYYY-MM-DD 日期".into(),
        ));
    }
    let parsed = chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| AppError::Validation("日志日期必须为有效的 YYYY-MM-DD 日期".into()))?;
    if parsed.format("%Y-%m-%d").to_string() != value {
        return Err(AppError::Validation(
            "日志日期必须为有效的 YYYY-MM-DD 日期".into(),
        ));
    }
    Ok(parsed)
}

pub fn list_with_dates(
    pool: &DbPool,
    page: i64,
    page_size: i64,
    module: Option<&str>,
    action: Option<&str>,
    user_name: Option<&str>,
    business_no: Option<&str>,
    start_date: Option<&str>,
    end_date: Option<&str>,
) -> Result<(Vec<AuditLogResponse>, i64)> {
    let start = start_date
        .filter(|value| !value.is_empty())
        .map(audit_date)
        .transpose()?;
    let end = end_date
        .filter(|value| !value.is_empty())
        .map(audit_date)
        .transpose()?;
    if start.zip(end).is_some_and(|(start, end)| start > end) {
        return Err(AppError::Validation("日志开始日期不能晚于结束日期".into()));
    }
    let conn = pool.get()?;
    let mut clauses: Vec<String> = Vec::new();
    let mut params: Vec<String> = Vec::new();
    match module {
        Some("work") => clauses.push("module='work'".into()),
        Some("rd") => clauses.push("module='rd'".into()),
        Some("sample_info") => clauses.push("module='sample_info'".into()),
        Some("shared") => clauses.push("module='shared'".into()),
        Some("personnel_change") => clauses.push("module='personnel_change'".into()),
        Some("notifications") => clauses.push("module='notifications'".into()),
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
    if let Some(value) = start {
        params.push(value.format("%Y-%m-%d").to_string());
        clauses.push(format!("created_at>=?{}", params.len()));
    }
    if let Some(value) = end {
        let next = value
            .succ_opt()
            .map(|date| date.format("%Y-%m-%d").to_string())
            .filter(|date| date.len() == 10)
            .ok_or_else(|| AppError::Validation("日志结束日期超出支持范围".into()))?;
        params.push(next);
        clauses.push(format!("created_at<?{}", params.len()));
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
    let page_size = page_size.clamp(1, 500);
    let offset = (page.max(1) - 1)
        .checked_mul(page_size)
        .ok_or_else(|| AppError::Validation("日志分页范围过大".into()))?;
    let base = "SELECT id,action,table_name,record_id,user_id,user_name,detail,module,created_at,
                       business_no,before_json,after_json,source,
                       operator_user_id,operator_username_snapshot,business_user_id,business_username_snapshot,
                       business_division_id,business_division_name_snapshot FROM audit_log";
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
            operator_user_id: row.get(13)?,
            operator_username_snapshot: row.get::<_, String>(14).unwrap_or_default(),
            business_user_id: row.get(15)?,
            business_username_snapshot: row.get::<_, String>(16).unwrap_or_default(),
            business_division_id: row.get(17)?,
            business_division_name_snapshot: row.get::<_, String>(18).unwrap_or_default(),
        })
    })?;
    let items: Vec<AuditLogResponse> = rows.collect::<std::result::Result<Vec<_>, _>>()?;
    Ok((items, count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_date_is_strict_and_rejects_impossible_dates() {
        assert!(audit_date("2028-02-29").is_ok());
        for value in [
            "2026-02-29",
            "2026-02-30",
            "2026-1-02",
            "2026-11-2",
            "2026-11-02T00:00:00",
            "-0001-01-01",
            " not-a-date",
            "",
        ] {
            assert!(audit_date(value).is_err(), "accepted {value}");
        }
    }

    #[test]
    fn audit_notifications_date_range_covers_last_second_and_excludes_next_day() {
        let pool = crate::db::init_pool("audit-date-range-test");
        crate::db::test_migrations::run(&pool.get().unwrap()).unwrap();
        let conn = pool.get().unwrap();
        for (date, module) in [
            ("2026-11-01 23:59:59", "notifications"),
            ("2026-11-02 00:00:00", "notifications"),
            ("2026-11-02T23:59:59", "notifications"),
            ("2026-11-03 00:00:00", "notifications"),
            ("2026-11-02 10:00:00", "shared"),
        ] {
            conn.execute("INSERT INTO audit_log(action,table_name,user_name,detail,module,created_at) VALUES('update','notification_rules','admin','date fixture',?1,?2)",postgres_compat::params![module,date]).unwrap();
        }
        let (items, total) = list_with_dates(
            &pool,
            1,
            50,
            Some("notifications"),
            None,
            None,
            None,
            Some("2026-11-02"),
            Some("2026-11-02"),
        )
        .unwrap();
        assert_eq!(total, 2);
        assert_eq!(items.len(), 2);
        assert!(items
            .iter()
            .all(|item| item.created_at.starts_with("2026-11-02")));
        assert!(list_with_dates(
            &pool,
            1,
            50,
            None,
            None,
            None,
            None,
            Some("2026-11-03"),
            Some("2026-11-02")
        )
        .is_err());
        assert!(list_with_dates(
            &pool,
            1,
            50,
            None,
            None,
            None,
            None,
            Some("2026-02-30"),
            None
        )
        .is_err());
        assert!(list_with_dates(
            &pool,
            1,
            50,
            None,
            None,
            None,
            None,
            None,
            Some("9999-12-31")
        )
        .is_err());
        assert!(list_with_dates(&pool, i64::MAX, 500, None, None, None, None, None, None).is_err());
        let (_, legacy_total) =
            list(&pool, 1, 50, Some("notifications"), None, None, None).unwrap();
        assert_eq!(legacy_total, 4);
    }
}
