use crate::error::Result;
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
         AND (?4 IS NULL OR {prefix}group_id = ?4)"
    )
}

/// Sheet 1: 全部记录明细
pub fn query_detail(
    conn: &Connection,
    start: &str,
    end: &str,
    subject_user_id: Option<i64>,
    scope_group_id: Option<i64>,
) -> Result<Vec<SampleInfoExportRow>> {
    let wc = range_where("sir");
    let end_closed = format!("{end}T23:59:59");
    let mut stmt = conn.prepare(&format!(
        "SELECT sir.seq_no, sir.batch_no, sir.user_name, sir.lab_name, sir.project_name, \
                sir.submitted_at, sir.detection_date, sir.sampled_by, COALESCE(sir.sampled_at, ''), sir.detected_by, \
                COALESCE(sit.label, sir.detection_type), sir.status, sir.main_components, sir.notes, sir.extra_fields \
         FROM sample_info_records sir \
         LEFT JOIN sample_info_types sit ON sit.type_key = sir.type_key \
         WHERE {} ORDER BY sir.created_at DESC",
        wc
    ))?;
    let rows = stmt.query_map(
        postgres_compat::params![start, end_closed, subject_user_id, scope_group_id],
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
            })
        },
    )?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

/// Sheet 2: 按状态
pub fn query_by_status(
    conn: &Connection,
    start: &str,
    end: &str,
    subject_user_id: Option<i64>,
    scope_group_id: Option<i64>,
) -> Result<Vec<NameCountRow>> {
    let wc = range_where("");
    let end_closed = format!("{end}T23:59:59");
    let mut stmt = conn.prepare(&format!(
        "SELECT status, COUNT(*) FROM sample_info_records WHERE {} GROUP BY status ORDER BY COUNT(*) DESC",
        wc
    ))?;
    let rows = stmt.query_map(
        postgres_compat::params![start, end_closed, subject_user_id, scope_group_id],
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
) -> Result<Vec<TypeCountRow>> {
    let wc = range_where("sir");
    let end_closed = format!("{end}T23:59:59");
    let mut stmt = conn.prepare(&format!(
        "SELECT COALESCE(sit.label, sir.detection_type) AS t, sir.type_key, COUNT(*) \
         FROM sample_info_records sir LEFT JOIN sample_info_types sit ON sit.type_key = sir.type_key \
         WHERE {} GROUP BY COALESCE(sit.label, sir.detection_type), sir.type_key ORDER BY COUNT(*) DESC",
        wc
    ))?;
    let rows = stmt.query_map(
        postgres_compat::params![start, end_closed, subject_user_id, scope_group_id],
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
) -> Result<Vec<NameCountRow>> {
    let wc = range_where("");
    let end_closed = format!("{end}T23:59:59");
    let mut stmt = conn.prepare(&format!(
        "SELECT lab_name, COUNT(*) FROM sample_info_records WHERE {} GROUP BY lab_name ORDER BY COUNT(*) DESC",
        wc
    ))?;
    let rows = stmt.query_map(
        postgres_compat::params![start, end_closed, subject_user_id, scope_group_id],
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
) -> Result<Vec<NameCountRow>> {
    let wc = range_where("");
    let end_closed = format!("{end}T23:59:59");
    let mut stmt = conn.prepare(&format!(
        "SELECT project_name, COUNT(*) FROM sample_info_records WHERE {} GROUP BY project_name ORDER BY COUNT(*) DESC",
        wc
    ))?;
    let rows = stmt.query_map(
        postgres_compat::params![start, end_closed, subject_user_id, scope_group_id],
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
) -> Result<Vec<NameCountRow>> {
    let wc = range_where("");
    let end_closed = format!("{end}T23:59:59");
    let mut stmt = conn.prepare(&format!(
        "SELECT user_name, COUNT(*) FROM sample_info_records WHERE {} GROUP BY user_name ORDER BY COUNT(*) DESC",
        wc
    ))?;
    let rows = stmt.query_map(
        postgres_compat::params![start, end_closed, subject_user_id, scope_group_id],
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
) -> Result<Vec<MonthCountRow>> {
    let wc = range_where("");
    let end_closed = format!("{end}T23:59:59");
    let mut stmt = conn.prepare(&format!(
        "SELECT LEFT(submitted_at, 7) AS m, COUNT(*) FROM sample_info_records WHERE {} GROUP BY LEFT(submitted_at, 7) ORDER BY LEFT(submitted_at, 7) ASC",
        wc
    ))?;
    let rows = stmt.query_map(
        postgres_compat::params![start, end_closed, subject_user_id, scope_group_id],
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
                 group_id INTEGER
             );
             INSERT INTO sample_info_types VALUES ('normal', 'Normal');
             INSERT INTO sample_info_records
                 (id,seq_no,batch_no,user_name,lab_name,project_name,submitted_at,
                  detection_date,sampled_by,sampled_at,detected_by,detection_type,
                  type_key,status,main_components,notes,extra_fields,deleted_at,
                  created_at,created_by_user_id,group_id)
             VALUES
                 (1,1,'B01','user10','Lab01','Project01','2026-07-17T09:00:00',
                  '','','','','Normal','normal','Pending','A','','{}',NULL,
                  '2026-07-17T09:00:00',10,1),
                 (2,2,'B02','user11','Lab01','Project01','2026-07-17T10:00:00',
                  '','','','','Normal','normal','Pending','B','','{}',NULL,
                  '2026-07-17T10:00:00',11,1),
                 (3,3,'B03','user12','Lab02','Project02','2026-07-17T11:00:00',
                  '','','','','Normal','normal','Done','C','','{}',NULL,
                  '2026-07-17T11:00:00',12,2),
                 (4,4,'B04','user10','Lab01','Project01','2026-07-17T12:00:00',
                  '','','','','Normal','normal','Pending','D','','{}','2026-07-18',
                  '2026-07-17T12:00:00',10,1);",
        )
        .unwrap();
        conn
    }

    #[test]
    fn export_scope_filters_own_lab_and_deleted_records() {
        let conn = test_connection();
        assert_eq!(
            query_detail(&conn, "2026-07-17", "2026-07-17", None, None)
                .unwrap()
                .len(),
            3
        );
        assert_eq!(
            query_detail(&conn, "2026-07-17", "2026-07-17", Some(10), None)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            query_detail(&conn, "2026-07-17", "2026-07-17", None, Some(1))
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            query_by_status(&conn, "2026-07-17", "2026-07-17", None, Some(1)).unwrap()[0].count,
            2
        );
        assert_eq!(
            query_by_type(&conn, "2026-07-17", "2026-07-17", Some(12), None).unwrap()[0].count,
            1
        );
        assert_eq!(
            query_by_lab(&conn, "2026-07-17", "2026-07-17", Some(10), None).unwrap()[0].name,
            "Lab01"
        );
        assert_eq!(
            query_by_project(&conn, "2026-07-17", "2026-07-17", None, Some(2)).unwrap()[0].name,
            "Project02"
        );
        assert_eq!(
            query_by_user(&conn, "2026-07-17", "2026-07-17", None, Some(1))
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            query_by_month(&conn, "2026-07-17", "2026-07-17", Some(11), None).unwrap()[0].count,
            1
        );
    }

    #[test]
    fn export_dates_are_bound_parameters() {
        let conn = test_connection();
        let rows = query_detail(&conn, "9999-12-31' OR 1=1 --", "2026-07-17", None, None).unwrap();
        assert!(rows.is_empty());
    }
}
