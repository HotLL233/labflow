use crate::error::{AppError, Result};
use crate::repo::settings_repo;
use postgres_compat::Connection;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const SETTING_KEY: &str = "sample_info_workload_mapping";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SampleWorkloadMapping {
    #[serde(default)]
    pub type_mappings: Vec<TypeMapping>,
    #[serde(default)]
    pub field_mappings: Vec<FieldMapping>,
    #[serde(default)]
    pub project_aliases: Vec<ProjectAlias>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypeMapping {
    pub type_key: String,
    pub method_type_ids: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldMapping {
    pub type_key: String,
    pub field_key: String,
    pub value: String,
    pub method_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectAlias {
    pub alias: String,
    pub project_id: i64,
    #[serde(default)]
    pub group_id: Option<i64>,
}

pub fn load(conn: &Connection) -> Result<SampleWorkloadMapping> {
    let Some(setting) = settings_repo::get_on_conn(conn, SETTING_KEY)? else {
        return Ok(SampleWorkloadMapping::default());
    };
    let mapping = serde_json::from_str(&setting.value).map_err(|_| {
        AppError::Validation("样品信息工作量映射配置损坏，请联系系统管理员修复".into())
    })?;
    validate_shape(&mapping)?;
    // 基础数据停用后，由录入候选检索忽略失效引用，不阻断其余有效方法。
    Ok(mapping)
}

fn required_text(value: &str, label: &str) -> Result<()> {
    if value.is_empty() || value.trim() != value {
        return Err(AppError::Validation(format!("{label}不能为空或含首尾空格")));
    }
    Ok(())
}

fn validate_shape(mapping: &SampleWorkloadMapping) -> Result<()> {
    let mut types = BTreeSet::new();
    for item in &mapping.type_mappings {
        required_text(&item.type_key, "登记类型标识")?;
        if !types.insert(&item.type_key) {
            return Err(AppError::Validation(
                "同一登记类型只能配置一条类型映射".into(),
            ));
        }
        let ids: BTreeSet<_> = item.method_type_ids.iter().copied().collect();
        if ids.is_empty()
            || ids.len() != item.method_type_ids.len()
            || ids.iter().any(|id| *id <= 0)
        {
            return Err(AppError::Validation(
                "方法类型必须选择有效且不重复的类型".into(),
            ));
        }
    }
    let mut fields = BTreeSet::new();
    for item in &mapping.field_mappings {
        required_text(&item.type_key, "登记类型标识")?;
        required_text(&item.field_key, "登记字段标识")?;
        required_text(&item.value, "登记填写值")?;
        if item.method_id <= 0 {
            return Err(AppError::Validation("字段映射请选择标准方法".into()));
        }
        if !fields.insert((&item.type_key, &item.field_key, item.value.to_lowercase())) {
            return Err(AppError::Validation(
                "同一类型、字段和填写值存在重复映射".into(),
            ));
        }
    }
    for (index, item) in mapping.project_aliases.iter().enumerate() {
        required_text(&item.alias, "项目别名")?;
        if item.project_id <= 0 || item.group_id.is_some_and(|id| id <= 0) {
            return Err(AppError::Validation(
                "项目别名请选择有效项目和实验室".into(),
            ));
        }
        if mapping.project_aliases[..index].iter().any(|previous| {
            previous.alias.to_lowercase() == item.alias.to_lowercase()
                && (previous.group_id == item.group_id
                    || previous.group_id.is_none()
                    || item.group_id.is_none())
        }) {
            return Err(AppError::Validation(
                "同一实验室范围内项目别名不能重复或与全局别名冲突".into(),
            ));
        }
    }
    Ok(())
}

pub fn validate(conn: &Connection, mapping: &SampleWorkloadMapping) -> Result<()> {
    validate_shape(mapping)?;
    for type_key in mapping
        .type_mappings
        .iter()
        .map(|item| &item.type_key)
        .chain(mapping.field_mappings.iter().map(|item| &item.type_key))
    {
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sample_info_types WHERE type_key=?1 AND deleted_at IS NULL AND is_active=1",
            [type_key], |row| row.get(0))?;
        if exists == 0 {
            return Err(AppError::Validation(format!(
                "登记类型「{type_key}」不存在或已停用"
            )));
        }
    }
    for item in &mapping.type_mappings {
        for id in &item.method_type_ids {
            let exists: i64 = conn.query_row(
                "SELECT COUNT(*) FROM method_types WHERE id=?1 AND deleted_at IS NULL",
                [id],
                |row| row.get(0),
            )?;
            if exists == 0 {
                return Err(AppError::Validation("映射的方法类型不存在或已删除".into()));
            }
        }
    }
    for item in &mapping.field_mappings {
        let column_exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sample_info_columns c JOIN sample_info_column_visibility v ON v.column_id=c.id AND v.type_key=?1 AND v.is_visible=1
             WHERE c.field_key=?2 AND c.is_active=1 AND c.deleted_at IS NULL AND c.data_type IN ('text','select','multi_select','multiselect','textarea')",
            postgres_compat::params![&item.type_key,&item.field_key], |row| row.get(0))?;
        if column_exists == 0 {
            return Err(AppError::Validation(format!(
                "字段「{}」未启用、不是文本/选项字段或对该登记类型不可见",
                item.field_key
            )));
        }
        let method_exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM methods m WHERE m.id=?1 AND m.is_active=1 AND m.deleted_at IS NULL
             AND (m.instrument_id IS NULL OR EXISTS(SELECT 1 FROM instruments i WHERE i.id=m.instrument_id AND i.is_active=1 AND i.deleted_at IS NULL))",
            [item.method_id], |row| row.get(0))?;
        if method_exists == 0 {
            return Err(AppError::Validation(
                "字段映射的方法或绑定仪器不存在或已停用".into(),
            ));
        }
    }
    for item in &mapping.project_aliases {
        let project_exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM projects p WHERE p.id=?1 AND p.is_active=1 AND p.deleted_at IS NULL
             AND COALESCE(p.project_status,'ongoing')='ongoing'
             AND EXISTS(SELECT 1 FROM project_lab_links pll
               JOIN project_groups g ON g.id=pll.group_id AND g.deleted_at IS NULL
               JOIN divisions d ON d.id=g.division_id AND d.deleted_at IS NULL AND d.is_active=1
               WHERE pll.project_id=p.id AND (CAST(?2 AS BIGINT) IS NULL OR pll.group_id=?2)
                 AND (COALESCE(p.project_division_id,(SELECT division_id FROM project_groups WHERE id=p.group_id))=d.id
                      OR EXISTS(SELECT 1 FROM project_collaboration_divisions pcd WHERE pcd.project_id=p.id AND pcd.division_id=d.id)))",
            postgres_compat::params![item.project_id,item.group_id], |row| row.get(0))?;
        if project_exists == 0 {
            return Err(AppError::Validation(
                "别名对应项目未启用或已归档，或实验室/部门无有效执行关联；请检查项目所属及协作部门"
                    .into(),
            ));
        }
        let canonical_conflict: i64 = conn.query_row(
            "SELECT COUNT(*) FROM projects p WHERE LOWER(BTRIM(p.name))=LOWER(?1) AND p.id<>?2 AND p.is_active=1 AND p.deleted_at IS NULL
             AND COALESCE(p.project_status,'ongoing')='ongoing'
             AND EXISTS(SELECT 1 FROM project_lab_links pll
               JOIN project_groups g ON g.id=pll.group_id AND g.deleted_at IS NULL
               JOIN divisions d ON d.id=g.division_id AND d.deleted_at IS NULL AND d.is_active=1
               WHERE pll.project_id=p.id AND (CAST(?3 AS BIGINT) IS NULL OR pll.group_id=?3)
                 AND (COALESCE(p.project_division_id,(SELECT division_id FROM project_groups WHERE id=p.group_id))=d.id
                      OR EXISTS(SELECT 1 FROM project_collaboration_divisions pcd WHERE pcd.project_id=p.id AND pcd.division_id=d.id)))",
            postgres_compat::params![&item.alias,item.project_id,item.group_id], |row| row.get(0))?;
        if canonical_conflict > 0 {
            return Err(AppError::Validation(
                "项目别名与同一实验室范围内其他标准项目名称冲突".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shape_rejects_unknown_keys_invalid_ids_and_ambiguous_mappings() {
        assert!(serde_json::from_str::<SampleWorkloadMapping>(
            r#"{"project_aliases":[],"projectAlias":[]}"#
        )
        .is_err());
        for value in [
            serde_json::json!({"type_mappings":[{"type_key":"icp","method_type_ids":[1,1]}]}),
            serde_json::json!({"field_mappings":[{"type_key":"icp","field_key":"method","value":"ICP","method_id":0}]}),
            serde_json::json!({"field_mappings":[{"type_key":"icp","field_key":"method","value":"ICP","method_id":1},{"type_key":"icp","field_key":"method","value":"icp","method_id":2}]}),
            serde_json::json!({"project_aliases":[{"alias":"A","project_id":1},{"alias":"a","project_id":2,"group_id":3}]}),
        ] {
            let mapping: SampleWorkloadMapping = serde_json::from_value(value).unwrap();
            assert!(validate_shape(&mapping).is_err());
        }
        let distinct_labs: SampleWorkloadMapping = serde_json::from_value(serde_json::json!({"project_aliases":[{"alias":"A","project_id":1,"group_id":2},{"alias":"A","project_id":3,"group_id":4}]})).unwrap();
        assert!(validate_shape(&distinct_labs).is_ok());
        assert!(validate_shape(&SampleWorkloadMapping::default()).is_ok());
    }

    #[test]
    fn validation_checks_active_master_data_and_visibility_on_one_connection() {
        let mut conn = Connection::open_test_database().expect("test database");
        let tx = conn.transaction().unwrap();
        tx.execute_batch("CREATE TEMP TABLE sample_info_types(type_key TEXT,is_active BIGINT,deleted_at TEXT) ON COMMIT DROP;
            CREATE TEMP TABLE method_types(id BIGINT,deleted_at TEXT) ON COMMIT DROP;
            CREATE TEMP TABLE sample_info_columns(id BIGINT,field_key TEXT,data_type TEXT,is_active BIGINT,deleted_at TEXT) ON COMMIT DROP;
            CREATE TEMP TABLE sample_info_column_visibility(column_id BIGINT,type_key TEXT,is_visible BIGINT) ON COMMIT DROP;
            CREATE TEMP TABLE methods(id BIGINT,is_active BIGINT,deleted_at TEXT,instrument_id BIGINT) ON COMMIT DROP;
            CREATE TEMP TABLE instruments(id BIGINT,is_active BIGINT,deleted_at TEXT) ON COMMIT DROP;
            CREATE TEMP TABLE projects(id BIGINT,name TEXT,is_active BIGINT,deleted_at TEXT,project_status TEXT,project_division_id BIGINT,group_id BIGINT) ON COMMIT DROP;
            CREATE TEMP TABLE project_groups(id BIGINT,deleted_at TEXT,division_id BIGINT) ON COMMIT DROP;
            CREATE TEMP TABLE divisions(id BIGINT,is_active BIGINT,deleted_at TEXT) ON COMMIT DROP;
            CREATE TEMP TABLE project_collaboration_divisions(project_id BIGINT,division_id BIGINT) ON COMMIT DROP;
            CREATE TEMP TABLE project_lab_links(project_id BIGINT,group_id BIGINT) ON COMMIT DROP;
            CREATE TEMP TABLE system_settings(key TEXT PRIMARY KEY,value TEXT,updated_at TEXT) ON COMMIT DROP;
            INSERT INTO sample_info_types VALUES('icp',1,NULL);
            INSERT INTO method_types VALUES(1,NULL);
            INSERT INTO sample_info_columns VALUES(1,'test_method','text',1,NULL);
            INSERT INTO sample_info_column_visibility VALUES(1,'icp',1);
            INSERT INTO methods VALUES(1,1,NULL,1);
            INSERT INTO instruments VALUES(1,1,NULL);
            INSERT INTO projects VALUES(1,'standard',1,NULL,'ongoing',1,1),(2,'other',1,NULL,'ongoing',1,1);
            INSERT INTO project_groups VALUES(1,NULL,1),(2,NULL,2);
            INSERT INTO divisions VALUES(1,1,NULL),(2,1,NULL);
            INSERT INTO project_lab_links VALUES(1,1),(2,1);").unwrap();
        let mut mapping: SampleWorkloadMapping = serde_json::from_value(serde_json::json!({
            "type_mappings":[{"type_key":"icp","method_type_ids":[1]}],
            "field_mappings":[{"type_key":"icp","field_key":"test_method","value":"ICP-MS","method_id":1}],
            "project_aliases":[{"alias":"alias","project_id":1,"group_id":1}]
        })).unwrap();
        assert!(validate(&tx, &mapping).is_ok());
        for (disable, restore) in [
            (
                "UPDATE sample_info_types SET is_active=0",
                "UPDATE sample_info_types SET is_active=1",
            ),
            (
                "UPDATE method_types SET deleted_at='2099-01-01'",
                "UPDATE method_types SET deleted_at=NULL",
            ),
            (
                "UPDATE sample_info_column_visibility SET is_visible=0",
                "UPDATE sample_info_column_visibility SET is_visible=1",
            ),
            (
                "UPDATE instruments SET is_active=0",
                "UPDATE instruments SET is_active=1",
            ),
            (
                "UPDATE project_groups SET deleted_at='2099-01-01'",
                "UPDATE project_groups SET deleted_at=NULL",
            ),
            (
                "UPDATE projects SET project_status='archived'",
                "UPDATE projects SET project_status='ongoing'",
            ),
            (
                "UPDATE divisions SET is_active=0",
                "UPDATE divisions SET is_active=1",
            ),
            (
                "UPDATE projects SET project_division_id=2 WHERE id=1",
                "UPDATE projects SET project_division_id=1 WHERE id=1",
            ),
        ] {
            tx.execute_batch(disable).unwrap();
            assert!(validate(&tx, &mapping).is_err(), "{disable}");
            tx.execute_batch(restore).unwrap();
            assert!(validate(&tx, &mapping).is_ok());
        }
        mapping.project_aliases[0].alias = "other".into();
        assert!(validate(&tx, &mapping).is_err());
        mapping.project_aliases[0].alias = "alias".into();
        mapping.project_aliases[0].group_id = Some(2);
        tx.execute_batch("INSERT INTO project_lab_links VALUES(1,2)")
            .unwrap();
        assert!(validate(&tx, &mapping).is_err());
        tx.execute_batch("INSERT INTO project_collaboration_divisions VALUES(1,2)")
            .unwrap();
        assert!(validate(&tx, &mapping).is_ok());
        tx.execute_batch("DELETE FROM project_collaboration_divisions; UPDATE projects SET project_division_id=NULL WHERE id=1").unwrap();
        mapping.project_aliases[0].group_id = Some(1);
        assert!(validate(&tx, &mapping).is_ok());
        mapping.project_aliases[0].group_id = None;
        assert!(validate(&tx, &mapping).is_ok());
        mapping.project_aliases[0].group_id = Some(999);
        assert!(validate(&tx, &mapping).is_err());
        assert!(load(&tx).unwrap().type_mappings.is_empty());
        settings_repo::upsert_on_conn(&tx, SETTING_KEY, "{bad JSON").unwrap();
        assert!(load(&tx).is_err());
    }
}
