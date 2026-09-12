use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::import::ImportMapping;
use crate::models::method::{MethodCreate, MethodResponse, MethodUpdate};
use crate::models::project::{
    ImportSummary, MethodImportItem, MethodTypeVisibility, MethodTypeVisibilityUpdate, TypeCount,
};
use crate::repo::{audit_repo, trash_repo};
use postgres_compat::Connection;
use std::collections::BTreeMap;

// ── helpers ──

fn parse_comma_i64(s: &str) -> Vec<i64> {
    if s.is_empty() {
        return vec![];
    }
    s.split(',').filter_map(|x| x.trim().parse().ok()).collect()
}

fn parse_comma_str(s: &str) -> Vec<String> {
    if s.is_empty() {
        return vec![];
    }
    s.split(',').map(|x| x.trim().to_string()).collect()
}

const METHOD_SQL: &str =
    "SELECT m.id, COALESCE(m.method_code,''), m.name, COALESCE(m.full_name,''), COALESCE(m.coefficient,1.0), \
     COALESCE(m.amount,0.0), COALESCE(m.multiplier,1.0), \
     COALESCE(m.notes,''), m.is_active, \
     COALESCE((SELECT string_agg(DISTINCT mtl.method_type_id::text, ',') \
               FROM method_type_links mtl WHERE mtl.method_id=m.id), '') as type_ids_str, \
     COALESCE((SELECT string_agg(DISTINCT mt.name, ',') \
               FROM method_type_links mtl JOIN method_types mt ON mt.id=mtl.method_type_id \
               WHERE mtl.method_id=m.id), '') as type_names_str, \
     COALESCE((SELECT string_agg(DISTINCT cmds.division_id::text, ',') \
               FROM common_method_division_scopes cmds WHERE cmds.method_id=m.id), '') as common_division_ids_str, \
     m.instrument_id, COALESCE(i.code,''), COALESCE(i.name,''), COALESCE(i.instrument_type,''), \
     m.created_at, m.show_in_work, m.show_in_rd, m.show_in_sample_info, m.is_common \
     FROM methods m \
     LEFT JOIN instruments i ON i.id=m.instrument_id";

fn row_to_method(row: &postgres_compat::Row) -> postgres_compat::Result<MethodResponse> {
    let type_ids_str: String = row.get::<_, String>(9).unwrap_or_default();
    let type_names_str: String = row.get::<_, String>(10).unwrap_or_default();
    let common_division_ids_str: String = row.get::<_, String>(11).unwrap_or_default();
    Ok(MethodResponse {
        id: row.get(0)?,
        method_code: row.get(1)?,
        name: row.get(2)?,
        full_name: row.get(3)?,
        coefficient: row.get::<_, f64>(4).unwrap_or(1.0),
        amount: row.get::<_, f64>(5).unwrap_or(0.0),
        multiplier: row.get::<_, f64>(6).unwrap_or(1.0),
        notes: row.get(7)?,
        is_active: row.get::<_, i64>(8).unwrap_or(1) != 0,
        type_ids: parse_comma_i64(&type_ids_str),
        type_names: parse_comma_str(&type_names_str),
        common_division_ids: parse_comma_i64(&common_division_ids_str),
        instrument_id: row.get(12)?,
        instrument_code: row.get(13)?,
        instrument_name: row.get(14)?,
        instrument_type: row.get(15)?,
        created_at: row.get(16)?,
        show_in_work: row.get::<_, bool>(17).unwrap_or(true),
        show_in_rd: row.get::<_, bool>(18).unwrap_or(true),
        show_in_sample_info: row.get::<_, bool>(19).unwrap_or(true),
        is_common: row.get::<_, bool>(20).unwrap_or(false),
    })
}

fn normalize_common_division_ids(ids: Option<&Vec<i64>>) -> Vec<i64> {
    let mut ids = ids.cloned().unwrap_or_default();
    ids.sort_unstable();
    ids.dedup();
    ids
}

fn normalize_method_type_ids(ids: Option<&Vec<i64>>) -> Result<Vec<i64>> {
    let mut normalized = ids.cloned().unwrap_or_default();
    normalized.retain(|id| *id > 0);
    normalized.sort_unstable();
    normalized.dedup();
    if normalized.len() > 1 {
        return Err(AppError::Validation("每个方法只能关联一个检测类型".into()));
    }
    Ok(normalized)
}

fn replace_common_division_scopes(
    conn: &Connection,
    method_id: i64,
    division_ids: &[i64],
) -> Result<()> {
    for division_id in division_ids {
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM divisions WHERE id=?1 AND deleted_at IS NULL AND is_active=1",
            [*division_id],
            |row| row.get(0),
        )?;
        if exists == 0 {
            return Err(AppError::Validation(format!(
                "通用方法显示部门不存在或已停用: {division_id}"
            )));
        }
    }
    conn.execute(
        "DELETE FROM common_method_division_scopes WHERE method_id=?1",
        [method_id],
    )?;
    for division_id in division_ids {
        conn.execute(
            "INSERT INTO common_method_division_scopes(method_id,division_id) VALUES(?1,?2)",
            postgres_compat::params![method_id, division_id],
        )?;
    }
    Ok(())
}

fn validate_identity(
    conn: &Connection,
    name: &str,
    instrument_id: i64,
    exclude_id: Option<i64>,
) -> Result<String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::Validation("方法名称不能为空".into()));
    }
    if name.contains('@') {
        return Err(AppError::Validation(
            "方法名称不能包含 @ 仪器识别字符".into(),
        ));
    }
    let instrument_exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM instruments WHERE id=?1 AND is_active=1",
        [instrument_id],
        |r| r.get(0),
    )?;
    if instrument_exists == 0 {
        return Err(AppError::Validation("请选择有效仪器".into()));
    }
    let duplicate_pair: i64 = conn.query_row(
        "SELECT COUNT(*) FROM methods WHERE name=?1 AND instrument_id=?2 AND (CAST(?3 AS BIGINT) IS NULL OR id<>CAST(?3 AS BIGINT))",
        postgres_compat::params![name, instrument_id, exclude_id], |r| r.get(0),
    )?;
    if duplicate_pair > 0 {
        return Err(AppError::Validation(
            "相同方法名称与仪器的实例已存在".into(),
        ));
    }
    Ok(name)
}

// ── CRUD ──

pub fn list(
    pool: &DbPool,
    type_filter: Option<i64>,
    portal: Option<&str>,
    group_id: Option<i64>,
) -> Result<Vec<MethodResponse>> {
    let conn = pool.get()?;
    let mut sql = format!(
        "{} WHERE m.deleted_at IS NULL AND (i.id IS NULL OR i.deleted_at IS NULL)",
        METHOD_SQL
    );
    let mut params: Vec<Box<dyn postgres_compat::types::ToSql>> = vec![];
    if let Some(tid) = type_filter {
        if tid > 0 {
            sql.push_str(" AND EXISTS (SELECT 1 FROM method_type_links filter_link WHERE filter_link.method_id=m.id AND filter_link.method_type_id=?1)");
            params.push(Box::new(tid));
        }
    }
    match portal {
        Some("work") => sql.push_str(" AND m.show_in_work=1 AND EXISTS (SELECT 1 FROM project_method_links pv_pml JOIN projects pv_p ON pv_p.id=pv_pml.project_id JOIN project_lab_links pv_pll ON pv_pll.project_id=pv_p.id JOIN project_groups pv_g ON pv_g.id=pv_pll.group_id LEFT JOIN divisions pv_d ON pv_d.id=pv_g.division_id WHERE pv_pml.method_id=m.id AND pv_p.deleted_at IS NULL AND pv_p.show_in_work=1 AND pv_g.deleted_at IS NULL AND pv_g.show_in_work=1 AND (pv_g.division_id IS NULL OR (pv_d.deleted_at IS NULL AND pv_d.show_in_work=1)))"),
        Some("rd") => sql.push_str(" AND m.show_in_rd=1 AND EXISTS (SELECT 1 FROM project_method_links pv_pml JOIN projects pv_p ON pv_p.id=pv_pml.project_id JOIN project_lab_links pv_pll ON pv_pll.project_id=pv_p.id JOIN project_groups pv_g ON pv_g.id=pv_pll.group_id LEFT JOIN divisions pv_d ON pv_d.id=pv_g.division_id WHERE pv_pml.method_id=m.id AND pv_p.deleted_at IS NULL AND pv_p.show_in_rd=1 AND pv_g.deleted_at IS NULL AND pv_g.show_in_rd=1 AND (pv_g.division_id IS NULL OR (pv_d.deleted_at IS NULL AND pv_d.show_in_rd=1)))"),
        Some("sample_info") => sql.push_str(" AND m.show_in_sample_info=1 AND EXISTS (SELECT 1 FROM project_method_links pv_pml JOIN projects pv_p ON pv_p.id=pv_pml.project_id JOIN project_lab_links pv_pll ON pv_pll.project_id=pv_p.id JOIN project_groups pv_g ON pv_g.id=pv_pll.group_id LEFT JOIN divisions pv_d ON pv_d.id=pv_g.division_id WHERE pv_pml.method_id=m.id AND pv_p.deleted_at IS NULL AND pv_p.show_in_sample_info=1 AND pv_g.deleted_at IS NULL AND pv_g.show_in_sample_info=1 AND (pv_g.division_id IS NULL OR (pv_d.deleted_at IS NULL AND pv_d.show_in_sample_info=1)))"),
        _ => {}
    }
    if let (Some(portal), Some(group_id)) = (portal, group_id) {
        let group_idx = params.len() + 1;
        params.push(Box::new(group_id));
        let portal_idx = params.len() + 1;
        params.push(Box::new(portal.to_string()));
        sql.push_str(&format!(" AND NOT EXISTS (SELECT 1 FROM method_type_links vis_link JOIN method_type_visibility vis ON vis.method_type_id=vis_link.method_type_id WHERE vis_link.method_id=m.id AND vis.group_id=?{group_idx} AND vis.portal=?{portal_idx} AND vis.is_visible=0)"));
    }
    sql.push_str(" ORDER BY m.id");
    let mut stmt = conn.prepare(&sql)?;
    let refs: Vec<&dyn postgres_compat::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let rows: Vec<MethodResponse> = stmt
        .query_map(postgres_compat::params_from_iter(refs.iter()), |row| {
            row_to_method(row)
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn get_by_id(pool: &DbPool, id: i64) -> Result<MethodResponse> {
    let conn = pool.get()?;
    get_by_id_on_conn(&conn, id)
}

fn get_by_id_on_conn(conn: &Connection, id: i64) -> Result<MethodResponse> {
    let sql = format!("{} WHERE m.id=?1", METHOD_SQL);
    conn.query_row(&sql, [id], |row| row_to_method(row))
        .map_err(|e| AppError::NotFound(format!("方法不存在: {}", e)))
}

pub fn create(pool: &DbPool, body: &MethodCreate, operator: &str) -> Result<MethodResponse> {
    let mut conn = pool.get()?;
    let name = validate_identity(&conn, &body.name, body.instrument_id, None)?;
    let type_ids = normalize_method_type_ids(body.type_ids.as_ref())?;
    let fn_ = body.full_name.as_deref().unwrap_or("");
    let cf = body.coefficient.unwrap_or(1.0);
    let amt = body.amount.unwrap_or(0.0);
    let mul = body.multiplier.unwrap_or(1.0);
    let nt = body.notes.as_deref().unwrap_or("");
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO methods (method_code,name,full_name,coefficient,amount,multiplier,notes,instrument_id,show_in_work,show_in_rd,show_in_sample_info,is_common) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
        postgres_compat::params!["",name,fn_,cf,amt,mul,nt,body.instrument_id,body.show_in_work.unwrap_or(true),body.show_in_rd.unwrap_or(true),body.show_in_sample_info.unwrap_or(true),body.is_common.unwrap_or(false)],
    )?;
    let mid = tx.last_insert_rowid();
    let method_code = format!("M-{mid:08}");
    tx.execute(
        "UPDATE methods SET method_code=?1 WHERE id=?2",
        postgres_compat::params![method_code, mid],
    )?;

    // Insert method_type_links
    for tid in type_ids {
        tx.execute(
            "INSERT OR IGNORE INTO method_type_links (method_id, method_type_id) VALUES (?1,?2)",
            postgres_compat::params![mid, tid],
        )?;
    }
    if body.is_common.unwrap_or(false) {
        tx.execute(
            "INSERT OR IGNORE INTO project_method_links(project_id,method_id) SELECT id,?1 FROM projects WHERE deleted_at IS NULL",
            [mid],
        )?;
        replace_common_division_scopes(
            &tx,
            mid,
            &normalize_common_division_ids(body.common_division_ids.as_ref()),
        )?;
    }

    let created = get_by_id_on_conn(&tx, mid)?;
    let after = serde_json::to_value(&created).unwrap_or(serde_json::Value::Null);
    audit_repo::log_structured_on_conn(
        &tx,
        "create",
        "methods",
        Some(mid),
        operator,
        &format!("创建方法「{}」", body.name),
        "shared",
        &created.method_code,
        None,
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(created)
}

pub fn update(
    pool: &DbPool,
    id: i64,
    body: &MethodUpdate,
    operator: &str,
) -> Result<MethodResponse> {
    let mut conn = pool.get()?;
    let current = get_by_id_on_conn(&conn, id)?;
    let next_name = body.name.as_deref().unwrap_or(&current.name);
    let next_instrument = body
        .instrument_id
        .or(current.instrument_id)
        .ok_or_else(|| AppError::Validation("请选择仪器".into()))?;
    let next_name = validate_identity(&conn, next_name, next_instrument, Some(id))?;
    let next_type_ids = body
        .type_ids
        .as_ref()
        .map(|ids| normalize_method_type_ids(Some(ids)))
        .transpose()?;
    let before = serde_json::to_value(&current).unwrap_or(serde_json::Value::Null);
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE methods SET name=?1,instrument_id=?2 WHERE id=?3",
        postgres_compat::params![next_name, next_instrument, id],
    )?;
    if let Some(ref f) = body.full_name {
        tx.execute("UPDATE methods SET full_name=?1 WHERE id=?2", (f, id))?;
    }
    if let Some(c) = body.coefficient {
        tx.execute("UPDATE methods SET coefficient=?1 WHERE id=?2", (c, id))?;
    }
    if let Some(a) = body.amount {
        tx.execute("UPDATE methods SET amount=?1 WHERE id=?2", (a, id))?;
    }
    if let Some(m) = body.multiplier {
        tx.execute("UPDATE methods SET multiplier=?1 WHERE id=?2", (m, id))?;
    }
    if let Some(ref n) = body.notes {
        tx.execute("UPDATE methods SET notes=?1 WHERE id=?2", (n, id))?;
    }
    if let Some(a) = body.is_active {
        tx.execute(
            "UPDATE methods SET is_active=?1 WHERE id=?2",
            (a as i64, id),
        )?;
    }
    if let Some(value) = body.show_in_work {
        tx.execute(
            "UPDATE methods SET show_in_work=?1 WHERE id=?2",
            (value, id),
        )?;
    }
    if let Some(value) = body.show_in_rd {
        tx.execute("UPDATE methods SET show_in_rd=?1 WHERE id=?2", (value, id))?;
    }
    if let Some(value) = body.show_in_sample_info {
        tx.execute(
            "UPDATE methods SET show_in_sample_info=?1 WHERE id=?2",
            (value, id),
        )?;
    }
    if let Some(value) = body.is_common {
        tx.execute("UPDATE methods SET is_common=?1 WHERE id=?2", (value, id))?;
        if value {
            tx.execute(
                "INSERT OR IGNORE INTO project_method_links(project_id,method_id) SELECT id,?1 FROM projects WHERE deleted_at IS NULL",
                [id],
            )?;
            if let Some(division_ids) = body.common_division_ids.as_ref() {
                replace_common_division_scopes(
                    &tx,
                    id,
                    &normalize_common_division_ids(Some(division_ids)),
                )?;
            }
        } else if current.is_common {
            tx.execute("DELETE FROM project_method_links WHERE method_id=?1", [id])?;
            tx.execute(
                "DELETE FROM common_method_division_scopes WHERE method_id=?1",
                [id],
            )?;
        }
    }
    if current.is_common && body.is_common != Some(false) {
        if let Some(division_ids) = body.common_division_ids.as_ref() {
            replace_common_division_scopes(
                &tx,
                id,
                &normalize_common_division_ids(Some(division_ids)),
            )?;
        }
    }
    // Replace type links
    if let Some(type_ids) = next_type_ids {
        tx.execute("DELETE FROM method_type_links WHERE method_id=?1", [id])?;
        for tid in type_ids {
            tx.execute(
                "INSERT OR IGNORE INTO method_type_links (method_id, method_type_id) VALUES (?1,?2)",
                postgres_compat::params![id, tid],
            )?;
        }
    }
    let updated = get_by_id_on_conn(&tx, id)?;
    let after = serde_json::to_value(&updated).unwrap_or(serde_json::Value::Null);
    audit_repo::log_structured_on_conn(
        &tx,
        "update",
        "methods",
        Some(id),
        operator,
        &format!("编辑方法实例「{}」", current.method_code),
        "shared",
        &current.method_code,
        Some(&before),
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(updated)
}

pub fn delete(pool: &DbPool, id: i64, operator: &str, reason: &str) -> Result<()> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let used: i64 = tx.query_row(
        "SELECT COUNT(*) FROM project_method_links WHERE method_id=?1",
        [id],
        |r| r.get(0),
    )?;
    let m = get_by_id_on_conn(&tx, id)?;
    let work_count: i64 = tx.query_row(
        "SELECT COUNT(*) FROM work_records WHERE method_id=?1",
        [id],
        |r| r.get(0),
    )?;
    let rd_count: i64 = tx.query_row(
        "SELECT COUNT(*) FROM rd_work_records WHERE method_id=?1",
        [id],
        |r| r.get(0),
    )?;
    let before = serde_json::to_value(&m).unwrap_or(serde_json::Value::Null);
    tx.execute("UPDATE methods SET deleted_at=datetime('now','localtime') WHERE id=?1 AND deleted_at IS NULL", [id])?;
    let dependency =
        format!("关联项目 {used} 个；分析记录 {work_count} 条；送样记录 {rd_count} 条");
    trash_repo::move_to_trash_on_conn(
        &tx,
        "检测方法",
        "methods",
        id,
        "master",
        "shared",
        &format!("{} · {}", m.name, m.instrument_code),
        "",
        &before,
        reason,
        operator,
        None,
        None,
        &dependency,
        used + work_count + rd_count == 0,
    )?;
    let after = serde_json::json!({"deleted_at":"now","data":before});
    audit_repo::log_structured_on_conn(
        &tx,
        "delete",
        "methods",
        Some(id),
        operator,
        &format!("删除方法「{}」；{}", m.name, dependency),
        "shared",
        "",
        Some(&before),
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod binding_tests {
    use super::*;
    use crate::models::method::MethodCreate;

    fn create_body(code: &str, name: &str, instrument_id: i64) -> MethodCreate {
        MethodCreate {
            method_code: Some(code.into()),
            name: name.into(),
            full_name: None,
            coefficient: None,
            amount: None,
            multiplier: None,
            notes: None,
            type_ids: None,
            instrument_id,
            show_in_work: None,
            show_in_rd: None,
            show_in_sample_info: None,
            is_common: None,
            common_division_ids: None,
        }
    }

    #[test]
    fn same_name_on_different_instruments_is_a_distinct_method_instance() {
        let pool = crate::db::init_pool("postgres-test");
        crate::db::test_migrations::run(&pool.get().unwrap()).unwrap();
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO instruments(code,name,instrument_type) VALUES('LC-01','LC01','液相')",
            [],
        )
        .unwrap();
        let first_instrument = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO instruments(code,name,instrument_type) VALUES('LC-02','LC02','液相')",
            [],
        )
        .unwrap();
        let second_instrument = conn.last_insert_rowid();
        drop(conn);

        let first = create(
            &pool,
            &create_body("M-LC01-001", "含量测定", first_instrument),
            "test",
        )
        .unwrap();
        let second = create(
            &pool,
            &create_body("M-LC02-001", "含量测定", second_instrument),
            "test",
        )
        .unwrap();
        assert_ne!(first.id, second.id);
        assert_eq!(
            (first.name.as_str(), first.instrument_code.as_str()),
            ("含量测定", "LC-01")
        );
        assert_eq!(
            (second.name.as_str(), second.instrument_code.as_str()),
            ("含量测定", "LC-02")
        );

        let duplicate_pair = create(
            &pool,
            &create_body("M-LC01-002", "含量测定", first_instrument),
            "test",
        );
        assert!(matches!(duplicate_pair, Err(AppError::Validation(_))));
        let encoded_name = create(
            &pool,
            &create_body("M-LC02-002", "含量测定@[LC-02]", second_instrument),
            "test",
        );
        assert!(matches!(encoded_name, Err(AppError::Validation(_))));
    }

    #[test]
    fn method_type_links_reject_multiple_types() {
        let ids = vec![2, 1, 2];
        let result = normalize_method_type_ids(Some(&ids));
        assert!(matches!(result, Err(AppError::Validation(_))));
    }

    #[test]
    fn common_method_links_all_projects_and_cancel_removes_every_link() {
        let pool = crate::db::init_pool("postgres-test");
        crate::db::test_migrations::run(&pool.get().unwrap()).unwrap();
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO project_groups(name) VALUES('实验室-通用方法测试')",
            [],
        )
        .unwrap();
        let group_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO projects(group_id,name,project_status) VALUES(?1,'项目-进行中','ongoing')",
            [group_id],
        )
        .unwrap();
        conn.execute("INSERT INTO projects(group_id,name,project_status) VALUES(?1,'项目-已归档','archived')", [group_id]).unwrap();
        conn.execute("INSERT INTO instruments(code,name,instrument_type) VALUES('COMMON-INS','通用仪器','液相')", []).unwrap();
        let instrument_id = conn.last_insert_rowid();
        drop(conn);

        let mut body = create_body("", "通用方法测试", instrument_id);
        body.is_common = Some(true);
        let created = create(&pool, &body, "tester").unwrap();
        let conn = pool.get().unwrap();
        let linked: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM project_method_links WHERE method_id=?1",
                [created.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(linked, 2, "通用方法必须关联进行中和已归档项目");
        drop(conn);

        update(
            &pool,
            created.id,
            &MethodUpdate {
                method_code: None,
                name: None,
                full_name: None,
                coefficient: None,
                amount: None,
                multiplier: None,
                notes: None,
                is_active: None,
                type_ids: None,
                instrument_id: None,
                show_in_work: None,
                show_in_rd: None,
                show_in_sample_info: None,
                is_common: Some(false),
                common_division_ids: None,
            },
            "tester",
        )
        .unwrap();
        let conn = pool.get().unwrap();
        let linked: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM project_method_links WHERE method_id=?1",
                [created.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(linked, 0, "取消通用必须清除所有项目关联");
    }

    #[test]
    fn common_method_department_scope_is_saved_and_replaced() {
        let pool = crate::db::init_pool("postgres-test");
        crate::db::test_migrations::run(&pool.get().unwrap()).unwrap();
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO divisions(name,sort_order,color,is_active) VALUES('通用方法部门A',1,'#1976d2',1)",
            [],
        )
        .unwrap();
        let division_a = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO divisions(name,sort_order,color,is_active) VALUES('通用方法部门B',2,'#1976d2',1)",
            [],
        )
        .unwrap();
        let division_b = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO instruments(code,name,instrument_type) VALUES('COMMON-SCOPE-INS','范围测试仪器','液相')",
            [],
        )
        .unwrap();
        let instrument_id = conn.last_insert_rowid();
        drop(conn);

        let mut body = create_body("", "通用方法部门范围测试", instrument_id);
        body.is_common = Some(true);
        body.common_division_ids = Some(vec![division_a]);
        let created = create(&pool, &body, "tester").unwrap();
        assert_eq!(created.common_division_ids, vec![division_a]);

        let updated = update(
            &pool,
            created.id,
            &MethodUpdate {
                method_code: None,
                name: None,
                full_name: None,
                coefficient: None,
                amount: None,
                multiplier: None,
                notes: None,
                is_active: None,
                type_ids: None,
                instrument_id: None,
                show_in_work: None,
                show_in_rd: None,
                show_in_sample_info: None,
                is_common: None,
                common_division_ids: Some(vec![division_b]),
            },
            "tester",
        )
        .unwrap();
        assert_eq!(updated.common_division_ids, vec![division_b]);
    }
}

// ── 方法类型 (method_types 表) ──

pub fn list_method_types(
    pool: &DbPool,
    portal: Option<&str>,
    group_id: Option<i64>,
) -> Result<Vec<crate::models::project::MethodType>> {
    use crate::models::project::MethodType;
    let conn = pool.get()?;
    let mut sql = String::from(
        "SELECT mt.id,mt.name,mt.sort_order FROM method_types mt WHERE mt.deleted_at IS NULL",
    );
    let mut params: Vec<Box<dyn postgres_compat::types::ToSql>> = Vec::new();
    if let (Some(portal), Some(group_id)) = (portal, group_id) {
        validate_portal(portal)?;
        params.push(Box::new(group_id));
        params.push(Box::new(portal.to_string()));
        sql.push_str(" AND NOT EXISTS (SELECT 1 FROM method_type_visibility v WHERE v.method_type_id=mt.id AND v.group_id=?1 AND v.portal=?2 AND v.is_visible=0)");
        let portal_clause = match portal {
            "work" => "m.show_in_work=1 AND p.show_in_work=1 AND g.show_in_work=1 AND (g.division_id IS NULL OR (d.deleted_at IS NULL AND d.show_in_work=1))",
            "rd" => "m.show_in_rd=1 AND p.show_in_rd=1 AND g.show_in_rd=1 AND (g.division_id IS NULL OR (d.deleted_at IS NULL AND d.show_in_rd=1))",
            "sample_info" => "m.show_in_sample_info=1 AND p.show_in_sample_info=1 AND g.show_in_sample_info=1 AND (g.division_id IS NULL OR (d.deleted_at IS NULL AND d.show_in_sample_info=1))",
            _ => unreachable!(),
        };
        sql.push_str(&format!(" AND EXISTS (SELECT 1 FROM method_type_links mtl JOIN methods m ON m.id=mtl.method_id JOIN project_method_links pml ON pml.method_id=m.id JOIN projects p ON p.id=pml.project_id JOIN project_lab_links pll ON pll.project_id=p.id JOIN project_groups g ON g.id=pll.group_id LEFT JOIN divisions d ON d.id=g.division_id LEFT JOIN instruments i ON i.id=m.instrument_id WHERE mtl.method_type_id=mt.id AND pll.group_id=?1 AND m.deleted_at IS NULL AND m.is_active=1 AND (i.id IS NULL OR i.deleted_at IS NULL) AND (i.id IS NULL OR i.is_active=1) AND p.deleted_at IS NULL AND g.deleted_at IS NULL AND {portal_clause})"));
    }
    sql.push_str(" ORDER BY mt.sort_order,mt.id");
    let mut stmt = conn.prepare(&sql)?;
    let refs: Vec<&dyn postgres_compat::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let rows: Vec<MethodType> = stmt
        .query_map(postgres_compat::params_from_iter(refs.iter()), |row| {
            Ok(MethodType {
                id: row.get(0)?,
                name: row.get(1)?,
                sort_order: row.get(2)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn validate_portal(portal: &str) -> Result<()> {
    if matches!(portal, "work" | "rd" | "sample_info") {
        Ok(())
    } else {
        Err(AppError::Validation("无效入口标识".into()))
    }
}

pub fn list_method_type_visibility(
    pool: &DbPool,
    method_type_id: i64,
    portal: &str,
) -> Result<Vec<MethodTypeVisibility>> {
    validate_portal(portal)?;
    let conn = pool.get()?;
    let mut stmt = conn.prepare("SELECT g.id,g.name,?2,NOT EXISTS(SELECT 1 FROM method_type_visibility v WHERE v.method_type_id=?1 AND v.group_id=g.id AND v.portal=?2 AND v.is_visible=0) FROM project_groups g WHERE g.deleted_at IS NULL ORDER BY g.sort_order,g.id")?;
    let rows = stmt
        .query_map(postgres_compat::params![method_type_id, portal], |row| {
            Ok(MethodTypeVisibility {
                group_id: row.get(0)?,
                group_name: row.get(1)?,
                portal: row.get(2)?,
                is_visible: row.get::<_, bool>(3).unwrap_or(true),
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn update_method_type_visibility(
    pool: &DbPool,
    method_type_id: i64,
    body: &MethodTypeVisibilityUpdate,
    operator: &str,
) -> Result<Vec<MethodTypeVisibility>> {
    validate_portal(&body.portal)?;
    let mut conn = pool.get()?;
    let type_exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM method_types WHERE id=?1 AND deleted_at IS NULL",
        [method_type_id],
        |r| r.get(0),
    )?;
    if type_exists == 0 {
        return Err(AppError::NotFound("检测类型不存在或已停用".into()));
    }
    let mut groups = body.visible_group_ids.clone();
    groups.retain(|id| *id > 0);
    groups.sort_unstable();
    groups.dedup();
    for group_id in &groups {
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM project_groups WHERE id=?1 AND deleted_at IS NULL",
            [*group_id],
            |r| r.get(0),
        )?;
        if exists == 0 {
            return Err(AppError::Validation(format!(
                "实验室不存在或已停用: {group_id}"
            )));
        }
    }
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM method_type_visibility WHERE method_type_id=?1 AND portal=?2",
        postgres_compat::params![method_type_id, body.portal.as_str()],
    )?;
    let all_groups: Vec<i64> = tx
        .prepare("SELECT id FROM project_groups WHERE deleted_at IS NULL")?
        .query_map([], |r| r.get(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for group_id in all_groups {
        if !groups.contains(&group_id) {
            tx.execute("INSERT INTO method_type_visibility(method_type_id,group_id,portal,is_visible) VALUES(?1,?2,?3,0)", postgres_compat::params![method_type_id,group_id,body.portal.as_str()])?;
        }
    }
    tx.execute("INSERT INTO audit_log(action,table_name,record_id,user_name,detail) VALUES('update','method_type_visibility',?1,?2,?3)", postgres_compat::params![method_type_id, operator, format!("入口 {} 可见实验室数 {}", body.portal, groups.len())])?;
    tx.commit()?;
    list_method_type_visibility(pool, method_type_id, &body.portal)
}

/// Reject stale entry pages after an administrator hides a method type.
pub fn ensure_method_visible_for_group(
    pool: &DbPool,
    method_id: i64,
    group_id: i64,
    portal: &str,
) -> Result<()> {
    validate_portal(portal)?;
    let conn = pool.get()?;
    let has_type: i64 = conn.query_row(
        "SELECT COUNT(*) FROM method_type_links WHERE method_id=?1",
        [method_id],
        |row| row.get(0),
    )?;
    if has_type == 0 {
        return Ok(());
    }
    let visible: i64 = conn.query_row(
        "SELECT COUNT(*) FROM method_type_links l
         WHERE l.method_id=?1
           AND NOT EXISTS (SELECT 1 FROM method_type_visibility v
                           WHERE v.method_type_id=l.method_type_id
                             AND v.group_id=?2 AND v.portal=?3 AND v.is_visible=0)",
        postgres_compat::params![method_id, group_id, portal],
        |row| row.get(0),
    )?;
    if visible == 0 {
        return Err(AppError::Validation(
            "所选检测类型已在当前实验室入口停用，请重新选择".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod detection_type_visibility_tests {
    use super::validate_portal;

    #[test]
    fn only_supported_portals_are_accepted() {
        assert!(validate_portal("work").is_ok());
        assert!(validate_portal("rd").is_ok());
        assert!(validate_portal("sample_info").is_ok());
        assert!(validate_portal("auxiliary").is_err());
    }
}

fn get_method_type_on_conn(
    conn: &Connection,
    id: i64,
) -> Result<crate::models::project::MethodType> {
    use crate::models::project::MethodType;
    conn.query_row(
        "SELECT id,name,sort_order FROM method_types WHERE id=?1",
        [id],
        |row| {
            Ok(MethodType {
                id: row.get(0)?,
                name: row.get(1)?,
                sort_order: row.get(2)?,
            })
        },
    )
    .map_err(|error| match error {
        postgres_compat::Error::QueryReturnedNoRows => AppError::NotFound("方法类型不存在".into()),
        _ => error.into(),
    })
}

pub fn create_method_type(
    pool: &DbPool,
    body: &crate::models::project::MethodTypeCreate,
    operator: &str,
) -> Result<crate::models::project::MethodType> {
    let mut conn = pool.get()?;
    let so = body.sort_order.unwrap_or(10);
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO method_types (name, sort_order) VALUES (?1,?2)",
        postgres_compat::params![body.name, so],
    )?;
    let id = tx.last_insert_rowid();
    let created = get_method_type_on_conn(&tx, id)?;
    let after = serde_json::to_value(&created).unwrap_or(serde_json::Value::Null);
    audit_repo::log_structured_on_conn(
        &tx,
        "create",
        "method_types",
        Some(id),
        operator,
        &format!("创建方法类型「{}」", body.name),
        "shared",
        "",
        None,
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(created)
}

pub fn update_method_type(
    pool: &DbPool,
    id: i64,
    body: &crate::models::project::MethodTypeUpdate,
    operator: &str,
) -> Result<crate::models::project::MethodType> {
    let mut conn = pool.get()?;
    let current = get_method_type_on_conn(&conn, id)?;
    let before = serde_json::to_value(&current).unwrap_or(serde_json::Value::Null);
    let tx = conn.transaction()?;
    if let Some(ref n) = body.name {
        tx.execute("UPDATE method_types SET name=?1 WHERE id=?2", (n, id))?;
    }
    if let Some(s) = body.sort_order {
        tx.execute("UPDATE method_types SET sort_order=?1 WHERE id=?2", (s, id))?;
    }
    let updated = get_method_type_on_conn(&tx, id)?;
    let after = serde_json::to_value(&updated).unwrap_or(serde_json::Value::Null);
    audit_repo::log_structured_on_conn(
        &tx,
        "update",
        "method_types",
        Some(id),
        operator,
        &format!("编辑方法类型「{}」", updated.name),
        "shared",
        "",
        Some(&before),
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(updated)
}

pub fn delete_method_type(pool: &DbPool, id: i64, operator: &str, reason: &str) -> Result<()> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let method_type = get_method_type_on_conn(&tx, id)?;
    let mt_name = method_type.name.clone();
    let used: i64 = tx.query_row(
        "SELECT COUNT(*) FROM method_type_links WHERE method_type_id=?",
        [id],
        |r| r.get(0),
    )?;
    let before = serde_json::to_value(&method_type).unwrap_or(serde_json::Value::Null);
    tx.execute("UPDATE method_types SET deleted_at=datetime('now','localtime') WHERE id=?1 AND deleted_at IS NULL", [id])?;
    let dependency = format!("关联方法 {used} 个");
    trash_repo::move_to_trash_on_conn(
        &tx,
        "方法类型",
        "method_types",
        id,
        "master",
        "shared",
        &mt_name,
        "",
        &before,
        reason,
        operator,
        None,
        None,
        &dependency,
        used == 0,
    )?;
    let after = serde_json::json!({"deleted_at":"now","data":before});
    audit_repo::log_structured_on_conn(
        &tx,
        "delete",
        "method_types",
        Some(id),
        operator,
        &format!("删除方法类型「{}」；{}", mt_name, dependency),
        "shared",
        "",
        Some(&before),
        Some(&after),
        "management",
    )?;
    tx.commit()?;
    Ok(())
}

// ── 导入映射配置 (v0.3.0) ──

/// 从连接加载活跃的导入映射（按优先级排序）
pub fn load_mappings_from_conn(conn: &Connection) -> Result<Vec<ImportMapping>> {
    let mut stmt = conn.prepare(
        "SELECT id, header_pattern, match_mode, target_table, COALESCE(default_type,''), priority, is_active
         FROM import_mappings WHERE is_active=1 ORDER BY priority ASC"
    )?;
    let items = stmt
        .query_map([], |r| {
            Ok(ImportMapping {
                id: r.get(0)?,
                header_pattern: r.get(1)?,
                match_mode: r.get(2)?,
                target_table: r.get(3)?,
                default_type: r.get(4)?,
                priority: r.get(5)?,
                is_active: r.get::<_, i64>(6)? != 0,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(items)
}

/// 从连接池加载活跃的导入映射
pub fn load_mappings(pool: &DbPool) -> Result<Vec<ImportMapping>> {
    let conn = pool.get()?;
    load_mappings_from_conn(&conn)
}

// ── 导入 ──

/// v0.2.17: 按列导入 — 分三路：实验室→project_groups, 研发项目→projects, 方法→methods
pub fn batch_import_column_split(
    conn: &Connection,
    group_names: &[String],
    project_names: &[String],
    method_items: &[(String, String, String)],
) -> Result<ImportSummary> {
    let mut group_count = 0usize;
    let mut project_count = 0usize;
    let mut method_count = 0usize;
    let mut type_counter: BTreeMap<String, usize> = BTreeMap::new();

    // 1. 实验室分组
    for gname in group_names {
        let existed: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM project_groups WHERE name=?1",
                postgres_compat::params![gname],
                |r| r.get(0),
            )
            .unwrap_or(0);
        conn.execute(
            "INSERT OR IGNORE INTO project_groups (name) VALUES (?1)",
            postgres_compat::params![gname],
        )?;
        if existed == 0 {
            group_count += 1;
        }
    }

    // 2. 研发项目 → projects 表
    // 确保"研发项目"分组存在
    conn.execute(
        "INSERT OR IGNORE INTO project_groups (name) VALUES ('研发项目')",
        [],
    )?;
    let proj_gid: i64 = conn.query_row(
        "SELECT id FROM project_groups WHERE name='研发项目'",
        [],
        |r| r.get(0),
    )?;

    for pname in project_names {
        // v0.3.3: 按项目名查重（不限制 group_id，因为不同项目可能关联不同实验室）
        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM projects WHERE name=?1",
                postgres_compat::params![pname],
                |r| r.get(0),
            )
            .ok();
        if existing.is_none() {
            conn.execute(
                "INSERT INTO projects (group_id, name, method_type) VALUES (?1,?2,'研发项目')",
                postgres_compat::params![proj_gid, pname],
            )?;
            project_count += 1;
        }
    }

    // 3. 方法 → methods 表（不创建 project_groups，避免污染实验室标签）
    // v0.3.4: 智能识别 — 若列头含"方法"，从列头提取真实类型名并自动创建 method_type
    for (header, item_name, method_type) in method_items {
        // v0.3.4: 智能提取类型名
        let effective_type: String = if header.contains("方法") {
            let extracted = header.replace("方法", "").trim().to_string();
            if !extracted.is_empty() {
                // 检查并自动创建 method_type（不存在则创建，sort_order=10）
                let mt_exists: Option<i64> = conn
                    .query_row(
                        "SELECT id FROM method_types WHERE name=?1",
                        postgres_compat::params![extracted],
                        |r| r.get(0),
                    )
                    .ok();
                if mt_exists.is_none() {
                    conn.execute(
                        "INSERT INTO method_types (name, sort_order) VALUES (?1, 10)",
                        postgres_compat::params![extracted],
                    )?;
                }
                extracted
            } else {
                method_type.clone()
            }
        } else {
            method_type.clone()
        };

        // Insert method（去重：已存在则跳过）
        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM methods WHERE name=?1",
                postgres_compat::params![item_name],
                |r| r.get(0),
            )
            .ok();

        let mid = if let Some(pid) = existing {
            // 已存在时也补全 full_name（防止旧数据缺失）
            if !item_name.is_empty() {
                conn.execute("UPDATE methods SET name=COALESCE(NULLIF(name,''),?1), full_name=COALESCE(NULLIF(full_name,''),?1) WHERE id=?2",
                    postgres_compat::params![item_name, pid]).ok();
            }
            pid
        } else {
            conn.execute(
                "INSERT INTO methods (name, full_name) VALUES (?1, ?2)",
                postgres_compat::params![item_name, item_name],
            )?;
            method_count += 1;
            conn.last_insert_rowid()
        };

        // 关联类型
        if !effective_type.is_empty() {
            let mt_id: Option<i64> = conn
                .query_row(
                    "SELECT id FROM method_types WHERE name=?1",
                    postgres_compat::params![effective_type],
                    |r| r.get(0),
                )
                .ok();
            if let Some(tid) = mt_id {
                conn.execute("DELETE FROM method_type_links WHERE method_id=?1", [mid])?;
                conn.execute(
                    "INSERT OR IGNORE INTO method_type_links (method_id, method_type_id) VALUES (?1,?2)",
                    postgres_compat::params![mid, tid],
                )?;
            }
        }
        *type_counter.entry(effective_type).or_insert(0) += 1;
    }

    // 审计日志
    if method_count > 0 {
        crate::repo::audit_repo::log_on_conn(
            conn,
            "import",
            "methods",
            None,
            "system",
            &format!("批量导入: {}条方法", method_count),
        )
        .ok();
    }
    if project_count > 0 {
        crate::repo::audit_repo::log_on_conn(
            conn,
            "import",
            "projects",
            None,
            "system",
            &format!("批量导入: {}个研发项目", project_count),
        )
        .ok();
    }
    if group_count > 0 {
        crate::repo::audit_repo::log_on_conn(
            conn,
            "import",
            "project_groups",
            None,
            "system",
            &format!("批量导入: {}个实验室分组", group_count),
        )
        .ok();
    }

    Ok(ImportSummary {
        total_methods: method_count,
        total_projects: project_count,
        total_groups: group_count,
        by_type: type_counter
            .into_iter()
            .map(|(k, v)| TypeCount {
                method_type: k,
                count: v,
            })
            .collect(),
    })
}

/// v0.2.17: 扁平导入
pub fn batch_import_flat(conn: &Connection, items: &[MethodImportItem]) -> Result<ImportSummary> {
    let mut method_count = 0usize;
    let mut project_count = 0usize;
    let mut group_count = 0usize;
    let mut type_counter: BTreeMap<String, usize> = BTreeMap::new();

    for item in items {
        // 1. 实验室 → project_groups
        conn.execute(
            "INSERT OR IGNORE INTO project_groups (name) VALUES (?1)",
            postgres_compat::params![item.group_name],
        )
        .ok();
        let gid: i64 = conn.query_row(
            "SELECT id FROM project_groups WHERE name=?1",
            postgres_compat::params![item.group_name],
            |r| r.get(0),
        )?;

        // 2. 研发项目 → projects (method_type='研发项目')
        let existing_proj: Option<i64> = conn
            .query_row(
                "SELECT id FROM projects WHERE name=?1 AND method_type='研发项目'",
                postgres_compat::params![item.project_name],
                |r| r.get(0),
            )
            .ok();
        let proj_id = if existing_proj.is_none() {
            conn.execute(
                "INSERT INTO projects (group_id, name, method_type) VALUES (?1,?2,'研发项目')",
                postgres_compat::params![gid, item.project_name],
            )?;
            let pid = conn.last_insert_rowid();
            // Link project → lab
            conn.execute(
                "INSERT OR IGNORE INTO project_lab_links (project_id, group_id) VALUES (?1,?2)",
                postgres_compat::params![pid, gid],
            )?;
            project_count += 1;
            pid
        } else {
            let pid = existing_proj.unwrap();
            // Ensure link
            conn.execute(
                "INSERT OR IGNORE INTO project_lab_links (project_id, group_id) VALUES (?1,?2)",
                postgres_compat::params![pid, gid],
            )?;
            pid
        };

        // 3. 方法 → methods
        let full_name = format!("{}/{}", item.group_name, item.project_name);
        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM methods WHERE name=?1",
                postgres_compat::params![item.method_name],
                |r| r.get(0),
            )
            .ok();
        let mid = if let Some(pid) = existing {
            conn.execute(
                "UPDATE methods SET full_name=?1, coefficient=?2 WHERE id=?3",
                postgres_compat::params![full_name, item.coefficient, pid],
            )?;
            pid
        } else {
            conn.execute(
                "INSERT INTO methods (name, full_name, coefficient) VALUES (?1,?2,?3)",
                postgres_compat::params![item.method_name, full_name, item.coefficient],
            )?;
            method_count += 1;
            conn.last_insert_rowid()
        };

        // Link method → type
        if !item.method_type.is_empty() {
            let mt_id: Option<i64> = conn
                .query_row(
                    "SELECT id FROM method_types WHERE name=?1",
                    postgres_compat::params![item.method_type],
                    |r| r.get(0),
                )
                .ok();
            if let Some(tid) = mt_id {
                conn.execute("DELETE FROM method_type_links WHERE method_id=?1", [mid])?;
                conn.execute(
                    "INSERT OR IGNORE INTO method_type_links (method_id, method_type_id) VALUES (?1,?2)",
                    postgres_compat::params![mid, tid],
                )?;
            }
        }

        // Link project → method
        conn.execute(
            "INSERT OR IGNORE INTO project_method_links (project_id, method_id) VALUES (?1,?2)",
            postgres_compat::params![proj_id, mid],
        )?;

        *type_counter.entry(item.method_type.clone()).or_insert(0) += 1;
        group_count += 1;
    }

    Ok(ImportSummary {
        total_methods: method_count,
        total_projects: project_count,
        total_groups: group_count,
        by_type: type_counter
            .into_iter()
            .map(|(k, v)| TypeCount {
                method_type: k,
                count: v,
            })
            .collect(),
    })
}
