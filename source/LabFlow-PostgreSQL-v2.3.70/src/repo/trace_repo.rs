use crate::db::DbPool;
use crate::error::Result;
use crate::models::trace::RecordEvent;

pub fn make_business_no(prefix: &str, timestamp: &str, id: i64) -> String {
    let digits: String = timestamp
        .chars()
        .filter(|c| c.is_ascii_digit())
        .take(8)
        .collect();
    let date = if digits.len() == 8 {
        digits
    } else {
        chrono::Local::now().format("%Y%m%d").to_string()
    };
    format!("{}-{}-{:06}", prefix, date, id)
}

pub fn log_event_on_conn(
    conn: &postgres_compat::Connection,
    module: &str,
    table_name: &str,
    record_id: i64,
    business_no: &str,
    event_type: &str,
    from_status: Option<&str>,
    to_status: Option<&str>,
    operator: &str,
    reason: &str,
    before_data: Option<&serde_json::Value>,
    after_data: Option<&serde_json::Value>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO record_events
         (module, table_name, record_id, business_no, event_type, from_status, to_status,
          operator, reason, before_json, after_json, operated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,datetime('now','localtime'))",
        postgres_compat::params![
            module,
            table_name,
            record_id,
            business_no,
            event_type,
            from_status,
            to_status,
            operator,
            reason,
            before_data.map(serde_json::Value::to_string),
            after_data.map(serde_json::Value::to_string),
        ],
    )?;
    Ok(())
}

pub fn list(pool: &DbPool, table_name: &str, record_id: i64) -> Result<Vec<RecordEvent>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT id, module, table_name, record_id, business_no, event_type,
                from_status, to_status, operator, operated_at, reason, before_json, after_json
         FROM record_events WHERE table_name=?1 AND record_id=?2
         ORDER BY operated_at ASC, id ASC",
    )?;
    let rows = stmt.query_map(postgres_compat::params![table_name, record_id], |row| {
        let before: Option<String> = row.get(11)?;
        let after: Option<String> = row.get(12)?;
        Ok(RecordEvent {
            id: row.get(0)?,
            module: row.get(1)?,
            table_name: row.get(2)?,
            record_id: row.get(3)?,
            business_no: row.get(4)?,
            event_type: row.get(5)?,
            from_status: row.get(6)?,
            to_status: row.get(7)?,
            operator: row.get(8)?,
            operated_at: row.get(9)?,
            reason: row.get::<_, String>(10).unwrap_or_default(),
            before_data: before.and_then(|value| serde_json::from_str(&value).ok()),
            after_data: after.and_then(|value| serde_json::from_str(&value).ok()),
        })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}
