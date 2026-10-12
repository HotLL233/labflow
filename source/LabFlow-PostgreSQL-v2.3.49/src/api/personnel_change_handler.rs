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
use chrono::{Duration, Local};
use rust_xlsxwriter::{Format, Workbook};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeSet, HashMap};
use std::fs::{self, OpenOptions};
use std::io::Write;
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
    name: String,
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
}

#[derive(Debug, Serialize)]
struct FeedbackConfig {
    change_types: Vec<ChangeTypeConfig>,
    fields: Vec<FeedbackFieldConfig>,
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

#[derive(Debug, Clone, Serialize)]
struct SummaryRow {
    lab_name: String,
    leader_name: String,
    person_names: Vec<String>,
    project_code: String,
    methods: Vec<String>,
}

#[derive(Debug, Serialize)]
struct PersonnelChangeOptions {
    labs: Vec<PersonnelChangeLab>,
    summary: Vec<SummaryRow>,
    lab_projects: HashMap<String, Vec<String>>,
    change_types: Vec<String>,
    fields: Vec<FeedbackFieldConfig>,
    summary_available: bool,
}

#[derive(Debug, Deserialize)]
struct NoticeCreate {
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
    decision: String,
    #[serde(default)]
    reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NoticeLog {
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
        item.name = normalize_change_type(&item.name);
    }
    if had_legacy_transfer
        && !change_types
            .change_types
            .iter()
            .any(|item| item.name == "跨实验室人员调入")
    {
        change_types.change_types.push(ChangeTypeConfig {
            name: "跨实验室人员调入".into(),
            enabled: true,
        });
    }
    for field in &mut fields {
        for change_type in &mut field.change_types {
            *change_type = normalize_change_type(change_type);
        }
    }
    Ok(FeedbackConfig {
        change_types: change_types.change_types,
        fields,
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
    let value_at =
        |row: &[DataType], index: usize| row.get(index).map(cell_text).unwrap_or_default();
    Ok(rows
        .filter_map(|row| {
            let lab_name = value_at(row, lab_index);
            let project_code = value_at(row, project_index);
            if lab_name.is_empty() || project_code.is_empty() {
                return None;
            }
            Some(SummaryRow {
                lab_name,
                leader_name: value_at(row, leader_index),
                person_names: split_values(&value_at(row, people_index)),
                project_code,
                methods: split_values(&value_at(row, method_index)),
            })
        })
        .collect())
}

fn context(pool: &DbPool, headers: &HeaderMap) -> Result<AuthContext> {
    authz_service::authenticate(pool, headers)
}
fn is_viewer(ctx: &AuthContext) -> bool {
    ctx.is_system_admin() || ctx.is_analysis_leader() || ctx.is_rd_leader()
}
fn is_reviewer(ctx: &AuthContext) -> bool {
    ctx.is_system_admin() || ctx.is_analysis_leader()
}

fn require_viewer(pool: &DbPool, headers: &HeaderMap) -> Result<AuthContext> {
    let ctx = context(pool, headers)?;
    if !is_viewer(&ctx) {
        return Err(AppError::Forbidden(
            "仅系统管理员、分析检测组长或研发送样组长可查看人员变动通知".into(),
        ));
    }
    Ok(ctx)
}

fn require_submitter(pool: &DbPool, headers: &HeaderMap) -> Result<AuthContext> {
    let ctx = require_viewer(pool, headers)?;
    if !ctx.is_system_admin() && !ctx.is_rd_leader() {
        return Err(AppError::Forbidden(
            "仅系统管理员或研发送样组长可以发起人员变动通知".into(),
        ));
    }
    Ok(ctx)
}

fn require_reviewer(pool: &DbPool, headers: &HeaderMap) -> Result<AuthContext> {
    let ctx = require_viewer(pool, headers)?;
    if !is_reviewer(&ctx) {
        return Err(AppError::Forbidden(
            "仅系统管理员或分析检测组长可以审批人员变动通知".into(),
        ));
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
    if ctx.is_system_admin() || ctx.is_analysis_leader() {
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
    if ctx.is_system_admin() || ctx.is_analysis_leader() {
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

fn validate_method_names(values: &[String]) -> Result<()> {
    if values.iter().any(|value| {
        value
            .chars()
            .any(|ch| matches!(ch, ',' | '，' | '、' | ';' | '；' | '\n' | '\r'))
    }) {
        return Err(AppError::Validation(
            "检测方法请逐条填写，不能包含逗号、顿号、分号或换行".into(),
        ));
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
    let change_type = normalize_change_type(&body.change_type);
    validate_method_names(&body.method_names)?;
    for field in fields.iter().filter(|field| {
        field.enabled
            && field.required
            && (field.change_types.is_empty()
                || field.change_types.iter().any(|value| value == &change_type))
            && !matches!(
                field.key.as_str(),
                "change_type"
                    | "lab_name"
                    | "person_name"
                    | "project_codes"
                    | "method_names"
                    | "effective_at"
                    | "is_high_tech"
                    | "high_tech_name"
                    | "notes"
            )
    }) {
        if body
            .extra_fields
            .get(&field.key)
            .map(|value| value.trim())
            .unwrap_or("")
            .is_empty()
        {
            return Err(AppError::Validation(format!("请填写{}", field.label)));
        }
    }
    if !change_types.iter().any(|item| item == &change_type) {
        return Err(AppError::Validation("变动类型不存在或已停用".into()));
    }
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

fn delivery_status(notice: &NoticeLog) -> String {
    let Ok(url) = std::env::var("PERSONNEL_CHANGE_WEBHOOK_URL") else {
        return "未配置外部通知渠道，已写入外部通知台账".into();
    };
    let content = format!("【人员变动通知】{}\n实验室：{}\n人员：{}\n项目：{}\n方法：{}\n生效日期：{}\n提交人：{}\n备注：{}", notice.change_type, notice.lab_name, notice.person_name, notice.project_codes.join("、"), notice.method_names.join("、"), notice.effective_at, notice.actor_username, notice.notes);
    match reqwest::blocking::Client::new()
        .post(url)
        .json(&json!({ "msgtype": "text", "text": { "content": content } }))
        .send()
    {
        Ok(response) if response.status().is_success() => "外部通知已发送".into(),
        Ok(response) => format!("外部通知发送失败: HTTP {}", response.status()),
        Err(error) => format!("外部通知发送失败: {error}"),
    }
}

fn normalize_notice(notice: &mut NoticeLog) {
    notice.change_type = normalize_change_type(&notice.change_type);
    // 2.2.11 legacy-state migration is read-safe and idempotent: old labels are
    // normalized without deleting the original notice line or changing summary data.
    match notice.review_status.trim() {
        "待调整" if notice.change_type == "实验室内人员调动" => {
            notice.review_status = "已同步".into();
            notice.delivery_status = "已调出".into();
        }
        "待迁出" if notice.change_type == "跨实验室人员调出" => {
            notice.review_status = "已同步".into();
            notice.delivery_status = "已迁出".into();
        }
        "未同步" if notice.delivery_status.contains("同步") => {
            notice.review_status = "已同步".into();
        }
        _ => {}
    }
    if notice.review_status.trim().is_empty() {
        notice.review_status = "待审批".into();
    }
    if notice.updated_at.trim().is_empty() {
        notice.updated_at = notice.created_at.clone();
    }
}

fn write_notice(notice: &NoticeLog) -> Result<()> {
    let dir = ensure_feature_dir()?;
    let path = dir.join(format!("notices-{}.jsonl", Local::now().format("%Y-%m")));
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| AppError::Internal(format!("写入外部通知台账失败: {error}")))?;
    let line = serde_json::to_string(notice)
        .map_err(|error| AppError::Internal(format!("生成通知台账失败: {error}")))?;
    writeln!(file, "{line}")
        .map_err(|error| AppError::Internal(format!("保存通知台账失败: {error}")))
}

fn migrate_legacy_notice_states(dir: &Path) -> Result<()> {
    let marker = dir.join("migration-2.2.11-state-v1.done");
    if marker.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)
        .map_err(|error| AppError::Internal(format!("读取通知台账失败: {error}")))?
    {
        let entry =
            entry.map_err(|error| AppError::Internal(format!("读取通知台账目录失败: {error}")))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("notices-") || !name.ends_with(".jsonl") {
            continue;
        }
        let content = fs::read_to_string(&path)
            .map_err(|error| AppError::Internal(format!("读取通知台账文件失败: {error}")))?;
        let mut changed = false;
        let mut lines = Vec::new();
        for line in content.lines() {
            if let Ok(mut notice) = serde_json::from_str::<NoticeLog>(line) {
                let before = serde_json::to_string(&notice).unwrap_or_default();
                normalize_notice(&mut notice);
                let after = serde_json::to_string(&notice).unwrap_or_default();
                changed |= before != after;
                lines.push(after);
            } else {
                lines.push(line.to_string());
            }
        }
        if changed {
            let temp = path.with_extension("jsonl.migrate.tmp");
            fs::write(&temp, format!("{}\n", lines.join("\n")))
                .map_err(|error| AppError::Internal(format!("写入迁移临时文件失败: {error}")))?;
            fs::rename(&temp, &path).map_err(|error| {
                AppError::Internal(format!("替换迁移后的通知台账失败: {error}"))
            })?;
        }
    }
    fs::write(marker, "2.2.11 state migration v1\n")
        .map_err(|error| AppError::Internal(format!("写入迁移标识失败: {error}")))?;
    Ok(())
}

fn read_notices(days: i64, actor_username: Option<&str>) -> Result<Vec<NoticeLog>> {
    let dir = ensure_feature_dir()?;
    migrate_legacy_notice_states(&dir)?;
    let cutoff = (Local::now().naive_local() - Duration::days(days.clamp(1, 3650)))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
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
        for line in content.lines() {
            let Ok(mut notice) = serde_json::from_str::<NoticeLog>(line) else {
                continue;
            };
            normalize_notice(&mut notice);
            if latest
                .get(&notice.notice_no)
                .map(|current| notice.updated_at >= current.updated_at)
                .unwrap_or(true)
            {
                latest.insert(notice.notice_no.clone(), notice);
            }
        }
    }
    let mut notices = latest
        .into_values()
        .filter(|notice| notice.created_at >= cutoff)
        .filter(|notice| {
            actor_username
                .map(|actor| notice.actor_username == actor)
                .unwrap_or(true)
        })
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
    match notice.change_type.as_str() {
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
                rows.sort_by(|left, right| {
                    (&left.lab_name, &left.project_code)
                        .cmp(&(&right.lab_name, &right.project_code))
                });
                return Ok(());
            }
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
        _ => {}
    }
    rows.sort_by(|left, right| {
        (&left.lab_name, &left.project_code).cmp(&(&right.lab_name, &right.project_code))
    });
    Ok(())
}

fn write_summary_with_backup(rows: &[SummaryRow], notice_no: &str) -> Result<()> {
    let dir = ensure_feature_dir()?;
    let path = summary_path();
    if !path.exists() {
        return Err(AppError::NotFound("尚未导入项目及人员汇总表".into()));
    }
    let history = dir.join("summary-history");
    fs::create_dir_all(&history)
        .map_err(|error| AppError::Internal(format!("创建汇总表备份目录失败: {error}")))?;
    let safe_no = notice_no.replace(
        |value: char| !value.is_ascii_alphanumeric() && value != '-',
        "_",
    );
    fs::copy(
        &path,
        history.join(format!(
            "{}-{}.xlsx",
            Local::now().format("%Y%m%d%H%M%S"),
            safe_no
        )),
    )
    .map_err(|error| AppError::Internal(format!("备份原汇总表失败: {error}")))?;
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
    }
    workbook.save(path)?;
    Ok(())
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
            "/api/personnel-change/notices/:notice_no",
            axum::routing::delete(delete_notice),
        )
        .route(
            "/api/personnel-change/labs/:lab_name/person-projects",
            get(person_projects),
        )
        .route("/api/personnel-change/summary/export", get(export_summary))
        .route("/api/personnel-change/summary/import", post(import_summary))
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
    let ctx = require_viewer(&pool, &headers)?;
    let labs = labs_for_context(&pool, &ctx)?;
    let all_summary = load_summary(&summary_path())?;
    let summary_available = !all_summary.is_empty();
    let summary = if is_reviewer(&ctx) {
        visible_summary(&ctx, &labs, all_summary.clone())
    } else {
        Vec::new()
    };
    let feedback_config = load_feedback_config()?;
    let change_types = feedback_config
        .change_types
        .iter()
        .filter(|item| item.enabled)
        .map(|item| normalize_change_type(&item.name))
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    let lab_projects = lab_projects(
        &all_summary
            .into_iter()
            .filter(|row| labs.iter().any(|lab| lab.name == row.lab_name))
            .collect::<Vec<_>>(),
    );
    Ok(Json(ApiResponse::ok(PersonnelChangeOptions {
        labs,
        summary_available,
        summary,
        lab_projects,
        change_types,
        fields: feedback_config.fields,
    })))
}

async fn notices(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(query): Query<NoticeQuery>,
) -> Result<Json<ApiResponse<Vec<NoticeLog>>>> {
    let ctx = require_viewer(&pool, &headers)?;
    let actor_username = if is_reviewer(&ctx) {
        None
    } else {
        Some(ctx.user.username.as_str())
    };
    Ok(Json(ApiResponse::ok(read_notices(
        query.days.unwrap_or(7),
        actor_username,
    )?)))
}

async fn person_projects(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    AxumPath(lab_name): AxumPath<String>,
    Query(query): Query<PersonProjectQuery>,
) -> Result<Json<ApiResponse<Vec<String>>>> {
    let ctx = require_submitter(&pool, &headers)?;
    let lab_name = lab_name.trim();
    if !labs_for_context(&pool, &ctx)?
        .iter()
        .any(|lab| lab.name == lab_name)
    {
        return Err(AppError::Forbidden(
            "只能查询当前账号关联实验室的人员项目".into(),
        ));
    }
    let person_name = query.person_name.trim();
    if person_name.is_empty() {
        return Ok(Json(ApiResponse::ok(Vec::new())));
    }
    let projects = load_summary(&summary_path())?
        .into_iter()
        .filter(|row| {
            row.lab_name == lab_name && row.person_names.iter().any(|name| name == person_name)
        })
        .map(|row| row.project_code)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    Ok(Json(ApiResponse::ok(projects)))
}

async fn create_notice(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(body): Json<NoticeCreate>,
) -> Result<Json<ApiResponse<NoticeLog>>> {
    let ctx = require_submitter(&pool, &headers)?;
    let summary = load_summary(&summary_path())?;
    let labs = labs_for_context(&pool, &ctx)?;
    let feedback_config = load_feedback_config()?;
    let change_types = feedback_config
        .change_types
        .iter()
        .filter(|item| item.enabled)
        .map(|item| normalize_change_type(&item.name))
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    validate_notice(
        &body,
        &labs,
        &summary,
        &change_types,
        &feedback_config.fields,
    )?;
    let now = Local::now();
    let notice_lab_name = if normalize_change_type(&body.change_type) == "跨实验室人员调入"
    {
        body.target_lab_name
            .clone()
            .unwrap_or_default()
            .trim()
            .to_string()
    } else {
        body.lab_name.trim().to_string()
    };
    let mut notice = NoticeLog {
        notice_no: format!(
            "PC-{}-{}",
            now.format("%Y%m%d%H%M%S"),
            &uuid::Uuid::new_v4().simple().to_string()[..6].to_uppercase()
        ),
        created_at: now.format("%Y-%m-%d %H:%M:%S").to_string(),
        actor_username: ctx.user.username.clone(),
        change_type: normalize_change_type(&body.change_type),
        lab_name: notice_lab_name,
        target_lab_name: body
            .target_lab_name
            .clone()
            .unwrap_or_default()
            .trim()
            .to_string(),
        person_name: body
            .person_name
            .clone()
            .unwrap_or_default()
            .trim()
            .to_string(),
        project_codes: cleaned_values(&body.project_codes).into_iter().collect(),
        target_project_codes: cleaned_values(&body.target_project_codes)
            .into_iter()
            .collect(),
        source_project_codes: cleaned_values(&body.source_project_codes)
            .into_iter()
            .collect(),
        transfer_notice_no: body
            .transfer_notice_no
            .clone()
            .unwrap_or_default()
            .trim()
            .to_string(),
        method_names: cleaned_values(&body.method_names).into_iter().collect(),
        new_project_code: body.new_project_code.unwrap_or_default().trim().to_string(),
        new_project_name: String::new(),
        is_high_tech: body.is_high_tech.unwrap_or(false),
        high_tech_name: body
            .high_tech_name
            .clone()
            .unwrap_or_default()
            .trim()
            .to_string(),
        effective_at: if body.effective_at.trim().is_empty() {
            now.format("%Y-%m-%d").to_string()
        } else {
            body.effective_at.trim().to_string()
        },
        notes: body.notes.clone().unwrap_or_default().trim().to_string(),
        extra_fields: body.extra_fields.clone(),
        delivery_status: String::new(),
        review_status: "待审批".into(),
        reviewed_by: String::new(),
        reviewed_at: String::new(),
        review_reason: String::new(),
        updated_at: now.format("%Y-%m-%d %H:%M:%S").to_string(),
    };
    notice.delivery_status = delivery_status(&notice);
    write_notice(&notice)?;

    // 补充实验室负责人
    let lab_leader = summary
        .iter()
        .find(|row| row.lab_name == notice.lab_name)
        .map(|row| row.leader_name.as_str())
        .unwrap_or("");

    let payload = json!({
        "notice_no": notice.notice_no,
        "created_at": notice.created_at,
        "submitted_by": notice.actor_username,
        "change_type": notice.change_type,
        "lab_name": notice.lab_name,
        "target_lab_name": notice.target_lab_name,
        "lab_leader": lab_leader,
        "group_id": labs.iter().find(|lab| lab.name == notice.lab_name).map(|lab| lab.id),
        "person_name": notice.person_name,
        "source_project_codes": notice.source_project_codes.join("、"),
        "target_project_codes": notice.target_project_codes.join("、"),
        "transfer_notice_no": notice.transfer_notice_no,
        "project": if notice.new_project_code.is_empty() {
            notice.project_codes.join("、")
        } else {
            notice.new_project_code.clone()
        },
        "project_name": notice.project_codes.join("、"),
        "new_project_code": notice.new_project_code,
        "method": notice.method_names.join("、"),
        "method_name": notice.method_names.join("、"),
        "effective_at": notice.effective_at,
        "is_high_tech": if notice.is_high_tech { "是" } else { "否" },
        "high_tech_name": notice.high_tech_name,
        "status": notice.review_status,
        "notes": notice.notes,
        "delivery_status": notice.delivery_status,
        "updated_at": notice.updated_at,
        "extra_fields": notice.extra_fields,
        "target_url": "/personnel-change",
    });
    crate::service::notification_service::enqueue_personnel_change(
        &pool,
        &notice.notice_no,
        &payload,
    )?;

    // 写入审计日志
    let conn = pool.get()?;
    let notice_json = serde_json::to_value(&notice)
        .map_err(|e| AppError::Internal(format!("序列化通知数据失败: {}", e)))?;
    crate::repo::audit_repo::log_structured_on_conn(
        &conn,
        "create",
        "personnel_change_notices",
        None,
        &ctx.user.username,
        &format!("创建人员变动通知: {}", notice.notice_no),
        "personnel_change",
        &notice.notice_no,
        None,
        Some(&notice_json),
        "personnel_change",
    )?;

    // The maintenance loop remains a retry fallback; newly submitted feedback is processed now.
    let notification_pool = pool.clone();
    tokio::task::spawn_blocking(move || {
        if let Err(error) =
            crate::service::notification_service::process_pending(&notification_pool, 20)
        {
            tracing::warn!("personnel change notification processing failed: {error}");
        }
    });
    Ok(Json(ApiResponse::ok(notice)))
}

async fn review_notice(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    AxumPath(notice_no): AxumPath<String>,
    Json(body): Json<NoticeReview>,
) -> Result<Json<ApiResponse<NoticeLog>>> {
    let ctx = require_reviewer(&pool, &headers)?;
    let mut notice = read_notices(3650, None)?
        .into_iter()
        .find(|item| item.notice_no == notice_no)
        .ok_or_else(|| AppError::NotFound("未找到人员变动通知".into()))?;
    if notice.actor_username == ctx.user.username {
        return Err(AppError::Forbidden("不能审批自己发起的人员变动通知".into()));
    }
    if notice.review_status != "待审批" {
        return Err(AppError::Conflict("该通知已完成审批，不能重复处理".into()));
    }
    let decision = body.decision.trim();
    if decision != "approve" && decision != "reject" {
        return Err(AppError::Validation("审批结果必须为通过或驳回".into()));
    }
    if decision == "reject" && body.reason.trim().is_empty() {
        return Err(AppError::Validation("驳回通知必须填写原因".into()));
    }

    // 保存审批前快照用于审计
    let before_notice = notice.clone();

    if decision == "approve" {
        let mut rows = load_summary(&summary_path())?;
        apply_summary_change(&notice, &mut rows)?;
        write_summary_with_backup(&rows, &notice.notice_no)?;
    }
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    notice.review_status = if decision == "approve" {
        "已同步"
    } else {
        "已驳回"
    }
    .into();
    notice.reviewed_by = ctx.user.username.clone();
    notice.reviewed_at = now.clone();
    notice.review_reason = body.reason.trim().to_string();
    if decision == "approve" {
        notice.delivery_status = "已同步到项目及人员汇总表".into();
    }
    notice.updated_at = now;
    write_notice(&notice)?;
    if decision == "reject" {
        let conn = pool.get()?;
        let submitted_by_user_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM users WHERE username=?1 AND deleted_at IS NULL LIMIT 1",
                [&notice.actor_username],
                |row| row.get(0),
            )
            .ok();
        let group_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM project_groups WHERE name=?1 AND deleted_at IS NULL LIMIT 1",
                [&notice.lab_name],
                |row| row.get(0),
            )
            .ok();
        let payload = json!({
            "notice_no": notice.notice_no.clone(),
            "created_at": notice.created_at.clone(),
            "submitted_by": notice.actor_username.clone(),
            "submitted_by_user_id": submitted_by_user_id,
            "change_type": notice.change_type.clone(),
            "lab_name": notice.lab_name.clone(),
            "target_lab_name": notice.target_lab_name.clone(),
            "lab_leader": "",
            "group_id": group_id,
            "person_name": notice.person_name.clone(),
            "source_project_codes": notice.source_project_codes.join("、"),
            "target_project_codes": notice.target_project_codes.join("、"),
            "transfer_notice_no": notice.transfer_notice_no.clone(),
            "project": if notice.new_project_code.is_empty() {
                notice.project_codes.join("、")
            } else {
                notice.new_project_code.clone()
            },
            "project_name": notice.project_codes.join("、"),
            "new_project_code": notice.new_project_code.clone(),
            "method": notice.method_names.join("、"),
            "method_name": notice.method_names.join("、"),
            "effective_at": notice.effective_at.clone(),
            "is_high_tech": if notice.is_high_tech { "是" } else { "否" },
            "high_tech_name": notice.high_tech_name.clone(),
            "status": notice.review_status.clone(),
            "reviewed_by": notice.reviewed_by.clone(),
            "reviewed_at": notice.reviewed_at.clone(),
            "review_reason": notice.review_reason.clone(),
            "notes": notice.notes.clone(),
            "delivery_status": notice.delivery_status.clone(),
            "updated_at": notice.updated_at.clone(),
            "extra_fields": notice.extra_fields.clone(),
            "target_url": "/personnel-change",
        });
        crate::service::notification_service::enqueue_personnel_change_rejected(
            &pool,
            &notice.notice_no,
            &payload,
        )?;
        let notification_pool = pool.clone();
        tokio::task::spawn_blocking(move || {
            if let Err(error) =
                crate::service::notification_service::process_pending(&notification_pool, 20)
            {
                tracing::warn!(
                    "personnel change rejection notification processing failed: {error}"
                );
            }
        });
    }

    // 写入审计日志
    let conn = pool.get()?;
    let action = if decision == "approve" {
        "approve"
    } else {
        "reject"
    };
    let detail = if decision == "approve" {
        format!("审批通过人员变动通知: {}", notice.notice_no)
    } else {
        format!(
            "驳回人员变动通知: {} - {}",
            notice.notice_no, notice.review_reason
        )
    };
    let before_json = serde_json::to_value(&before_notice)
        .map_err(|e| AppError::Internal(format!("序列化审批前数据失败: {}", e)))?;
    let after_json = serde_json::to_value(&notice)
        .map_err(|e| AppError::Internal(format!("序列化审批后数据失败: {}", e)))?;
    crate::repo::audit_repo::log_structured_on_conn(
        &conn,
        action,
        "personnel_change_notices",
        None,
        &ctx.user.username,
        &detail,
        "personnel_change",
        &notice.notice_no,
        Some(&before_json),
        Some(&after_json),
        "personnel_change",
    )?;

    Ok(Json(ApiResponse::ok(notice)))
}

async fn delete_notice(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    AxumPath(notice_no): AxumPath<String>,
    Json(body): Json<NoticeReview>,
) -> Result<Json<ApiResponse<()>>> {
    let ctx = require_viewer(&pool, &headers)?;
    let notices = read_notices(3650, None)?;
    let notice = notices
        .iter()
        .filter(|n| n.notice_no == notice_no)
        .max_by_key(|n| &n.updated_at)
        .ok_or_else(|| AppError::NotFound("通知不存在".into()))?;

    // 删除通知是管理操作。已同步记录只删除展示台账，绝不回滚汇总关系。
    if !is_reviewer(&ctx) {
        return Err(AppError::Forbidden(
            "仅系统管理员或分析检测组长可删除通知".into(),
        ));
    }

    if body.reason.trim().is_empty() {
        return Err(AppError::Validation("删除通知必须填写原因".into()));
    }

    let before_notice = notice.clone();
    let mut deleted = notice.clone();
    deleted.review_status = "已删除".into();
    deleted.review_reason = body.reason.trim().to_string();
    deleted.reviewed_by = ctx.user.username.clone();
    deleted.reviewed_at = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    deleted.updated_at = deleted.reviewed_at.clone();

    write_notice(&deleted)?;

    let conn = pool.get()?;
    let before_json = serde_json::to_value(&before_notice)
        .map_err(|e| AppError::Internal(format!("序列化删除前数据失败: {}", e)))?;
    let after_json = serde_json::to_value(&deleted)
        .map_err(|e| AppError::Internal(format!("序列化删除后数据失败: {}", e)))?;
    crate::repo::audit_repo::log_structured_on_conn(
        &conn,
        "delete",
        "personnel_change_notices",
        None,
        &ctx.user.username,
        &format!("删除人员变动通知: {} - {}", notice_no, body.reason.trim()),
        "personnel_change",
        &notice_no,
        Some(&before_json),
        Some(&after_json),
        "personnel_change",
    )?;

    Ok(Json(ApiResponse::ok(())))
}

async fn export_summary(State(pool): State<DbPool>, headers: HeaderMap) -> Result<Response> {
    let ctx = require_viewer(&pool, &headers)?;
    let path = summary_path();
    if !path.exists() {
        return Err(AppError::NotFound("尚未导入项目及人员汇总表".into()));
    }
    let rows = visible_summary(&ctx, &labs_for_context(&pool, &ctx)?, load_summary(&path)?);
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
        let row_no = (index + 1) as u32;
        sheet.write_string(row_no, 0, &row.lab_name)?;
        sheet.write_string(row_no, 1, &row.leader_name)?;
        sheet.write_string_with_format(row_no, 2, row.person_names.join("、"), &wrapped)?;
        sheet.write_string_with_format(row_no, 3, &row.project_code, &wrapped)?;
        sheet.write_string_with_format(row_no, 4, row.methods.join("\n"), &wrapped)?;
    }
    let bytes = workbook.save_to_buffer()?;
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
        .body(Body::from(bytes))
        .map_err(|error| AppError::Internal(format!("生成汇总表下载响应失败: {error}")))?)
}

async fn get_config(
    State(pool): State<DbPool>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<FeedbackConfig>>> {
    let ctx = require_viewer(&pool, &headers)?;
    if !is_reviewer(&ctx) {
        return Err(AppError::Forbidden(
            "仅系统管理员或分析检测组长可读取人员反馈配置".into(),
        ));
    }
    Ok(Json(ApiResponse::ok(load_feedback_config()?)))
}

async fn update_config(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(input): Json<FeedbackConfigInput>,
) -> Result<Json<ApiResponse<()>>> {
    let ctx = context(&pool, &headers)?;
    if !ctx.is_system_admin() {
        return Err(AppError::Forbidden(
            "仅系统管理员可编辑人员反馈字段配置".into(),
        ));
    }
    if input
        .change_types
        .iter()
        .all(|item| !item.enabled || item.name.trim().is_empty())
    {
        return Err(AppError::Validation(
            "至少保留一个启用的人员变动类型".into(),
        ));
    }
    if input
        .fields
        .iter()
        .any(|field| field.key.trim().is_empty() || field.label.trim().is_empty())
    {
        return Err(AppError::Validation(
            "人员反馈字段的键和名称不能为空".into(),
        ));
    }
    let mut field_keys = BTreeSet::new();
    if input
        .fields
        .iter()
        .any(|field| !field_keys.insert(field.key.trim().to_string()))
    {
        return Err(AppError::Validation("人员反馈字段键不能重复".into()));
    }
    ensure_feature_dir()?;
    fs::write(
        change_types_path(),
        serde_json::to_string_pretty(&ChangeTypeSettings {
            change_types: input.change_types,
        })
        .map_err(|error| AppError::Internal(error.to_string()))?,
    )
    .map_err(|error| AppError::Internal(format!("保存人员变动类型配置失败: {error}")))?;
    fs::write(
        fields_path(),
        serde_json::to_string_pretty(&input.fields)
            .map_err(|error| AppError::Internal(error.to_string()))?,
    )
    .map_err(|error| AppError::Internal(format!("保存人员反馈字段配置失败: {error}")))?;
    Ok(Json(ApiResponse::ok(())))
}

async fn import_summary(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<ApiResponse<()>>> {
    require_reviewer(&pool, &headers)?;
    let mut bytes = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| AppError::Validation(format!("读取上传文件失败: {error}")))?
    {
        if field.name() == Some("file") {
            bytes = Some(
                field
                    .bytes()
                    .await
                    .map_err(|error| AppError::Validation(format!("读取上传文件失败: {error}")))?,
            );
            break;
        }
    }
    let bytes = bytes.ok_or_else(|| AppError::Validation("请选择项目及人员汇总表文件".into()))?;
    if bytes.is_empty() {
        return Err(AppError::Validation("上传的项目及人员汇总表为空".into()));
    }
    let dir = ensure_feature_dir()?;
    let temporary = dir.join(format!(".summary-upload-{}.xlsx", uuid::Uuid::new_v4()));
    fs::write(&temporary, bytes)
        .map_err(|error| AppError::Internal(format!("保存上传汇总表失败: {error}")))?;
    let result = load_summary(&temporary);
    if result.as_ref().map_or(true, |rows| rows.is_empty()) {
        let _ = fs::remove_file(&temporary);
        return Err(result
            .err()
            .unwrap_or_else(|| AppError::Validation("项目及人员汇总表没有有效数据行".into())));
    }
    fs::copy(&temporary, summary_path())
        .map_err(|error| AppError::Internal(format!("替换项目及人员汇总表失败: {error}")))?;
    fs::remove_file(&temporary)
        .map_err(|error| AppError::Internal(format!("清理临时汇总表失败: {error}")))?;
    Ok(Json(ApiResponse::ok_msg(
        "项目及人员汇总表已更新；未写入程序数据库",
    )))
}

#[cfg(test)]
mod tests {
    use super::{apply_summary_change, split_values, NoticeLog, SummaryRow};

    fn notice(change_type: &str, projects: &[&str]) -> NoticeLog {
        NoticeLog {
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
                lab_name: "实验室A".into(),
                leader_name: "负责人".into(),
                person_names: vec!["张三".into()],
                project_code: "P1".into(),
                methods: vec!["方法A".into()],
            },
            SummaryRow {
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
}
