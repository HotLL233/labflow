use crate::error::Result;
use crate::models::sample_info::SampleInfoScopeFilter;
/// 样品信息登记导出 — 数据查询层（独立模块，不引用分析检测表）
use postgres_compat::Connection;

#[derive(Debug, Clone, serde::Serialize)]
pub struct SampleInfoExportRow {
    pub seq_no: i64,
    pub batch_no: String,
    pub user_name: String,
    pub lab_name: String,
    pub project_name: String,
    pub submitted_at: String,
    pub detection_date: String,
    pub sampled_by: String,
    pub sampled_at: String,
    pub detected_by: String,
    pub detection_type: String,
    pub status: String,
    pub main_components: String,
    pub notes: String,
    pub extra_fields: Option<String>,
    // 仅用于服务端导出范围过滤，不写入 Excel 表格。
    pub type_key: String,
    pub division_id: Option<i64>,
    pub created_by_user_id: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct NameCountRow {
    pub name: String,
    pub count: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TypeCountRow {
    pub type_key: String,
    pub label: String,
    pub count: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MonthCountRow {
    pub month: String,
    pub count: i64,
}

fn range_where(alias: &str) -> String {
    let prefix = if alias.is_empty() {
        String::new()
    } else {
        format!("{alias}.")
    };
    format!(
        "{prefix}deleted_at IS NULL \
         AND {prefix}submitted_at >= ?1 AND {prefix}submitted_at <= ?2 \
         AND (?3 IS NULL OR {prefix}created_by_user_id = ?3) \
         AND (?4 IS NULL OR {prefix}group_id = ?4) \
         AND (?5 = '' OR {prefix}type_key = ?5) \
         AND (COALESCE(current_setting('workload.export_include_pending_ownership',true),'false')='true' \
              OR COALESCE({prefix}ownership_status,'confirmed')<>'pending_confirmation') \
         AND (NULLIF(current_setting('workload.export_division_id',true),'') IS NULL \
              OR CAST(current_setting('workload.export_division_id',true) AS BIGINT) = 0 \
              OR CASE COALESCE(current_setting('workload.export_ownership_basis',true),'submitted') \
                   WHEN 'project' THEN {prefix}project_division_id \
                   WHEN 'execution' THEN {prefix}execution_division_id \
                   ELSE {prefix}submitted_division_id \
                 END = CAST(current_setting('workload.export_division_id',true) AS BIGINT))"
    )
}

/// Sheet 1: 全部记录明细
pub fn query_detail(
    conn: &Connection,
    start: &str,
    end: &str,
    subject_user_id: Option<i64>,
    scope_group_id: Option<i64>,
    type_key: Option<&str>,
) -> Result<Vec<SampleInfoExportRow>> {
    let wc = range_where("sir");
    let end_closed = super::export_data::end_of_day_bound(end);
    let mut stmt = conn.prepare(&format!(
        "SELECT sir.seq_no, sir.batch_no, sir.user_name, sir.lab_name, sir.project_name, \
                sir.submitted_at, sir.detection_date, sir.sampled_by, COALESCE(sir.sampled_at, ''), sir.detected_by, \
                COALESCE(sit.label, sir.detection_type), sir.status, sir.main_components, sir.notes, sir.extra_fields, \
                sir.type_key, sir.division_id, sir.created_by_user_id \
         FROM sample_info_records sir \
         LEFT JOIN sample_info_types sit ON sit.type_key = sir.type_key \
         WHERE {} ORDER BY sir.created_at DESC",
        wc
    ))?;
    let rows = stmt.query_map(
        postgres_compat::params![
            start,
            end_closed,
            subject_user_id,
            scope_group_id,
            type_key.unwrap_or("")
        ],
        |row| {
            Ok(SampleInfoExportRow {
                seq_no: row.get(0)?,
                batch_no: row.get(1)?,
                user_name: row.get(2)?,
                lab_name: row.get(3)?,
                project_name: row.get(4)?,
                submitted_at: row.get(5)?,
                detection_date: row.get::<_, String>(6).unwrap_or_default(),
                sampled_by: row.get::<_, String>(7).unwrap_or_default(),
                sampled_at: row.get::<_, String>(8).unwrap_or_default(),
                detected_by: row.get::<_, String>(9).unwrap_or_default(),
                detection_type: row.get::<_, String>(10).unwrap_or_default(),
                status: row.get(11)?,
                main_components: row.get(12)?,
                notes: row.get::<_, String>(13).unwrap_or_default(),
                extra_fields: row
                    .get::<_, Option<String>>(14)
                    .unwrap_or(Some("{}".into())),
                type_key: row.get::<_, String>(15).unwrap_or_default(),
                division_id: row.get::<_, Option<i64>>(16).unwrap_or(None),
                created_by_user_id: row.get::<_, Option<i64>>(17).unwrap_or(None),
            })
        },
    )?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

/// Enforces the same per-role department/type semantics used by sample-info list and
/// statistics. Each configured role is evaluated independently, then the permitted
/// record sets are unioned; this prevents a cross-product between two different roles.
pub fn filter_by_role_scopes(
    rows: Vec<SampleInfoExportRow>,
    scopes: &[SampleInfoScopeFilter],
) -> Vec<SampleInfoExportRow> {
    if scopes.is_empty() {
        return rows;
    }
    rows.into_iter()
        .filter(|row| {
            scopes.iter().any(|scope| {
                if let Some(owner_id) = scope.created_by_user_id {
                    return row.created_by_user_id == Some(owner_id);
                }
                let division_matches = scope.division_ids.is_empty()
                    || row
                        .division_id
                        .is_some_and(|id| scope.division_ids.contains(&id));
                let type_matches = scope.type_keys.is_empty()
                    || scope.type_keys.iter().any(|key| key == &row.type_key);
                division_matches && type_matches
            })
        })
        .collect()
}

fn sorted_counts(values: std::collections::HashMap<String, i64>) -> Vec<NameCountRow> {
    let mut rows: Vec<NameCountRow> = values
        .into_iter()
        .map(|(name, count)| NameCountRow { name, count })
        .collect();
    rows.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));
    rows
}

pub fn summarize_by_status(rows: &[SampleInfoExportRow]) -> Vec<NameCountRow> {
    let mut counts = std::collections::HashMap::new();
    for row in rows {
        *counts.entry(row.status.clone()).or_insert(0) += 1;
    }
    sorted_counts(counts)
}

pub fn summarize_by_type(rows: &[SampleInfoExportRow]) -> Vec<TypeCountRow> {
    let mut counts = std::collections::HashMap::<(String, String), i64>::new();
    for row in rows {
        *counts
            .entry((row.detection_type.clone(), row.type_key.clone()))
            .or_insert(0) += 1;
    }
    let mut result: Vec<TypeCountRow> = counts
        .into_iter()
        .map(|((label, type_key), count)| TypeCountRow {
            label,
            type_key,
            count,
        })
        .collect();
    result.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.label.cmp(&b.label)));
    result
}

pub fn summarize_by_lab(rows: &[SampleInfoExportRow]) -> Vec<NameCountRow> {
    let mut counts = std::collections::HashMap::new();
    for row in rows {
        *counts.entry(row.lab_name.clone()).or_insert(0) += 1;
    }
    sorted_counts(counts)
}

pub fn summarize_by_project(rows: &[SampleInfoExportRow]) -> Vec<NameCountRow> {
    let mut counts = std::collections::HashMap::new();
    for row in rows {
        *counts.entry(row.project_name.clone()).or_insert(0) += 1;
    }
    sorted_counts(counts)
}

pub fn summarize_by_user(rows: &[SampleInfoExportRow]) -> Vec<NameCountRow> {
    let mut counts = std::collections::HashMap::new();
    for row in rows {
        *counts.entry(row.user_name.clone()).or_insert(0) += 1;
    }
    sorted_counts(counts)
}

pub fn summarize_by_month(rows: &[SampleInfoExportRow]) -> Vec<MonthCountRow> {
    let mut counts = std::collections::BTreeMap::new();
    for row in rows {
        let month = row.submitted_at.get(..7).unwrap_or_default().to_string();
        *counts.entry(month).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .map(|(month, count)| MonthCountRow { month, count })
        .collect()
}

/// Sheet 2: 按状态
pub fn query_by_status(
    conn: &Connection,
    start: &str,
    end: &str,
    subject_user_id: Option<i64>,
    scope_group_id: Option<i64>,
    type_key: Option<&str>,
) -> Result<Vec<NameCountRow>> {
    let wc = range_where("");
    let end_closed = super::export_data::end_of_day_bound(end);
    let mut stmt = conn.prepare(&format!(
        "SELECT status, COUNT(*) FROM sample_info_records WHERE {} GROUP BY status ORDER BY COUNT(*) DESC",
        wc
    ))?;
    let rows = stmt.query_map(
        postgres_compat::params![
            start,
            end_closed,
            subject_user_id,
            scope_group_id,
            type_key.unwrap_or("")
        ],
        |row| {
            Ok(NameCountRow {
                name: row.get(0)?,
                count: row.get(1)?,
            })
        },
    )?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

/// Sheet 3: 按检测类型
pub fn query_by_type(
    conn: &Connection,
    start: &str,
    end: &str,
    subject_user_id: Option<i64>,
    scope_group_id: Option<i64>,
    type_key: Option<&str>,
) -> Result<Vec<TypeCountRow>> {
    let wc = range_where("sir");
    let end_closed = super::export_data::end_of_day_bound(end);
    let mut stmt = conn.prepare(&format!(
        "SELECT COALESCE(sit.label, sir.detection_type) AS t, sir.type_key, COUNT(*) \
         FROM sample_info_records sir LEFT JOIN sample_info_types sit ON sit.type_key = sir.type_key \
         WHERE {} GROUP BY COALESCE(sit.label, sir.detection_type), sir.type_key ORDER BY COUNT(*) DESC",
        wc
    ))?;
    let rows = stmt.query_map(
        postgres_compat::params![
            start,
            end_closed,
            subject_user_id,
            scope_group_id,
            type_key.unwrap_or("")
        ],
        |row| {
            Ok(TypeCountRow {
                label: row.get(0)?,
                type_key: row.get::<_, String>(1).unwrap_or_default(),
                count: row.get(2)?,
            })
        },
    )?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

/// Sheet 4: 按实验室
pub fn query_by_lab(
    conn: &Connection,
    start: &str,
    end: &str,
    subject_user_id: Option<i64>,
    scope_group_id: Option<i64>,
    type_key: Option<&str>,
) -> Result<Vec<NameCountRow>> {
    let wc = range_where("");
    let end_closed = super::export_data::end_of_day_bound(end);
    let mut stmt = conn.prepare(&format!(
        "SELECT lab_name, COUNT(*) FROM sample_info_records WHERE {} GROUP BY lab_name ORDER BY COUNT(*) DESC",
        wc
    ))?;
    let rows = stmt.query_map(
        postgres_compat::params![
            start,
            end_closed,
            subject_user_id,
            scope_group_id,
            type_key.unwrap_or("")
        ],
        |row| {
            Ok(NameCountRow {
                name: row.get(0)?,
                count: row.get(1)?,
            })
        },
    )?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

/// Sheet 5: 按项目
pub fn query_by_project(
    conn: &Connection,
    start: &str,
    end: &str,
    subject_user_id: Option<i64>,
    scope_group_id: Option<i64>,
    type_key: Option<&str>,
) -> Result<Vec<NameCountRow>> {
    let wc = range_where("");
    let end_closed = super::export_data::end_of_day_bound(end);
    let mut stmt = conn.prepare(&format!(
        "SELECT project_name, COUNT(*) FROM sample_info_records WHERE {} GROUP BY project_name ORDER BY COUNT(*) DESC",
        wc
    ))?;
    let rows = stmt.query_map(
        postgres_compat::params![
            start,
            end_closed,
            subject_user_id,
            scope_group_id,
            type_key.unwrap_or("")
        ],
        |row| {
            Ok(NameCountRow {
                name: row.get(0)?,
                count: row.get(1)?,
            })
        },
    )?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

/// Sheet 6: 按送样人
pub fn query_by_user(
    conn: &Connection,
    start: &str,
    end: &str,
    subject_user_id: Option<i64>,
    scope_group_id: Option<i64>,
    type_key: Option<&str>,
) -> Result<Vec<NameCountRow>> {
    let wc = range_where("");
    let end_closed = super::export_data::end_of_day_bound(end);
    let mut stmt = conn.prepare(&format!(
        "SELECT user_name, COUNT(*) FROM sample_info_records WHERE {} GROUP BY user_name ORDER BY COUNT(*) DESC",
        wc
    ))?;
    let rows = stmt.query_map(
        postgres_compat::params![
            start,
            end_closed,
            subject_user_id,
            scope_group_id,
            type_key.unwrap_or("")
        ],
        |row| {
            Ok(NameCountRow {
                name: row.get(0)?,
                count: row.get(1)?,
            })
        },
    )?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

/// Sheet 7: 按月份
pub fn query_by_month(
    conn: &Connection,
    start: &str,
    end: &str,
    subject_user_id: Option<i64>,
    scope_group_id: Option<i64>,
    type_key: Option<&str>,
) -> Result<Vec<MonthCountRow>> {
    let wc = range_where("");
    let end_closed = super::export_data::end_of_day_bound(end);
    let mut stmt = conn.prepare(&format!(
        "SELECT LEFT(submitted_at, 7) AS m, COUNT(*) FROM sample_info_records WHERE {} GROUP BY LEFT(submitted_at, 7) ORDER BY LEFT(submitted_at, 7) ASC",
        wc
    ))?;
    let rows = stmt.query_map(
        postgres_compat::params![
            start,
            end_closed,
            subject_user_id,
            scope_group_id,
            type_key.unwrap_or("")
        ],
        |row| {
            Ok(MonthCountRow {
                month: row.get::<_, String>(0).unwrap_or_default(),
                count: row.get(1)?,
            })
        },
    )?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_connection() -> Connection {
        let conn = Connection::open_test_database().unwrap();
        conn.execute_batch(
            "CREATE TABLE sample_info_types (type_key TEXT PRIMARY KEY, label TEXT);
             CREATE TABLE sample_info_records (
                 id INTEGER PRIMARY KEY,
                 seq_no INTEGER,
                 batch_no TEXT,
                 user_name TEXT,
                 lab_name TEXT,
                 project_name TEXT,
                 submitted_at TEXT,
                 detection_date TEXT,
                 sampled_by TEXT,
                 sampled_at TEXT,
                 detected_by TEXT,
                 detection_type TEXT,
                 type_key TEXT,
                 status TEXT,
                 main_components TEXT,
                 notes TEXT,
                 extra_fields TEXT,
                 deleted_at TEXT,
                 created_at TEXT,
                 created_by_user_id INTEGER,
                 group_id INTEGER,
                 division_id INTEGER
             );
             INSERT INTO sample_info_types VALUES ('normal', 'Normal');
             INSERT INTO sample_info_records
                 (id,seq_no,batch_no,user_name,lab_name,project_name,submitted_at,
                  detection_date,sampled_by,sampled_at,detected_by,detection_type,
                  type_key,status,main_components,notes,extra_fields,deleted_at,
                  created_at,created_by_user_id,group_id,division_id)
             VALUES
                 (1,1,'B01','user10','Lab01','Project01','2026-07-17T09:00:00',
                  '','','','','Normal','normal','Pending','A','','{}',NULL,
                  '2026-07-17T09:00:00',10,1,1),
                 (2,2,'B02','user11','Lab01','Project01','2026-07-17T10:00:00',
                  '','','','','Normal','normal','Pending','B','','{}',NULL,
                  '2026-07-17T10:00:00',11,1,1),
                 (3,3,'B03','user12','Lab02','Project02','2026-07-17T11:00:00',
                  '','','','','Normal','normal','Done','C','','{}',NULL,
                  '2026-07-17T11:00:00',12,2,2),
                 (4,4,'B04','user10','Lab01','Project01','2026-07-17T12:00:00',
                  '','','','','Normal','normal','Pending','D','','{}','2026-07-18',
                  '2026-07-17T12:00:00',10,1,1);",
        )
        .unwrap();
        conn.execute_batch(
            "ALTER TABLE sample_info_records
             ADD COLUMN ownership_status TEXT NOT NULL DEFAULT 'confirmed';
             ALTER TABLE sample_info_records ADD COLUMN project_division_id BIGINT;
             ALTER TABLE sample_info_records ADD COLUMN execution_division_id BIGINT;
             ALTER TABLE sample_info_records ADD COLUMN submitted_division_id BIGINT",
        )
        .unwrap();
        conn
    }

    #[test]
    fn export_scope_filters_own_lab_and_deleted_records() {
        let conn = test_connection();
        assert_eq!(
            query_detail(&conn, "2026-07-17", "2026-07-17", None, None, None)
                .unwrap()
                .len(),
            3
        );
        assert_eq!(
            query_detail(&conn, "2026-07-17", "2026-07-17", Some(10), None, None)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            query_detail(&conn, "2026-07-17", "2026-07-17", None, Some(1), None)
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            query_by_status(&conn, "2026-07-17", "2026-07-17", None, Some(1), None).unwrap()[0]
                .count,
            2
        );
        assert_eq!(
            query_by_type(&conn, "2026-07-17", "2026-07-17", Some(12), None, None).unwrap()[0]
                .count,
            1
        );
        assert_eq!(
            query_by_lab(&conn, "2026-07-17", "2026-07-17", Some(10), None, None).unwrap()[0].name,
            "Lab01"
        );
        assert_eq!(
            query_by_project(&conn, "2026-07-17", "2026-07-17", None, Some(2), None).unwrap()[0]
                .name,
            "Project02"
        );
        assert_eq!(
            query_by_user(&conn, "2026-07-17", "2026-07-17", None, Some(1), None)
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            query_by_month(&conn, "2026-07-17", "2026-07-17", Some(11), None, None).unwrap()[0]
                .count,
            1
        );
    }

    #[test]
    fn export_dates_are_bound_parameters() {
        let conn = test_connection();
        let rows = query_detail(
            &conn,
            "9999-12-31' OR 1=1 --",
            "2026-07-17",
            None,
            None,
            None,
        )
        .unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn export_type_filter_applies_to_detail_and_summaries() {
        let conn = test_connection();
        assert_eq!(
            query_detail(
                &conn,
                "2026-07-17",
                "2026-07-17",
                None,
                None,
                Some("normal")
            )
            .unwrap()
            .len(),
            3
        );
        assert!(query_by_status(
            &conn,
            "2026-07-17",
            "2026-07-17",
            None,
            None,
            Some("missing")
        )
        .unwrap()
        .is_empty());
    }

    #[test]
    fn role_scopes_keep_department_type_pairs_and_own_records() {
        let rows = query_detail(
            &test_connection(),
            "2026-07-17",
            "2026-07-17",
            None,
            None,
            None,
        )
        .unwrap();
        let scoped = filter_by_role_scopes(
            rows,
            &[
                SampleInfoScopeFilter {
                    division_ids: vec![1],
                    type_keys: vec!["normal".into()],
                    created_by_user_id: None,
                    business_user_id: None,
                },
                SampleInfoScopeFilter {
                    division_ids: vec![],
                    type_keys: vec![],
                    created_by_user_id: Some(12),
                    business_user_id: None,
                },
            ],
        );
        assert_eq!(scoped.len(), 3);
        assert!(scoped.iter().any(|row| row.created_by_user_id == Some(12)));
        assert!(scoped
            .iter()
            .all(|row| row.division_id == Some(1) || row.created_by_user_id == Some(12)));
    }
}
