use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use calamine::{open_workbook_from_rs, DataType, Reader};
use chrono::Local;
use postgres_compat::{types::ValueRef, Connection, OptionalExtension, Transaction};
use rust_xlsxwriter::{Format, Workbook};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, io::Cursor, path::PathBuf};
use uuid::Uuid;
use zip::{write::SimpleFileOptions, ZipWriter};

use crate::{
    config::AppConfig,
    db::DbPool,
    error::{AppError, Result},
    models::ApiResponse,
    repo::{audit_repo, rd_record_repo, record_repo, sample_info_repo},
    service::{auth_service, authz_service, backup_service},
};

const MAX_IMPORT_BYTES: usize = 20 * 1024 * 1024;

#[derive(Debug, Clone, Copy)]
enum Module {
    Work,
    Rd,
    SampleInfo,
}

impl Module {
    fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "work" | "analysis" | "分析检测" => Ok(Self::Work),
            "rd" | "research" | "研发送样" => Ok(Self::Rd),
            "sample-info" | "sample_info" | "sampleinfo" | "样品信息登记" => {
                Ok(Self::SampleInfo)
            }
            _ => Err(AppError::Validation("不支持的业务数据类型".into())),
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Work => "work",
            Self::Rd => "rd",
            Self::SampleInfo => "sample_info",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Work => "分析检测",
            Self::Rd => "研发送样",
            Self::SampleInfo => "样品信息登记",
        }
    }

    fn table(self) -> &'static str {
        match self {
            Self::Work => "work_records",
            Self::Rd => "rd_work_records",
            Self::SampleInfo => "sample_info_records",
        }
    }

    fn date_column(self) -> &'static str {
        match self {
            Self::Work | Self::Rd => "recorded_at",
            Self::SampleInfo => "submitted_at",
        }
    }
}

#[derive(Debug, Serialize)]
struct ModuleSummary {
    module: String,
    label: String,
    total: i64,
    active: i64,
    deleted: i64,
    earliest: Option<String>,
    latest: Option<String>,
    attachment_count: i64,
    attachment_bytes: i64,
}

#[derive(Debug, Serialize)]
struct GovernanceSummary {
    db_size: u64,
    attachment_bytes: u64,
    modules: Vec<ModuleSummary>,
}

#[derive(Debug, Serialize)]
struct ImportPreview {
    module: String,
    file_sha256: String,
    total_rows: usize,
    new_rows: usize,
    duplicate_rows: usize,
    invalid_rows: usize,
    issues: Vec<String>,
}

#[derive(Debug, Serialize)]
struct PurgePreview {
    module: String,
    start: String,
    end: String,
    active_count: i64,
    deleted_count: i64,
    attachment_count: i64,
    attachment_bytes: i64,
    confirmation_token: String,
    expires_at: String,
}

#[derive(Debug, Serialize)]
struct PurgeResult {
    module: String,
    moved_count: i64,
    preserved_attachment_count: i64,
    export_file: String,
    export_sha256: String,
}

#[derive(Debug, Deserialize)]
struct DateRange {
    start: String,
    end: String,
}

#[derive(Debug, Deserialize)]
struct PurgeRequest {
    start: String,
    end: String,
    confirmation_token: String,
    admin_username: String,
    admin_password: String,
}

#[derive(Debug, Clone)]
struct ImportRow {
    values: BTreeMap<String, String>,
    row_number: usize,
}

#[derive(Debug, Clone)]
enum PreparedRow {
    Work {
        business_no: String,
        project_id: i64,
        method_id: Option<i64>,
        group_id: Option<i64>,
        user_name: String,
        quantity: i64,
        recorded_at: String,
        batch_no: String,
        multiplier: f64,
        high_item: String,
        notes: String,
        project_name: String,
        lab_name: String,
        method_name: String,
        instrument_id: Option<i64>,
        instrument_code: String,
        instrument_type: String,
        subject_user_id: Option<i64>,
    },
    Rd {
        business_no: String,
        project_id: i64,
        method_id: Option<i64>,
        group_id: Option<i64>,
        division_id: Option<i64>,
        user_name: String,
        quantity: i64,
        recorded_at: String,
        batch_no: String,
        notes: String,
        status: String,
        sampler: String,
        sampled_at: String,
        detected_by: String,
        detected_at: String,
        project_name: String,
        lab_name: String,
        method_name: String,
        high_item: String,
        instrument_id: Option<i64>,
        instrument_code: String,
        instrument_type: String,
        subject_user_id: Option<i64>,
    },
    SampleInfo {
        business_no: String,
        status: String,
        seq_no: i64,
        batch_no: String,
        user_name: String,
        lab_name: String,
        project_name: String,
        submitted_at: String,
        detection_date: String,
        main_components: String,
        detection_type: String,
        type_key: String,
        quantity: i64,
        notes: String,
        extra_fields: String,
        group_id: Option<i64>,
        division_id: Option<i64>,
    },
}

impl PreparedRow {
    fn business_no(&self) -> &str {
        match self {
            Self::Work { business_no, .. }
            | Self::Rd { business_no, .. }
            | Self::SampleInfo { business_no, .. } => business_no,
        }
    }

    fn fingerprint(&self) -> String {
        let value = match self {
            Self::Work { project_id, method_id, group_id, user_name, quantity, recorded_at, batch_no, .. } =>
                format!("work|{project_id}|{:?}|{:?}|{}|{quantity}|{recorded_at}|{batch_no}", method_id, group_id, user_name.trim(),),
            Self::Rd { project_id, method_id, group_id, user_name, quantity, recorded_at, batch_no, .. } =>
                format!("rd|{project_id}|{:?}|{:?}|{}|{quantity}|{recorded_at}|{batch_no}", method_id, group_id, user_name.trim(),),
            Self::SampleInfo { status, batch_no, user_name, lab_name, project_name, submitted_at, detection_date, main_components, type_key, quantity, .. } =>
                format!("sample_info|{status}|{batch_no}|{user_name}|{lab_name}|{project_name}|{submitted_at}|{detection_date}|{main_components}|{type_key}|{quantity}"),
        };
        hex_sha256(value.as_bytes())
    }
}

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/data-governance/summary", get(summary))
        .route("/api/data-governance/:module/template", get(template))
        .route(
            "/api/data-governance/:module/import/precheck",
            post(import_precheck),
        )
        .route("/api/data-governance/:module/import", post(import_execute))
        .route("/api/data-governance/:module/export", get(export_data))
        .route(
            "/api/data-governance/:module/purge/precheck",
            post(purge_precheck),
        )
        .route("/api/data-governance/:module/purge", post(purge_execute))
        .with_state(pool)
}

fn require(
    pool: &DbPool,
    headers: &HeaderMap,
    permission: &str,
) -> Result<authz_service::AuthContext> {
    let ctx = authz_service::authenticate(pool, headers)?;
    authz_service::require_permission(&ctx, permission)?;
    Ok(ctx)
}

async fn summary(
    State(pool): State<DbPool>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<GovernanceSummary>>> {
    require(&pool, &headers, "manage:data-governance:view")?;
    let conn = pool.get()?;
    let cfg = AppConfig::load();
    // PostgreSQL owns database storage; the application does not expose a database-file size.
    let db_size = conn
        .query_row("SELECT pg_database_size(current_database())", [], |row| {
            row.get::<_, i64>(0)
        })
        .map(|bytes| bytes.max(0) as u64)
        .unwrap_or(0);
    let attachment_bytes = directory_size(&cfg.attachments_dir());
    let modules = [Module::Work, Module::Rd, Module::SampleInfo]
        .iter()
        .map(|module| summary_for(&conn, *module))
        .collect::<Result<Vec<_>>>()?;
    Ok(Json(ApiResponse::ok(GovernanceSummary {
        db_size,
        attachment_bytes,
        modules,
    })))
}

fn summary_for(conn: &Connection, module: Module) -> Result<ModuleSummary> {
    let sql = format!("SELECT COUNT(*), CAST(COALESCE(SUM(CASE WHEN deleted_at IS NULL THEN 1 ELSE 0 END),0) AS BIGINT), CAST(COALESCE(SUM(CASE WHEN deleted_at IS NOT NULL THEN 1 ELSE 0 END),0) AS BIGINT), MIN({}), MAX({}) FROM {}", module.date_column(), module.date_column(), module.table());
    let (total, active, deleted, earliest, latest): (
        i64,
        i64,
        i64,
        Option<String>,
        Option<String>,
    ) = conn.query_row(&sql, [], |row| {
        Ok((
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
        ))
    })?;
    let (attachment_count, attachment_bytes) = if matches!(module, Module::SampleInfo) {
        conn.query_row(
            "SELECT COUNT(*), CAST(COALESCE(SUM(file_size),0) AS BIGINT) FROM sample_info_attachments",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?
    } else {
        (0, 0)
    };
    Ok(ModuleSummary {
        module: module.key().to_string(),
        label: module.label().to_string(),
        total,
        active,
        deleted,
        earliest,
        latest,
        attachment_count,
        attachment_bytes,
    })
}

async fn read_file(mut multipart: Multipart) -> Result<(Vec<u8>, String)> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Validation(format!("读取上传文件失败: {e}")))?
    {
        if field.name() != Some("file") {
            continue;
        }
        let name = field.file_name().unwrap_or("import.xlsx").to_string();
        let bytes = field
            .bytes()
            .await
            .map_err(|e| AppError::Validation(format!("读取上传文件失败: {e}")))?
            .to_vec();
        if bytes.is_empty() || bytes.len() > MAX_IMPORT_BYTES {
            return Err(AppError::Validation("导入文件为空或超过 20 MB".into()));
        }
        return Ok((bytes, name));
    }
    Err(AppError::Validation("请选择 Excel 文件".into()))
}

fn hex_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn cell_to_string(cell: &DataType) -> String {
    match cell {
        DataType::String(s) => s.trim().to_string(),
        DataType::Float(v) => {
            if v.fract() == 0.0 {
                format!("{}", *v as i64)
            } else {
                v.to_string()
            }
        }
        DataType::Int(v) => v.to_string(),
        DataType::Bool(v) => v.to_string(),
        DataType::DateTime(v) => v.to_string(),
        DataType::DateTimeIso(v) | DataType::DurationIso(v) => v.trim().to_string(),
        DataType::Duration(v) => v.to_string(),
        DataType::Empty | DataType::Error(_) => String::new(),
    }
}

fn parse_rows(bytes: &[u8]) -> Result<Vec<ImportRow>> {
    let cursor = Cursor::new(bytes.to_vec());
    let mut workbook: calamine::Xlsx<_> = open_workbook_from_rs(cursor)
        .map_err(|e| AppError::Validation(format!("无法打开 Excel 文件: {e}")))?;
    let sheet = workbook
        .sheet_names()
        .iter()
        .find(|name| !name.contains("说明"))
        .cloned()
        .ok_or_else(|| AppError::Validation("Excel 中没有可导入的数据表".into()))?;
    let range = workbook
        .worksheet_range(&sheet)
        .map_err(|e| AppError::Validation(format!("读取工作表失败: {e}")))?;
    let mut rows = range.rows();
    let headers: Vec<String> = rows
        .next()
        .ok_or_else(|| AppError::Validation("Excel 缺少表头".into()))?
        .iter()
        .map(cell_to_string)
        .collect();
    let mut result = Vec::new();
    for (index, row) in rows.enumerate() {
        let mut values = BTreeMap::new();
        for (column, header) in headers.iter().enumerate() {
            if !header.is_empty() {
                values.insert(
                    header.clone(),
                    row.get(column).map(cell_to_string).unwrap_or_default(),
                );
            }
        }
        if values.values().all(|v| v.trim().is_empty()) {
            continue;
        }
        let business_no = values
            .get("业务编号")
            .or_else(|| values.get("business_no"))
            .cloned()
            .unwrap_or_default();
        if business_no.starts_with("示例-") {
            continue;
        }
        result.push(ImportRow {
            values,
            row_number: index + 2,
        });
    }
    Ok(result)
}

fn value(row: &ImportRow, names: &[&str]) -> String {
    names
        .iter()
        .find_map(|name| row.values.get(*name).cloned())
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn parse_i64(value: &str, field: &str, row: usize) -> Result<i64> {
    value
        .parse()
        .map_err(|_| AppError::Validation(format!("第 {row} 行：{field} 必须是数字")))
}
fn parse_f64(value: &str, field: &str, row: usize, default: f64) -> Result<f64> {
    if value.is_empty() {
        Ok(default)
    } else {
        value
            .parse()
            .map_err(|_| AppError::Validation(format!("第 {row} 行：{field} 必须是数字")))
    }
}

fn find_project(conn: &Connection, name: &str, row: usize) -> Result<(i64, f64, String)> {
    conn.query_row(
        "SELECT id, COALESCE(coefficient,1.0), COALESCE(high_item,'') FROM projects WHERE name=?1",
        [name],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .optional()?
    .ok_or_else(|| AppError::Validation(format!("第 {row} 行：项目不存在：{name}")))
}

fn find_lab(conn: &Connection, name: &str, row: usize) -> Result<Option<i64>> {
    if name.is_empty() {
        return Ok(None);
    }
    conn.query_row("SELECT id FROM project_groups WHERE name=?1", [name], |r| {
        r.get(0)
    })
    .optional()?
    .ok_or_else(|| AppError::Validation(format!("第 {row} 行：实验室不存在：{name}")))
    .map(Some)
}

fn find_user(conn: &Connection, name: &str, row: usize) -> Result<Option<i64>> {
    if name.is_empty() {
        return Ok(None);
    }
    conn.query_row(
        "SELECT id FROM users WHERE username=?1 AND is_active=1",
        [name],
        |r| r.get(0),
    )
    .optional()?
    .ok_or_else(|| AppError::Validation(format!("第 {row} 行：用户不存在或已停用：{name}")))
    .map(Some)
}

fn find_method(
    conn: &Connection,
    name: &str,
    instrument: &str,
    row: usize,
) -> Result<(Option<i64>, Option<i64>, String, String, String)> {
    if name.is_empty() {
        return Ok((None, None, String::new(), String::new(), String::new()));
    }
    let found = if instrument.is_empty() {
        let count: i64 =
            conn.query_row("SELECT COUNT(*) FROM methods WHERE name=?1", [name], |r| {
                r.get(0)
            })?;
        if count > 1 {
            return Err(AppError::Validation(format!(
                "第 {row} 行：方法「{name}」绑定了多台仪器，必须填写仪器编号"
            )));
        }
        conn.query_row("SELECT m.id,m.instrument_id,m.name,COALESCE(i.code,''),COALESCE(i.instrument_type,'') FROM methods m LEFT JOIN instruments i ON i.id=m.instrument_id WHERE m.name=?1 ORDER BY m.id LIMIT 1", [name], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))).optional()?
    } else {
        conn.query_row("SELECT m.id,m.instrument_id,m.name,COALESCE(i.code,''),COALESCE(i.instrument_type,'') FROM methods m JOIN instruments i ON i.id=m.instrument_id WHERE m.name=?1 AND i.code=?2", postgres_compat::params![name, instrument], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))).optional()?
    };
    found.ok_or_else(|| {
        AppError::Validation(format!(
            "第 {row} 行：方法或仪器绑定不存在：{name} / {instrument}"
        ))
    })
}

fn prepare_row(
    conn: &Connection,
    module: Module,
    row: &ImportRow,
    _operator_id: i64,
) -> Result<PreparedRow> {
    match module {
        Module::Work => {
            let project_name = value(row, &["项目", "项目名称", "project_name"]);
            if project_name.is_empty() {
                return Err(AppError::Validation(format!(
                    "第 {} 行：项目不能为空",
                    row.row_number
                )));
            }
            let (project_id, _, project_high) = find_project(conn, &project_name, row.row_number)?;
            let lab_name = value(row, &["实验室", "实验室名称", "lab_name"]);
            let group_id = find_lab(conn, &lab_name, row.row_number)?;
            let user_name = value(row, &["检测人员", "人员", "user_name"]);
            let subject_user_id = find_user(conn, &user_name, row.row_number)?;
            let method_name = value(row, &["方法", "方法名称", "method_name"]);
            let instrument_code = value(row, &["仪器编号", "instrument_code"]);
            let (method_id, instrument_id, method_name, instrument_code, instrument_type) =
                find_method(conn, &method_name, &instrument_code, row.row_number)?;
            let recorded_at = value(row, &["记录时间", "时间", "recorded_at"]);
            if recorded_at.is_empty() {
                return Err(AppError::Validation(format!(
                    "第 {} 行：记录时间不能为空",
                    row.row_number
                )));
            }
            Ok(PreparedRow::Work {
                business_no: value(row, &["业务编号", "business_no"]),
                project_id,
                method_id,
                group_id,
                user_name,
                quantity: parse_i64(&value(row, &["数量", "quantity"]), "数量", row.row_number)?,
                recorded_at,
                batch_no: value(row, &["批号", "batch_no"]),
                multiplier: parse_f64(
                    &value(row, &["倍率", "multiplier"]),
                    "倍率",
                    row.row_number,
                    1.0,
                )?,
                high_item: value(row, &["高项", "high_item"]).if_empty_then(&project_high),
                notes: value(row, &["备注", "说明", "notes"]),
                project_name,
                lab_name,
                method_name,
                instrument_id,
                instrument_code,
                instrument_type,
                subject_user_id,
            })
        }
        Module::Rd => {
            let project_name = value(row, &["项目", "项目名称", "project_name"]);
            let (project_id, _, project_high) = find_project(conn, &project_name, row.row_number)?;
            let lab_name = value(row, &["实验室", "实验室名称", "lab_name"]);
            let group_id = find_lab(conn, &lab_name, row.row_number)?;
            let user_name = value(row, &["送样人", "人员", "user_name"]);
            let subject_user_id = find_user(conn, &user_name, row.row_number)?;
            let method_name = value(row, &["方法", "方法名称", "method_name"]);
            let instrument_code = value(row, &["仪器编号", "instrument_code"]);
            let (method_id, instrument_id, method_name, instrument_code, instrument_type) =
                find_method(conn, &method_name, &instrument_code, row.row_number)?;
            let status = value(row, &["状态", "status"]);
            let status = if status.is_empty() {
                "待取样".to_string()
            } else {
                status
            };
            if !["待取样", "已取样"].contains(&status.as_str()) {
                return Err(AppError::Validation(format!(
                    "第 {} 行：状态必须是待取样或已取样",
                    row.row_number
                )));
            }
            let recorded_at = value(row, &["送样时间", "记录时间", "recorded_at"]);
            if recorded_at.is_empty() {
                return Err(AppError::Validation(format!(
                    "第 {} 行：送样时间不能为空",
                    row.row_number
                )));
            }
            Ok(PreparedRow::Rd {
                business_no: value(row, &["业务编号", "business_no"]),
                project_id,
                method_id,
                group_id,
                division_id: None,
                user_name,
                quantity: parse_i64(&value(row, &["数量", "quantity"]), "数量", row.row_number)?,
                recorded_at,
                batch_no: value(row, &["批号", "batch_no"]),
                notes: value(row, &["备注", "notes"]),
                status,
                sampler: value(row, &["取样人", "sampler"]),
                sampled_at: value(row, &["取样时间", "sampled_at"]),
                detected_by: value(row, &["检测人", "detected_by"]),
                detected_at: value(row, &["检测完成时间", "detected_at"]),
                project_name,
                lab_name,
                method_name,
                high_item: value(row, &["高项", "high_item"]).if_empty_then(&project_high),
                instrument_id,
                instrument_code,
                instrument_type,
                subject_user_id,
            })
        }
        Module::SampleInfo => {
            let status = value(row, &["状态", "status"]);
            let status = if status.is_empty() {
                "待取样".to_string()
            } else {
                status
            };
            if !["待取样", "待检测", "已检测"].contains(&status.as_str()) {
                return Err(AppError::Validation(format!(
                    "第 {} 行：状态必须是待取样、待检测或已检测",
                    row.row_number
                )));
            }
            let lab_name = value(row, &["实验室", "实验室名称", "lab_name"]);
            let group_id = find_lab(conn, &lab_name, row.row_number)?;
            let user_name = value(row, &["送样人", "人员", "user_name"]);
            let _subject_user_id = find_user(conn, &user_name, row.row_number)?;
            let quantity = parse_i64(&value(row, &["数量", "quantity"]), "数量", row.row_number)?;
            let submitted_at = value(row, &["送样时间", "submitted_at"]);
            if submitted_at.is_empty() {
                return Err(AppError::Validation(format!(
                    "第 {} 行：送样时间不能为空",
                    row.row_number
                )));
            }
            Ok(PreparedRow::SampleInfo {
                business_no: value(row, &["业务编号", "business_no"]),
                status,
                seq_no: value(row, &["序号", "seq_no"]).parse().unwrap_or(0),
                batch_no: value(row, &["批号", "batch_no"]),
                user_name,
                lab_name,
                project_name: value(row, &["项目", "项目名称", "project_name"]),
                submitted_at,
                detection_date: value(row, &["检测日期", "detection_date"]),
                main_components: value(row, &["主要成分", "成分", "main_components"]),
                detection_type: value(row, &["检测类型", "detection_type"]),
                type_key: value(row, &["类型标识", "type_key"]),
                quantity,
                notes: value(row, &["备注", "notes"]),
                extra_fields: value(row, &["扩展字段", "extra_fields"]).if_empty_then("{}"),
                group_id,
                division_id: None,
            })
        }
    }
}

trait EmptyThen {
    fn if_empty_then(self, fallback: &str) -> String;
}
impl EmptyThen for String {
    fn if_empty_then(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.to_string()
        } else {
            self
        }
    }
}

fn already_exists(conn: &Connection, module: Module, row: &PreparedRow) -> Result<bool> {
    if !row.business_no().is_empty() {
        let sql = format!(
            "SELECT EXISTS(SELECT 1 FROM {} WHERE business_no=?1)",
            module.table()
        );
        return Ok(conn.query_row(&sql, [row.business_no()], |r| r.get(0))?);
    }
    let exists = match row {
        PreparedRow::Work { project_id, method_id, group_id, user_name, quantity, recorded_at, batch_no, .. } => conn.query_row("SELECT EXISTS(SELECT 1 FROM work_records WHERE project_id=?1 AND COALESCE(method_id,0)=COALESCE(?2,0) AND COALESCE(group_id,0)=COALESCE(?3,0) AND user_name=?4 AND quantity=?5 AND recorded_at=?6 AND COALESCE(batch_no,'')=?7)", postgres_compat::params![project_id, method_id, group_id, user_name, quantity, recorded_at, batch_no], |r| r.get(0))?,
        PreparedRow::Rd { project_id, method_id, group_id, user_name, quantity, recorded_at, batch_no, .. } => conn.query_row("SELECT EXISTS(SELECT 1 FROM rd_work_records WHERE project_id=?1 AND COALESCE(method_id,0)=COALESCE(?2,0) AND COALESCE(group_id,0)=COALESCE(?3,0) AND user_name=?4 AND quantity=?5 AND recorded_at=?6 AND COALESCE(batch_no,'')=?7)", postgres_compat::params![project_id, method_id, group_id, user_name, quantity, recorded_at, batch_no], |r| r.get(0))?,
        PreparedRow::SampleInfo { status, batch_no, user_name, lab_name, project_name, submitted_at, type_key, quantity, .. } => conn.query_row("SELECT EXISTS(SELECT 1 FROM sample_info_records WHERE status=?1 AND batch_no=?2 AND user_name=?3 AND lab_name=?4 AND project_name=?5 AND submitted_at=?6 AND type_key=?7 AND quantity=?8)", postgres_compat::params![status, batch_no, user_name, lab_name, project_name, submitted_at, type_key, quantity], |r| r.get(0))?,
    };
    Ok(exists)
}

fn build_preview(
    conn: &Connection,
    module: Module,
    bytes: &[u8],
    operator_id: i64,
) -> Result<ImportPreview> {
    let rows = parse_rows(bytes)?;
    let mut new_rows = 0;
    let mut duplicate_rows = 0;
    let mut invalid_rows = 0;
    let mut issues = Vec::new();
    let mut fingerprints = std::collections::HashSet::new();
    for row in &rows {
        match prepare_row(conn, module, row, operator_id) {
            Ok(prepared) => {
                let duplicate = already_exists(conn, module, &prepared)?
                    || !fingerprints.insert(prepared.fingerprint());
                if duplicate {
                    duplicate_rows += 1;
                } else {
                    new_rows += 1;
                }
            }
            Err(error) => {
                invalid_rows += 1;
                if issues.len() < 100 {
                    issues.push(error.to_string());
                }
            }
        }
    }
    Ok(ImportPreview {
        module: module.key().to_string(),
        file_sha256: hex_sha256(bytes),
        total_rows: rows.len(),
        new_rows,
        duplicate_rows,
        invalid_rows,
        issues,
    })
}

async fn import_precheck(
    State(pool): State<DbPool>,
    Path(module): Path<String>,
    headers: HeaderMap,
    multipart: Multipart,
) -> Result<Json<ApiResponse<ImportPreview>>> {
    let ctx = require(&pool, &headers, "manage:data-governance:import")?;
    let module = Module::parse(&module)?;
    let (bytes, _) = read_file(multipart).await?;
    let conn = pool.get()?;
    Ok(Json(ApiResponse::ok(build_preview(
        &conn,
        module,
        &bytes,
        ctx.user.id,
    )?)))
}

async fn import_execute(
    State(pool): State<DbPool>,
    Path(module): Path<String>,
    headers: HeaderMap,
    multipart: Multipart,
) -> Result<Json<ApiResponse<ImportPreview>>> {
    let ctx = require(&pool, &headers, "manage:data-governance:import")?;
    let module = Module::parse(&module)?;
    let (bytes, filename) = read_file(multipart).await?;
    let mut conn = pool.get()?;
    let preview = build_preview(&conn, module, &bytes, ctx.user.id)?;
    if preview.invalid_rows > 0 {
        return Err(AppError::Validation(format!(
            "导入预检未通过：{} 行无效",
            preview.invalid_rows
        )));
    }
    let rows = parse_rows(&bytes)?;
    let tx = conn.transaction()?;
    let mut inserted = 0usize;
    let mut seen = std::collections::HashSet::new();
    for row in rows {
        let prepared = prepare_row(&tx, module, &row, ctx.user.id)?;
        if already_exists(&tx, module, &prepared)? || !seen.insert(prepared.fingerprint()) {
            continue;
        }
        insert_row(&tx, module, &prepared, ctx.user.id, &ctx.user.username)?;
        inserted += 1;
    }
    tx.execute("INSERT INTO governance_import_batches(module,file_name,file_sha256,operator_user_id,operator_username,total_rows,inserted_rows,skipped_rows,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,datetime('now','localtime'))", postgres_compat::params![module.key(), filename, preview.file_sha256, ctx.user.id, ctx.user.username, preview.total_rows, inserted, preview.total_rows.saturating_sub(inserted)])?;
    audit_repo::log_on_conn_with_module(
        &tx,
        "import",
        module.table(),
        None,
        &ctx.user.username,
        &format!(
            "数据治理导入：{}，新增{}条，跳过{}条",
            filename,
            inserted,
            preview.total_rows.saturating_sub(inserted)
        ),
        module.key(),
    )?;
    tx.commit()?;
    Ok(Json(ApiResponse::ok(ImportPreview {
        new_rows: inserted,
        ..preview
    })))
}

fn insert_row(
    tx: &Transaction<'_>,
    _module: Module,
    row: &PreparedRow,
    operator_id: i64,
    operator: &str,
) -> Result<()> {
    match row {
        PreparedRow::Work {
            business_no,
            project_id,
            method_id,
            group_id,
            user_name,
            quantity,
            recorded_at,
            batch_no,
            multiplier,
            high_item,
            notes,
            project_name,
            lab_name,
            method_name,
            instrument_id,
            instrument_code,
            instrument_type,
            subject_user_id,
        } => {
            tx.execute("INSERT INTO work_records(project_id,method_id,user_name,quantity,recorded_at,group_id,batch_no,extra_info,multiplier,high_item,business_no,project_name_snapshot,lab_name_snapshot,method_name_snapshot,high_item_snapshot,coefficient_snapshot,subject_user_id,created_by_user_id,created_by_username_snapshot,instrument_id_snapshot,instrument_code_snapshot,instrument_type_snapshot) SELECT ?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,COALESCE(p.coefficient,1.0),?16,?17,?18,?19,?20,?21 FROM projects p WHERE p.id=?1", postgres_compat::params![project_id,method_id,user_name,quantity,recorded_at,group_id,batch_no,notes,multiplier,high_item,business_no,project_name,lab_name,method_name,high_item,subject_user_id,operator_id,operator,instrument_id,instrument_code,instrument_type])?;
            let id = tx.last_insert_rowid();
            let final_no = if business_no.is_empty() {
                format!("WK-{}-{:06}", Local::now().format("%Y%m%d"), id)
            } else {
                business_no.clone()
            };
            tx.execute(
                "UPDATE work_records SET business_no=?1 WHERE id=?2",
                postgres_compat::params![final_no, id],
            )?;
            tx.execute("INSERT INTO record_events(module,table_name,record_id,business_no,event_type,operator,reason) VALUES('work','work_records',?1,?2,'import',?3,'数据治理导入')", postgres_compat::params![id,final_no,operator])?;
        }
        PreparedRow::Rd {
            business_no,
            project_id,
            method_id,
            group_id,
            division_id,
            user_name,
            quantity,
            recorded_at,
            batch_no,
            notes,
            status,
            sampler,
            sampled_at,
            detected_by,
            detected_at,
            project_name,
            lab_name,
            method_name,
            high_item,
            instrument_id,
            instrument_code,
            instrument_type,
            subject_user_id,
        } => {
            tx.execute("INSERT INTO rd_work_records(project_id,method_id,user_name,quantity,recorded_at,group_id,division_id,batch_no,notes,status,sampler,sampled_at,detected_by,detected_at,business_no,project_name_snapshot,lab_name_snapshot,method_name_snapshot,high_item_snapshot,subject_user_id,created_by_user_id,created_by_username_snapshot,instrument_id_snapshot,instrument_code_snapshot,instrument_type_snapshot) SELECT ?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25 FROM projects p WHERE p.id=?1", postgres_compat::params![project_id,method_id,user_name,quantity,recorded_at,group_id,division_id,batch_no,notes,status,sampler,sampled_at,detected_by,detected_at,business_no,project_name,lab_name,method_name,high_item,subject_user_id,operator_id,operator,instrument_id,instrument_code,instrument_type])?;
            let id = tx.last_insert_rowid();
            let final_no = if business_no.is_empty() {
                format!("RD-{}-{:06}", Local::now().format("%Y%m%d"), id)
            } else {
                business_no.clone()
            };
            tx.execute(
                "UPDATE rd_work_records SET business_no=?1 WHERE id=?2",
                postgres_compat::params![final_no, id],
            )?;
            tx.execute("INSERT INTO record_events(module,table_name,record_id,business_no,event_type,to_status,operator,reason) VALUES('rd','rd_work_records',?1,?2,'import',?3,?4,'数据治理导入')", postgres_compat::params![id,final_no,status,operator])?;
        }
        PreparedRow::SampleInfo {
            business_no,
            status,
            seq_no,
            batch_no,
            user_name,
            lab_name,
            project_name,
            submitted_at,
            detection_date,
            main_components,
            detection_type,
            type_key,
            quantity,
            notes,
            extra_fields,
            group_id,
            division_id,
        } => {
            let seq = if *seq_no > 0 {
                *seq_no
            } else {
                tx.query_row(
                    "SELECT COALESCE(MAX(seq_no),0)+1 FROM sample_info_records",
                    [],
                    |r| r.get::<_, i64>(0),
                )?
            };
            tx.execute("INSERT INTO sample_info_records(status,seq_no,batch_no,user_name,lab_name,project_name,submitted_at,detection_date,main_components,detection_type,type_key,division_id,quantity,notes,extra_fields,group_id,created_by_user_id,created_by_username_snapshot,business_no) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)", postgres_compat::params![status,seq,batch_no,user_name,lab_name,project_name,submitted_at,detection_date,main_components,detection_type,type_key,division_id,quantity,notes,extra_fields,group_id,operator_id,operator,business_no])?;
            let id = tx.last_insert_rowid();
            let final_no = if business_no.is_empty() {
                format!("SI-{}-{:06}", Local::now().format("%Y%m%d"), id)
            } else {
                business_no.clone()
            };
            tx.execute(
                "UPDATE sample_info_records SET business_no=?1 WHERE id=?2",
                postgres_compat::params![final_no, id],
            )?;
            tx.execute("INSERT INTO record_events(module,table_name,record_id,business_no,event_type,to_status,operator,reason) VALUES('sample_info','sample_info_records',?1,?2,'import',?3,?4,'数据治理导入')", postgres_compat::params![id,final_no,status,operator])?;
        }
    }
    Ok(())
}

fn csv_escape(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}
fn value_ref(value: ValueRef<'_>) -> String {
    match value {
        ValueRef::Null => String::new(),
        ValueRef::Integer(v) => v.to_string(),
        ValueRef::Real(v) => v.to_string(),
        ValueRef::Text(v) => String::from_utf8_lossy(v).to_string(),
        ValueRef::Blob(v) => format!("[blob:{} bytes]", v.len()),
    }
}

fn export_rows(
    conn: &Connection,
    module: Module,
    start: &str,
    end: &str,
) -> Result<(Vec<String>, Vec<Vec<String>>)> {
    let sql = match module {
        Module::Work => "SELECT business_no,project_name_snapshot,lab_name_snapshot,method_name_snapshot,instrument_code_snapshot,user_name,quantity,recorded_at,batch_no,multiplier,high_item,extra_info,deleted_at,created_at FROM work_records WHERE recorded_at>=?1 AND recorded_at<=?2 ORDER BY recorded_at,id",
        Module::Rd => "SELECT business_no,project_name_snapshot,lab_name_snapshot,method_name_snapshot,instrument_code_snapshot,user_name,quantity,recorded_at,batch_no,status,sampler,sampled_at,detected_by,detected_at,notes,deleted_at,created_at FROM rd_work_records WHERE recorded_at>=?1 AND recorded_at<=?2 ORDER BY recorded_at,id",
        Module::SampleInfo => "SELECT business_no,status,seq_no,batch_no,user_name,lab_name,project_name,submitted_at,detection_date,sampled_by,sampled_at,detected_by,main_components,detection_type,type_key,quantity,notes,extra_fields,deleted_at,created_at FROM sample_info_records WHERE submitted_at>=?1 AND submitted_at<=?2 ORDER BY submitted_at,id",
    };
    let mut stmt = conn.prepare(sql)?;
    let headers = (0..stmt.column_count())
        .map(|i| stmt.column_name(i).unwrap_or("").to_string())
        .collect::<Vec<_>>();
    let count = stmt.column_count();
    let rows = stmt
        .query_map(
            postgres_compat::params![start, format!("{}T23:59:59", end)],
            |row| {
                let mut values = Vec::with_capacity(count);
                for i in 0..count {
                    values.push(value_ref(row.get_ref(i)?));
                }
                Ok(values)
            },
        )?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok((headers, rows))
}

fn build_archive(
    conn: &Connection,
    module: Module,
    start: &str,
    end: &str,
    cfg: &AppConfig,
) -> Result<(Vec<u8>, String)> {
    let (headers, rows) = export_rows(conn, module, start, end)?;
    let mut csv = vec![0xEF, 0xBB, 0xBF];
    csv.extend_from_slice(
        headers
            .iter()
            .map(|v| csv_escape(v))
            .collect::<Vec<_>>()
            .join(",")
            .as_bytes(),
    );
    csv.push(b'\n');
    for row in rows {
        csv.extend_from_slice(
            row.iter()
                .map(|v| csv_escape(v))
                .collect::<Vec<_>>()
                .join(",")
                .as_bytes(),
        );
        csv.push(b'\n');
    }
    let csv_hash = hex_sha256(&csv);
    let manifest = serde_json::json!({"format_version":1,"module":module.key(),"label":module.label(),"start":start,"end":end,"created_at":Local::now().to_rfc3339(),"csv_sha256":csv_hash,"includes_deleted":true});
    let mut cursor = Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(&mut cursor);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file(format!("{}_原始记录.csv", module.label()), opts)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    std::io::Write::write_all(&mut zip, &csv).map_err(|e| AppError::Internal(e.to_string()))?;
    let manifest_bytes =
        serde_json::to_vec_pretty(&manifest).map_err(|e| AppError::Internal(e.to_string()))?;
    zip.start_file("manifest.json", opts)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    std::io::Write::write_all(&mut zip, &manifest_bytes)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let checksum = format!(
        "records.csv  {}\nmanifest.json  {}\n",
        csv_hash,
        hex_sha256(&manifest_bytes)
    );
    zip.start_file("sha256.txt", opts)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    std::io::Write::write_all(&mut zip, checksum.as_bytes())
        .map_err(|e| AppError::Internal(e.to_string()))?;
    if matches!(module, Module::SampleInfo) {
        let dir = cfg.attachments_dir();
        let mut stmt=conn.prepare("SELECT stored_name FROM sample_info_attachments a JOIN sample_info_records r ON r.id=a.record_id WHERE r.submitted_at>=?1 AND r.submitted_at<=?2")?;
        let names = stmt
            .query_map(
                postgres_compat::params![start, format!("{}T23:59:59", end)],
                |r| r.get::<_, String>(0),
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for name in names {
            let path = dir.join(&name);
            if path.is_file() {
                let bytes = std::fs::read(&path).unwrap_or_default();
                zip.start_file(
                    format!(
                        "attachments/{}",
                        std::path::Path::new(&name)
                            .file_name()
                            .and_then(|v| v.to_str())
                            .unwrap_or("attachment")
                    ),
                    opts,
                )
                .map_err(|e| AppError::Internal(e.to_string()))?;
                std::io::Write::write_all(&mut zip, &bytes)
                    .map_err(|e| AppError::Internal(e.to_string()))?;
            }
        }
    }
    zip.finish()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let bytes = cursor.into_inner();
    let hash = hex_sha256(&bytes);
    Ok((bytes, hash))
}

fn filename(module: Module, start: &str, end: &str) -> String {
    format!(
        "{}_原始记录_{}_{}.zip",
        module.label(),
        start.replace(':', "-"),
        end.replace(':', "-")
    )
}

async fn export_data(
    State(pool): State<DbPool>,
    Path(module): Path<String>,
    headers: HeaderMap,
    Query(range): Query<DateRange>,
) -> Result<Response> {
    let ctx = require(&pool, &headers, "manage:data-governance:export")?;
    let module = Module::parse(&module)?;
    validate_range(&range.start, &range.end)?;
    let conn = pool.get()?;
    let cfg = AppConfig::load();
    let (bytes, hash) = build_archive(&conn, module, &range.start, &range.end, &cfg)?;
    audit_repo::log_actor(
        &pool,
        "export",
        module.table(),
        None,
        ctx.user.id,
        &ctx.user.username,
        &format!(
            "数据治理原始导出 {}~{} SHA-256 {}",
            range.start, range.end, hash
        ),
        module.key(),
    )
    .ok();
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/zip")
        .header(
            header::CONTENT_DISPOSITION,
            format!(
                "attachment; filename*=UTF-8''{}",
                url_escape::encode_component(&filename(module, &range.start, &range.end))
            ),
        )
        .body(Body::from(bytes))
        .map_err(|e| AppError::Internal(e.to_string()))?)
}

fn validate_range(start: &str, end: &str) -> Result<()> {
    if start.is_empty() || end.is_empty() || start > end {
        return Err(AppError::Validation(
            "时间范围无效，请确认开始时间不晚于结束时间".into(),
        ));
    }
    Ok(())
}

async fn purge_precheck(
    State(pool): State<DbPool>,
    Path(module): Path<String>,
    headers: HeaderMap,
    Json(range): Json<DateRange>,
) -> Result<Json<ApiResponse<PurgePreview>>> {
    let ctx = require(&pool, &headers, "manage:data-governance:purge")?;
    let module = Module::parse(&module)?;
    validate_range(&range.start, &range.end)?;
    let conn = pool.get()?;
    let date = module.date_column();
    let sql=format!("SELECT CAST(SUM(CASE WHEN deleted_at IS NULL THEN 1 ELSE 0 END) AS BIGINT),CAST(SUM(CASE WHEN deleted_at IS NOT NULL THEN 1 ELSE 0 END) AS BIGINT) FROM {} WHERE {}>=?1 AND {}<=?2",module.table(),date,date);
    let (active, deleted): (Option<i64>, Option<i64>) = conn.query_row(
        &sql,
        postgres_compat::params![range.start, format!("{}T23:59:59", range.end)],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let (attachment_count, attachment_bytes) = if matches!(module, Module::SampleInfo) {
        conn.query_row("SELECT COUNT(*),CAST(COALESCE(SUM(file_size),0) AS BIGINT) FROM sample_info_attachments a JOIN sample_info_records r ON r.id=a.record_id WHERE r.submitted_at>=?1 AND r.submitted_at<=?2",postgres_compat::params![range.start,format!("{}T23:59:59",range.end)],|r|Ok((r.get(0)?,r.get(1)?)))?
    } else {
        (0, 0)
    };
    let token = Uuid::new_v4().to_string();
    let expires = (Local::now() + chrono::Duration::minutes(10))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    conn.execute("INSERT INTO governance_confirmations(token,module,start_at,end_at,request_user_id,expires_at,created_at) VALUES(?1,?2,?3,?4,?5,?6,datetime('now','localtime'))",postgres_compat::params![token,module.key(),range.start,range.end,ctx.user.id,expires])?;
    Ok(Json(ApiResponse::ok(PurgePreview {
        module: module.key().to_string(),
        start: range.start,
        end: range.end,
        active_count: active.unwrap_or(0),
        deleted_count: deleted.unwrap_or(0),
        attachment_count,
        attachment_bytes,
        confirmation_token: token,
        expires_at: expires,
    })))
}

async fn purge_execute(
    State(pool): State<DbPool>,
    Path(module): Path<String>,
    headers: HeaderMap,
    Json(request): Json<PurgeRequest>,
) -> Result<Json<ApiResponse<PurgeResult>>> {
    let ctx = require(&pool, &headers, "manage:data-governance:purge")?;
    let module = Module::parse(&module)?;
    validate_range(&request.start, &request.end)?;
    let admin = user_admin(&pool, &request.admin_username, &request.admin_password)?;
    let conn = pool.get()?;
    let confirmation: Option<(i64,String,String,String)>=conn.query_row("SELECT request_user_id,module,start_at,end_at FROM governance_confirmations WHERE token=?1 AND datetime(expires_at)>datetime('now','localtime') AND used_at IS NULL",[request.confirmation_token.as_str()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
    let confirmation = confirmation
        .ok_or_else(|| AppError::Validation("删除确认已过期或无效，请重新预检".into()))?;
    if confirmation.0 != ctx.user.id
        || confirmation.1 != module.key()
        || confirmation.2 != request.start
        || confirmation.3 != request.end
    {
        return Err(AppError::Validation(
            "删除确认范围已变化，请重新预检".into(),
        ));
    }
    let cfg = AppConfig::load();
    let safety_cfg = AppConfig {
        backup_mode: "full".to_string(),
        ..cfg.clone()
    };
    backup_service::create_backup(&safety_cfg, false)
        .map_err(|e| AppError::Internal(format!("删除前全量备份失败：{e}")))?;
    let (archive, hash) = build_archive(&conn, module, &request.start, &request.end, &cfg)?;
    let export_dir = cfg.backup_dir().join("governance_exports");
    std::fs::create_dir_all(&export_dir).map_err(|e| AppError::Internal(e.to_string()))?;
    let export_name = filename(module, &request.start, &request.end);
    let export_path = export_dir.join(&export_name);
    std::fs::write(&export_path, &archive)
        .map_err(|e| AppError::Internal(format!("原始导出包保存失败：{e}")))?;
    let end = format!("{}T23:59:59", request.end);
    let preserved_attachment_count = if matches!(module, Module::SampleInfo) {
        conn.query_row(
            "SELECT COUNT(*) FROM sample_info_attachments a
             JOIN sample_info_records r ON r.id=a.record_id
             WHERE a.deleted_at IS NULL AND r.deleted_at IS NULL
               AND r.submitted_at>=?1 AND r.submitted_at<=?2",
            postgres_compat::params![request.start, end],
            |row| row.get(0),
        )?
    } else {
        0
    };
    drop(conn);
    let reason = format!(
        "数据治理批量移入回收站（{} 至 {}，管理员 {} 复核）",
        request.start, request.end, admin.username
    );
    let moved_count = match module {
        Module::Work => record_repo::soft_delete_range(
            &pool,
            &request.start,
            &end,
            &ctx.user.username,
            &reason,
        )?,
        Module::Rd => rd_record_repo::soft_delete_range(
            &pool,
            &request.start,
            &end,
            &ctx.user.username,
            &reason,
        )?,
        Module::SampleInfo => sample_info_repo::soft_delete_range(
            &pool,
            &request.start,
            &end,
            &ctx.user.username,
            &reason,
        )?,
    };
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let table = module.table();
    tx.execute(
        "UPDATE governance_confirmations SET used_at=datetime('now','localtime') WHERE token=?1",
        [request.confirmation_token.as_str()],
    )?;
    let event_summary = format!(
        "数据治理批量移入回收站：{}，范围 {}~{}，{} 条，复核管理员 {}，附件保留 {} 个，导出包 {}，SHA-256 {}",
        module.label(),
        request.start,
        request.end,
        moved_count,
        admin.username,
        preserved_attachment_count,
        export_name,
        hash
    );
    audit_repo::log_structured_actor_on_conn(
        &tx,
        "delete",
        table,
        None,
        ctx.user.id,
        &ctx.user.username,
        &event_summary,
        module.key(),
        "",
        Some(&serde_json::json!({
            "range": {"start": request.start, "end": request.end},
            "active": true,
        })),
        Some(&serde_json::json!({
            "range": {"start": request.start, "end": request.end},
            "moved_to_trash": moved_count,
            "attachments_preserved": preserved_attachment_count,
        })),
        "governance",
    )?;
    tx.execute("INSERT INTO governance_jobs(module,job_type,start_at,end_at,operator_user_id,operator_username,status,detail,created_at) VALUES(?1,'move_to_trash',?2,?3,?4,?5,'completed',?6,datetime('now','localtime'))", postgres_compat::params![module.key(),request.start,request.end,ctx.user.id,ctx.user.username,event_summary])?;
    tx.commit()?;
    Ok(Json(ApiResponse::ok(PurgeResult {
        module: module.key().to_string(),
        moved_count,
        preserved_attachment_count,
        export_file: export_path.to_string_lossy().to_string(),
        export_sha256: hash,
    })))
}

struct AdminUser {
    username: String,
}
fn user_admin(pool: &DbPool, username: &str, password: &str) -> Result<AdminUser> {
    let user = crate::repo::user_repo::find_by_username(pool, username)?
        .ok_or_else(|| AppError::Forbidden("管理员账号或密码错误".into()))?;
    if !user.is_active
        || !auth_service::verify_password(password, &user.password)
        || !(user.is_admin
            || user
                .role_names
                .iter()
                .any(|name| name == authz_service::ROLE_SYSTEM_ADMIN))
    {
        return Err(AppError::Forbidden("管理员账号或密码错误".into()));
    }
    Ok(AdminUser {
        username: user.username,
    })
}

fn directory_size(path: &PathBuf) -> u64 {
    if !path.exists() {
        return 0;
    }
    std::fs::read_dir(path)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| {
                    if entry.path().is_dir() {
                        directory_size(&entry.path())
                    } else {
                        entry.metadata().map(|m| m.len()).unwrap_or(0)
                    }
                })
                .sum()
        })
        .unwrap_or(0)
}

async fn template(
    State(pool): State<DbPool>,
    Path(module): Path<String>,
    headers: HeaderMap,
) -> Result<Response> {
    let ctx = require(&pool, &headers, "manage:data-governance:import")?;
    let module = Module::parse(&module)?;
    let mut workbook = Workbook::new();
    let mut sheet = workbook.add_worksheet();
    let header = Format::new().set_bold();
    let columns: Vec<&str> = match module {
        Module::Work => vec![
            "业务编号",
            "项目",
            "实验室",
            "方法",
            "仪器编号",
            "检测人员",
            "数量",
            "记录时间",
            "批号",
            "倍率",
            "高项",
            "备注",
        ],
        Module::Rd => vec![
            "业务编号",
            "项目",
            "实验室",
            "方法",
            "仪器编号",
            "送样人",
            "数量",
            "送样时间",
            "批号",
            "状态",
            "取样人",
            "取样时间",
            "检测人",
            "检测完成时间",
            "备注",
        ],
        Module::SampleInfo => vec![
            "业务编号",
            "状态",
            "序号",
            "批号",
            "送样人",
            "实验室",
            "项目",
            "送样时间",
            "检测日期",
            "主要成分",
            "检测类型",
            "类型标识",
            "数量",
            "备注",
            "扩展字段",
        ],
    };
    for (i, c) in columns.iter().enumerate() {
        sheet
            .write_string_with_format(0, i as u16, *c, &header)
            .map_err(|e| AppError::Internal(e.to_string()))?;
    }
    let example = match module {
        Module::Work => vec![
            "示例-不导入",
            "项目名称示例",
            "实验室示例",
            "方法名称示例",
            "LC-01",
            "用户示例",
            "1",
            "2026-07-20 09:00:00",
            "批号示例",
            "1",
            "",
            "示例行删除",
        ],
        Module::Rd => vec![
            "示例-不导入",
            "项目名称示例",
            "实验室示例",
            "方法名称示例",
            "LC-01",
            "用户示例",
            "1",
            "2026-07-20 09:00:00",
            "批号示例",
            "待取样",
            "",
            "",
            "",
            "",
            "示例行删除",
        ],
        Module::SampleInfo => vec![
            "示例-不导入",
            "待取样",
            "1",
            "批号示例",
            "用户示例",
            "实验室示例",
            "项目名称示例",
            "2026-07-20 09:00:00",
            "2026-07-21",
            "主要成分示例",
            "ICP",
            "icp",
            "1",
            "示例行删除",
            "{}",
        ],
    };
    for (i, v) in example.iter().enumerate() {
        sheet
            .write_string(1, i as u16, *v)
            .map_err(|e| AppError::Internal(e.to_string()))?;
    }
    let mut note = workbook.add_worksheet();
    note.set_name("填写说明").ok();
    note.write_string(0, 0, format!("{}数据治理导入模板 v0.4.103", module.label()))
        .map_err(|e| AppError::Internal(e.to_string()))?;
    note.write_string(
        1,
        0,
        "第一行是字段名；示例行以“示例-”开头，系统不会导入。默认仅新增，重复数据自动跳过。",
    )
    .map_err(|e| AppError::Internal(e.to_string()))?;
    let bytes = workbook
        .save_to_buffer()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    audit_repo::log_actor(
        &pool,
        "template",
        module.table(),
        None,
        ctx.user.id,
        &ctx.user.username,
        "下载数据治理导入模板",
        module.key(),
    )
    .ok();
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        )
        .header(
            header::CONTENT_DISPOSITION,
            format!(
                "attachment; filename*=UTF-8''{}",
                url_escape::encode_component(&format!(
                    "{}_数据治理导入模板_v0.4.103.xlsx",
                    module.label()
                ))
            ),
        )
        .body(Body::from(bytes))
        .map_err(|e| AppError::Internal(e.to_string()))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_migrations;

    fn test_connection() -> Connection {
        let conn = Connection::open_test_database().unwrap();
        test_migrations::run(&conn).unwrap();
        conn.execute("INSERT INTO project_groups(name) VALUES('实验室-A')", [])
            .unwrap();
        let lab_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO projects(group_id,name,coefficient,high_item) VALUES(?1,'项目-A',1.5,'高项-A')",
            [lab_id],
        ).unwrap();
        conn.execute(
            "INSERT INTO instruments(code,name,instrument_type) VALUES('LC-01','液相-1','液相')",
            [],
        )
        .unwrap();
        let instrument_1 = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO instruments(code,name,instrument_type) VALUES('LC-02','液相-2','液相')",
            [],
        )
        .unwrap();
        let instrument_2 = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO methods(name,full_name,instrument_id) VALUES('方法-A','方法-A',?1)",
            [instrument_1],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO methods(name,full_name,instrument_id) VALUES('方法-A','方法-A',?1)",
            [instrument_2],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO users(username,password,is_active) VALUES('检测员-A','hash',1)",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO users(username,password,is_active,is_admin) VALUES('导入管理员','hash',1,1)", []).unwrap();
        conn
    }

    fn work_row(instrument_code: &str) -> ImportRow {
        let mut values = BTreeMap::new();
        for (key, value) in [
            ("项目", "项目-A"),
            ("实验室", "实验室-A"),
            ("方法", "方法-A"),
            ("仪器编号", instrument_code),
            ("检测人员", "检测员-A"),
            ("数量", "2"),
            ("记录时间", "2026-07-20 09:00:00"),
            ("批号", "BATCH-001"),
        ] {
            values.insert(key.to_string(), value.to_string());
        }
        ImportRow {
            values,
            row_number: 2,
        }
    }

    #[test]
    fn duplicate_method_names_require_instrument_code() {
        let conn = test_connection();
        let error = prepare_row(&conn, Module::Work, &work_row(""), 1).unwrap_err();
        assert!(error.to_string().contains("必须填写仪器编号"));

        let prepared = prepare_row(&conn, Module::Work, &work_row("LC-02"), 1).unwrap();
        match prepared {
            PreparedRow::Work {
                instrument_code,
                instrument_id,
                ..
            } => {
                assert_eq!(instrument_code, "LC-02");
                assert!(instrument_id.is_some());
            }
            _ => panic!("unexpected module"),
        }
    }

    #[test]
    fn empty_tables_have_zero_summary_counts() {
        let conn = test_connection();
        for module in [Module::Work, Module::Rd, Module::SampleInfo] {
            let summary = summary_for(&conn, module).unwrap();
            assert_eq!(summary.total, 0);
            assert_eq!(summary.active, 0);
            assert_eq!(summary.deleted, 0);
        }
    }

    #[test]
    fn import_keeps_subject_and_operator_id_separate_and_deduplicates() {
        let mut conn = test_connection();
        let subject_id: i64 = conn
            .query_row(
                "SELECT id FROM users WHERE username='检测员-A'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let operator_id: i64 = conn
            .query_row(
                "SELECT id FROM users WHERE username='导入管理员'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let prepared = prepare_row(&conn, Module::Work, &work_row("LC-01"), operator_id).unwrap();

        let tx = conn.transaction().unwrap();
        insert_row(&tx, Module::Work, &prepared, operator_id, "导入管理员").unwrap();
        tx.commit().unwrap();

        let saved: (Option<i64>, Option<i64>, String) = conn.query_row(
            "SELECT subject_user_id,created_by_user_id,instrument_code_snapshot FROM work_records LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).unwrap();
        assert_eq!(saved.0, Some(subject_id));
        assert_eq!(saved.1, Some(operator_id));
        assert_eq!(saved.2, "LC-01");
        assert!(already_exists(&conn, Module::Work, &prepared).unwrap());
    }
}
