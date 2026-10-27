use crate::config::AppConfig;
use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::ApiResponse;
use crate::service::authz_service::{self, AuthContext};
use axum::{
    body::Body,
    extract::{Multipart, Path as AxumPath, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use calamine::{open_workbook_auto, DataType, Reader};
use chrono::{Duration, Local, NaiveDate};
use sha2::{Digest, Sha256};
#[path = "personnel_change_storage.rs"]
mod storage;
use rust_xlsxwriter::{Format, Workbook};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

const SUMMARY_FILE: &str = "项目及人员汇总表.xlsx";
const SUMMARY_SHEET: &str = "当前实验室人员统计表";
const CHANGE_TYPES_FILE: &str = "change-types.json";
const FIELDS_FILE: &str = "fields.json";
const DEFAULT_TYPES: &[&str] = &[
    "员工加入",
    "员工离职",
    "实验室内人员调动",
    "跨实验室人员调出",
    "跨实验室人员调入",
    "实验室新增项目",
    "实验室新增方法",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChangeTypeConfig {
    #[serde(default)]
    name: String,
    #[serde(default)]
    label: String,
    #[serde(default)]
    action_key: String,
    enabled: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChangeTypeSettings {
    change_types: Vec<ChangeTypeConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FeedbackFieldConfig {
    key: String,
    label: String,
    field_type: String,
    required: bool,
    enabled: bool,
    notify: bool,
    sort_order: i64,
    change_types: Vec<String>,
    #[serde(default)]
    options: Vec<String>,
}

#[derive(Debug, Serialize)]
struct FeedbackConfig {
    change_types: Vec<ChangeTypeConfig>,
    fields: Vec<FeedbackFieldConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    capabilities: Option<Capabilities>,
}

#[derive(Debug, Deserialize)]
struct FeedbackConfigInput {
    change_types: Vec<ChangeTypeConfig>,
    fields: Vec<FeedbackFieldConfig>,
}

#[derive(Debug, Clone, Serialize)]
struct PersonnelChangeLab {
    id: i64,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct SummaryRow {
    #[serde(default)]
    row_key: String,
    lab_name: String,
    leader_name: String,
    person_names: Vec<String>,
    project_code: String,
    methods: Vec<String>,
}

#[derive(Debug, Serialize)]
struct PersonnelChangeOptions {
    labs: Vec<PersonnelChangeLab>,
    summary_labs: Vec<PersonnelChangeLab>,
    summary: Vec<SummaryRow>,
    lab_projects: HashMap<String, Vec<String>>,
    change_types: Vec<String>,
    fields: Vec<FeedbackFieldConfig>,
    summary_available: bool,
    summary_revision: String,
    capabilities: Capabilities,
    change_type_configs: Vec<ChangeTypeConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NoticeCreate {
    #[serde(default)]
    action_key: String,
    #[serde(default)]
    summary_revision: String,
    #[serde(default)]
    reason: String,
    #[serde(default)]
    notice_revision: u64,
    change_type: String,
    lab_name: String,
    #[serde(default)]
    person_name: Option<String>,
    #[serde(default)]
    project_codes: Vec<String>,
    #[serde(default)]
    source_project_codes: Vec<String>,
    #[serde(default)]
    target_lab_name: Option<String>,
    #[serde(default)]
    target_project_codes: Vec<String>,
    #[serde(default)]
    transfer_notice_no: Option<String>,
    #[serde(default)]
    method_names: Vec<String>,
    #[serde(default)]
    new_project_code: Option<String>,
    #[serde(default)]
    is_high_tech: Option<bool>,
    #[serde(default)]
    high_tech_name: Option<String>,
    #[serde(default)]
    effective_at: String,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    extra_fields: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct NoticeReview {
    #[serde(default)]
    decision: String,
    #[serde(default)]
    summary_revision: String,
    #[serde(default)]
    notice_revision: u64,
    #[serde(default)]
    reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NoticeLog {
    #[serde(default)]
    summary_applied: bool,
    #[serde(default)]
    affected_rows: usize,
    #[serde(default)]
    baseline_fingerprint: String,
    #[serde(default)]
    actor_user_id: Option<i64>,
    #[serde(default)]
    action_key: String,
    #[serde(default = "first_revision")]
    revision: u64,
    #[serde(default)]
    summary_revision: String,
    #[serde(default)]
    can_review: bool,
    #[serde(default)]
    can_revise: bool,
    #[serde(default)]
    can_withdraw: bool,
    notice_no: String,
    created_at: String,
    actor_username: String,
    change_type: String,
    lab_name: String,
    #[serde(default)]
    target_lab_name: String,
    #[serde(default)]
    person_name: String,
    #[serde(default)]
    project_codes: Vec<String>,
    #[serde(default)]
    target_project_codes: Vec<String>,
    #[serde(default)]
    source_project_codes: Vec<String>,
    #[serde(default)]
    transfer_notice_no: String,
    #[serde(default)]
    method_names: Vec<String>,
    #[serde(default)]
    new_project_code: String,
    #[serde(default)]
    new_project_name: String,
    #[serde(default)]
    is_high_tech: bool,
    #[serde(default)]
    high_tech_name: String,
    effective_at: String,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    extra_fields: HashMap<String, String>,
    #[serde(default)]
    delivery_status: String,
    #[serde(default)]
    review_status: String,
    #[serde(default)]
    reviewed_by: String,
    #[serde(default)]
    reviewed_at: String,
    #[serde(default)]
    review_reason: String,
    #[serde(default)]
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct NoticeQuery {
    days: Option<i64>,
    notice_no: Option<String>,
    start: Option<String>,
    end: Option<String>,
    status: Option<String>,
    keyword: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PersonProjectQuery {
    person_name: String,
}

fn feature_dir() -> PathBuf {
    std::env::var("PERSONNEL_CHANGE_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| AppConfig::load().data_dir().join("personnel-change-data"))
}

fn summary_path() -> PathBuf {
    feature_dir().join(SUMMARY_FILE)
}
fn change_types_path() -> PathBuf {
    feature_dir().join(CHANGE_TYPES_FILE)
}
fn fields_path() -> PathBuf {
    feature_dir().join(FIELDS_FILE)
}

fn ensure_feature_dir() -> Result<PathBuf> {
    let dir = feature_dir();
    fs::create_dir_all(&dir)
        .map_err(|error| AppError::Internal(format!("创建人员变动功能目录失败: {error}")))?;
    Ok(dir)
}

fn default_change_types() -> Vec<ChangeTypeConfig> {
    DEFAULT_TYPES
        .iter()
        .map(|name| ChangeTypeConfig {
            name: (*name).to_string(),
            label: (*name).to_string(),
            action_key: action_for_label(name).unwrap_or("notice_only").to_string(),
            enabled: true,
        })
        .collect()
}

fn default_feedback_fields() -> Vec<FeedbackFieldConfig> {
    [
        ("change_type", "变动类型", "select", true),
        ("lab_name", "实验室", "text", true),
        ("person_name", "人员", "text", false),
        ("project_codes", "项目", "multi-select", false),
        ("method_names", "检测方法", "multi-text", false),
        ("effective_at", "生效日期", "date", true),
        ("is_high_tech", "高新项目", "boolean", false),
        ("high_tech_name", "高新项目名称", "text", false),
        ("notes", "备注", "textarea", false),
        ("source_project_codes", "来源项目", "multi-select", false),
        ("target_lab_name", "目标实验室", "text", false),
        ("target_project_codes", "目标项目", "multi-select", false),
        ("transfer_notice_no", "关联调出通知", "text", false),
        ("new_project_code", "新增项目代号", "text", false),
    ]
    .into_iter()
    .enumerate()
    .map(
        |(index, (key, label, field_type, required))| FeedbackFieldConfig {
            key: key.into(),
            label: label.into(),
            field_type: field_type.into(),
            required,
            enabled: true,
            notify: true,
            sort_order: index as i64 + 1,
            change_types: vec![],
            options: vec![],
        },
    )
    .collect()
}

fn load_feedback_config() -> Result<FeedbackConfig> {
    ensure_feature_dir()?;
    let types_path = change_types_path();
    if !types_path.exists() {
        fs::write(
            &types_path,
            serde_json::to_string_pretty(&ChangeTypeSettings {
                change_types: default_change_types(),
            })
            .map_err(|error| AppError::Internal(format!("生成人员变动类型配置失败: {error}")))?,
        )
        .map_err(|error| AppError::Internal(format!("写入人员变动类型配置失败: {error}")))?;
    }
    let mut change_types = serde_json::from_str::<ChangeTypeSettings>(
        &fs::read_to_string(&types_path)
            .map_err(|error| AppError::Internal(format!("读取人员变动类型配置失败: {error}")))?,
    )
    .map_err(|error| AppError::Validation(format!("人员变动类型配置格式错误: {error}")))?;
    let fields_path = fields_path();
    if !fields_path.exists() {
        fs::write(
            &fields_path,
            serde_json::to_string_pretty(&default_feedback_fields()).map_err(|error| {
                AppError::Internal(format!("生成人员反馈字段配置失败: {error}"))
            })?,
        )
        .map_err(|error| AppError::Internal(format!("写入人员反馈字段配置失败: {error}")))?;
    }
    let mut fields = serde_json::from_str::<Vec<FeedbackFieldConfig>>(
        &fs::read_to_string(&fields_path)
            .map_err(|error| AppError::Internal(format!("读取人员反馈字段配置失败: {error}")))?,
    )
    .map_err(|error| AppError::Validation(format!("人员反馈字段配置格式错误: {error}")))?;
    let had_legacy_transfer = change_types
        .change_types
        .iter()
        .any(|item| item.name.trim() == "跨实验室人员调动");
    for item in &mut change_types.change_types {
        item.name = normalize_change_type(if item.label.trim().is_empty() {
            &item.name
        } else {
            &item.label
        });
        item.label = item.name.clone();
        if item.action_key.is_empty() {
            item.action_key = action_for_label(&item.name).unwrap_or("notice_only").into();
        }
    }
    if had_legacy_transfer
        && !change_types
            .change_types
            .iter()
            .any(|item| item.name == "跨实验室人员调入")
    {
        change_types.change_types.push(ChangeTypeConfig {
            name: "跨实验室人员调入".into(),
            label: "跨实验室人员调入".into(),
            action_key: "transfer_in".into(),
            enabled: true,
        });
    }
    for field in &mut fields {
        for change_type in &mut field.change_types {
            *change_type = action_for_label(change_type)
                .unwrap_or(change_type)
                .to_string();
        }
    }
    for field in default_feedback_fields() {
        if !fields.iter().any(|f| f.key == field.key) {
            fields.push(field);
        }
    }
    Ok(FeedbackConfig {
        change_types: change_types.change_types,
        fields,
        capabilities: None,
    })
}

fn normalize_change_type(name: &str) -> String {
    match name.trim() {
        "新员工加入" => "员工加入".to_string(),
        "跨实验室人员调动" => "跨实验室人员调出".to_string(),
        value => value.to_string(),
    }
}

fn cell_text(cell: &DataType) -> String {
    match cell {
        DataType::String(value) => value.trim().to_string(),
        DataType::Float(value) if value.fract() == 0.0 => format!("{}", *value as i64),
        DataType::Float(value) => value.to_string(),
        DataType::Int(value) => value.to_string(),
        DataType::Bool(value) => value.to_string(),
        DataType::DateTime(value) => value.to_string(),
        DataType::DateTimeIso(value) | DataType::DurationIso(value) => value.trim().to_string(),
        DataType::Duration(value) => value.to_string(),
        DataType::Empty | DataType::Error(_) => String::new(),
    }
}

fn split_values(value: &str) -> Vec<String> {
    value
        .split(|c| matches!(c, ',' | '，' | '、' | ';' | '；' | '\n' | '\r'))
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn load_summary(path: &Path) -> Result<Vec<SummaryRow>> {
    if !path.exists() {
        return Ok(vec![]);
    }
    let mut workbook = open_workbook_auto(path)
        .map_err(|error| AppError::Validation(format!("无法读取项目及人员汇总表: {error}")))?;
    let sheet_name = if workbook
        .sheet_names()
        .iter()
        .any(|name| name == SUMMARY_SHEET)
    {
        SUMMARY_SHEET.to_string()
    } else {
        workbook
            .sheet_names()
            .first()
            .cloned()
            .ok_or_else(|| AppError::Validation("项目及人员汇总表没有工作表".into()))?
    };
    let range = workbook
        .worksheet_range(&sheet_name)
        .map_err(|error| AppError::Validation(format!("读取汇总表工作表失败: {error}")))?;
    let mut rows = range.rows();
    let headers = rows
        .next()
        .ok_or_else(|| AppError::Validation("项目及人员汇总表缺少表头".into()))?;
    let find_header = |name: &str| {
        headers
            .iter()
            .position(|cell| cell_text(cell) == name)
            .ok_or_else(|| AppError::Validation(format!("项目及人员汇总表缺少“{name}”列")))
    };
    let lab_index = find_header("实验室")?;
    let leader_index = find_header("实验室负责人")?;
    let people_index = find_header("实验人员")?;
    let project_index = find_header("实验室项目代号")?;
    let method_index = find_header("检测方法")?;
    let id_index = headers
        .iter()
        .position(|cell| cell_text(cell) == "LabFlow行标识");
    let value_at =
        |row: &[DataType], index: usize| row.get(index).map(cell_text).unwrap_or_default();
    let mut result: Vec<SummaryRow> = rows
        .filter(|row| row.iter().any(|cell| !cell_text(cell).is_empty()))
        .map(|row| {
            let lab_name = value_at(row, lab_index);
            let project_code = value_at(row, project_index);
            if lab_name.is_empty() || project_code.is_empty() {
                return Err(AppError::Validation(
                    "汇总表存在缺少实验室或项目代号的数据行，禁止跳过导入".into(),
                ));
            }
            Ok(SummaryRow {
                row_key: id_index
                    .map(|index| value_at(row, index))
                    .unwrap_or_default(),
                lab_name,
                leader_name: value_at(row, leader_index),
                person_names: split_values(&value_at(row, people_index)),
                project_code,
                methods: split_values(&value_at(row, method_index)),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    validate_row_identities(&mut result)?;
    Ok(result)
}

fn context(pool: &DbPool, headers: &HeaderMap) -> Result<AuthContext> {
    authz_service::authenticate(pool, headers)
}
#[derive(Debug, Clone, Serialize)]
struct Capabilities {
    can_view_summary: bool,
    can_edit_summary: bool,
    can_view_notices: bool,
    can_submit: bool,
    can_review: bool,
    can_import_summary: bool,
    can_export_summary: bool,
    can_edit_config: bool,
    view_all_summary: bool,
    edit_all_summary: bool,
}
fn capabilities(ctx: &AuthContext) -> Capabilities {
    let admin = ctx.is_system_admin();
    let view = admin || ctx.has_permission("feedback:personnel:view");
    let edit = admin || ctx.has_permission("feedback:personnel:edit");
    let leader = ctx.is_analysis_leader() || ctx.is_rd_leader();
    let summary_edit = view && edit && (admin || leader);
    Capabilities {
        can_view_summary: view,
        can_edit_summary: summary_edit,
        can_view_notices: view && (admin || leader),
        can_submit: view && edit && (admin || ctx.is_rd_leader()),
        can_review: view && edit && (admin || ctx.is_analysis_leader()),
        can_import_summary: summary_edit && (admin || ctx.is_analysis_leader()),
        can_export_summary: view,
        can_edit_config: admin,
        view_all_summary: admin || ctx.is_analysis_member() || ctx.is_analysis_leader(),
        edit_all_summary: admin || ctx.is_analysis_leader(),
    }
}
fn is_reviewer(ctx: &AuthContext) -> bool {
    capabilities(ctx).can_review
}
fn require_viewer(pool: &DbPool, headers: &HeaderMap) -> Result<AuthContext> {
    let ctx = context(pool, headers)?;
    if !capabilities(&ctx).can_view_notices {
        return Err(AppError::Forbidden("缺少人员变动通知查看权限".into()));
    }
    Ok(ctx)
}
fn require_summary_viewer(pool: &DbPool, headers: &HeaderMap) -> Result<AuthContext> {
    let ctx = context(pool, headers)?;
    if !capabilities(&ctx).can_view_summary {
        return Err(AppError::Forbidden("缺少人员汇总表查看权限".into()));
    }
    Ok(ctx)
}
fn require_submitter(pool: &DbPool, headers: &HeaderMap) -> Result<AuthContext> {
    let ctx = require_viewer(pool, headers)?;
    if !capabilities(&ctx).can_submit {
        return Err(AppError::Forbidden(
            "仅有编辑权限的管理员或研发送样组长可以发起人员变动".into(),
        ));
    }
    Ok(ctx)
}
fn require_reviewer(pool: &DbPool, headers: &HeaderMap) -> Result<AuthContext> {
    let ctx = require_viewer(pool, headers)?;
    if !is_reviewer(&ctx) {
        return Err(AppError::Forbidden("缺少人员变动审批权限".into()));
    }
    Ok(ctx)
}

fn labs_for_user(pool: &DbPool, user_id: i64) -> Result<Vec<PersonnelChangeLab>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare("SELECT DISTINCT g.id,g.name FROM project_groups g JOIN (SELECT group_id FROM users WHERE id=?1 AND group_id IS NOT NULL UNION SELECT group_id FROM user_groups WHERE user_id=?1) own ON own.group_id=g.id WHERE g.deleted_at IS NULL ORDER BY g.name,g.id")?;
    Ok(stmt
        .query_map([user_id], |row| {
            Ok(PersonnelChangeLab {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?)
}

fn all_labs(pool: &DbPool) -> Result<Vec<PersonnelChangeLab>> {
    let conn = pool.get()?;
    let mut stmt = conn
        .prepare("SELECT id,name FROM project_groups WHERE deleted_at IS NULL ORDER BY name,id")?;
    Ok(stmt
        .query_map([], |row| {
            Ok(PersonnelChangeLab {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?)
}

fn labs_for_context(pool: &DbPool, ctx: &AuthContext) -> Result<Vec<PersonnelChangeLab>> {
    if ctx.is_system_admin() {
        all_labs(pool)
    } else {
        labs_for_user(pool, ctx.user.id)
    }
}

fn visible_summary(
    ctx: &AuthContext,
    labs: &[PersonnelChangeLab],
    rows: Vec<SummaryRow>,
) -> Vec<SummaryRow> {
    if capabilities(ctx).view_all_summary {
        return rows;
    }
    let allowed = labs
        .iter()
        .map(|lab| lab.name.as_str())
        .collect::<BTreeSet<_>>();
    rows.into_iter()
        .filter(|row| allowed.contains(row.lab_name.as_str()))
        .collect()
}

fn lab_projects(summary: &[SummaryRow]) -> HashMap<String, Vec<String>> {
    let mut projects = HashMap::<String, BTreeSet<String>>::new();
    for row in summary {
        projects
            .entry(row.lab_name.clone())
            .or_default()
            .insert(row.project_code.clone());
    }
    projects
        .into_iter()
        .map(|(lab, values)| (lab, values.into_iter().collect()))
        .collect()
}

fn validate_atomic_names(values: &[String], label: &str) -> Result<()> {
    if values.iter().any(|value| {
        value
            .chars()
            .any(|ch| matches!(ch, ',' | '，' | '、' | ';' | '；' | '\n' | '\r'))
    }) {
        return Err(AppError::Validation(format!(
            "{label}请逐条填写，不能包含逗号、顿号、分号或换行"
        )));
    }
    Ok(())
}

fn cleaned_values(values: &[String]) -> BTreeSet<String> {
    values
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn validate_notice(
    body: &NoticeCreate,
    labs: &[PersonnelChangeLab],
    summary: &[SummaryRow],
    change_types: &[String],
    fields: &[FeedbackFieldConfig],
) -> Result<()> {
    let change_type = canonical_label(&body.action_key)?.to_string();
    validate_atomic_names(&body.method_names, "检测方法")?;
    if let Some(name) = &body.person_name {
        validate_atomic_names(std::slice::from_ref(name), "人员姓名")?;
    }
    validate_field_values(body, fields)?;
    if change_type == "仅通知" {
        if !labs.iter().any(|lab| lab.name == body.lab_name.trim()) {
            return Err(AppError::Forbidden("实验室不在提交范围内".into()));
        }
        validate_date(&body.effective_at)?;
        return Ok(());
    }
    if !change_types.iter().any(|item| item == &change_type) {
        return Err(AppError::Validation("变动类型不存在或已停用".into()));
    }
    validate_date(&body.effective_at)?;
    if change_type != "实验室新增方法" && body.effective_at.trim().is_empty() {
        return Err(AppError::Validation("请填写生效日期".into()));
    }
    let target_lab_name = body.target_lab_name.as_deref().unwrap_or("").trim();
    let effective_lab_name = if change_type == "跨实验室人员调入" {
        target_lab_name
    } else {
        body.lab_name.trim()
    };
    if !labs.iter().any(|lab| lab.name == effective_lab_name) {
        return Err(AppError::Forbidden(
            "只能选择当前账号关联范围内的实验室".into(),
        ));
    }
    if !summary.iter().any(|row| row.lab_name == effective_lab_name) {
        return Err(AppError::Validation(
            "所选实验室不在当前项目及人员汇总表中".into(),
        ));
    }
    if change_type == "跨实验室人员调入" {
        if target_lab_name.is_empty() {
            return Err(AppError::Validation(
                "跨实验室人员调入必须选择目标实验室".into(),
            ));
        }
        if !labs.iter().any(|lab| lab.name == target_lab_name) {
            return Err(AppError::Forbidden(
                "目标实验室不在当前账号关联范围内".into(),
            ));
        }
    }
    let person_required = matches!(
        change_type.as_str(),
        "员工加入" | "员工离职" | "实验室内人员调动" | "跨实验室人员调出" | "跨实验室人员调入"
    );
    if person_required && body.person_name.as_deref().unwrap_or("").trim().is_empty() {
        return Err(AppError::Validation("该变动类型必须填写人员".into()));
    }
    let project_codes = cleaned_values(&body.project_codes);
    let source_project_codes = cleaned_values(&body.source_project_codes);
    let target_project_codes = cleaned_values(&body.target_project_codes);
    let lab_project_codes = summary
        .iter()
        .filter(|row| row.lab_name == body.lab_name.trim())
        .map(|row| row.project_code.as_str())
        .collect::<BTreeSet<_>>();
    if project_codes
        .iter()
        .any(|code| !lab_project_codes.contains(code.as_str()))
    {
        return Err(AppError::Validation(
            "关联项目必须来自所选实验室的汇总表".into(),
        ));
    }
    let target_projects_lab = if change_type == "跨实验室人员调入" {
        target_lab_name
    } else {
        body.lab_name.trim()
    };
    let target_lab_project_codes = summary
        .iter()
        .filter(|row| row.lab_name == target_projects_lab)
        .map(|row| row.project_code.as_str())
        .collect::<BTreeSet<_>>();
    if source_project_codes
        .iter()
        .any(|code| !lab_project_codes.contains(code.as_str()))
        || target_project_codes
            .iter()
            .any(|code| !target_lab_project_codes.contains(code.as_str()))
    {
        return Err(AppError::Validation(
            "来源或目标项目必须来自对应实验室的汇总表".into(),
        ));
    }
    if change_type == "跨实验室人员调入" && target_project_codes.is_empty() {
        return Err(AppError::Validation(
            "人员迁入需至少选择一个目标实验室项目".into(),
        ));
    }
    if change_type == "实验室内人员调动"
        && (source_project_codes.is_empty() || target_project_codes.is_empty())
    {
        return Err(AppError::Validation(
            "实验室内人员调动需分别选择来源项目和目标项目".into(),
        ));
    }
    if change_type == "实验室内人员调动" {
        let person_name = body.person_name.as_deref().unwrap_or("").trim();
        let person_projects = summary
            .iter()
            .filter(|row| {
                row.lab_name == body.lab_name.trim()
                    && row.person_names.iter().any(|name| name == person_name)
            })
            .map(|row| row.project_code.as_str())
            .collect::<BTreeSet<_>>();
        if source_project_codes
            .iter()
            .any(|code| !person_projects.contains(code.as_str()))
        {
            return Err(AppError::Validation(
                "来源项目必须是该人员当前已关联的项目".into(),
            ));
        }
    }
    if change_type == "员工加入" && project_codes.is_empty() {
        return Err(AppError::Validation(
            "人员加入或迁入需至少选择一个当前实验室项目".into(),
        ));
    }
    if change_type == "跨实验室人员调出" {
        let person_name = body.person_name.as_deref().unwrap_or("").trim();
        if !summary.iter().any(|row| {
            row.lab_name == body.lab_name.trim()
                && row.person_names.iter().any(|name| name == person_name)
        }) {
            return Err(AppError::Validation(
                "跨实验室人员调出仅适用于当前实验室已关联项目的人员".into(),
            ));
        }
    }
    if change_type == "实验室新增项目" {
        let new_code = body.new_project_code.as_deref().unwrap_or("").trim();
        if new_code.is_empty() {
            return Err(AppError::Validation("新增项目必须填写项目代号".into()));
        }
        if lab_project_codes.contains(new_code) {
            return Err(AppError::Conflict("该实验室已存在相同项目代号".into()));
        }
        if body.method_names.iter().all(|item| item.trim().is_empty()) {
            return Err(AppError::Validation(
                "新增项目需手动填写至少一个检测方法".into(),
            ));
        }
        if body.is_high_tech.unwrap_or(false)
            && body
                .high_tech_name
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty()
        {
            return Err(AppError::Validation("高新项目需填写项目名称".into()));
        }
    }
    if change_type == "实验室新增方法"
        && (project_codes.is_empty() || body.method_names.iter().all(|item| item.trim().is_empty()))
    {
        return Err(AppError::Validation(
            "新增方法需选择项目并填写方法名称".into(),
        ));
    }
    Ok(())
}

fn normalize_notice(notice: &mut NoticeLog) {
    if notice.action_key.is_empty() {
        notice.action_key = action_for_label(&notice.change_type)
            .unwrap_or("legacy_unknown")
            .into();
    }
    notice.change_type = normalize_change_type(&notice.change_type);
    // Old labels are not evidence of approval or a completed workbook update.
    // Keep unresolved states read-only for manual reconciliation.
    if notice.updated_at.trim().is_empty() {
        notice.updated_at = notice.created_at.clone();
    }
}

fn notice_matches_query(
    notice: &NoticeLog,
    cutoff: &str,
    actor_username: Option<&str>,
    notice_no: Option<&str>,
) -> bool {
    let matches_target = match notice_no {
        Some(number) => notice.notice_no == number,
        None => notice.created_at.as_str() >= cutoff,
    };
    matches_target && actor_username.is_none_or(|actor| notice.actor_username == actor)
}

fn merge_notice_lines(
    name: &str,
    content: &str,
    latest: &mut HashMap<String, NoticeLog>,
) -> Result<()> {
    for (index, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let mut notice = serde_json::from_str::<NoticeLog>(line).map_err(|_| {
            AppError::Validation(format!(
                "人员台账{name}第{}行损坏，已阻止操作；请管理员从备份核对恢复",
                index + 1
            ))
        })?;
        normalize_notice(&mut notice);
        let was_applied = notice.summary_applied
            || notice.review_status == "已同步"
            || latest
                .get(&notice.notice_no)
                .is_some_and(|n| n.summary_applied);
        if latest
            .get(&notice.notice_no)
            .map(|current| {
                (notice.revision, &notice.updated_at) >= (current.revision, &current.updated_at)
            })
            .unwrap_or(true)
        {
            notice.summary_applied = was_applied;
            latest.insert(notice.notice_no.clone(), notice);
        } else if was_applied {
            if let Some(current) = latest.get_mut(&notice.notice_no) {
                current.summary_applied = true;
            }
        }
    }
    Ok(())
}

fn read_notices(
    days: i64,
    actor_username: Option<&str>,
    notice_no: Option<&str>,
) -> Result<Vec<NoticeLog>> {
    if notice_no.is_some_and(|value| {
        value.is_empty()
            || value.len() > 100
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    }) {
        return Err(AppError::Validation("人员变动通知编号无效".into()));
    }
    let dir = ensure_feature_dir()?;
    let cutoff = if days == i64::MAX {
        String::new()
    } else {
        (Local::now().naive_local() - Duration::days(days.clamp(1, 3650)))
            .format("%Y-%m-%d %H:%M:%S")
            .to_string()
    };
    let mut latest = HashMap::<String, NoticeLog>::new();
    for entry in fs::read_dir(dir)
        .map_err(|error| AppError::Internal(format!("读取通知台账失败: {error}")))?
    {
        let entry =
            entry.map_err(|error| AppError::Internal(format!("读取通知台账目录失败: {error}")))?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("notices-") || !name.ends_with(".jsonl") {
            continue;
        }
        let content = fs::read_to_string(entry.path())
            .map_err(|error| AppError::Internal(format!("读取通知台账文件失败: {error}")))?;
        merge_notice_lines(&name, &content, &mut latest)?;
    }
    let mut notices = latest
        .into_values()
        .filter(|notice| notice_matches_query(notice, &cutoff, actor_username, notice_no))
        .collect::<Vec<_>>();
    notices.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    Ok(notices)
}

fn append_name(names: &mut Vec<String>, name: &str) {
    let mut values = names.iter().cloned().collect::<BTreeSet<_>>();
    if !name.trim().is_empty() {
        values.insert(name.trim().to_string());
    }
    *names = values.into_iter().collect();
}

fn remove_name(names: &mut Vec<String>, name: &str) {
    names.retain(|item| item != name.trim());
}

fn merge_methods(methods: &mut Vec<String>, additions: &[String]) {
    let mut values = methods.iter().cloned().collect::<BTreeSet<_>>();
    values.extend(cleaned_values(additions));
    *methods = values.into_iter().collect();
}

fn apply_summary_change(notice: &NoticeLog, rows: &mut Vec<SummaryRow>) -> Result<()> {
    let selected = notice
        .project_codes
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let source_selected = if notice.source_project_codes.is_empty() {
        selected.clone()
    } else {
        notice.source_project_codes.iter().cloned().collect()
    };
    let target_selected = if notice.target_project_codes.is_empty() {
        selected.clone()
    } else {
        notice.target_project_codes.iter().cloned().collect()
    };
    let all_current = rows
        .iter()
        .filter(|row| {
            row.lab_name == notice.lab_name && row.person_names.contains(&notice.person_name)
        })
        .map(|row| row.project_code.clone())
        .collect::<BTreeSet<_>>();
    match canonical_label(&notice.action_key)? {
        "员工加入" => {
            if selected.is_empty() {
                return Err(AppError::Validation("员工加入需选择关联项目".into()));
            }
            for row in rows.iter_mut().filter(|row| {
                row.lab_name == notice.lab_name && selected.contains(&row.project_code)
            }) {
                append_name(&mut row.person_names, &notice.person_name);
            }
        }
        "跨实验室人员调入" => {
            if target_selected.is_empty() {
                return Err(AppError::Validation("人员迁入需选择目标项目".into()));
            }
            for row in rows.iter_mut().filter(|row| {
                row.lab_name == notice.target_lab_name
                    && target_selected.contains(&row.project_code)
            }) {
                append_name(&mut row.person_names, &notice.person_name);
            }
        }
        "员工离职" => {
            let affected = if selected.is_empty() {
                &all_current
            } else {
                &selected
            };
            for row in rows.iter_mut().filter(|row| {
                row.lab_name == notice.lab_name && affected.contains(&row.project_code)
            }) {
                remove_name(&mut row.person_names, &notice.person_name);
            }
        }
        "实验室内人员调动" => {
            if notice.source_project_codes.is_empty() && notice.target_project_codes.is_empty() {
                for row in rows
                    .iter_mut()
                    .filter(|row| row.lab_name == notice.lab_name)
                {
                    if selected.contains(&row.project_code) {
                        append_name(&mut row.person_names, &notice.person_name);
                    } else {
                        remove_name(&mut row.person_names, &notice.person_name);
                    }
                }
            } else {
                for row in rows.iter_mut().filter(|row| {
                    row.lab_name == notice.lab_name && source_selected.contains(&row.project_code)
                }) {
                    remove_name(&mut row.person_names, &notice.person_name);
                }
                for row in rows.iter_mut().filter(|row| {
                    row.lab_name == notice.lab_name && target_selected.contains(&row.project_code)
                }) {
                    append_name(&mut row.person_names, &notice.person_name);
                }
            }
        }
        "跨实验室人员调出" => {
            let affected = if source_selected.is_empty() {
                &all_current
            } else {
                &source_selected
            };
            for row in rows.iter_mut().filter(|row| {
                row.lab_name == notice.lab_name && affected.contains(&row.project_code)
            }) {
                remove_name(&mut row.person_names, &notice.person_name);
            }
        }
        "实验室新增项目" => {
            if rows.iter().any(|row| {
                row.lab_name == notice.lab_name && row.project_code == notice.new_project_code
            }) {
                return Err(AppError::Conflict("该实验室已存在相同项目代号".into()));
            }
            let leader_name = rows
                .iter()
                .find(|row| row.lab_name == notice.lab_name)
                .map(|row| row.leader_name.clone())
                .unwrap_or_default();
            rows.push(SummaryRow {
                row_key: fresh_row_key(),
                lab_name: notice.lab_name.clone(),
                leader_name,
                person_names: split_values(&notice.person_name),
                project_code: notice.new_project_code.clone(),
                methods: cleaned_values(&notice.method_names).into_iter().collect(),
            });
        }
        "实验室新增方法" => {
            for row in rows.iter_mut().filter(|row| {
                row.lab_name == notice.lab_name && selected.contains(&row.project_code)
            }) {
                merge_methods(&mut row.methods, &notice.method_names);
            }
        }
        "仅通知" => {}
        _ => return Err(AppError::Validation("变动动作不支持同步".into())),
    }
    // Applying a departure can legitimately make two previously distinct
    // contexts identical. Keep one stable identity only in this business flow.
    rows.sort_by(|left, right| left.row_key.cmp(&right.row_key));
    let mut contexts = BTreeSet::new();
    rows.retain(|row| contexts.insert(business_row_key(row)));
    validate_row_identities(rows)?;
    rows.sort_by(|left, right| {
        (&left.lab_name, &left.project_code, &left.row_key).cmp(&(
            &right.lab_name,
            &right.project_code,
            &right.row_key,
        ))
    });
    Ok(())
}

fn summary_bytes(rows: &[SummaryRow]) -> Result<Vec<u8>> {
    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();
    sheet.set_name(SUMMARY_SHEET)?;
    for (column, width) in [20.0, 18.0, 28.0, 22.0, 32.0].iter().enumerate() {
        sheet.set_column_width(column as u16, *width)?;
    }
    let header = Format::new().set_bold();
    let wrapped = Format::new()
        .set_text_wrap()
        .set_align(rust_xlsxwriter::FormatAlign::Top);
    for (column, title) in [
        "实验室",
        "实验室负责人",
        "实验人员",
        "实验室项目代号",
        "检测方法",
    ]
    .iter()
    .enumerate()
    {
        sheet.write_string_with_format(0, column as u16, *title, &header)?;
    }
    for (index, row) in rows.iter().enumerate() {
        let excel_row = (index + 1) as u32;
        sheet.write_string(excel_row, 0, &row.lab_name)?;
        sheet.write_string(excel_row, 1, &row.leader_name)?;
        sheet.write_string_with_format(excel_row, 2, row.person_names.join("、"), &wrapped)?;
        sheet.write_string_with_format(excel_row, 3, &row.project_code, &wrapped)?;
        sheet.write_string_with_format(excel_row, 4, row.methods.join("\n"), &wrapped)?;
        sheet.write_string(
            excel_row,
            5,
            if row.row_key.is_empty() {
                business_row_key(row)
            } else {
                row.row_key.clone()
            },
        )?;
    }
    sheet.write_string(0, 5, "LabFlow行标识")?;
    sheet.set_column_hidden(5)?;
    Ok(workbook.save_to_buffer()?)
}

fn first_revision() -> u64 {
    1
}
fn action_for_label(label: &str) -> Option<&'static str> {
    match label.trim() {
        "员工加入" | "新员工加入" => Some("employee_join"),
        "员工离职" => Some("employee_leave"),
        "实验室内人员调动" => Some("internal_transfer"),
        "跨实验室人员调出" | "跨实验室人员调动" => Some("transfer_out"),
        "跨实验室人员调入" => Some("transfer_in"),
        "实验室新增项目" => Some("project_add"),
        "实验室新增方法" => Some("method_add"),
        "仅通知" => Some("notice_only"),
        _ => None,
    }
}
fn canonical_label(action: &str) -> Result<&'static str> {
    Ok(match action {
        "employee_join" => "员工加入",
        "employee_leave" => "员工离职",
        "internal_transfer" => "实验室内人员调动",
        "transfer_out" => "跨实验室人员调出",
        "transfer_in" => "跨实验室人员调入",
        "project_add" => "实验室新增项目",
        "method_add" => "实验室新增方法",
        "notice_only" => "仅通知",
        _ => {
            return Err(AppError::Validation(
                "该变动类型没有可执行动作，请管理员明确配置后重新提交".into(),
            ))
        }
    })
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn business_row_key(row: &SummaryRow) -> String {
    digest(
        &serde_json::to_vec(&(
            row.lab_name.trim(),
            row.leader_name.trim(),
            row.project_code.trim(),
            cleaned_values(&row.person_names),
            cleaned_values(&row.methods),
        ))
        .expect("serializing strings cannot fail"),
    )
}
fn fresh_row_key() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}
fn validate_row_identities(rows: &mut [SummaryRow]) -> Result<()> {
    let mut identities = BTreeSet::new();
    let mut contexts = BTreeSet::new();
    // Reserve all supplied identities first, regardless of workbook row order.
    for row in rows.iter() {
        let context = business_row_key(row);
        if !contexts.insert(context) {
            return Err(AppError::Validation(
                "汇总表存在完全相同的业务行，请先去重".into(),
            ));
        }
        if row.row_key.is_empty() {
            continue;
        }
        if row.row_key.len() != 64
            || !row.row_key.bytes().all(|b| b.is_ascii_hexdigit())
            || !identities.insert(row.row_key.clone())
        {
            return Err(AppError::Validation(
                "汇总行标识重复或无效，请核对隐藏行标识列".into(),
            ));
        }
    }
    for row in rows.iter_mut().filter(|row| row.row_key.is_empty()) {
        let mut identity = business_row_key(row);
        while !identities.insert(identity.clone()) {
            identity = fresh_row_key();
        }
        row.row_key = identity;
    }
    Ok(())
}
fn summary_revision() -> Result<String> {
    match fs::read(summary_path()) {
        Ok(bytes) => Ok(digest(&bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(digest(b"no-summary")),
        Err(error) => Err(AppError::Internal(format!("读取汇总基线失败: {error}"))),
    }
}
fn check_revision(expected: &str) -> Result<()> {
    if expected.is_empty() || expected != summary_revision()? {
        return Err(AppError::Conflict(
            "汇总表已变化或缺少基线，请刷新核对后重试".into(),
        ));
    }
    Ok(())
}
fn validate_date(value: &str) -> Result<()> {
    if value.len() != 10 || NaiveDate::parse_from_str(value, "%Y-%m-%d").is_err() {
        return Err(AppError::Validation(
            "日期必须为有效的 YYYY-MM-DD；仅记录日期，不会定时执行".into(),
        ));
    }
    Ok(())
}
fn reason(value: &str) -> Result<&str> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 1000 {
        return Err(AppError::Validation("请填写1至1000字的操作原因".into()));
    }
    Ok(value)
}
fn builtin_type(key: &str) -> Option<&'static str> {
    match key {
        "change_type" => Some("select"),
        "lab_name" | "person_name" | "high_tech_name" | "target_lab_name"
        | "transfer_notice_no" | "new_project_code" => Some("text"),
        "project_codes" | "source_project_codes" | "target_project_codes" => Some("multi-select"),
        "method_names" => Some("multi-text"),
        "effective_at" => Some("date"),
        "is_high_tech" => Some("boolean"),
        "notes" => Some("textarea"),
        _ => None,
    }
}
fn is_builtin_key(key: &str) -> bool {
    builtin_type(key).is_some()
}
fn reserved_key(key: &str) -> bool {
    matches!(
        key,
        "action_key"
            | "summary_revision"
            | "baseline_fingerprint"
            | "summary_applied"
            | "revision"
            | "reason"
            | "notice_revision"
            | "actor_user_id"
            | "actor_username"
            | "notice_no"
            | "created_at"
            | "updated_at"
            | "review_status"
            | "delivery_status"
            | "reviewed_at"
            | "reviewed_by"
            | "review_reason"
            | "can_review"
            | "can_revise"
            | "can_withdraw"
            | "group_id"
            | "lab_id"
            | "submitted_by_user_id"
            | "reviewed_by_user_id"
            | "target_url"
            | "status"
            | "submitted_by"
    )
}
fn validate_field_values(body: &NoticeCreate, fields: &[FeedbackFieldConfig]) -> Result<()> {
    for field in fields.iter().filter(|f| {
        f.enabled
            && (f.change_types.is_empty()
                || f.change_types.contains(&body.action_key)
                || f.change_types.contains(&body.change_type))
    }) {
        let builtin_empty = match field.key.as_str() {
            "change_type" => Some(body.change_type.trim().is_empty()),
            "lab_name" => Some(body.lab_name.trim().is_empty()),
            "person_name" => Some(body.person_name.as_deref().unwrap_or("").trim().is_empty()),
            "project_codes" => Some(cleaned_values(&body.project_codes).is_empty()),
            "method_names" => Some(cleaned_values(&body.method_names).is_empty()),
            "source_project_codes" => Some(cleaned_values(&body.source_project_codes).is_empty()),
            "target_project_codes" => Some(cleaned_values(&body.target_project_codes).is_empty()),
            "effective_at" => Some(body.effective_at.is_empty()),
            "is_high_tech" => Some(body.is_high_tech.is_none()),
            "target_lab_name" => Some(
                body.target_lab_name
                    .as_deref()
                    .unwrap_or("")
                    .trim()
                    .is_empty(),
            ),
            "transfer_notice_no" => Some(
                body.transfer_notice_no
                    .as_deref()
                    .unwrap_or("")
                    .trim()
                    .is_empty(),
            ),
            "new_project_code" => Some(
                body.new_project_code
                    .as_deref()
                    .unwrap_or("")
                    .trim()
                    .is_empty(),
            ),
            _ => None,
        };
        if let Some(empty) = builtin_empty {
            if field.required && empty {
                return Err(AppError::Validation(format!("请填写{}", field.label)));
            }
            continue;
        }
        let value = match field.key.as_str() {
            "notes" => body.notes.as_deref().unwrap_or(""),
            "high_tech_name" => body.high_tech_name.as_deref().unwrap_or(""),
            key => body.extra_fields.get(key).map(String::as_str).unwrap_or(""),
        };
        if value.chars().count() > 10000 {
            return Err(AppError::Validation(format!("{}内容过长", field.label)));
        }
        if field.required && value.trim().is_empty() {
            return Err(AppError::Validation(format!("请填写{}", field.label)));
        }
        if value.is_empty() {
            continue;
        }
        match field.field_type.as_str() {
            "date" => validate_date(value)?,
            "boolean" if !["true", "false"].contains(&value) => {
                return Err(AppError::Validation(format!("{}必须为是或否", field.label)))
            }
            "select" if !field.options.iter().any(|v| v == value) => {
                return Err(AppError::Validation(format!(
                    "{}不在配置选项内",
                    field.label
                )))
            }
            "multi-select" | "multi-text" => {
                let values: Vec<String> = serde_json::from_str(value)
                    .map_err(|_| AppError::Validation(format!("{}列表格式无效", field.label)))?;
                if values.len() > 200
                    || values
                        .iter()
                        .any(|v| v.trim().is_empty() || v.chars().count() > 1000)
                    || (field.field_type == "multi-select"
                        && values.iter().any(|v| !field.options.contains(v)))
                    || (field.required && values.is_empty())
                {
                    return Err(AppError::Validation(format!(
                        "{}包含空值、过长内容或不可用选项",
                        field.label
                    )));
                }
            }
            _ => {}
        }
    }
    if body.extra_fields.keys().any(|key| {
        reserved_key(key)
            || !fields.iter().any(|f| {
                f.key == *key
                    && f.enabled
                    && !is_builtin_key(key)
                    && (f.change_types.is_empty()
                        || f.change_types.contains(&body.action_key)
                        || f.change_types.contains(&body.change_type))
            })
    }) {
        return Err(AppError::Validation(
            "附加字段包含未启用或未知字段，请刷新表单".into(),
        ));
    }
    Ok(())
}
fn all_notices(pool: &DbPool) -> Result<Vec<NoticeLog>> {
    let mut notices = read_notices(i64::MAX, None, None)?;
    let conn = pool.get()?;
    for notice in &mut notices {
        if notice.actor_user_id.is_none() {
            // Only an existing immutable creation audit can bind a legacy actor.
            // A matching current username is insufficient after renames/reuse.
            let identities: (i64, Option<i64>) = conn.query_row("SELECT COUNT(DISTINCT COALESCE(operator_user_id,user_id)),MIN(COALESCE(operator_user_id,user_id)) FROM audit_log WHERE table_name='personnel_change_notices' AND action='create' AND business_no=?1", [&notice.notice_no], |row| Ok((row.get(0)?,row.get(1)?)))?;
            if identities.0 == 1 {
                notice.actor_user_id = identities.1;
            }
        }
    }
    Ok(notices)
}
fn notice_visible(ctx: &AuthContext, notice: &NoticeLog) -> bool {
    capabilities(ctx).can_view_notices
        && (ctx.is_system_admin()
            || ctx.is_analysis_leader()
            || notice.actor_user_id == Some(ctx.user.id))
}
fn present_notice(pool: &DbPool, ctx: &AuthContext, mut notice: NoticeLog) -> Result<NoticeLog> {
    let own = notice.actor_user_id == Some(ctx.user.id);
    notice.can_review = capabilities(ctx).can_review
        && !own
        && notice.actor_user_id.is_some()
        && notice.review_status == "待审批";
    notice.can_revise = capabilities(ctx).can_submit
        && own
        && ["已驳回", "已撤回"].contains(&notice.review_status.as_str());
    notice.can_withdraw = capabilities(ctx).can_submit && own && notice.review_status == "待审批";
    notice.delivery_status =
        crate::service::notification_service::personnel_delivery_status(pool, &notice.notice_no)?;
    Ok(notice)
}
fn find_notice(pool: &DbPool, number: &str) -> Result<NoticeLog> {
    all_notices(pool)?
        .into_iter()
        .find(|n| n.notice_no == number)
        .ok_or_else(|| AppError::NotFound("未找到人员变动通知".into()))
}
fn notice_file(notice: &NoticeLog) -> Result<storage::FileChange> {
    let name = format!("notices-{}.jsonl", Local::now().format("%Y-%m"));
    let path = ensure_feature_dir()?.join(&name);
    let mut bytes = if path.exists() {
        fs::read(path).map_err(|e| AppError::Internal(e.to_string()))?
    } else {
        vec![]
    };
    if bytes.last().is_some_and(|v| *v != b'\n') {
        bytes.push(b'\n');
    }
    bytes.extend(serde_json::to_vec(notice).map_err(|e| AppError::Internal(e.to_string()))?);
    bytes.push(b'\n');
    storage::change(&name, bytes)
}
fn notification_payload(
    pool: &DbPool,
    notice: &NoticeLog,
    reviewer: Option<&AuthContext>,
) -> Result<serde_json::Value> {
    let group_id = all_labs(pool)?
        .into_iter()
        .find(|l| l.name == notice.lab_name)
        .map(|l| l.id);
    Ok(
        json!({"notice_no":notice.notice_no,"revision":notice.revision,"action_key":notice.action_key,"created_at":notice.created_at,
        "submitted_by":notice.actor_username,"submitted_by_user_id":notice.actor_user_id,"actor_user_id":notice.actor_user_id,
        "lab_name":notice.lab_name,"target_lab_name":notice.target_lab_name,"group_id":group_id,"lab_id":group_id,
        "change_type":notice.change_type,"person_name":notice.person_name,"project":notice.project_codes.join("、"),
        "project_name":notice.project_codes.join("、"),"project_codes":notice.project_codes.join("、"),
        "source_project_codes":notice.source_project_codes.join("、"),"target_project_codes":notice.target_project_codes.join("、"),
        "transfer_notice_no":notice.transfer_notice_no,"new_project_code":notice.new_project_code,
        "method":notice.method_names.join("、"),"method_name":notice.method_names.join("、"),"method_names":notice.method_names.join("、"),
        "effective_at":notice.effective_at,"is_high_tech":if notice.is_high_tech {"是"} else {"否"},"high_tech_name":notice.high_tech_name,
        "status":notice.review_status,"notes":notice.notes,"extra_fields":notice.extra_fields,"reviewed_by":notice.reviewed_by,
        "reviewed_by_user_id":reviewer.map(|c| c.user.id),"reviewed_at":notice.reviewed_at,"review_reason":notice.review_reason,
        "updated_at":notice.updated_at,"target_url":"/personnel-change"}),
    )
}
fn validate_transfer(pool: &DbPool, notice: &NoticeLog) -> Result<()> {
    if notice.action_key != "transfer_in" {
        return Ok(());
    }
    if notice.transfer_notice_no.is_empty() {
        return Err(AppError::Validation(
            "跨实验室迁入必须关联已同步调出通知；外部新入职请选择员工加入".into(),
        ));
    }
    let source = find_notice(pool, &notice.transfer_notice_no)?;
    if source.action_key != "transfer_out"
        || source.review_status != "已同步"
        || source.person_name != notice.person_name
        || source.lab_name == notice.lab_name
    {
        return Err(AppError::Validation(
            "关联调出通知必须已同步、人员相同且来自其他实验室".into(),
        ));
    }
    if all_notices(pool)?.iter().any(|n| {
        n.notice_no != notice.notice_no
            && n.transfer_notice_no == notice.transfer_notice_no
            && transfer_reserves_source(n)
    }) {
        return Err(AppError::Conflict(
            "该调出通知已有关联调入，不能重复迁入".into(),
        ));
    }
    Ok(())
}
fn transfer_reserves_source(notice: &NoticeLog) -> bool {
    notice.action_key == "transfer_in"
        && (notice.summary_applied
            || !["已删除", "已撤回", "已驳回"].contains(&notice.review_status.as_str()))
}
fn prepare_body(mut body: NoticeCreate, config: &FeedbackConfig) -> Result<NoticeCreate> {
    let configured = config
        .change_types
        .iter()
        .find(|t| {
            t.enabled
                && if body.action_key.is_empty() {
                    t.label == body.change_type || t.name == body.change_type
                } else {
                    t.action_key == body.action_key
                        && (body.change_type.is_empty()
                            || t.label == body.change_type
                            || t.name == body.change_type)
                }
        })
        .ok_or_else(|| AppError::Validation("变动类型不存在或已停用，请刷新".into()))?;
    body.action_key = configured.action_key.clone();
    body.change_type = configured.label.clone();
    canonical_label(&body.action_key)?;
    if body.action_key == "transfer_in" {
        body.lab_name = body.target_lab_name.clone().unwrap_or_default();
    }
    Ok(body)
}
fn new_notice(body: &NoticeCreate, ctx: &AuthContext) -> NoticeLog {
    let now = Local::now();
    NoticeLog {
        summary_applied: false,
        affected_rows: 0,
        baseline_fingerprint: String::new(),
        actor_user_id: Some(ctx.user.id),
        action_key: body.action_key.clone(),
        revision: 1,
        summary_revision: body.summary_revision.clone(),
        can_review: false,
        can_revise: false,
        can_withdraw: false,
        notice_no: format!(
            "PC-{}-{}",
            now.format("%Y%m%d%H%M%S"),
            uuid::Uuid::new_v4().simple()
        ),
        created_at: now.format("%Y-%m-%d %H:%M:%S").to_string(),
        actor_username: ctx.user.username.clone(),
        change_type: body.change_type.clone(),
        lab_name: body.lab_name.trim().into(),
        target_lab_name: body
            .target_lab_name
            .clone()
            .unwrap_or_default()
            .trim()
            .into(),
        person_name: body.person_name.clone().unwrap_or_default().trim().into(),
        project_codes: cleaned_values(&body.project_codes).into_iter().collect(),
        source_project_codes: cleaned_values(&body.source_project_codes)
            .into_iter()
            .collect(),
        target_project_codes: cleaned_values(&body.target_project_codes)
            .into_iter()
            .collect(),
        transfer_notice_no: body
            .transfer_notice_no
            .clone()
            .unwrap_or_default()
            .trim()
            .into(),
        method_names: cleaned_values(&body.method_names).into_iter().collect(),
        new_project_code: body
            .new_project_code
            .clone()
            .unwrap_or_default()
            .trim()
            .into(),
        new_project_name: String::new(),
        is_high_tech: body.is_high_tech.unwrap_or(false),
        high_tech_name: body
            .high_tech_name
            .clone()
            .unwrap_or_default()
            .trim()
            .into(),
        effective_at: body.effective_at.clone(),
        notes: body.notes.clone().unwrap_or_default().trim().into(),
        extra_fields: body.extra_fields.clone(),
        delivery_status: "待发送".into(),
        review_status: "待审批".into(),
        reviewed_by: String::new(),
        reviewed_at: String::new(),
        review_reason: String::new(),
        updated_at: now.format("%Y-%m-%d %H:%M:%S").to_string(),
    }
}
fn notice_body(notice: &NoticeLog) -> NoticeCreate {
    NoticeCreate {
        action_key: notice.action_key.clone(),
        summary_revision: notice.summary_revision.clone(),
        reason: String::new(),
        notice_revision: notice.revision,
        change_type: notice.change_type.clone(),
        lab_name: notice.lab_name.clone(),
        person_name: Some(notice.person_name.clone()),
        project_codes: notice.project_codes.clone(),
        source_project_codes: notice.source_project_codes.clone(),
        target_lab_name: Some(notice.target_lab_name.clone()),
        target_project_codes: notice.target_project_codes.clone(),
        transfer_notice_no: Some(notice.transfer_notice_no.clone()),
        method_names: notice.method_names.clone(),
        new_project_code: Some(notice.new_project_code.clone()),
        is_high_tech: Some(notice.is_high_tech),
        high_tech_name: Some(notice.high_tech_name.clone()),
        effective_at: notice.effective_at.clone(),
        notes: Some(notice.notes.clone()),
        extra_fields: notice.extra_fields.clone(),
    }
}
fn affected_fingerprint(notice: &NoticeLog, rows: &[SummaryRow]) -> Result<String> {
    let chosen: BTreeSet<&str> = match notice.action_key.as_str() {
        "transfer_in" | "internal_transfer" => notice
            .target_project_codes
            .iter()
            .chain(notice.source_project_codes.iter())
            .map(String::as_str)
            .collect(),
        "transfer_out" if !notice.source_project_codes.is_empty() => notice
            .source_project_codes
            .iter()
            .map(String::as_str)
            .collect(),
        _ => notice.project_codes.iter().map(String::as_str).collect(),
    };
    let mut relevant = rows
        .iter()
        .filter(|r| r.lab_name == notice.lab_name)
        .filter(|r| match notice.action_key.as_str() {
            "notice_only" => false,
            "project_add" => r.project_code == notice.new_project_code,
            "employee_leave" | "transfer_out" if chosen.is_empty() => {
                r.person_names.contains(&notice.person_name)
            }
            _ => chosen.contains(r.project_code.as_str()),
        })
        .collect::<Vec<_>>();
    relevant.sort_by(|a, b| {
        (&a.lab_name, &a.project_code, &a.row_key).cmp(&(&b.lab_name, &b.project_code, &b.row_key))
    });
    let leader = if notice.action_key == "project_add" {
        rows.iter()
            .find(|r| r.lab_name == notice.lab_name)
            .map(|r| r.leader_name.as_str())
    } else {
        None
    };
    Ok(digest(
        &serde_json::to_vec(&(relevant, leader)).map_err(|e| AppError::Internal(e.to_string()))?,
    ))
}

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/personnel-change/options", get(options))
        .route(
            "/api/personnel-change/notices",
            get(notices).post(create_notice),
        )
        .route(
            "/api/personnel-change/notices/:notice_no/review",
            post(review_notice),
        )
        .route(
            "/api/personnel-change/notices/:notice_no/withdraw",
            post(withdraw_notice),
        )
        .route(
            "/api/personnel-change/notices/:notice_no/revise",
            post(revise_notice),
        )
        .route(
            "/api/personnel-change/notices/:notice_no",
            axum::routing::delete(delete_notice),
        )
        .route(
            "/api/personnel-change/labs/:lab_name/person-projects",
            get(person_projects),
        )
        .route(
            "/api/personnel-change/summary/rows",
            post(create_summary_row),
        )
        .route(
            "/api/personnel-change/summary/rows/:row_key",
            axum::routing::put(update_summary_row).delete(delete_summary_row),
        )
        .route("/api/personnel-change/summary/export", get(export_summary))
        .route(
            "/api/personnel-change/summary/import/preview",
            post(preview_import),
        )
        .route(
            "/api/personnel-change/summary/import/confirm",
            post(confirm_import),
        )
        .route(
            "/api/personnel-change/config",
            get(get_config).put(update_config),
        )
        .with_state(pool)
}
async fn options(
    State(pool): State<DbPool>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<PersonnelChangeOptions>>> {
    let ctx = require_summary_viewer(&pool, &headers)?;
    let _guard = storage::lock(&pool)?;
    let labs = labs_for_context(&pool, &ctx)?;
    let rows = load_summary(&summary_path())?;
    let config = load_feedback_config()?;
    let projects = lab_projects(
        &rows
            .iter()
            .filter(|r| labs.iter().any(|l| l.name == r.lab_name))
            .cloned()
            .collect::<Vec<_>>(),
    );
    Ok(Json(ApiResponse::ok(PersonnelChangeOptions {
        summary_available: !rows.is_empty(),
        summary: visible_summary(&ctx, &labs, rows),
        labs,
        summary_labs: if capabilities(&ctx).edit_all_summary {
            all_labs(&pool)?
        } else {
            labs_for_user(&pool, ctx.user.id)?
        },
        lab_projects: projects,
        change_types: config
            .change_types
            .iter()
            .filter(|t| t.enabled)
            .map(|t| t.label.clone())
            .collect(),
        fields: config.fields,
        change_type_configs: config.change_types,
        summary_revision: summary_revision()?,
        capabilities: capabilities(&ctx),
    })))
}
async fn notices(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(query): Query<NoticeQuery>,
) -> Result<Json<ApiResponse<Vec<NoticeLog>>>> {
    let ctx = require_viewer(&pool, &headers)?;
    let _guard = storage::lock(&pool)?;
    if let Some(start) = &query.start {
        validate_date(start)?;
    }
    if let Some(end) = &query.end {
        validate_date(end)?;
    }
    if query
        .start
        .as_ref()
        .zip(query.end.as_ref())
        .is_some_and(|(s, e)| s > e)
    {
        return Err(AppError::Validation("开始日期不能晚于结束日期".into()));
    }
    if query.notice_no.as_ref().is_some_and(|n| {
        n.is_empty() || n.len() > 100 || !n.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    }) {
        return Err(AppError::Validation("人员变动通知编号无效".into()));
    }
    let cutoff = (Local::now() - Duration::days(query.days.unwrap_or(7).clamp(1, 3650)))
        .format("%Y-%m-%d")
        .to_string();
    let keyword = query.keyword.unwrap_or_default().to_lowercase();
    let rows = all_notices(&pool)?
        .into_iter()
        .filter(|n| notice_visible(&ctx, n))
        .filter(|n| {
            if let Some(number) = &query.notice_no {
                return &n.notice_no == number;
            }
            let day = n.created_at.get(..10).unwrap_or("");
            let date_ok = if query.start.is_some() || query.end.is_some() {
                query.start.as_ref().is_none_or(|s| day >= s.as_str())
                    && query.end.as_ref().is_none_or(|e| day <= e.as_str())
            } else {
                n.review_status == "待审批" || day >= cutoff.as_str()
            };
            date_ok
                && query
                    .status
                    .as_ref()
                    .is_none_or(|s| s.is_empty() || s == &n.review_status)
                && (keyword.is_empty()
                    || format!(
                        "{} {} {} {} {}",
                        n.notice_no, n.lab_name, n.person_name, n.change_type, n.actor_username
                    )
                    .to_lowercase()
                    .contains(&keyword))
        })
        .map(|n| present_notice(&pool, &ctx, n))
        .collect::<Result<Vec<_>>>()?;
    Ok(Json(ApiResponse::ok(rows)))
}
async fn person_projects(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    AxumPath(lab): AxumPath<String>,
    Query(query): Query<PersonProjectQuery>,
) -> Result<Json<ApiResponse<Vec<String>>>> {
    let ctx = require_submitter(&pool, &headers)?;
    let _guard = storage::lock(&pool)?;
    if !labs_for_context(&pool, &ctx)?.iter().any(|l| l.name == lab) {
        return Err(AppError::Forbidden("实验室不在提交范围内".into()));
    }
    Ok(Json(ApiResponse::ok(
        load_summary(&summary_path())?
            .into_iter()
            .filter(|r| {
                r.lab_name == lab && r.person_names.iter().any(|p| p == query.person_name.trim())
            })
            .map(|r| r.project_code)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
    )))
}
async fn create_notice(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(body): Json<NoticeCreate>,
) -> Result<Json<ApiResponse<NoticeLog>>> {
    let ctx = require_submitter(&pool, &headers)?;
    let _guard = storage::lock(&pool)?;
    check_revision(&body.summary_revision)?;
    let config = load_feedback_config()?;
    let body = prepare_body(body, &config)?;
    validate_notice(
        &body,
        &labs_for_context(&pool, &ctx)?,
        &load_summary(&summary_path())?,
        &config
            .change_types
            .iter()
            .filter(|t| t.enabled)
            .map(|t| canonical_label(&t.action_key).unwrap_or("").to_string())
            .collect::<Vec<_>>(),
        &config.fields,
    )?;
    let mut notice = new_notice(&body, &ctx);
    notice.baseline_fingerprint = affected_fingerprint(&notice, &load_summary(&summary_path())?)?;
    validate_transfer(&pool, &notice)?;
    let payload = notification_payload(&pool, &notice, None)?;
    storage::mutate(
        &pool,
        &ctx,
        "create",
        &notice.notice_no,
        "发起人员变动通知",
        json!(null),
        json!(notice),
        vec![notice_file(&notice)?],
        Some(("submitted", &payload)),
    )?;
    Ok(Json(ApiResponse::ok(present_notice(&pool, &ctx, notice)?)))
}
fn check_notice_revision(notice: &NoticeLog, revision: u64) -> Result<()> {
    if notice.revision != revision {
        return Err(AppError::Conflict("通知已变化，请刷新后重试".into()));
    }
    Ok(())
}
async fn review_notice(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    AxumPath(number): AxumPath<String>,
    Json(body): Json<NoticeReview>,
) -> Result<Json<ApiResponse<NoticeLog>>> {
    let ctx = require_reviewer(&pool, &headers)?;
    let _guard = storage::lock(&pool)?;
    let mut notice = find_notice(&pool, &number)?;
    check_notice_revision(&notice, body.notice_revision)?;
    if notice.actor_user_id.is_none() {
        return Err(AppError::Conflict(
            "历史发起人身份无法可靠确认，禁止审批；请管理员核对原始审计后人工维护汇总".into(),
        ));
    }
    if notice.actor_user_id == Some(ctx.user.id) {
        return Err(AppError::Forbidden("不能审批自己发起的人员变动通知".into()));
    }
    if notice.review_status != "待审批" {
        return Err(AppError::Conflict("该通知不是待审批状态".into()));
    }
    if !["approve", "reject"].contains(&body.decision.as_str()) {
        return Err(AppError::Validation("审批结果必须为通过或驳回".into()));
    }
    if body.decision == "reject" {
        reason(&body.reason)?;
    }
    let before = notice.clone();
    let mut files = vec![];
    if body.decision == "approve" {
        check_revision(&body.summary_revision)?;
        canonical_label(&notice.action_key)?;
        validate_transfer(&pool, &notice)?;
        let mut rows = load_summary(&summary_path())?;
        let original = rows.clone();
        if notice.baseline_fingerprint.is_empty()
            || notice.baseline_fingerprint != affected_fingerprint(&notice, &rows)?
        {
            return Err(AppError::Conflict(
                "该通知涉及的人员或项目已变化，请驳回后核对修订重提".into(),
            ));
        }
        let config = load_feedback_config()?;
        let input = notice_body(&notice);
        validate_notice(
            &input,
            &all_labs(&pool)?,
            &rows,
            &[canonical_label(&notice.action_key)?.to_string()],
            &config.fields,
        )?;
        apply_summary_change(&notice, &mut rows)?;
        notice.affected_rows = rows
            .iter()
            .filter(|row| !original.iter().any(|old| old == *row))
            .count()
            + original
                .iter()
                .filter(|old| !rows.iter().any(|row| row.row_key == old.row_key))
                .count();
        if notice.action_key != "notice_only" {
            if notice.affected_rows == 0 {
                return Err(AppError::Conflict(
                    "该通知未改变任何汇总关系，禁止标记为已同步；请核对当前人员和项目".into(),
                ));
            }
            files.push(storage::change(SUMMARY_FILE, summary_bytes(&rows)?)?);
            notice.summary_applied = true;
        }
        notice.review_status = if notice.action_key == "notice_only" {
            "已通过"
        } else {
            "已同步"
        }
        .into();
    } else {
        notice.review_status = "已驳回".into();
    }
    notice.revision += 1;
    notice.reviewed_by = ctx.user.username.clone();
    notice.reviewed_at = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    notice.updated_at = notice.reviewed_at.clone();
    notice.review_reason = body.reason.trim().into();
    notice.delivery_status = "待发送".into();
    files.push(notice_file(&notice)?);
    let payload = notification_payload(&pool, &notice, Some(&ctx))?;
    storage::mutate(
        &pool,
        &ctx,
        if body.decision == "approve" {
            "approve"
        } else {
            "reject"
        },
        &number,
        if body.reason.trim().is_empty() {
            "审批通过人员变动"
        } else {
            body.reason.trim()
        },
        json!(before),
        json!(notice),
        files,
        Some((
            if body.decision == "approve" {
                "approved"
            } else {
                "rejected"
            },
            &payload,
        )),
    )?;
    Ok(Json(ApiResponse::ok(present_notice(&pool, &ctx, notice)?)))
}
async fn withdraw_notice(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    AxumPath(number): AxumPath<String>,
    Json(body): Json<NoticeReview>,
) -> Result<Json<ApiResponse<NoticeLog>>> {
    let ctx = require_submitter(&pool, &headers)?;
    let _guard = storage::lock(&pool)?;
    reason(&body.reason)?;
    let mut notice = find_notice(&pool, &number)?;
    check_notice_revision(&notice, body.notice_revision)?;
    if notice.actor_user_id != Some(ctx.user.id) {
        return Err(AppError::Forbidden("只能撤回本人发起的通知".into()));
    }
    if notice.review_status != "待审批" {
        return Err(AppError::Conflict(
            "只能撤回待审批通知；已同步记录不会反向撤销".into(),
        ));
    }
    let before = notice.clone();
    notice.review_status = "已撤回".into();
    notice.revision += 1;
    notice.review_reason = body.reason;
    notice.updated_at = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    storage::mutate(
        &pool,
        &ctx,
        "withdraw",
        &number,
        "发起人撤回通知",
        json!(before),
        json!(notice),
        vec![notice_file(&notice)?],
        None,
    )?;
    Ok(Json(ApiResponse::ok(present_notice(&pool, &ctx, notice)?)))
}
async fn revise_notice(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    AxumPath(number): AxumPath<String>,
    Json(body): Json<NoticeCreate>,
) -> Result<Json<ApiResponse<NoticeLog>>> {
    let ctx = require_submitter(&pool, &headers)?;
    let _guard = storage::lock(&pool)?;
    reason(&body.reason)?;
    check_revision(&body.summary_revision)?;
    let old = find_notice(&pool, &number)?;
    check_notice_revision(&old, body.notice_revision)?;
    if old.actor_user_id != Some(ctx.user.id) {
        return Err(AppError::Forbidden("只能修订本人发起的通知".into()));
    }
    if !["已驳回", "已撤回"].contains(&old.review_status.as_str()) {
        return Err(AppError::Conflict(
            "只有已驳回或已撤回通知可以修订重提".into(),
        ));
    }
    let config = load_feedback_config()?;
    let body = prepare_body(body, &config)?;
    validate_notice(
        &body,
        &labs_for_context(&pool, &ctx)?,
        &load_summary(&summary_path())?,
        &[canonical_label(&body.action_key)?.to_string()],
        &config.fields,
    )?;
    let mut notice = new_notice(&body, &ctx);
    notice.notice_no = number.clone();
    notice.created_at = old.created_at.clone();
    notice.revision = old.revision + 1;
    notice.baseline_fingerprint = affected_fingerprint(&notice, &load_summary(&summary_path())?)?;
    validate_transfer(&pool, &notice)?;
    let payload = notification_payload(&pool, &notice, None)?;
    storage::mutate(
        &pool,
        &ctx,
        "resubmit",
        &number,
        &body.reason,
        json!(old),
        json!(notice),
        vec![notice_file(&notice)?],
        Some(("submitted", &payload)),
    )?;
    Ok(Json(ApiResponse::ok(present_notice(&pool, &ctx, notice)?)))
}
async fn delete_notice(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    AxumPath(number): AxumPath<String>,
    Json(body): Json<NoticeReview>,
) -> Result<Json<ApiResponse<()>>> {
    let ctx = require_reviewer(&pool, &headers)?;
    let _guard = storage::lock(&pool)?;
    reason(&body.reason)?;
    let mut notice = find_notice(&pool, &number)?;
    check_notice_revision(&notice, body.notice_revision)?;
    if notice.review_status == "已删除" {
        return Err(AppError::Conflict("通知已删除".into()));
    }
    let before = notice.clone();
    notice.review_status = "已删除".into();
    notice.revision += 1;
    notice.review_reason = body.reason.clone();
    notice.updated_at = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    storage::mutate(
        &pool,
        &ctx,
        "delete",
        &number,
        &body.reason,
        json!(before),
        json!(notice),
        vec![notice_file(&notice)?],
        None,
    )?;
    Ok(Json(ApiResponse::ok(())))
}

#[derive(Deserialize)]
struct SummaryInput {
    summary_revision: String,
    reason: String,
    row: SummaryRow,
}
#[derive(Deserialize)]
struct MutationInput {
    summary_revision: String,
    reason: String,
}
#[derive(Serialize)]
struct SummaryResult {
    summary_revision: String,
    rows: Vec<SummaryRow>,
}
fn summary_result(pool: &DbPool, ctx: &AuthContext) -> Result<SummaryResult> {
    Ok(SummaryResult {
        summary_revision: summary_revision()?,
        rows: visible_summary(
            ctx,
            &labs_for_user(pool, ctx.user.id)?,
            load_summary(&summary_path())?,
        ),
    })
}
fn require_summary_editor(pool: &DbPool, headers: &HeaderMap) -> Result<AuthContext> {
    let ctx = require_summary_viewer(pool, headers)?;
    if !capabilities(&ctx).can_edit_summary {
        return Err(AppError::Forbidden(
            "仅有编辑权限的管理员和组长可以维护汇总表".into(),
        ));
    }
    Ok(ctx)
}
fn assert_edit_lab(pool: &DbPool, ctx: &AuthContext, lab: &str) -> Result<()> {
    if capabilities(ctx).edit_all_summary
        || labs_for_user(pool, ctx.user.id)?
            .iter()
            .any(|l| l.name == lab)
    {
        Ok(())
    } else {
        Err(AppError::Forbidden("只能维护关联实验室的汇总行".into()))
    }
}
fn clean_rows(pool: &DbPool, rows: &mut [SummaryRow]) -> Result<()> {
    let labs = all_labs(pool)?;
    for row in rows.iter_mut() {
        row.lab_name = row.lab_name.trim().into();
        row.project_code = row.project_code.trim().into();
        row.leader_name = row.leader_name.trim().into();
        if row.project_code.is_empty() || !labs.iter().any(|l| l.name == row.lab_name) {
            return Err(AppError::Validation(
                "汇总行必须选择有效实验室并填写项目代号".into(),
            ));
        }
        if [&row.lab_name, &row.project_code, &row.leader_name]
            .iter()
            .any(|s| s.chars().count() > 500)
            || row.person_names.len() > 1000
            || row.methods.len() > 1000
        {
            return Err(AppError::Validation("汇总行字段或列表过长".into()));
        }
        row.person_names = cleaned_values(&row.person_names).into_iter().collect();
        row.methods = cleaned_values(&row.methods).into_iter().collect();
        validate_atomic_names(&row.person_names, "人员姓名")?;
        validate_atomic_names(&row.methods, "检测方法")?;
        if row
            .person_names
            .iter()
            .chain(row.methods.iter())
            .any(|v| v.chars().count() > 1000)
        {
            return Err(AppError::Validation("人员或方法名称过长".into()));
        }
    }
    validate_row_identities(rows)
}
fn persist_summary(
    pool: &DbPool,
    ctx: &AuthContext,
    action: &str,
    reason_text: &str,
    before: &[SummaryRow],
    after: &[SummaryRow],
    extra: Vec<storage::FileChange>,
) -> Result<()> {
    let mut files = vec![storage::change(SUMMARY_FILE, summary_bytes(after)?)?];
    files.extend(extra);
    storage::mutate(
        pool,
        ctx,
        action,
        "personnel-summary",
        reason_text,
        json!(before),
        json!(after),
        files,
        None,
    )
}
async fn create_summary_row(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(body): Json<SummaryInput>,
) -> Result<Json<ApiResponse<SummaryResult>>> {
    let ctx = require_summary_editor(&pool, &headers)?;
    let _guard = storage::lock(&pool)?;
    check_revision(&body.summary_revision)?;
    reason(&body.reason)?;
    assert_edit_lab(&pool, &ctx, &body.row.lab_name)?;
    let before = load_summary(&summary_path())?;
    let mut after = before.clone();
    let mut new_row = body.row;
    new_row.row_key = fresh_row_key();
    after.push(new_row);
    clean_rows(&pool, &mut after)?;
    persist_summary(
        &pool,
        &ctx,
        "summary_create",
        &body.reason,
        &before,
        &after,
        vec![],
    )?;
    Ok(Json(ApiResponse::ok(summary_result(&pool, &ctx)?)))
}
async fn update_summary_row(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    AxumPath(key): AxumPath<String>,
    Json(body): Json<SummaryInput>,
) -> Result<Json<ApiResponse<SummaryResult>>> {
    let ctx = require_summary_editor(&pool, &headers)?;
    let _guard = storage::lock(&pool)?;
    check_revision(&body.summary_revision)?;
    reason(&body.reason)?;
    let before = load_summary(&summary_path())?;
    let mut after = before.clone();
    let row = after
        .iter_mut()
        .find(|r| r.row_key == key)
        .ok_or_else(|| AppError::NotFound("汇总行不存在".into()))?;
    assert_edit_lab(&pool, &ctx, &row.lab_name)?;
    assert_edit_lab(&pool, &ctx, &body.row.lab_name)?;
    let identity = row.row_key.clone();
    *row = body.row;
    row.row_key = identity;
    clean_rows(&pool, &mut after)?;
    if before == after {
        return Err(AppError::Validation("未修改汇总内容".into()));
    }
    persist_summary(
        &pool,
        &ctx,
        "summary_update",
        &body.reason,
        &before,
        &after,
        vec![],
    )?;
    Ok(Json(ApiResponse::ok(summary_result(&pool, &ctx)?)))
}
async fn delete_summary_row(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    AxumPath(key): AxumPath<String>,
    Json(body): Json<MutationInput>,
) -> Result<Json<ApiResponse<SummaryResult>>> {
    let ctx = require_summary_editor(&pool, &headers)?;
    let _guard = storage::lock(&pool)?;
    check_revision(&body.summary_revision)?;
    reason(&body.reason)?;
    let before = load_summary(&summary_path())?;
    let row = before
        .iter()
        .find(|r| r.row_key == key)
        .ok_or_else(|| AppError::NotFound("汇总行不存在".into()))?;
    assert_edit_lab(&pool, &ctx, &row.lab_name)?;
    let after = before
        .iter()
        .filter(|r| r.row_key != key)
        .cloned()
        .collect::<Vec<_>>();
    persist_summary(
        &pool,
        &ctx,
        "summary_delete",
        &body.reason,
        &before,
        &after,
        vec![],
    )?;
    Ok(Json(ApiResponse::ok(summary_result(&pool, &ctx)?)))
}
#[derive(Serialize, Deserialize)]
struct ImportPreview {
    preview_id: String,
    summary_revision: String,
    actor_user_id: i64,
    expires_at: String,
    added: usize,
    changed: usize,
    removed: usize,
    rows: Vec<SummaryRow>,
}
#[derive(Deserialize)]
struct ImportConfirm {
    preview_id: String,
    summary_revision: String,
    reason: String,
}
fn import_name(id: &str) -> Result<String> {
    if uuid::Uuid::parse_str(id).is_err() {
        return Err(AppError::Validation("导入预览标识无效".into()));
    }
    Ok(format!(".summary-import-{id}.json"))
}
fn require_importer(pool: &DbPool, headers: &HeaderMap) -> Result<AuthContext> {
    let ctx = require_summary_editor(pool, headers)?;
    if !capabilities(&ctx).can_import_summary {
        return Err(AppError::Forbidden(
            "全量导入仅限管理员或分析组长；请使用本实验室行维护".into(),
        ));
    }
    Ok(ctx)
}
async fn preview_import(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<ApiResponse<ImportPreview>>> {
    let ctx = require_importer(&pool, &headers)?;
    let mut bytes = None;
    let mut revision = String::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Validation(e.to_string()))?
    {
        let name = field.name().unwrap_or("").to_string();
        let data = field
            .bytes()
            .await
            .map_err(|e| AppError::Validation(e.to_string()))?;
        if data.len() > 8 * 1024 * 1024 {
            return Err(AppError::Validation("汇总表不能超过8MiB".into()));
        }
        if name == "file" {
            bytes = Some(data.to_vec());
        } else if name == "summary_revision" {
            revision = String::from_utf8(data.to_vec())
                .map_err(|_| AppError::Validation("基线格式无效".into()))?;
        }
    }
    let _guard = storage::lock(&pool)?;
    check_revision(&revision)?;
    let bytes = bytes.ok_or_else(|| AppError::Validation("请选择XLSX汇总表".into()))?;
    let temporary =
        ensure_feature_dir()?.join(format!(".summary-read-{}.xlsx", uuid::Uuid::new_v4()));
    storage::atomic_write(&temporary, &bytes)?;
    let parsed = load_summary(&temporary);
    let _ = fs::remove_file(&temporary);
    let mut rows = parsed?;
    if rows.is_empty() {
        return Err(AppError::Validation(
            "导入文件没有有效数据行，不允许整表清空".into(),
        ));
    }
    clean_rows(&pool, &mut rows)?;
    let previous = load_summary(&summary_path())?;
    let old = previous
        .iter()
        .map(|r| (r.row_key.clone(), r.clone()))
        .collect::<HashMap<_, _>>();
    let next = rows
        .iter()
        .map(|r| (r.row_key.clone(), r.clone()))
        .collect::<HashMap<_, _>>();
    let preview = ImportPreview {
        preview_id: uuid::Uuid::new_v4().to_string(),
        summary_revision: revision,
        actor_user_id: ctx.user.id,
        expires_at: (Local::now() + Duration::minutes(15)).to_rfc3339(),
        added: next.keys().filter(|k| !old.contains_key(*k)).count(),
        removed: old.keys().filter(|k| !next.contains_key(*k)).count(),
        changed: next
            .iter()
            .filter(|(k, v)| old.get(*k).is_some_and(|previous| previous != *v))
            .count(),
        rows,
    };
    storage::atomic_write(
        &ensure_feature_dir()?.join(import_name(&preview.preview_id)?),
        &serde_json::to_vec(&preview).map_err(|e| AppError::Internal(e.to_string()))?,
    )?;
    Ok(Json(ApiResponse::ok(preview)))
}
async fn confirm_import(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(body): Json<ImportConfirm>,
) -> Result<Json<ApiResponse<SummaryResult>>> {
    let ctx = require_importer(&pool, &headers)?;
    let _guard = storage::lock(&pool)?;
    check_revision(&body.summary_revision)?;
    reason(&body.reason)?;
    let name = import_name(&body.preview_id)?;
    let bytes = fs::read(ensure_feature_dir()?.join(&name))
        .map_err(|_| AppError::Conflict("导入预览不存在或已使用".into()))?;
    let mut preview: ImportPreview = serde_json::from_slice(&bytes)
        .map_err(|_| AppError::Conflict("导入预览已使用或无效".into()))?;
    if preview.actor_user_id != ctx.user.id || preview.summary_revision != body.summary_revision {
        return Err(AppError::Forbidden(
            "不能确认他人或过期基线的导入预览".into(),
        ));
    }
    let expires = chrono::DateTime::parse_from_rfc3339(&preview.expires_at)
        .map_err(|_| AppError::Conflict("预览有效期无效".into()))?;
    if expires <= Local::now() {
        return Err(AppError::Conflict("导入预览已过期，请重新上传核对".into()));
    }
    clean_rows(&pool, &mut preview.rows)?;
    let before = load_summary(&summary_path())?;
    if before == preview.rows {
        return Err(AppError::Validation("导入内容与当前汇总一致".into()));
    }
    persist_summary(
        &pool,
        &ctx,
        "summary_import",
        &body.reason,
        &before,
        &preview.rows,
        vec![storage::change(&name, b"{\"consumed\":true}".to_vec())?],
    )?;
    Ok(Json(ApiResponse::ok(summary_result(&pool, &ctx)?)))
}
async fn export_summary(State(pool): State<DbPool>, headers: HeaderMap) -> Result<Response> {
    let ctx = require_summary_viewer(&pool, &headers)?;
    let _guard = storage::lock(&pool)?;
    let rows = visible_summary(
        &ctx,
        &labs_for_user(&pool, ctx.user.id)?,
        load_summary(&summary_path())?,
    );
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        )
        .header(
            header::CONTENT_DISPOSITION,
            "attachment; filename=personnel-change-summary.xlsx",
        )
        .body(Body::from(summary_bytes(&rows)?))
        .map_err(|e| AppError::Internal(e.to_string()))?)
}
async fn get_config(
    State(pool): State<DbPool>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<FeedbackConfig>>> {
    let ctx = require_summary_viewer(&pool, &headers)?;
    let _guard = storage::lock(&pool)?;
    let mut config = load_feedback_config()?;
    config.capabilities = Some(capabilities(&ctx));
    Ok(Json(ApiResponse::ok(config)))
}
fn validate_config(input: &mut FeedbackConfigInput) -> Result<()> {
    if input.change_types.len() > 100 || input.fields.len() > 100 {
        return Err(AppError::Validation("配置项数量过多".into()));
    }
    let mut labels = BTreeSet::new();
    for item in &mut input.change_types {
        if item.label.trim().is_empty() {
            item.label = normalize_change_type(&item.name);
        }
        item.label = item.label.trim().into();
        item.name = item.label.clone();
        if item.action_key.is_empty() {
            item.action_key = action_for_label(&item.label)
                .unwrap_or("notice_only")
                .into();
        }
        canonical_label(&item.action_key)?;
        if item.label.is_empty()
            || item.label.chars().count() > 100
            || !labels.insert(item.label.clone())
        {
            return Err(AppError::Validation("变动类型名称不能为空或重复".into()));
        }
    }
    if !input.change_types.iter().any(|t| t.enabled) {
        return Err(AppError::Validation("至少保留一个启用的变动类型".into()));
    }
    let mut keys = BTreeSet::new();
    for field in &mut input.fields {
        field.key = field.key.trim().into();
        field.label = field.label.trim().into();
        if reserved_key(&field.key)
            || field.key.is_empty()
            || field.key.len() > 64
            || !field
                .key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
            || field.label.is_empty()
            || !keys.insert(field.key.clone())
        {
            return Err(AppError::Validation(
                "字段标识、名称为空、无效或重复".into(),
            ));
        }
        if ![
            "text",
            "textarea",
            "date",
            "boolean",
            "select",
            "multi-select",
            "multi-text",
        ]
        .contains(&field.field_type.as_str())
        {
            return Err(AppError::Validation("不支持的字段类型".into()));
        }
        field.options = cleaned_values(&field.options).into_iter().collect();
        if field.options.len() > 200 || field.options.iter().any(|v| v.chars().count() > 500) {
            return Err(AppError::Validation("字段选项过多或过长".into()));
        }
        if let Some(kind) = builtin_type(&field.key) {
            let always_enabled = [
                "change_type",
                "lab_name",
                "person_name",
                "project_codes",
                "method_names",
                "effective_at",
            ]
            .contains(&field.key.as_str());
            if field.field_type != kind || (always_enabled && !field.enabled) {
                return Err(AppError::Validation(format!(
                    "{}必须保留原类型，核心业务字段必须启用",
                    field.label
                )));
            }
        } else if !is_builtin_key(&field.key)
            && ["select", "multi-select"].contains(&field.field_type.as_str())
            && field.options.is_empty()
        {
            return Err(AppError::Validation(format!("{}必须配置选项", field.label)));
        }
        for action in &mut field.change_types {
            *action = action_for_label(action).unwrap_or(action).to_string();
            canonical_label(action)?;
        }
    }
    for field in default_feedback_fields() {
        if !keys.contains(&field.key) {
            return Err(AppError::Validation(format!(
                "不能删除内置业务字段{}，非核心字段可停用",
                field.key
            )));
        }
    }
    Ok(())
}
async fn update_config(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(mut input): Json<FeedbackConfigInput>,
) -> Result<Json<ApiResponse<()>>> {
    let ctx = context(&pool, &headers)?;
    if !ctx.is_system_admin() {
        return Err(AppError::Forbidden("仅系统管理员可编辑人员字段配置".into()));
    }
    let _guard = storage::lock(&pool)?;
    validate_config(&mut input)?;
    let previous = load_feedback_config()?;
    let types = serde_json::to_vec_pretty(&ChangeTypeSettings {
        change_types: input.change_types.clone(),
    })
    .map_err(|e| AppError::Internal(e.to_string()))?;
    let fields =
        serde_json::to_vec_pretty(&input.fields).map_err(|e| AppError::Internal(e.to_string()))?;
    storage::mutate(
        &pool,
        &ctx,
        "config_update",
        "personnel-config",
        "管理员维护人员变动配置",
        json!(previous),
        json!({"change_types":input.change_types,"fields":input.fields}),
        vec![
            storage::change(CHANGE_TYPES_FILE, types)?,
            storage::change(FIELDS_FILE, fields)?,
        ],
        None,
    )?;
    Ok(Json(ApiResponse::ok(())))
}

#[cfg(test)]
mod tests {
    use super::{
        action_for_label, apply_summary_change, notice_matches_query, read_notices, split_values,
        NoticeLog, SummaryRow,
    };

    fn notice(change_type: &str, projects: &[&str]) -> NoticeLog {
        NoticeLog {
            summary_applied: false,
            affected_rows: 0,
            baseline_fingerprint: String::new(),
            actor_user_id: Some(1),
            action_key: action_for_label(change_type).unwrap().into(),
            revision: 1,
            summary_revision: String::new(),
            can_review: false,
            can_revise: false,
            can_withdraw: false,
            notice_no: "PC-test".into(),
            created_at: "2026-08-22 10:00:00".into(),
            actor_username: "rd".into(),
            change_type: change_type.into(),
            lab_name: "实验室A".into(),
            target_lab_name: String::new(),
            person_name: "张三".into(),
            project_codes: projects.iter().map(|value| (*value).into()).collect(),
            target_project_codes: vec![],
            source_project_codes: vec![],
            transfer_notice_no: String::new(),
            method_names: vec!["方法B".into()],
            new_project_code: "P3".into(),
            new_project_name: String::new(),
            is_high_tech: false,
            high_tech_name: String::new(),
            effective_at: "2026-08-22".into(),
            notes: String::new(),
            extra_fields: std::collections::HashMap::new(),
            delivery_status: String::new(),
            review_status: "待审批".into(),
            reviewed_by: String::new(),
            reviewed_at: String::new(),
            review_reason: String::new(),
            updated_at: "2026-08-22 10:00:00".into(),
        }
    }

    fn rows() -> Vec<SummaryRow> {
        vec![
            SummaryRow {
                row_key: String::new(),
                lab_name: "实验室A".into(),
                leader_name: "负责人".into(),
                person_names: vec!["张三".into()],
                project_code: "P1".into(),
                methods: vec!["方法A".into()],
            },
            SummaryRow {
                row_key: String::new(),
                lab_name: "实验室A".into(),
                leader_name: "负责人".into(),
                person_names: vec!["李四".into()],
                project_code: "P2".into(),
                methods: vec!["方法A".into()],
            },
        ]
    }

    #[test]
    fn split_values_deduplicates_mixed_separators() {
        assert_eq!(
            split_values("张三、李四, 张三；王五"),
            vec!["张三", "李四", "王五"]
        );
    }

    #[test]
    fn targeted_personnel_notice_ignores_date_but_retains_actor_scope() {
        let data = notice("员工加入", &["P1"]);
        let cutoff = "2026-10-01 00:00:00";
        assert!(!notice_matches_query(&data, cutoff, None, None));
        assert!(notice_matches_query(&data, cutoff, None, Some("PC-test")));
        assert!(notice_matches_query(
            &data,
            cutoff,
            Some("rd"),
            Some("PC-test")
        ));
        assert!(!notice_matches_query(
            &data,
            cutoff,
            Some("other"),
            Some("PC-test")
        ));
        assert!(!notice_matches_query(&data, cutoff, None, Some("PC-other")));
        for invalid in ["", "../PC-test", "PC test", "PC-test\n"] {
            assert!(matches!(
                read_notices(7, None, Some(invalid)),
                Err(crate::error::AppError::Validation(_))
            ));
        }
    }

    #[test]
    fn lab_transfer_replaces_person_projects_with_all_lab_candidates() {
        let mut data = rows();
        apply_summary_change(&notice("实验室内人员调动", &["P2"]), &mut data).unwrap();
        assert!(data[0].person_names.is_empty());
        assert_eq!(data[1].person_names, vec!["张三", "李四"]);
    }

    #[test]
    fn cross_lab_transfer_without_projects_clears_all_current_projects() {
        let mut data = rows();
        apply_summary_change(&notice("跨实验室人员调出", &[]), &mut data).unwrap();
        assert!(data[0].person_names.is_empty());
        assert_eq!(data[1].person_names, vec!["李四"]);
    }

    #[test]
    fn action_keys_survive_display_renames_and_unknown_actions_cannot_fake_sync() {
        let mut data = rows();
        let mut change = notice("员工加入", &["P2"]);
        change.change_type = "新名称".into();
        apply_summary_change(&change, &mut data).unwrap();
        assert!(data[1].person_names.contains(&"张三".to_string()));
        change.action_key = "not_implemented".into();
        assert!(apply_summary_change(&change, &mut data).is_err());
    }

    #[test]
    fn approval_fingerprint_ignores_unrelated_rows_but_detects_changed_targets() {
        let change = notice("员工加入", &["P1"]);
        let original = rows();
        let fingerprint = super::affected_fingerprint(&change, &original).unwrap();
        let mut changed = original.clone();
        changed[1].methods.push("其他项目新增方法".into());
        assert_eq!(
            fingerprint,
            super::affected_fingerprint(&change, &changed).unwrap()
        );
        changed[0].person_names.push("新人员".into());
        assert_ne!(
            fingerprint,
            super::affected_fingerprint(&change, &changed).unwrap()
        );
        changed.remove(0);
        assert_ne!(
            fingerprint,
            super::affected_fingerprint(&change, &changed).unwrap()
        );
    }

    #[test]
    fn dynamic_fields_reject_invalid_choices_and_reserved_business_keys() {
        let mut config = super::FeedbackConfigInput {
            change_types: super::default_change_types(),
            fields: super::default_feedback_fields(),
        };
        super::validate_config(&mut config).unwrap();
        let mut field = config.fields.last().unwrap().clone();
        field.key = "safety".into();
        field.label = "注意事项".into();
        field.field_type = "multi-select".into();
        field.required = true;
        field.options = vec!["防护".into(), "低温".into()];
        config.fields.push(field.clone());
        super::validate_config(&mut config).unwrap();
        let mut body = super::notice_body(&notice("员工加入", &["P1"]));
        body.extra_fields
            .insert("safety".into(), "[\"防护\"]".into());
        assert!(super::validate_field_values(&body, &config.fields).is_ok());
        body.extra_fields
            .insert("safety".into(), "[\"未知\"]".into());
        assert!(super::validate_field_values(&body, &config.fields).is_err());
        let extra = config.fields.last_mut().unwrap();
        extra.field_type = "multi-text".into();
        for value in ["[]", "[\"\"]", "[\" \"]", "not-json"] {
            body.extra_fields.insert("safety".into(), value.into());
            assert!(super::validate_field_values(&body, &config.fields).is_err());
        }
        body.extra_fields
            .insert("safety".into(), "[\"新内容\"]".into());
        assert!(super::validate_field_values(&body, &config.fields).is_ok());
        field.key = "action_key".into();
        config.fields.push(field);
        assert!(super::validate_config(&mut config).is_err());
    }

    #[test]
    fn dates_and_notice_revision_are_real_preconditions() {
        assert!(super::validate_date("2026-02-30").is_err());
        assert!(super::validate_date("2026-10-15").is_ok());
        let notice = notice("员工加入", &["P1"]);
        assert!(super::check_notice_revision(&notice, 0).is_err());
        assert!(super::check_notice_revision(&notice, 1).is_ok());
    }

    #[test]
    fn legacy_delivery_labels_never_infer_approval_or_summary_sync() {
        for (status, delivery) in [
            ("未同步", "未同步"),
            ("未同步", "同步失败"),
            ("待调整", "已发送"),
            ("待迁出", "已发送"),
            ("", "已发送"),
        ] {
            let mut record = notice("实验室内人员调动", &["P1"]);
            record.review_status = status.into();
            record.delivery_status = delivery.into();
            super::normalize_notice(&mut record);
            assert_eq!(record.review_status, status);
            assert_eq!(record.delivery_status, delivery);
        }
    }

    #[test]
    fn corrupt_notice_lines_fail_instead_of_falling_back_to_an_older_revision() {
        let previous = notice("员工加入", &["P1"]);
        let mut latest = std::collections::HashMap::new();
        let content = format!(
            "{}\n\n{{broken-private-content\n",
            serde_json::to_string(&previous).unwrap()
        );
        let error =
            super::merge_notice_lines("notices-test.jsonl", &content, &mut latest).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("notices-test.jsonl第3行"));
        assert!(!message.contains("private-content"));
        let mut current = previous.clone();
        current.revision += 1;
        current.review_status = "已同步".into();
        let valid = format!(
            "{}\n \n{}\n",
            serde_json::to_string(&previous).unwrap(),
            serde_json::to_string(&current).unwrap()
        );
        super::merge_notice_lines("notices-test.jsonl", &valid, &mut latest).unwrap();
        assert_eq!(latest[&current.notice_no].revision, current.revision);
        assert_eq!(latest[&current.notice_no].review_status, "已同步");
    }

    #[test]
    fn legacy_summary_contexts_are_distinct_and_row_ids_survive_workbook_edits() {
        let mut records = vec![rows()[0].clone(), rows()[0].clone()];
        records[1].person_names = vec!["李四".into()];
        records[1].methods = vec!["方法B".into()];
        super::validate_row_identities(&mut records).unwrap();
        assert_ne!(records[0].row_key, records[1].row_key);

        let root =
            std::env::temp_dir().join(format!("labflow-summary-{}.xlsx", uuid::Uuid::new_v4()));
        std::fs::write(&root, super::summary_bytes(&records).unwrap()).unwrap();
        let mut loaded = super::load_summary(&root).unwrap();
        assert_eq!(records, loaded);
        let first_id = loaded[0].row_key.clone();
        loaded[0].person_names.push("新人员".into());
        std::fs::write(&root, super::summary_bytes(&loaded).unwrap()).unwrap();
        let updated = super::load_summary(&root).unwrap();
        std::fs::remove_file(root).unwrap();
        assert_eq!(updated[0].row_key, first_id);
        assert_eq!(updated, loaded);
        let mut duplicate = vec![updated[0].clone(), updated[0].clone()];
        duplicate[1].row_key.clear();
        assert!(super::validate_row_identities(&mut duplicate).is_err());

        let mut reordered = records.clone();
        reordered.reverse();
        let change = notice("员工加入", &["P1"]);
        assert_eq!(
            super::affected_fingerprint(&change, &records).unwrap(),
            super::affected_fingerprint(&change, &reordered).unwrap()
        );
    }

    #[test]
    fn all_builtin_types_stay_fixed_while_noncore_fields_can_be_disabled() {
        let mut config = super::FeedbackConfigInput {
            change_types: super::default_change_types(),
            fields: super::default_feedback_fields(),
        };
        let index = config
            .fields
            .iter()
            .position(|f| f.key == "is_high_tech")
            .unwrap();
        config.fields[index].enabled = false;
        super::validate_config(&mut config).unwrap();
        config.fields[index].field_type = "text".into();
        assert!(super::validate_config(&mut config).is_err());
        config.fields[index].field_type = "boolean".into();
        config.fields[0].enabled = false;
        assert!(super::validate_config(&mut config).is_err());
    }

    #[test]
    fn new_online_row_ids_do_not_reuse_an_edited_legacy_context_identity() {
        let mut original = rows()[0].clone();
        super::validate_row_identities(std::slice::from_mut(&mut original)).unwrap();
        let old_id = original.row_key.clone();
        let mut edited = original.clone();
        edited.person_names = vec!["新人员".into()];
        let mut new_original = original;
        new_original.row_key = super::fresh_row_key();
        let mut records = vec![edited, new_original];
        super::validate_row_identities(&mut records).unwrap();
        assert_eq!(records[0].row_key, old_id);
        assert_ne!(records[1].row_key, old_id);
    }

    #[test]
    fn importing_empty_row_ids_never_reuses_an_explicit_edited_identity_in_either_row_order() {
        let mut original = rows()[0].clone();
        let expected_legacy_id = super::business_row_key(&original);
        let mut edited = original.clone();
        edited.row_key = expected_legacy_id.clone();
        edited.person_names = vec!["新人员".into()];
        for reversed in [false, true] {
            let mut records = if reversed {
                vec![original.clone(), edited.clone()]
            } else {
                vec![edited.clone(), original.clone()]
            };
            super::validate_row_identities(&mut records).unwrap();
            let retained = records
                .iter()
                .find(|r| r.person_names == edited.person_names)
                .unwrap();
            let added = records
                .iter()
                .find(|r| r.person_names == original.person_names)
                .unwrap();
            assert_eq!(retained.row_key, expected_legacy_id);
            assert_ne!(added.row_key, expected_legacy_id);
            assert_eq!(added.row_key.len(), 64);
            let assigned = records.clone();
            super::validate_row_identities(&mut records).unwrap();
            assert_eq!(records, assigned);
        }
        super::validate_row_identities(std::slice::from_mut(&mut original)).unwrap();
        assert_eq!(original.row_key, expected_legacy_id);
    }

    #[test]
    fn departure_coalesces_only_contexts_that_become_identical_and_keeps_smallest_id() {
        let mut records = vec![rows()[0].clone(), rows()[0].clone()];
        records[1].person_names.push("离职人员".into());
        super::validate_row_identities(&mut records).unwrap();
        let survivor = records.iter().map(|r| r.row_key.clone()).min().unwrap();
        let mut change = notice("员工离职", &["P1"]);
        change.person_name = "离职人员".into();
        apply_summary_change(&change, &mut records).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].row_key, survivor);
        assert_eq!(records[0].person_names, vec!["张三"]);
    }

    #[test]
    fn deleting_a_synced_transfer_preserves_its_source_consumption_but_pending_delete_does_not() {
        for synced in [false, true] {
            let mut previous = notice("跨实验室人员调入", &["P1"]);
            previous.transfer_notice_no = "PC-source".into();
            previous.review_status = if synced { "已同步" } else { "待审批" }.into();
            let mut deleted = previous.clone();
            deleted.review_status = "已删除".into();
            deleted.revision += 1;
            for reversed in [false, true] {
                let records = if reversed {
                    [&deleted, &previous]
                } else {
                    [&previous, &deleted]
                };
                let content = records
                    .iter()
                    .map(|n| serde_json::to_string(n).unwrap())
                    .collect::<Vec<_>>()
                    .join("\n");
                let mut latest = std::collections::HashMap::new();
                super::merge_notice_lines("notices-test.jsonl", &content, &mut latest).unwrap();
                let current = &latest[&previous.notice_no];
                assert_eq!(current.review_status, "已删除");
                assert_eq!(current.summary_applied, synced);
                assert_eq!(super::transfer_reserves_source(current), synced);
            }
        }
    }

    #[test]
    fn workbook_names_cannot_change_meaning_when_written_and_reloaded() {
        for separator in [',', '，', '、', ';', '；', '\n', '\r'] {
            let value = vec![format!("甲{separator}乙")];
            assert!(super::validate_atomic_names(&value, "人员姓名").is_err());
            assert!(super::validate_atomic_names(&value, "检测方法").is_err());
        }
        assert!(
            super::validate_atomic_names(&["方法A".into(), "方法B".into()], "检测方法").is_ok()
        );
    }
}
