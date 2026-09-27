use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, State},
    http::{header, HeaderMap, Response, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use calamine::{open_workbook_auto, DataType, Range, Reader};
use postgres_compat::{Connection, OptionalExtension, Transaction};
use rust_xlsxwriter::{
    Color, DataValidation, Format, FormatAlign, FormatBorder, Workbook, XlsxError,
};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;

use crate::api::master_import_utils::{
    cell_to_string, check_duplicates, is_hex_color, method_key, method_key_parts, method_label,
    push_error, push_warning, split_multi, workbook_error,
};
use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::ApiResponse;
use crate::repo::trash_repo;
use crate::service::authz_service;

const MAX_UPLOAD_BYTES: usize = 20 * 1024 * 1024;
const DATA_SHEETS: [&str; 8] = [
    "部门",
    "实验室",
    "检测类型",
    "仪器",
    "检测方法",
    "研发项目",
    "项目关联",
    "预检结果",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImportMode {
    Upsert,
    Skip,
}

impl ImportMode {
    fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "upsert" => Ok(Self::Upsert),
            "skip" => Ok(Self::Skip),
            _ => Err(AppError::Validation("导入策略仅支持 upsert 或 skip".into())),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportIssue {
    pub sheet: String,
    pub row: usize,
    pub entity_type: String,
    pub name: String,
    pub action: String,
    pub level: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ImportCounts {
    pub total_rows: usize,
    pub departments: usize,
    pub labs: usize,
    pub method_types: usize,
    pub instruments: usize,
    pub methods: usize,
    pub projects: usize,
    pub relations: usize,
    pub creates: usize,
    pub updates: usize,
    pub deletes: usize,
    pub skips: usize,
    pub errors: usize,
    pub warnings: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct MasterImportPreview {
    pub valid: bool,
    pub mode: String,
    pub counts: ImportCounts,
    pub issues: Vec<ImportIssue>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MasterImportResult {
    pub success: bool,
    pub created: usize,
    pub updated: usize,
    pub deleted: usize,
    pub skipped: usize,
    pub relation_sets: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MasterDataPreviewSheet {
    pub name: String,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub total_rows: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct MasterDataPreview {
    pub generated_at: String,
    pub sheets: Vec<MasterDataPreviewSheet>,
}

#[derive(Debug, Clone)]
struct DepartmentRow {
    row: usize,
    name: String,
    sort_order: i64,
    color: String,
    is_active: bool,
    show_in_work: bool,
    show_in_rd: bool,
    show_in_sample_info: bool,
}

#[derive(Debug, Clone)]
struct LabRow {
    row: usize,
    name: String,
    department: String,
    sort_order: i64,
    show_in_work: bool,
    show_in_rd: bool,
    show_in_sample_info: bool,
}

#[derive(Debug, Clone)]
struct MethodTypeRow {
    row: usize,
    name: String,
    sort_order: i64,
}

#[derive(Debug, Clone)]
struct InstrumentRow {
    row: usize,
    code: String,
    name: String,
    instrument_type: String,
    is_active: bool,
    notes: String,
}

#[derive(Debug, Clone)]
struct MethodRow {
    row: usize,
    // Internal import key: method name + instrument code. It is never shown or persisted as a business field.
    method_code: String,
    name: String,
    instrument_code: String,
    full_name: String,
    method_types: Vec<String>,
    coefficient: f64,
    multiplier: f64,
    amount: f64,
    is_active: bool,
    notes: String,
    show_in_work: bool,
    show_in_rd: bool,
    show_in_sample_info: bool,
    is_common: bool,
    // Empty means every active department, preserving the former global common-method behaviour.
    common_divisions: Vec<String>,
}

#[derive(Debug, Clone)]
struct ProjectRow {
    row: usize,
    name: String,
    full_name: String,
    high_item: Option<String>,
    sort_order: i64,
    is_active: bool,
    project_status: String,
    notes: String,
    show_in_work: bool,
    show_in_rd: bool,
    show_in_sample_info: bool,
}

#[derive(Debug, Clone)]
struct RelationRow {
    row: usize,
    project: String,
    labs: Vec<String>,
    methods: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct ParsedData {
    departments: Vec<DepartmentRow>,
    labs: Vec<LabRow>,
    method_types: Vec<MethodTypeRow>,
    instruments: Vec<InstrumentRow>,
    methods: Vec<MethodRow>,
    projects: Vec<ProjectRow>,
    relations: Vec<RelationRow>,
}

fn merge_relation(
    relations: &mut Vec<RelationRow>,
    row: usize,
    project: String,
    lab: String,
    method: String,
) {
    let relation = if let Some(existing) = relations.iter_mut().find(|item| item.project == project)
    {
        existing
    } else {
        relations.push(RelationRow {
            row,
            project,
            labs: Vec::new(),
            methods: Vec::new(),
        });
        relations.last_mut().expect("relation inserted")
    };
    if !lab.is_empty() && !relation.labs.contains(&lab) {
        relation.labs.push(lab);
    }
    if !method.is_empty() && !relation.methods.contains(&method) {
        relation.methods.push(method);
    }
}

#[derive(Debug, Clone, Default)]
struct ApplyCounts {
    created: usize,
    updated: usize,
    deleted: usize,
    skipped: usize,
    relation_sets: usize,
}

#[derive(Debug, Default)]
struct SnapshotDeletePlan {
    departments: Vec<(i64, String)>,
    labs: Vec<(i64, String)>,
    method_types: Vec<(i64, String)>,
    instruments: Vec<(i64, String)>,
    methods: Vec<(i64, String)>,
    projects: Vec<(i64, String)>,
}

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/master-import/template", get(download_template))
        .route("/api/master-import/precheck", post(precheck))
        .route("/api/master-import/execute", post(execute))
        .route("/api/master-data/preview", get(preview))
        .route("/api/master-data/export", get(download_export))
        .with_state(pool)
        // 框架默认 2 MiB。这里按业务上限提前拦截，避免超大文件被整体读进内存后才报错。
        .layer(DefaultBodyLimit::max(MAX_UPLOAD_BYTES + 1024 * 1024))
}

async fn download_template(
    State(pool): State<DbPool>,
    headers: HeaderMap,
) -> Result<impl IntoResponse> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "manage:master-import")?;
    let _conn = pool.get()?;
    let bytes = build_template()?;
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        )
        .header(
            header::CONTENT_DISPOSITION,
            "attachment; filename=master_data_import_v1.1.6-hotfix.8-beta.xlsx; filename*=UTF-8''%E4%B8%BB%E6%95%B0%E6%8D%AE%E4%B8%80%E9%94%AE%E5%AF%BC%E5%85%A5%E6%A8%A1%E6%9D%BF_v1.1.6-hotfix.8-beta.xlsx",
        )
        .body(Body::from(bytes))
        .map_err(|e| AppError::Internal(format!("构建模板响应失败: {e}")))?;
    Ok(response)
}

async fn download_export(
    State(pool): State<DbPool>,
    headers: HeaderMap,
) -> Result<impl IntoResponse> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "manage:master-import")?;
    let conn = pool.get()?;
    let bytes = build_export_workbook(&conn)?;
    let date = chrono::Local::now().format("%Y%m%d");
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        )
        .header(
            header::CONTENT_DISPOSITION,
            format!(
                "attachment; filename=master_data_export_{date}.xlsx; filename*=UTF-8''%E4%B8%BB%E6%95%B0%E6%8D%AE%E5%AF%BC%E5%87%BA_{date}.xlsx"
            ),
        )
        .body(Body::from(bytes))
        .map_err(|e| AppError::Internal(format!("构建主数据导出响应失败: {e}")))?;
    Ok(response)
}

async fn preview(
    State(pool): State<DbPool>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<MasterDataPreview>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "manage:master-import")?;
    let conn = pool.get()?;
    Ok(Json(ApiResponse::ok(build_master_data_preview(&conn)?)))
}

async fn precheck(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    multipart: Multipart,
) -> Result<Json<ApiResponse<MasterImportPreview>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "manage:master-import")?;
    let (bytes, mode) = read_upload(multipart).await?;
    let (data, mut issues) = parse_uploaded_workbook(&bytes)?;
    let conn = pool.get()?;
    let preview = build_preview(&conn, &data, mode, &mut issues)?;
    Ok(Json(ApiResponse::ok(preview)))
}

async fn execute(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    multipart: Multipart,
) -> Result<Json<ApiResponse<MasterImportResult>>> {
    let ctx = authz_service::authenticate(&pool, &headers)?;
    authz_service::require_permission(&ctx, "manage:master-import")?;
    let (bytes, mode) = read_upload(multipart).await?;
    let (data, mut issues) = parse_uploaded_workbook(&bytes)?;
    let mut conn = pool.get()?;
    let preview = build_preview(&conn, &data, mode, &mut issues)?;
    if !preview.valid {
        return Err(AppError::Validation(format!(
            "预检未通过：{} 个错误，请修正模板后重试",
            preview.counts.errors
        )));
    }

    let tx = conn.transaction()?;
    let applied = match apply_import_as(&tx, &data, mode, &ctx.user.username) {
        Ok(value) => value,
        Err(error) => {
            let detail = format!("主数据导入失败，事务已回滚：{error}");
            let _ = tx.rollback();
            return Err(AppError::Validation(detail));
        }
    };
    if let Err(error) = tx.execute(
        "INSERT INTO audit_log (action, table_name, user_id, user_name, detail) VALUES ('import','master_data',?1,?2,?3)",
        postgres_compat::params![
            ctx.user.id,
            ctx.user.username,
            format!(
            "主数据一键导入：新增{}，更新{}，删除{}，跳过{}，关联{}",
            applied.created, applied.updated, applied.deleted, applied.skipped, applied.relation_sets
            )
        ],
    ) {
        let detail = format!("主数据导入审计写入失败，事务已回滚：{error}");
        let _ = tx.rollback();
        return Err(AppError::Validation(detail));
    }
    if let Err(error) = tx.commit() {
        return Err(AppError::Validation(format!("主数据导入提交失败：{error}")));
    }

    let message = format!(
        "导入完成：新增 {}，更新 {}，删除 {}，跳过 {}，项目关联 {}",
        applied.created, applied.updated, applied.deleted, applied.skipped, applied.relation_sets
    );
    Ok(Json(ApiResponse::ok(MasterImportResult {
        success: true,
        created: applied.created,
        updated: applied.updated,
        deleted: applied.deleted,
        skipped: applied.skipped,
        relation_sets: applied.relation_sets,
        message,
    })))
}

async fn read_upload(mut multipart: Multipart) -> Result<(Vec<u8>, ImportMode)> {
    let mut file: Option<Vec<u8>> = None;
    let mut mode = ImportMode::Upsert;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Validation(format!("读取上传内容失败: {e}")))?
    {
        match field.name() {
            Some("file") => {
                let filename = field.file_name().unwrap_or_default().to_ascii_lowercase();
                if !filename.is_empty() && !filename.ends_with(".xlsx") {
                    return Err(AppError::Validation("请选择 .xlsx 模板文件".into()));
                }
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|e| AppError::Validation(format!("读取上传文件失败: {e}")))?;
                if bytes.is_empty() {
                    return Err(AppError::Validation("上传文件为空".into()));
                }
                if bytes.len() > MAX_UPLOAD_BYTES {
                    return Err(AppError::Validation("上传文件不能超过 20 MB".into()));
                }
                if !bytes.starts_with(b"PK") {
                    return Err(AppError::Validation("文件不是有效的 xlsx 工作簿".into()));
                }
                file = Some(bytes.to_vec());
            }
            Some("mode") => {
                let value = field
                    .text()
                    .await
                    .map_err(|e| AppError::Validation(format!("读取导入策略失败: {e}")))?;
                mode = ImportMode::parse(&value)?;
            }
            _ => {}
        }
    }
    Ok((
        file.ok_or_else(|| AppError::Validation("未收到模板文件".into()))?,
        mode,
    ))
}

fn build_export_workbook(conn: &Connection) -> Result<Vec<u8>> {
    let mut workbook = build_template_workbook()?;
    let mut exported_method_keys = HashMap::<i64, (String, String)>::new();

    {
        let ws = workbook.worksheet_from_name("部门")?;
        let mut stmt = conn.prepare("SELECT name,sort_order,color,is_active,show_in_work,show_in_rd,show_in_sample_info FROM divisions WHERE deleted_at IS NULL ORDER BY sort_order,id")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
            ))
        })?;
        for (index, row) in rows.enumerate() {
            let (name, sort_order, color, active, show_work, show_rd, show_sample) = row?;
            ws.write((index + 1) as u32, 0, name)?;
            ws.write((index + 1) as u32, 1, sort_order)?;
            ws.write((index + 1) as u32, 2, color)?;
            ws.write((index + 1) as u32, 3, if active != 0 { "是" } else { "否" })?;
            ws.write(
                (index + 1) as u32,
                4,
                if show_work != 0 { "是" } else { "否" },
            )?;
            ws.write(
                (index + 1) as u32,
                5,
                if show_rd != 0 { "是" } else { "否" },
            )?;
            ws.write(
                (index + 1) as u32,
                6,
                if show_sample != 0 { "是" } else { "否" },
            )?;
            ws.write((index + 1) as u32, 7, (index + 1) as u32)?;
        }
    }
    {
        let ws = workbook.worksheet_from_name("实验室")?;
        let mut stmt = conn.prepare("SELECT g.name,COALESCE(d.name,''),g.sort_order,g.show_in_work,g.show_in_rd,g.show_in_sample_info FROM project_groups g LEFT JOIN divisions d ON d.id=g.division_id WHERE g.deleted_at IS NULL ORDER BY g.sort_order,g.id")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })?;
        for (index, row) in rows.enumerate() {
            let (name, division, sort_order, show_work, show_rd, show_sample) = row?;
            ws.write((index + 1) as u32, 0, name)?;
            ws.write((index + 1) as u32, 1, division)?;
            ws.write((index + 1) as u32, 2, sort_order)?;
            ws.write(
                (index + 1) as u32,
                3,
                if show_work != 0 { "是" } else { "否" },
            )?;
            ws.write(
                (index + 1) as u32,
                4,
                if show_rd != 0 { "是" } else { "否" },
            )?;
            ws.write(
                (index + 1) as u32,
                5,
                if show_sample != 0 { "是" } else { "否" },
            )?;
            ws.write((index + 1) as u32, 6, (index + 1) as u32)?;
        }
    }
    {
        let ws = workbook.worksheet_from_name("检测类型")?;
        let mut stmt = conn.prepare("SELECT name,sort_order FROM method_types WHERE deleted_at IS NULL ORDER BY sort_order,id")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        for (index, row) in rows.enumerate() {
            let (name, sort_order) = row?;
            ws.write((index + 1) as u32, 0, name)?;
            ws.write((index + 1) as u32, 1, sort_order)?;
            ws.write((index + 1) as u32, 2, (index + 1) as u32)?;
        }
    }
    {
        let ws = workbook.worksheet_from_name("仪器")?;
        let mut stmt = conn.prepare("SELECT code,name,instrument_type,is_active,notes FROM instruments WHERE deleted_at IS NULL ORDER BY code,id")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        for (index, row) in rows.enumerate() {
            let (code, name, instrument_type, active, notes) = row?;
            ws.write((index + 1) as u32, 0, code)?;
            ws.write((index + 1) as u32, 1, name)?;
            ws.write((index + 1) as u32, 2, instrument_type)?;
            ws.write((index + 1) as u32, 3, if active != 0 { "是" } else { "否" })?;
            ws.write((index + 1) as u32, 4, notes)?;
            ws.write((index + 1) as u32, 5, (index + 1) as u32)?;
        }
    }
    for (sheet_name, common) in [("检测方法", false), ("通用方法", true)] {
        let ws = workbook.worksheet_from_name(sheet_name)?;
        let sql = "SELECT m.id,m.name,m.full_name,COALESCE(i.code,''),m.coefficient,m.multiplier,m.amount,m.is_active,m.notes,m.show_in_work,m.show_in_rd,m.show_in_sample_info FROM methods m LEFT JOIN instruments i ON i.id=m.instrument_id WHERE m.deleted_at IS NULL AND m.is_common=?1 ORDER BY m.name,i.code,m.id";
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map([common], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, f64>(4)?,
                row.get::<_, f64>(5)?,
                row.get::<_, f64>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, i64>(9)?,
                row.get::<_, i64>(10)?,
                row.get::<_, i64>(11)?,
            ))
        })?;
        for (index, row) in rows.enumerate() {
            let (
                method_id,
                name,
                full_name,
                instrument_code,
                coefficient,
                multiplier,
                amount,
                active,
                notes,
                show_work,
                show_rd,
                show_sample,
            ) = row?;
            if !common {
                exported_method_keys.insert(method_id, (name.clone(), instrument_code.clone()));
            }
            let mut types_stmt = conn.prepare("SELECT mt.name FROM method_type_links mtl JOIN method_types mt ON mt.id=mtl.method_type_id WHERE mtl.method_id=?1 AND mt.deleted_at IS NULL ORDER BY mt.sort_order,mt.id")?;
            let types = types_stmt
                .query_map([method_id], |type_row| type_row.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            ws.write((index + 1) as u32, 0, name)?;
            ws.write((index + 1) as u32, 1, full_name)?;
            ws.write((index + 1) as u32, 2, instrument_code)?;
            if let Some(type_name) = types.first() {
                ws.write((index + 1) as u32, 3, type_name)?;
            }
            ws.write((index + 1) as u32, 4, coefficient)?;
            ws.write((index + 1) as u32, 5, multiplier)?;
            ws.write((index + 1) as u32, 6, amount)?;
            ws.write((index + 1) as u32, 7, if active != 0 { "是" } else { "否" })?;
            ws.write((index + 1) as u32, 8, notes)?;
            ws.write(
                (index + 1) as u32,
                9,
                if show_work != 0 { "是" } else { "否" },
            )?;
            ws.write(
                (index + 1) as u32,
                10,
                if show_rd != 0 { "是" } else { "否" },
            )?;
            ws.write(
                (index + 1) as u32,
                11,
                if show_sample != 0 { "是" } else { "否" },
            )?;
            ws.write(
                (index + 1) as u32,
                if common { 13 } else { 12 },
                (index + 1) as u32,
            )?;
            if common {
                let mut divisions_stmt = conn.prepare(
                    "SELECT d.name FROM common_method_division_scopes cmds JOIN divisions d ON d.id=cmds.division_id WHERE cmds.method_id=?1 AND d.deleted_at IS NULL ORDER BY d.sort_order,d.id",
                )?;
                let divisions = divisions_stmt
                    .query_map([method_id], |division_row| division_row.get::<_, String>(0))?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                ws.write((index + 1) as u32, 12, divisions.join("；"))?;
            }
        }
    }
    {
        let ws = workbook.worksheet_from_name("研发项目")?;
        let mut stmt = conn.prepare("SELECT name,full_name,COALESCE(high_item,''),project_status,sort_order,is_active,notes,show_in_work,show_in_rd,show_in_sample_info FROM projects WHERE deleted_at IS NULL ORDER BY sort_order,id")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, i64>(8)?,
                row.get::<_, i64>(9)?,
            ))
        })?;
        for (index, row) in rows.enumerate() {
            let (
                name,
                full_name,
                high_item,
                status,
                sort_order,
                active,
                notes,
                show_work,
                show_rd,
                show_sample,
            ) = row?;
            ws.write((index + 1) as u32, 0, name)?;
            ws.write((index + 1) as u32, 1, full_name)?;
            ws.write((index + 1) as u32, 2, high_item)?;
            ws.write(
                (index + 1) as u32,
                3,
                if status == "archived" {
                    "已归档"
                } else {
                    "进行中"
                },
            )?;
            ws.write((index + 1) as u32, 4, sort_order)?;
            ws.write((index + 1) as u32, 5, if active != 0 { "是" } else { "否" })?;
            ws.write((index + 1) as u32, 6, notes)?;
            ws.write(
                (index + 1) as u32,
                7,
                if show_work != 0 { "是" } else { "否" },
            )?;
            ws.write(
                (index + 1) as u32,
                8,
                if show_rd != 0 { "是" } else { "否" },
            )?;
            ws.write(
                (index + 1) as u32,
                9,
                if show_sample != 0 { "是" } else { "否" },
            )?;
            ws.write((index + 1) as u32, 10, (index + 1) as u32)?;
        }
    }
    {
        let ws = workbook.worksheet_from_name("项目关联")?;
        let mut next_row = 1u32;
        let mut project_stmt = conn.prepare(
            "SELECT id,name FROM projects WHERE deleted_at IS NULL ORDER BY sort_order,id",
        )?;
        let projects = project_stmt
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        for (project_id, project_name) in projects {
            let mut lab_stmt = conn.prepare(
                "SELECT g.name FROM (
                    SELECT group_id FROM project_lab_links WHERE project_id=?1
                    UNION
                    SELECT group_id FROM projects WHERE id=?1 AND group_id IS NOT NULL
                ) links
                JOIN project_groups g ON g.id=links.group_id
                WHERE g.deleted_at IS NULL
                ORDER BY g.sort_order,g.id",
            )?;
            let labs = lab_stmt
                .query_map([project_id], |row| row.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;

            let mut method_stmt = conn.prepare(
                "SELECT pml.method_id
                 FROM project_method_links pml
                 JOIN methods m ON m.id=pml.method_id
                 WHERE pml.project_id=?1 AND m.deleted_at IS NULL AND m.is_common=0
                 ORDER BY m.name,m.id",
            )?;
            let methods = method_stmt
                .query_map([project_id], |row| row.get::<_, i64>(0))?
                .filter_map(|row| row.ok())
                .filter_map(|method_id| exported_method_keys.get(&method_id).cloned())
                .collect::<Vec<_>>();

            match (labs.is_empty(), methods.is_empty()) {
                (false, false) => {
                    // The database stores project-lab and project-method links independently.
                    // Export every combination so no association is hidden by positional zipping.
                    for lab in &labs {
                        for (method, instrument_code) in &methods {
                            ws.write(next_row, 0, &project_name)?;
                            ws.write(next_row, 1, lab)?;
                            ws.write(next_row, 2, method)?;
                            ws.write(next_row, 3, instrument_code)?;
                            ws.write(next_row, 4, next_row)?;
                            next_row += 1;
                        }
                    }
                }
                (false, true) => {
                    for lab in &labs {
                        ws.write(next_row, 0, &project_name)?;
                        ws.write(next_row, 1, lab)?;
                        ws.write(next_row, 4, next_row)?;
                        next_row += 1;
                    }
                }
                (true, false) => {
                    for (method, instrument_code) in &methods {
                        ws.write(next_row, 0, &project_name)?;
                        ws.write(next_row, 2, method)?;
                        ws.write(next_row, 3, instrument_code)?;
                        ws.write(next_row, 4, next_row)?;
                        next_row += 1;
                    }
                }
                (true, true) => {}
            }
        }
    }

    Ok(workbook.save_to_buffer()?)
}

fn build_template_workbook() -> std::result::Result<Workbook, XlsxError> {
    let mut workbook = Workbook::new();
    let title = Format::new()
        .set_bold()
        .set_font_size(18)
        .set_font_color(Color::RGB(0x0B6E69));
    let section = Format::new()
        .set_bold()
        .set_font_color(Color::White)
        .set_background_color(Color::RGB(0x1976D2))
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center);
    let wrap = Format::new().set_text_wrap().set_align(FormatAlign::Top);

    {
        let ws = workbook.add_worksheet();
        ws.set_name("使用说明")?;
        ws.set_column_width(0, 20)?;
        ws.set_column_width(1, 90)?;
        let workbook_title = format!("主数据导入/导出工作簿 v{}", env!("CARGO_PKG_VERSION"));
        ws.merge_range(0, 0, 0, 1, &workbook_title, &title)?;
        let rows = [
            ("导入顺序", "部门 → 实验室 → 检测类型 → 仪器 → 检测方法/通用方法 → 研发项目 → 项目关联。系统会按该顺序自动处理。"),
            ("填写要求", "填写部门、实验室、检测类型、仪器、检测方法、通用方法、研发项目、项目关联工作表。带 * 的字段必填；不要修改工作表名称和表头。各业务表最后一列“序号”仅用于定位，不参与导入匹配。"),
            ("直接引用", "带下拉箭头的单元格直接引用前面的元数据表。请先完成元数据表，再填写项目关联。"),
            ("方法实例", "方法名称可以相同，但一条方法只绑定一台仪器；同名方法绑定不同仪器时属于不同方法实例。内部编号由程序自动生成。"),
            ("多行关联", "项目关联固定为“项目简称 + 关联实验室 + 关联方法名称 + 关联仪器编号”。一个项目有多个实验室或方法时逐行填写；导出会完整展开每个实验室与方法组合，回导时按项目自动去重合并。最后一列序号只用于确认导出的第几条。"),
            ("通用方法", "通用方法表内的方法自动关联全部项目，不需要在项目关联表重复填写；取消通用时请将该方法移到检测方法表，系统会清除其全部项目关联。分析检测显示部门留空表示全部部门；填写多个部门时请用中文分号“；”分隔。"),
            ("门户显示", "部门、实验室、项目、方法均可分别设置分析检测、研发送样、样品登记三个门户；父级隐藏时子级不会在该门户显示。"),
            ("导入策略", "覆盖更新：Excel 是完整快照。部门、实验室、类型、项目按名称匹配；仪器按仪器编号匹配；方法按“方法名称 + 仪器编号”匹配。匹配数据按 Excel 覆盖；Excel 中未出现的旧主数据永久删除。跳过已有：已有数据不修改、不删除，只新增 Excel 中不存在的数据。"),
            ("高项逻辑", "高项直接写入研发项目的高项文本字段，与当前项目管理逻辑一致。"),
            ("项目状态", "项目状态默认为进行中；选择已归档后保留历史数据，但不在前台录入项目列表中显示。"),
            ("示例说明", "“填写示例（不导入）”工作表内提供多组完整示例，仅用于参考，程序明确忽略该工作表，不参与预检和正式导入。"),
            ("安全机制", "请先在管理页面执行预检。正式导入使用单个事务，任一写入失败会整批回滚。"),
        ];
        for (idx, (name, description)) in rows.iter().enumerate() {
            ws.write_with_format((idx + 2) as u32, 0, *name, &section)?;
            ws.write_with_format((idx + 2) as u32, 1, *description, &wrap)?;
        }
    }

    {
        let ws = workbook.add_worksheet();
        ws.set_name("字段字典")?;
        let headers = ["工作表", "字段", "必填", "落库字段", "说明"];
        write_headers(ws, &headers)?;
        let rows = [
            ["部门", "部门名称*", "是", "divisions.name", "按名称匹配"],
            [
                "实验室",
                "所属部门*",
                "是",
                "project_groups.division_id",
                "可引用同一模板内的部门",
            ],
            [
                "检测方法",
                "检测类型*",
                "是",
                "method_type_links",
                "支持多个类型",
            ],
            [
                "仪器",
                "仪器编号*",
                "是",
                "instruments.code",
                "全局唯一，方法通过编号绑定",
            ],
            [
                "检测方法",
                "对应仪器编号*",
                "是",
                "methods.instrument_id",
                "每条方法实例只绑定一台仪器",
            ],
            [
                "通用方法",
                "分析检测显示部门",
                "否",
                "common_method_division_scopes",
                "留空表示全部部门；多个部门使用中文分号“；”分隔",
            ],
            [
                "研发项目",
                "高项",
                "否",
                "projects.high_item",
                "纯文本，不使用旧高项表",
            ],
            [
                "项目关联",
                "关联实验室",
                "否",
                "project_lab_links",
                "同一项目可用多行填写，空值允许",
            ],
            [
                "项目关联",
                "关联方法名称 + 关联仪器编号",
                "否",
                "project_method_links",
                "两列必须同时填写，用于准确引用同名方法实例",
            ],
        ];
        for (r, values) in rows.iter().enumerate() {
            for (c, value) in values.iter().enumerate() {
                ws.write((r + 1) as u32, c as u16, *value)?;
            }
        }
        for (col, width) in [18.0, 24.0, 10.0, 30.0, 48.0].iter().enumerate() {
            ws.set_column_width(col as u16, *width)?;
        }
        ws.set_freeze_panes(1, 0)?;
    }

    add_data_sheet(
        &mut workbook,
        "部门",
        &[
            "部门名称*",
            "排序",
            "颜色",
            "启用",
            "分析检测显示",
            "研发送样显示",
            "样品登记显示",
            "序号",
        ],
        &[24.0, 10.0, 14.0, 10.0, 14.0, 14.0, 14.0, 10.0],
        &[3, 4, 5, 6],
    )?;
    add_data_sheet(
        &mut workbook,
        "实验室",
        &[
            "实验室名称*",
            "所属部门*",
            "排序",
            "分析检测显示",
            "研发送样显示",
            "样品登记显示",
            "序号",
        ],
        &[24.0, 24.0, 10.0, 14.0, 14.0, 14.0, 10.0],
        &[3, 4, 5],
    )?;
    add_data_sheet(
        &mut workbook,
        "检测类型",
        &["类型名称*", "排序", "序号"],
        &[24.0, 10.0, 10.0],
        &[],
    )?;
    add_data_sheet(
        &mut workbook,
        "仪器",
        &["仪器编号*", "仪器名称", "仪器类型*", "启用", "备注", "序号"],
        &[20.0, 30.0, 20.0, 10.0, 36.0, 10.0],
        &[3],
    )?;
    add_data_sheet(
        &mut workbook,
        "检测方法",
        &[
            "方法名称*",
            "方法全称",
            "对应仪器编号*",
            "检测类型*",
            "系数",
            "倍率",
            "金额",
            "启用",
            "备注",
            "分析检测显示",
            "研发送样显示",
            "样品登记显示",
            "序号",
        ],
        &[
            28.0, 38.0, 22.0, 22.0, 10.0, 10.0, 12.0, 10.0, 36.0, 14.0, 14.0, 14.0, 10.0,
        ],
        &[7, 9, 10, 11],
    )?;
    add_data_sheet(
        &mut workbook,
        "通用方法",
        &[
            "方法名称*",
            "方法全称",
            "对应仪器编号*",
            "检测类型*",
            "系数",
            "倍率",
            "金额",
            "启用",
            "备注",
            "分析检测显示",
            "研发送样显示",
            "样品登记显示",
            "分析检测显示部门",
            "序号",
        ],
        &[
            28.0, 38.0, 22.0, 22.0, 10.0, 10.0, 12.0, 10.0, 36.0, 14.0, 14.0, 14.0, 32.0, 10.0,
        ],
        &[7, 9, 10, 11],
    )?;
    add_data_sheet(
        &mut workbook,
        "研发项目",
        &[
            "项目简称*",
            "项目全称",
            "高项",
            "项目状态",
            "排序",
            "启用",
            "备注",
            "分析检测显示",
            "研发送样显示",
            "样品登记显示",
            "序号",
        ],
        &[
            26.0, 38.0, 20.0, 14.0, 10.0, 10.0, 36.0, 14.0, 14.0, 14.0, 10.0,
        ],
        &[5, 7, 8, 9],
    )?;
    let relation_headers = [
        "项目简称*",
        "关联实验室",
        "关联方法名称",
        "关联仪器编号",
        "序号",
    ];
    add_data_sheet(
        &mut workbook,
        "项目关联",
        &relation_headers,
        &[26.0, 28.0, 38.0, 22.0, 10.0],
        &[],
    )?;
    add_data_sheet(
        &mut workbook,
        "预检结果",
        &[
            "工作表",
            "行号",
            "对象类型",
            "对象名称",
            "处理方式",
            "状态",
            "提示",
        ],
        &[18.0, 10.0, 18.0, 28.0, 16.0, 12.0, 56.0],
        &[],
    )?;

    workbook.define_name(
        "DepartmentOptions",
        "=OFFSET('部门'!$A$2,0,0,MAX(1,COUNTA('部门'!$A:$A)-1),1)",
    )?;
    workbook.define_name(
        "LabOptions",
        "=OFFSET('实验室'!$A$2,0,0,MAX(1,COUNTA('实验室'!$A:$A)-1),1)",
    )?;
    workbook.define_name(
        "MethodTypeOptions",
        "=OFFSET('检测类型'!$A$2,0,0,MAX(1,COUNTA('检测类型'!$A:$A)-1),1)",
    )?;
    workbook.define_name(
        "InstrumentOptions",
        "=OFFSET('仪器'!$A$2,0,0,MAX(1,COUNTA('仪器'!$A:$A)-1),1)",
    )?;
    workbook.define_name(
        "MethodOptions",
        "=OFFSET('检测方法'!$A$2,0,0,MAX(1,COUNTA('检测方法'!$A:$A)-1),1)",
    )?;
    workbook.define_name(
        "ProjectOptions",
        "=OFFSET('研发项目'!$A$2,0,0,MAX(1,COUNTA('研发项目'!$A:$A)-1),1)",
    )?;

    add_named_list_validation(&mut workbook, "实验室", &[1], "DepartmentOptions")?;
    add_named_list_validation(&mut workbook, "检测方法", &[2], "InstrumentOptions")?;
    add_named_list_validation(&mut workbook, "检测方法", &[3], "MethodTypeOptions")?;
    add_named_list_validation(&mut workbook, "通用方法", &[2], "InstrumentOptions")?;
    add_named_list_validation(&mut workbook, "通用方法", &[3], "MethodTypeOptions")?;
    add_named_list_validation(&mut workbook, "项目关联", &[0], "ProjectOptions")?;
    add_named_list_validation(&mut workbook, "项目关联", &[1], "LabOptions")?;
    add_named_list_validation(&mut workbook, "项目关联", &[2], "MethodOptions")?;
    add_named_list_validation(&mut workbook, "项目关联", &[3], "InstrumentOptions")?;
    let project_status = DataValidation::new().allow_list_strings(&["进行中", "已归档"])?;
    workbook
        .worksheet_from_name("研发项目")?
        .add_data_validation(1, 3, 1000, 3, &project_status)?;

    {
        let ws = workbook.add_worksheet();
        ws.set_name("填写示例（不导入）")?;
        write_headers(
            ws,
            &["步骤", "工作表", "示例内容", "说明（本表不参与导入）"],
        )?;
        let examples = [
            ["1", "仪器", "LC-01｜Agilent 1260｜液相｜是｜主液相仪器", "先在仪器表建立 LC-01"],
            ["2", "仪器", "LC-02｜Waters e2695｜液相｜是｜备用液相仪器", "同类型可以有多台仪器"],
            ["3", "仪器", "GC-01｜Agilent 8890｜气相｜是｜气相仪器", "仪器类型直接填写，不从编号猜测"],
            ["4", "检测方法", "含量测定-A｜高效液相色谱法含量测定｜LC-01｜含量｜液相｜｜1.5｜1｜50｜是｜主机方法", "方法名称保持纯净，内部编号由程序生成"],
            ["5", "检测方法", "含量测定-A｜高效液相色谱法含量测定｜LC-02｜含量｜液相｜｜1.5｜1｜50｜是｜备用机方法", "同名方法绑定不同仪器，属于新方法实例"],
            ["6", "检测方法", "残留溶剂-B｜气相色谱法残留溶剂测定｜GC-01｜残留溶剂｜气相｜｜1｜1｜60｜是｜常规", "类型和仪器均来自模板绑定"],
            ["7", "项目关联", "项目001｜液相实验室｜含量测定-A｜LC-01", "实验室和方法可以在同一行填写"],
            ["8", "项目关联", "项目001｜制备实验室｜含量测定-A｜LC-02", "同一项目可继续填写下一行"],
            ["9", "项目关联", "项目001｜｜残留溶剂-B｜GC-01", "数量不等时允许实验室留空；方法名称和仪器编号必须同时填写"],
        ];
        let wrap = Format::new().set_text_wrap().set_align(FormatAlign::Top);
        for (r, row) in examples.iter().enumerate() {
            for (c, value) in row.iter().enumerate() {
                ws.write_with_format((r + 1) as u32, c as u16, *value, &wrap)?;
            }
        }
        for (col, width) in [10.0, 18.0, 72.0, 52.0].iter().enumerate() {
            ws.set_column_width(col as u16, *width)?;
        }
        ws.set_freeze_panes(1, 0)?;
    }

    Ok(workbook)
}

fn build_template() -> std::result::Result<Vec<u8>, XlsxError> {
    build_template_workbook()?.save_to_buffer()
}

fn add_named_list_validation(
    workbook: &mut Workbook,
    sheet: &str,
    columns: &[u16],
    range_name: &str,
) -> std::result::Result<(), XlsxError> {
    let formula = format!("={range_name}");
    let validation = DataValidation::new().allow_list_formula(formula.as_str().into());
    let ws = workbook.worksheet_from_name(sheet)?;
    for col in columns {
        ws.add_data_validation(1, *col, 1000, *col, &validation)?;
    }
    Ok(())
}

fn write_headers(
    ws: &mut rust_xlsxwriter::Worksheet,
    headers: &[&str],
) -> std::result::Result<(), XlsxError> {
    let format = Format::new()
        .set_bold()
        .set_font_color(Color::White)
        .set_background_color(Color::RGB(0x1976D2))
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center)
        .set_text_wrap();
    for (col, header) in headers.iter().enumerate() {
        ws.write_with_format(0, col as u16, *header, &format)?;
    }
    Ok(())
}

fn add_data_sheet(
    workbook: &mut Workbook,
    name: &str,
    headers: &[&str],
    widths: &[f64],
    yes_no_columns: &[u16],
) -> std::result::Result<(), XlsxError> {
    let ws = workbook.add_worksheet();
    ws.set_name(name)?;
    write_headers(ws, headers)?;
    for (col, width) in widths.iter().enumerate() {
        ws.set_column_width(col as u16, *width)?;
    }
    ws.set_freeze_panes(1, 0)?;
    ws.autofilter(0, 0, 1000, (headers.len() - 1) as u16)?;
    if !yes_no_columns.is_empty() {
        let validation = DataValidation::new().allow_list_strings(&["是", "否"])?;
        for col in yes_no_columns {
            ws.add_data_validation(1, *col, 1000, *col, &validation)?;
        }
    }
    if name == "主数据" {
        let entity_validation = DataValidation::new().allow_list_strings(&[
            "部门",
            "实验室",
            "检测类型",
            "检测方法",
            "研发项目",
        ])?;
        ws.add_data_validation(1, 0, 1000, 0, &entity_validation)?;
        let status_validation = DataValidation::new().allow_list_strings(&["进行中", "已归档"])?;
        ws.add_data_validation(1, 6, 1000, 6, &status_validation)?;
    }
    Ok(())
}

fn build_master_data_preview(conn: &Connection) -> Result<MasterDataPreview> {
    // The online preview is read back from the same workbook builder used by
    // Excel export. This keeps sheet names, headers, relation expansion and
    // display values identical between preview, export and re-import.
    let bytes = build_export_workbook(conn)?;
    let path = std::env::temp_dir().join(format!(
        "master_data_preview_{}.xlsx",
        uuid::Uuid::new_v4().simple()
    ));
    fs::write(&path, bytes)
        .map_err(|error| AppError::Internal(format!("写入主数据预览临时文件失败: {error}")))?;

    let result = (|| {
        let mut workbook = open_workbook_auto(&path)
            .map_err(|error| AppError::Internal(format!("读取主数据预览工作簿失败: {error}")))?;
        let mut sheets = Vec::new();
        for sheet_name in [
            "部门",
            "实验室",
            "检测类型",
            "仪器",
            "检测方法",
            "通用方法",
            "研发项目",
            "项目关联",
        ] {
            let range = workbook
                .worksheet_range(sheet_name)
                .map_err(workbook_error)?;
            let mut rows = range.rows();
            let headers = rows
                .next()
                .ok_or_else(|| AppError::Validation(format!("工作表「{sheet_name}」缺少表头")))?
                .iter()
                .map(cell_to_string)
                .collect::<Vec<_>>();
            let data = rows
                .map(|row| {
                    let values = (0..headers.len())
                        .map(|column| row.get(column).map(cell_to_string).unwrap_or_default())
                        .collect::<Vec<_>>();
                    if values.iter().any(|value| !value.is_empty()) {
                        Some(values)
                    } else {
                        None
                    }
                })
                .flatten()
                .collect::<Vec<_>>();
            sheets.push(MasterDataPreviewSheet {
                name: sheet_name.to_string(),
                total_rows: data.len(),
                headers,
                rows: data,
            });
        }
        Ok(MasterDataPreview {
            generated_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            sheets,
        })
    })();
    let _ = fs::remove_file(&path);
    result
}

fn parse_uploaded_workbook(bytes: &[u8]) -> Result<(ParsedData, Vec<ImportIssue>)> {
    let path = std::env::temp_dir().join(format!("master_import_{}.xlsx", uuid::Uuid::new_v4()));
    std::fs::write(&path, bytes)
        .map_err(|e| AppError::Internal(format!("写入临时模板失败: {e}")))?;
    let result = parse_workbook_path(&path);
    let _ = std::fs::remove_file(path);
    result
}

fn parse_workbook_path(path: &std::path::Path) -> Result<(ParsedData, Vec<ImportIssue>)> {
    let mut workbook = open_workbook_auto(path)
        .map_err(|e| AppError::Validation(format!("无法打开 Excel 文件: {e}")))?;
    let sheet_names: HashSet<String> = workbook.sheet_names().iter().cloned().collect();
    if sheet_names.contains("主数据") {
        return Err(AppError::Validation(
            "旧版“主数据”合并模板已停用，请重新下载当前版本模板，分别填写仪器、检测方法和项目关联"
                .into(),
        ));
    }
    for sheet in DATA_SHEETS {
        if !sheet_names.contains(sheet) {
            return Err(AppError::Validation(format!(
                "缺少工作表「{sheet}」，请使用系统下载的模板"
            )));
        }
    }
    let uses_old_relation_layout = {
        let range = workbook
            .worksheet_range("项目关联")
            .map_err(workbook_error)?;
        range
            .rows()
            .next()
            .map(|headers| {
                headers.iter().map(cell_to_string).any(|header| {
                    header == "主实验室"
                        || header.starts_with("关联实验室1")
                        || header.starts_with("关联方法编号1")
                })
            })
            .unwrap_or(false)
    };
    if uses_old_relation_layout {
        return Err(AppError::Validation(
            "项目关联表仍是旧版横向结构，请重新下载当前版本模板并按项目多行填写".into(),
        ));
    }

    let mut data = ParsedData::default();
    let mut issues = Vec::new();

    let departments = read_sheet_rows(&workbook.worksheet_range("部门").map_err(workbook_error)?)?;
    for row in departments {
        let name = required(&row, "部门名称*", "部门", "部门", &mut issues);
        if name.is_empty() {
            continue;
        }
        let sort_order = parse_i64(&row, "排序", 0, "部门", &name, &mut issues);
        let color = value(&row, "颜色");
        let color = if color.is_empty() {
            "#1976d2".into()
        } else if is_hex_color(&color) {
            color
        } else {
            push_error(
                &mut issues,
                "部门",
                row.row,
                "部门",
                &name,
                "颜色必须是 #RRGGBB 格式",
            );
            "#1976d2".into()
        };
        let is_active = parse_bool(&row, "启用", true, "部门", &name, &mut issues);
        data.departments.push(DepartmentRow {
            row: row.row,
            name: name.clone(),
            sort_order,
            color,
            is_active,
            show_in_work: parse_bool(&row, "分析检测显示", true, "部门", &name, &mut issues),
            show_in_rd: parse_bool(&row, "研发送样显示", true, "部门", &name, &mut issues),
            show_in_sample_info: parse_bool(&row, "样品登记显示", true, "部门", &name, &mut issues),
        });
    }

    let labs = read_sheet_rows(&workbook.worksheet_range("实验室").map_err(workbook_error)?)?;
    for row in labs {
        let name = required(&row, "实验室名称*", "实验室", "实验室", &mut issues);
        let department = value(&row, "所属部门*");
        if name.is_empty() {
            continue;
        }
        data.labs.push(LabRow {
            row: row.row,
            sort_order: parse_i64(&row, "排序", 0, "实验室", &name, &mut issues),
            show_in_work: parse_bool(&row, "分析检测显示", true, "实验室", &name, &mut issues),
            show_in_rd: parse_bool(&row, "研发送样显示", true, "实验室", &name, &mut issues),
            show_in_sample_info: parse_bool(
                &row,
                "样品登记显示",
                true,
                "实验室",
                &name,
                &mut issues,
            ),
            name,
            department,
        });
    }

    let method_types = read_sheet_rows(
        &workbook
            .worksheet_range("检测类型")
            .map_err(workbook_error)?,
    )?;
    for row in method_types {
        let name = required(&row, "类型名称*", "检测类型", "检测类型", &mut issues);
        if name.is_empty() {
            continue;
        }
        data.method_types.push(MethodTypeRow {
            row: row.row,
            sort_order: parse_i64(&row, "排序", 0, "检测类型", &name, &mut issues),
            name,
        });
    }

    let instruments = read_sheet_rows(&workbook.worksheet_range("仪器").map_err(workbook_error)?)?;
    for row in instruments {
        let code = required(&row, "仪器编号*", "仪器", "仪器", &mut issues);
        let instrument_type = required(&row, "仪器类型*", "仪器", "仪器", &mut issues);
        if code.is_empty() || instrument_type.is_empty() {
            continue;
        }
        data.instruments.push(InstrumentRow {
            row: row.row,
            name: value(&row, "仪器名称"),
            is_active: parse_bool(&row, "启用", true, "仪器", &code, &mut issues),
            notes: value(&row, "备注"),
            code,
            instrument_type,
        });
    }

    for (sheet_name, is_common) in [("检测方法", false), ("通用方法", true)] {
        if !sheet_names.contains(sheet_name) {
            continue;
        }
        let methods = read_sheet_rows(
            &workbook
                .worksheet_range(sheet_name)
                .map_err(workbook_error)?,
        )?;
        for row in methods {
            let name = required(&row, "方法名称*", "检测方法", "检测方法", &mut issues);
            let instrument_code = value(&row, "对应仪器编号*");
            let method_types =
                collect_numbered_values(&row, &["检测类型*", "检测类型"], "检测类型", 3);
            if name.is_empty() {
                continue;
            }
            if method_types.len() > 1 {
                push_error(
                    &mut issues,
                    sheet_name,
                    row.row,
                    "检测方法",
                    &name,
                    "每个方法只能填写一个检测类型，请删除多余的检测类型列或值",
                );
            }
            let method_code = method_key(&name, &instrument_code);
            data.methods.push(MethodRow {
                row: row.row,
                method_code,
                instrument_code,
                full_name: value(&row, "方法全称"),
                coefficient: parse_f64(&row, "系数", 1.0, "检测方法", &name, &mut issues),
                multiplier: parse_f64(&row, "倍率", 1.0, "检测方法", &name, &mut issues),
                amount: parse_f64(&row, "金额", 0.0, "检测方法", &name, &mut issues),
                is_active: parse_bool(&row, "启用", true, "检测方法", &name, &mut issues),
                notes: value(&row, "备注"),
                show_in_work: parse_bool(
                    &row,
                    "分析检测显示",
                    true,
                    sheet_name,
                    &name,
                    &mut issues,
                ),
                show_in_rd: parse_bool(&row, "研发送样显示", true, sheet_name, &name, &mut issues),
                show_in_sample_info: parse_bool(
                    &row,
                    "样品登记显示",
                    true,
                    sheet_name,
                    &name,
                    &mut issues,
                ),
                is_common,
                common_divisions: if is_common {
                    split_multi(&value(&row, "分析检测显示部门"))
                } else {
                    vec![]
                },
                name,
                method_types,
            });
        }
    }

    let projects = read_sheet_rows(
        &workbook
            .worksheet_range("研发项目")
            .map_err(workbook_error)?,
    )?;
    for row in projects {
        let name = required(&row, "项目简称*", "研发项目", "研发项目", &mut issues);
        if name.is_empty() {
            continue;
        }
        let high_item = value(&row, "高项");
        data.projects.push(ProjectRow {
            row: row.row,
            full_name: value(&row, "项目全称"),
            high_item: if high_item.is_empty() {
                None
            } else {
                Some(high_item)
            },
            sort_order: parse_i64(&row, "排序", 0, "研发项目", &name, &mut issues),
            is_active: parse_bool(&row, "启用", true, "研发项目", &name, &mut issues),
            project_status: parse_project_status(&row, "项目状态", "研发项目", &name, &mut issues),
            notes: value(&row, "备注"),
            show_in_work: parse_bool(&row, "分析检测显示", true, "研发项目", &name, &mut issues),
            show_in_rd: parse_bool(&row, "研发送样显示", true, "研发项目", &name, &mut issues),
            show_in_sample_info: parse_bool(
                &row,
                "样品登记显示",
                true,
                "研发项目",
                &name,
                &mut issues,
            ),
            name,
        });
    }

    let relations = read_sheet_rows(
        &workbook
            .worksheet_range("项目关联")
            .map_err(workbook_error)?,
    )?;
    for row in relations {
        let project = value(&row, "项目简称*");
        let lab = value(&row, "关联实验室");
        let method_name = value(&row, "关联方法名称");
        let method_instrument = value(&row, "关联仪器编号");
        if project.is_empty()
            && lab.is_empty()
            && method_name.is_empty()
            && method_instrument.is_empty()
        {
            continue;
        }
        if project.is_empty() {
            push_error(
                &mut issues,
                "项目关联",
                row.row,
                "项目关联",
                "",
                "项目简称不能为空",
            );
            continue;
        }
        if method_name.is_empty() && !method_instrument.is_empty() {
            push_error(
                &mut issues,
                "项目关联",
                row.row,
                "项目关联",
                &project,
                "关联方法名称和关联仪器编号必须同时填写",
            );
            continue;
        }
        let method = if method_name.is_empty() {
            String::new()
        } else {
            method_key(&method_name, &method_instrument)
        };
        if lab.is_empty() && method.is_empty() {
            push_error(
                &mut issues,
                "项目关联",
                row.row,
                "项目关联",
                &project,
                "关联实验室或关联方法至少填写一项",
            );
            continue;
        }
        merge_relation(&mut data.relations, row.row, project, lab, method);
    }
    reconcile_relation_method_keys(&mut data);
    check_duplicates(
        &data.departments,
        |x| (&x.name, x.row),
        "部门",
        "部门",
        &mut issues,
    );
    check_duplicates(
        &data.labs,
        |x| (&x.name, x.row),
        "实验室",
        "实验室",
        &mut issues,
    );
    check_duplicates(
        &data.method_types,
        |x| (&x.name, x.row),
        "检测类型",
        "检测类型",
        &mut issues,
    );
    check_duplicates(
        &data.instruments,
        |x| (&x.code, x.row),
        "仪器",
        "仪器",
        &mut issues,
    );
    let mut method_instrument_pairs = HashMap::<String, usize>::new();
    for method in &data.methods {
        let key = format!("{}\u{1f}{}", method.name, method.instrument_code);
        if let Some(first_row) = method_instrument_pairs.insert(key, method.row) {
            push_error(
                &mut issues,
                "检测方法",
                method.row,
                "检测方法",
                &method_label(&method.method_code),
                &format!("同一方法名称与仪器只能建立一个方法实例，首次出现在第 {first_row} 行"),
            );
        }
    }
    check_duplicates(
        &data.projects,
        |x| (&x.name, x.row),
        "研发项目",
        "研发项目",
        &mut issues,
    );
    check_duplicates(
        &data.relations,
        |x| (&x.project, x.row),
        "项目关联",
        "项目关联",
        &mut issues,
    );

    Ok((data, issues))
}

// Older exports could write a stale instrument identifier in 项目关联 even
// when the method name uniquely identified one method row in the workbook.
// Canonicalize that legacy reference before snapshot validation and applying it.
fn reconcile_relation_method_keys(data: &mut ParsedData) {
    let mut methods_by_name: HashMap<String, Vec<String>> = HashMap::new();
    for method in &data.methods {
        methods_by_name
            .entry(method.name.clone())
            .or_default()
            .push(method.method_code.clone());
    }
    for relation in &mut data.relations {
        for method_key_value in &mut relation.methods {
            if data
                .methods
                .iter()
                .any(|method| method.method_code == *method_key_value)
            {
                continue;
            }
            let Some((method_name, _stale_instrument)) = method_key_parts(method_key_value) else {
                continue;
            };
            let Some(candidates) = methods_by_name.get(method_name) else {
                continue;
            };
            if candidates.len() == 1 {
                *method_key_value = candidates[0].clone();
            }
        }
    }
}

#[cfg(test)]
mod relation_key_tests {
    use super::*;

    #[test]
    fn legacy_relation_instrument_is_replaced_when_method_name_is_unique() {
        let mut data = ParsedData {
            methods: vec![MethodRow {
                row: 2,
                method_code: method_key("含量测定", "LC-01"),
                name: "含量测定".into(),
                instrument_code: "LC-01".into(),
                full_name: String::new(),
                method_types: Vec::new(),
                coefficient: 1.0,
                multiplier: 1.0,
                amount: 0.0,
                is_active: true,
                notes: String::new(),
                show_in_work: true,
                show_in_rd: true,
                show_in_sample_info: true,
                is_common: false,
                common_divisions: Vec::new(),
            }],
            relations: vec![RelationRow {
                row: 2,
                project: "项目001".into(),
                labs: vec!["实验室001".into()],
                methods: vec![method_key("含量测定", "OLD-LC-01")],
            }],
            ..ParsedData::default()
        };

        reconcile_relation_method_keys(&mut data);

        assert_eq!(
            data.relations[0].methods,
            vec![method_key("含量测定", "LC-01")]
        );
    }
}

#[derive(Debug)]
struct SheetRow {
    row: usize,
    values: HashMap<String, String>,
}

fn read_sheet_rows(range: &Range<DataType>) -> Result<Vec<SheetRow>> {
    let mut rows = range.rows();
    let headers: Vec<String> = rows
        .next()
        .ok_or_else(|| AppError::Validation("工作表缺少表头".into()))?
        .iter()
        .map(cell_to_string)
        .collect();
    let mut result = Vec::new();
    for (index, cells) in rows.enumerate() {
        let mut values = HashMap::new();
        let mut has_value = false;
        for (col, header) in headers.iter().enumerate() {
            if header.is_empty() {
                continue;
            }
            let cell = cells.get(col).map(cell_to_string).unwrap_or_default();
            if !cell.is_empty() {
                has_value = true;
            }
            values.insert(header.clone(), cell);
        }
        if has_value {
            result.push(SheetRow {
                row: index + 2,
                values,
            });
        }
    }
    Ok(result)
}

fn value(row: &SheetRow, header: &str) -> String {
    row.values
        .get(header)
        .map(|x| x.trim().to_string())
        .unwrap_or_default()
}

fn required(
    row: &SheetRow,
    header: &str,
    sheet: &str,
    entity_type: &str,
    issues: &mut Vec<ImportIssue>,
) -> String {
    let result = value(row, header);
    if result.is_empty() {
        push_error(
            issues,
            sheet,
            row.row,
            entity_type,
            "",
            &format!("必填字段「{header}」不能为空"),
        );
    }
    result
}

fn parse_i64(
    row: &SheetRow,
    header: &str,
    default: i64,
    entity_type: &str,
    name: &str,
    issues: &mut Vec<ImportIssue>,
) -> i64 {
    let raw = value(row, header);
    if raw.is_empty() {
        return default;
    }
    raw.parse::<i64>().unwrap_or_else(|_| {
        push_error(
            issues,
            entity_type,
            row.row,
            entity_type,
            name,
            &format!("字段「{header}」必须是整数"),
        );
        default
    })
}

fn parse_f64(
    row: &SheetRow,
    header: &str,
    default: f64,
    entity_type: &str,
    name: &str,
    issues: &mut Vec<ImportIssue>,
) -> f64 {
    let raw = value(row, header);
    if raw.is_empty() {
        return default;
    }
    match raw.parse::<f64>() {
        Ok(value) if value >= 0.0 => value,
        _ => {
            push_error(
                issues,
                entity_type,
                row.row,
                entity_type,
                name,
                &format!("字段「{header}」必须是非负数字"),
            );
            default
        }
    }
}

fn parse_bool(
    row: &SheetRow,
    header: &str,
    default: bool,
    entity_type: &str,
    name: &str,
    issues: &mut Vec<ImportIssue>,
) -> bool {
    let raw = value(row, header);
    if raw.is_empty() {
        return default;
    }
    match raw.to_ascii_lowercase().as_str() {
        "是" | "启用" | "true" | "1" | "yes" => true,
        "否" | "停用" | "false" | "0" | "no" => false,
        _ => {
            push_error(
                issues,
                entity_type,
                row.row,
                entity_type,
                name,
                &format!("字段「{header}」只能填写是或否"),
            );
            default
        }
    }
}

fn parse_project_status(
    row: &SheetRow,
    header: &str,
    sheet: &str,
    name: &str,
    issues: &mut Vec<ImportIssue>,
) -> String {
    let raw = value(row, header);
    match raw.trim().to_ascii_lowercase().as_str() {
        "" | "进行中" | "ongoing" => "ongoing".into(),
        "已归档" | "归档" | "archived" => "archived".into(),
        _ => {
            push_error(
                issues,
                sheet,
                row.row,
                "研发项目",
                name,
                "字段「项目状态」只能填写进行中或已归档",
            );
            "ongoing".into()
        }
    }
}

fn collect_numbered_values(
    row: &SheetRow,
    legacy_headers: &[&str],
    numbered_prefix: &str,
    max_columns: usize,
) -> Vec<String> {
    let mut values = Vec::new();
    for header in legacy_headers {
        values.extend(split_multi(&value(row, header)));
    }
    for index in 1..=max_columns {
        let header = format!("{numbered_prefix}{index}");
        values.extend(split_multi(&value(row, &header)));
        if index == 1 {
            values.extend(split_multi(&value(row, &format!("{header}*"))));
        }
    }
    let mut seen = HashSet::new();
    values.retain(|value| seen.insert(value.clone()));
    values
}

fn load_names(conn: &Connection, sql: &str) -> Result<HashSet<String>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    Ok(rows.filter_map(|row| row.ok()).collect())
}

fn build_preview(
    conn: &Connection,
    data: &ParsedData,
    mode: ImportMode,
    issues: &mut Vec<ImportIssue>,
) -> Result<MasterImportPreview> {
    let existing_departments = load_names(conn, "SELECT name FROM divisions")?;
    let existing_labs = load_names(conn, "SELECT name FROM project_groups")?;
    let existing_types = load_names(conn, "SELECT name FROM method_types")?;
    let existing_instruments = load_names(conn, "SELECT code FROM instruments")?;
    let existing_instrument_names = load_names(
        conn,
        "SELECT name FROM instruments WHERE trim(COALESCE(name, '')) <> ''",
    )?;
    let existing_projects = load_names(conn, "SELECT name FROM projects")?;

    let imported_departments: HashSet<_> =
        data.departments.iter().map(|x| x.name.clone()).collect();
    let imported_labs: HashSet<_> = data.labs.iter().map(|x| x.name.clone()).collect();
    let imported_types: HashSet<_> = data.method_types.iter().map(|x| x.name.clone()).collect();
    let imported_instruments: HashSet<_> =
        data.instruments.iter().map(|x| x.code.clone()).collect();
    let imported_methods: HashSet<_> = data.methods.iter().map(|x| x.method_code.clone()).collect();
    let imported_projects: HashSet<_> = data.projects.iter().map(|x| x.name.clone()).collect();

    for row in &data.instruments {
        match instrument_id_by_identity(conn, &row.code, &row.name) {
            Ok(_) => {}
            Err(AppError::Validation(message)) => {
                push_error(issues, "仪器", row.row, "仪器", &row.code, &message)
            }
            Err(error) => return Err(error),
        }
    }

    for row in &data.labs {
        if row.department.is_empty() {
            if mode == ImportMode::Upsert {
                // Empty is an explicit value in a full snapshot: clear the department.
            } else if existing_labs.contains(&row.name) {
                push_warning(
                    issues,
                    "实验室",
                    row.row,
                    "实验室",
                    &row.name,
                    "导出数据未提供所属部门，回导时保留数据库中的原所属部门",
                );
            } else {
                push_error(
                    issues,
                    "实验室",
                    row.row,
                    "实验室",
                    &row.name,
                    "新增实验室必须填写所属部门",
                );
            }
        } else if !existing_departments.contains(&row.department)
            && !imported_departments.contains(&row.department)
        {
            push_error(
                issues,
                "实验室",
                row.row,
                "实验室",
                &row.name,
                &format!("所属部门「{}」不存在", row.department),
            );
        }
    }
    for row in &data.methods {
        if row.method_types.len() > 1 {
            push_error(
                issues,
                "检测方法",
                row.row,
                "检测方法",
                &method_label(&row.method_code),
                "每个方法只能关联一个检测类型",
            );
        }
        let existing_method = method_id_by_import_identity(conn, &row.name, &row.instrument_code)?;
        if row.name.contains('@') {
            if existing_method.is_some() {
                push_warning(
                    issues,
                    "检测方法",
                    row.row,
                    "检测方法",
                    &row.name,
                    "检测方法名称包含历史仪器识别字符，回导时保留原名称；新建方法请将仪器填写在对应仪器编号列",
                );
            } else {
                push_error(
                    issues,
                    "检测方法",
                    row.row,
                    "检测方法",
                    &row.name,
                    "新增方法名称不得包含仪器识别字符，请将仪器填写在对应仪器编号列",
                );
            }
        }
        if row.instrument_code.is_empty() {
            if mode == ImportMode::Upsert {
                // The method identity includes its instrument. An empty value means
                // an explicitly unbound method, not "keep the old instrument".
            } else if existing_method.is_some() {
                push_warning(
                    issues,
                    "检测方法",
                    row.row,
                    "检测方法",
                    &method_label(&row.method_code),
                    "导出数据未提供仪器编号，回导时保留数据库中的原仪器绑定",
                );
            } else {
                push_error(
                    issues,
                    "检测方法",
                    row.row,
                    "检测方法",
                    &method_label(&row.method_code),
                    "新增检测方法必须填写对应仪器编号",
                );
            }
        } else if !existing_instruments.contains(&row.instrument_code)
            && !imported_instruments.contains(&row.instrument_code)
        {
            push_error(
                issues,
                "检测方法",
                row.row,
                "检测方法",
                &method_label(&row.method_code),
                &format!("对应仪器编号「{}」不存在", row.instrument_code),
            );
        }
        if row.method_types.is_empty() {
            if mode == ImportMode::Upsert {
                // Empty type links are intentional in a full snapshot.
            } else if existing_method.is_some() {
                push_warning(
                    issues,
                    "检测方法",
                    row.row,
                    "检测方法",
                    &method_label(&row.method_code),
                    "导出数据未提供检测类型，回导时保留数据库中的原检测类型",
                );
            } else {
                push_error(
                    issues,
                    "检测方法",
                    row.row,
                    "检测方法",
                    &method_label(&row.method_code),
                    "新增检测方法至少选择一个检测类型",
                );
            }
        } else {
            for type_name in &row.method_types {
                if !existing_types.contains(type_name) && !imported_types.contains(type_name) {
                    push_error(
                        issues,
                        "检测方法",
                        row.row,
                        "检测方法",
                        &method_label(&row.method_code),
                        &format!("检测类型「{type_name}」不存在"),
                    );
                }
            }
        }
        if row.is_common {
            for division_name in &row.common_divisions {
                if !existing_departments.contains(division_name)
                    && !imported_departments.contains(division_name)
                {
                    push_error(
                        issues,
                        "通用方法",
                        row.row,
                        "通用方法",
                        &method_label(&row.method_code),
                        &format!("分析检测显示部门「{division_name}」不存在"),
                    );
                }
            }
        }
    }
    let relation_projects: HashSet<_> = data.relations.iter().map(|x| x.project.clone()).collect();
    for row in &data.projects {
        if !existing_projects.contains(&row.name) && !relation_projects.contains(&row.name) {
            push_error(
                issues,
                "研发项目",
                row.row,
                "研发项目",
                &row.name,
                "新增项目必须在「项目关联」工作表中配置至少一个实验室和检测方法",
            );
        }
    }
    for row in &data.relations {
        if !existing_projects.contains(&row.project) && !imported_projects.contains(&row.project) {
            push_error(
                issues,
                "项目关联",
                row.row,
                "项目关联",
                &row.project,
                "引用的研发项目不存在",
            );
        }
        for lab in &row.labs {
            if !existing_labs.contains(lab) && !imported_labs.contains(lab) {
                push_error(
                    issues,
                    "项目关联",
                    row.row,
                    "项目关联",
                    &row.project,
                    &format!("关联实验室「{lab}」不存在"),
                );
            }
        }
        for method in &row.methods {
            let (method_name, instrument_code) = method_key_parts(method).unwrap_or((method, ""));
            let existing_method = method_id_by_import_identity(conn, method_name, instrument_code)?;
            if existing_method.is_none() && !imported_methods.contains(method) {
                push_error(
                    issues,
                    "项目关联",
                    row.row,
                    "项目关联",
                    &row.project,
                    &format!("关联方法「{}」不存在", method_label(method)),
                );
            }
        }
        if row.labs.is_empty() {
            if mode == ImportMode::Upsert {
                // The snapshot-specific validation below reports this as an error.
            } else if existing_projects.contains(&row.project) {
                push_warning(
                    issues,
                    "项目关联",
                    row.row,
                    "项目关联",
                    &row.project,
                    "导出数据未提供实验室关联，回导时保留数据库中的原实验室关联",
                );
            } else {
                push_error(
                    issues,
                    "项目关联",
                    row.row,
                    "项目关联",
                    &row.project,
                    "新增项目至少需要关联一个实验室",
                );
            }
        }
        if row.methods.is_empty() {
            if mode == ImportMode::Upsert {
                // A project can intentionally have no non-common methods.
            } else if existing_projects.contains(&row.project) {
                push_warning(
                    issues,
                    "项目关联",
                    row.row,
                    "项目关联",
                    &row.project,
                    "导出数据未提供检测方法关联，回导时保留数据库中的原检测方法关联",
                );
            } else {
                push_error(
                    issues,
                    "项目关联",
                    row.row,
                    "项目关联",
                    &row.project,
                    "新增项目至少需要关联一个检测方法",
                );
            }
        }
    }

    if mode == ImportMode::Upsert {
        validate_snapshot_references(data, issues);
        let plan = build_snapshot_delete_plan(conn, data)?;
        validate_snapshot_deletions(conn, &plan, issues)?;
    }

    let mut counts = ImportCounts {
        departments: data.departments.len(),
        labs: data.labs.len(),
        method_types: data.method_types.len(),
        instruments: data.instruments.len(),
        methods: data.methods.len(),
        projects: data.projects.len(),
        relations: data.relations.len(),
        ..ImportCounts::default()
    };
    counts.total_rows = counts.departments
        + counts.labs
        + counts.method_types
        + counts.instruments
        + counts.methods
        + counts.projects
        + counts.relations;

    append_actions(
        issues,
        "部门",
        "部门",
        data.departments.iter().map(|x| (x.row, &x.name)),
        &existing_departments,
        mode,
        &mut counts,
    );
    append_actions(
        issues,
        "实验室",
        "实验室",
        data.labs.iter().map(|x| (x.row, &x.name)),
        &existing_labs,
        mode,
        &mut counts,
    );
    append_actions(
        issues,
        "检测类型",
        "检测类型",
        data.method_types.iter().map(|x| (x.row, &x.name)),
        &existing_types,
        mode,
        &mut counts,
    );
    for row in &data.instruments {
        let exists_by_code = existing_instruments.contains(&row.code);
        let exists_by_name =
            !row.name.trim().is_empty() && existing_instrument_names.contains(&row.name);
        let existing = exists_by_code || exists_by_name;
        let action = if existing && mode == ImportMode::Skip {
            counts.skips += 1;
            "跳过"
        } else if existing {
            counts.updates += 1;
            "更新"
        } else {
            counts.creates += 1;
            "新增"
        };
        issues.push(ImportIssue {
            sheet: "仪器".into(),
            row: row.row,
            entity_type: "仪器".into(),
            name: row.code.clone(),
            action: action.into(),
            level: "info".into(),
            message: if exists_by_code && exists_by_name {
                "按仪器编号和名称匹配已有数据".into()
            } else if exists_by_name {
                "按仪器名称匹配已有数据".into()
            } else if exists_by_code {
                "按仪器编号匹配已有数据".into()
            } else {
                "新仪器".into()
            },
        });
    }
    for row in &data.methods {
        let exists = method_id_by_import_identity(conn, &row.name, &row.instrument_code)?.is_some();
        let action = if !exists {
            counts.creates += 1;
            "新增"
        } else if mode == ImportMode::Upsert {
            counts.updates += 1;
            "更新"
        } else {
            counts.skips += 1;
            "跳过"
        };
        issues.push(ImportIssue {
            sheet: "检测方法".into(),
            row: row.row,
            entity_type: "检测方法".into(),
            name: method_label(&row.method_code),
            action: action.into(),
            level: "info".into(),
            message: if exists {
                "按方法名称与仪器绑定匹配已有数据；缺失字段回导时保留原值".into()
            } else {
                "名称和仪器校验通过".into()
            },
        });
    }
    append_actions(
        issues,
        "研发项目",
        "研发项目",
        data.projects.iter().map(|x| (x.row, &x.name)),
        &existing_projects,
        mode,
        &mut counts,
    );
    for row in &data.relations {
        issues.push(ImportIssue {
            sheet: "项目关联".into(),
            row: row.row,
            entity_type: "项目关联".into(),
            name: row.project.clone(),
            action: if mode == ImportMode::Skip && existing_projects.contains(&row.project) {
                "跳过".into()
            } else {
                "写入关联".into()
            },
            level: "info".into(),
            message: format!(
                "{} 个实验室，{} 个检测方法",
                row.labs.len(),
                row.methods.len()
            ),
        });
    }

    if mode == ImportMode::Upsert {
        let plan = build_snapshot_delete_plan(conn, data)?;
        append_snapshot_delete_preview(&plan, &mut counts, issues);
    }

    counts.errors = issues.iter().filter(|x| x.level == "error").count();
    counts.warnings = issues.iter().filter(|x| x.level == "warning").count();
    let valid = counts.errors == 0 && counts.total_rows > 0;
    if counts.total_rows == 0 {
        issues.push(ImportIssue {
            sheet: "模板".into(),
            row: 0,
            entity_type: "模板".into(),
            name: String::new(),
            action: "阻止导入".into(),
            level: "error".into(),
            message: "模板中没有可导入数据".into(),
        });
        counts.errors += 1;
    }

    Ok(MasterImportPreview {
        valid,
        mode: if mode == ImportMode::Upsert {
            "upsert"
        } else {
            "skip"
        }
        .into(),
        counts,
        issues: issues.clone(),
    })
}

fn append_actions<'a, I>(
    issues: &mut Vec<ImportIssue>,
    sheet: &str,
    entity_type: &str,
    rows: I,
    existing: &HashSet<String>,
    mode: ImportMode,
    counts: &mut ImportCounts,
) where
    I: Iterator<Item = (usize, &'a String)>,
{
    for (row, name) in rows {
        let exists = existing.contains(name);
        let action = if !exists {
            counts.creates += 1;
            "新增"
        } else if mode == ImportMode::Upsert {
            counts.updates += 1;
            "更新"
        } else {
            counts.skips += 1;
            "跳过"
        };
        issues.push(ImportIssue {
            sheet: sheet.into(),
            row,
            entity_type: entity_type.into(),
            name: name.clone(),
            action: action.into(),
            level: "info".into(),
            message: if exists {
                "数据库中已存在同名数据".into()
            } else {
                "名称校验通过".into()
            },
        });
    }
}

// A snapshot import must be self-contained. It may not quietly depend on
// master data that happens to be present in the database at import time.
fn validate_snapshot_references(data: &ParsedData, issues: &mut Vec<ImportIssue>) {
    let departments: HashSet<&str> = data
        .departments
        .iter()
        .map(|row| row.name.as_str())
        .collect();
    let labs: HashSet<&str> = data.labs.iter().map(|row| row.name.as_str()).collect();
    let method_types: HashSet<&str> = data
        .method_types
        .iter()
        .map(|row| row.name.as_str())
        .collect();
    let instruments: HashSet<&str> = data
        .instruments
        .iter()
        .map(|row| row.code.as_str())
        .collect();
    let projects: HashSet<&str> = data.projects.iter().map(|row| row.name.as_str()).collect();
    let methods: HashSet<String> = data
        .methods
        .iter()
        .map(|row| method_key(&row.name, &row.instrument_code))
        .collect();
    let relations: HashMap<&str, &RelationRow> = data
        .relations
        .iter()
        .map(|row| (row.project.as_str(), row))
        .collect();

    for row in &data.labs {
        if !row.department.is_empty() && !departments.contains(row.department.as_str()) {
            push_error(
                issues,
                "实验室",
                row.row,
                "实验室",
                &row.name,
                "覆盖更新要求所属部门也必须存在于本次 Excel 中，不能引用系统中未导出的部门。",
            );
        }
    }
    for row in &data.methods {
        if !row.instrument_code.is_empty() && !instruments.contains(row.instrument_code.as_str()) {
            push_error(
                issues,
                "检测方法",
                row.row,
                "检测方法",
                &method_label(&row.method_code),
                "覆盖更新要求对应仪器也必须存在于本次 Excel 中。",
            );
        }
        for method_type in &row.method_types {
            if !method_types.contains(method_type.as_str()) {
                push_error(
                    issues,
                    "检测方法",
                    row.row,
                    "检测方法",
                    &method_label(&row.method_code),
                    "覆盖更新要求关联检测类型也必须存在于本次 Excel 中。",
                );
            }
        }
    }
    for project in &data.projects {
        let relation = relations.get(project.name.as_str()).copied();
        if relation.is_none_or(|item| item.labs.is_empty()) {
            push_error(
                issues,
                "项目关联",
                project.row,
                "研发项目",
                &project.name,
                "覆盖更新中每个项目必须在“项目关联”表至少关联一个实验室，用于完整重建项目归属。",
            );
        }
    }
    for relation in &data.relations {
        if !projects.contains(relation.project.as_str()) {
            push_error(
                issues,
                "项目关联",
                relation.row,
                "项目关联",
                &relation.project,
                "覆盖更新要求关联项目也必须存在于本次 Excel 的“研发项目”表中。",
            );
        }
        for lab in &relation.labs {
            if !labs.contains(lab.as_str()) {
                push_error(
                    issues,
                    "项目关联",
                    relation.row,
                    "项目关联",
                    &relation.project,
                    "覆盖更新要求关联实验室也必须存在于本次 Excel 的“实验室”表中。",
                );
            }
        }
        for method in &relation.methods {
            if !methods.contains(method) {
                push_error(
                    issues,
                    "项目关联",
                    relation.row,
                    "项目关联",
                    &relation.project,
                    "覆盖更新要求关联方法也必须存在于本次 Excel 的方法表中。",
                );
            }
        }
    }
}

fn snapshot_has_reference(conn: &Connection, table: &str, column: &str, id: i64) -> Result<bool> {
    let sql = format!("SELECT COUNT(1) FROM {table} WHERE {column}=?1");
    Ok(conn.query_row(&sql, [id], |row| row.get::<_, i64>(0))? > 0)
}

fn validate_snapshot_deletions(
    conn: &Connection,
    plan: &SnapshotDeletePlan,
    issues: &mut Vec<ImportIssue>,
) -> Result<()> {
    let checks: [(&str, &Vec<(i64, String)>, &[(&str, &str)]); 6] = [
        (
            "研发项目",
            &plan.projects,
            &[
                ("personnel_change_feedback_projects", "project_id"),
                ("rd_work_records", "project_id"),
                ("sample_records", "project_id"),
                ("work_records", "project_id"),
            ],
        ),
        (
            "检测方法",
            &plan.methods,
            &[
                ("rd_work_records", "method_id"),
                ("work_records", "method_id"),
            ],
        ),
        (
            "仪器",
            &plan.instruments,
            &[
                ("rd_work_records", "instrument_id_snapshot"),
                ("work_records", "instrument_id_snapshot"),
            ],
        ),
        ("检测类型", &plan.method_types, &[]),
        (
            "实验室",
            &plan.labs,
            &[
                ("personnel_change_feedbacks", "lab_id"),
                ("rd_work_records", "group_id"),
                ("sample_info_records", "group_id"),
                ("sample_records", "group_id"),
                ("users", "group_id"),
                ("work_records", "group_id"),
            ],
        ),
        (
            "部门",
            &plan.departments,
            &[
                ("rd_work_records", "division_id"),
                ("users", "division_id"),
                ("work_records", "division_id"),
            ],
        ),
    ];
    for (entity_type, rows, references) in checks {
        for (id, name) in rows {
            let mut referenced_by = None;
            for (table, column) in references {
                if snapshot_has_reference(conn, table, column, *id)? {
                    referenced_by = Some(*table);
                    break;
                }
            }
            if let Some(table) = referenced_by {
                push_error(
                    issues,
                    "系统现有数据",
                    0,
                    entity_type,
                    name,
                    &format!("该数据未出现在 Excel 中，本应永久删除；但仍被 {table} 中的历史或用户数据引用，整批覆盖更新会回滚。请先保留该主数据或处理引用数据。"),
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
fn apply_import(tx: &Transaction<'_>, data: &ParsedData, mode: ImportMode) -> Result<ApplyCounts> {
    apply_import_as(tx, data, mode, "import")
}

fn apply_import_as(
    tx: &Transaction<'_>,
    data: &ParsedData,
    mode: ImportMode,
    operator: &str,
) -> Result<ApplyCounts> {
    let mut counts = ApplyCounts::default();
    let existing_project_names = load_names(tx, "SELECT name FROM projects")?;
    for row in &data.departments {
        let existing = id_by_name(tx, "divisions", &row.name)?;
        match (existing, mode) {
            (Some(id), ImportMode::Upsert) => {
                tx.execute("UPDATE divisions SET sort_order=?1,color=?2,is_active=?3,show_in_work=?4,show_in_rd=?5,show_in_sample_info=?6,deleted_at=NULL WHERE id=?7", postgres_compat::params![row.sort_order,row.color,row.is_active,row.show_in_work,row.show_in_rd,row.show_in_sample_info,id])?;
                trash_repo::mark_restored_on_conn(tx, "divisions", id, operator)?;
                counts.updated += 1;
            }
            (Some(_), ImportMode::Skip) => counts.skipped += 1,
            (None, _) => {
                tx.execute(
                    "INSERT INTO divisions (name,sort_order,color,is_active,show_in_work,show_in_rd,show_in_sample_info) VALUES (?1,?2,?3,?4,?5,?6,?7)",
                    postgres_compat::params![row.name,row.sort_order,row.color,row.is_active,row.show_in_work,row.show_in_rd,row.show_in_sample_info],
                )?;
                counts.created += 1;
            }
        }
    }
    for row in &data.labs {
        let division_id = if row.department.trim().is_empty() {
            None
        } else {
            Some(
                id_by_name(tx, "divisions", &row.department)?.ok_or_else(|| {
                    AppError::Validation(format!("部门不存在: {}", row.department))
                })?,
            )
        };
        let existing = id_by_name(tx, "project_groups", &row.name)?;
        match (existing, mode) {
            (Some(id), ImportMode::Upsert) => {
                // In snapshot mode an empty department cell explicitly clears the
                // affiliation. Do not retain a stale database value with COALESCE.
                tx.execute("UPDATE project_groups SET sort_order=?1,show_in_work=?2,show_in_rd=?3,show_in_sample_info=?4,division_id=?5,deleted_at=NULL WHERE id=?6", postgres_compat::params![row.sort_order,row.show_in_work,row.show_in_rd,row.show_in_sample_info,division_id,id])?;
                trash_repo::mark_restored_on_conn(tx, "project_groups", id, operator)?;
                counts.updated += 1;
            }
            (Some(_), ImportMode::Skip) => counts.skipped += 1,
            (None, _) => {
                tx.execute("INSERT INTO project_groups (name,sort_order,show_in_work,show_in_rd,show_in_sample_info,division_id) VALUES (?1,?2,?3,?4,?5,?6)", postgres_compat::params![row.name,row.sort_order,row.show_in_work,row.show_in_rd,row.show_in_sample_info,division_id])?;
                counts.created += 1;
            }
        }
    }
    for row in &data.method_types {
        let existing = id_by_name(tx, "method_types", &row.name)?;
        match (existing, mode) {
            (Some(id), ImportMode::Upsert) => {
                tx.execute(
                    "UPDATE method_types SET sort_order=?1 WHERE id=?2",
                    postgres_compat::params![row.sort_order, id],
                )?;
                trash_repo::mark_restored_on_conn(tx, "method_types", id, operator)?;
                counts.updated += 1;
            }
            (Some(_), ImportMode::Skip) => counts.skipped += 1,
            (None, _) => {
                tx.execute(
                    "INSERT INTO method_types (name,sort_order) VALUES (?1,?2)",
                    postgres_compat::params![row.name, row.sort_order],
                )?;
                counts.created += 1;
            }
        }
    }
    for row in &data.instruments {
        let existing = instrument_id_by_identity(tx, &row.code, &row.name)?;
        match (existing, mode) {
            (Some(id), ImportMode::Upsert) => {
                tx.execute("UPDATE instruments SET code=?1,name=?2,instrument_type=?3,is_active=?4,notes=?5,deleted_at=NULL WHERE id=?6", postgres_compat::params![row.code,row.name,row.instrument_type,row.is_active,row.notes,id])?;
                trash_repo::mark_restored_on_conn(tx, "instruments", id, operator)?;
                counts.updated += 1;
            }
            (Some(_), ImportMode::Skip) => counts.skipped += 1,
            (None, _) => {
                tx.execute("INSERT INTO instruments(code,name,instrument_type,is_active,notes) VALUES(?1,?2,?3,?4,?5)", postgres_compat::params![row.code,row.name,row.instrument_type,row.is_active,row.notes])?;
                counts.created += 1;
            }
        }
    }
    for row in &data.methods {
        let instrument_id = if row.instrument_code.trim().is_empty() {
            None
        } else {
            Some(
                id_by_name(tx, "instruments", &row.instrument_code)?.ok_or_else(|| {
                    AppError::Validation(format!("仪器不存在: {}", row.instrument_code))
                })?,
            )
        };
        let existing = method_id_by_import_identity(tx, &row.name, &row.instrument_code)?;
        let was_common = existing
            .map(|id| {
                tx.query_row("SELECT is_common FROM methods WHERE id=?1", [id], |r| {
                    r.get::<_, bool>(0)
                })
            })
            .transpose()?
            .unwrap_or(false);
        let method_id = match (existing, mode) {
            (Some(id), ImportMode::Upsert) => {
                tx.execute("UPDATE methods SET name=?1,full_name=?2,instrument_id=?3,coefficient=?4,multiplier=?5,amount=?6,is_active=?7,notes=?8,show_in_work=?9,show_in_rd=?10,show_in_sample_info=?11,is_common=?12 WHERE id=?13", postgres_compat::params![row.name,row.full_name,instrument_id,row.coefficient,row.multiplier,row.amount,row.is_active,row.notes,row.show_in_work,row.show_in_rd,row.show_in_sample_info,row.is_common,id])?;
                trash_repo::mark_restored_on_conn(tx, "methods", id, operator)?;
                counts.updated += 1;
                id
            }
            (Some(id), ImportMode::Skip) => {
                counts.skipped += 1;
                id
            }
            (None, _) => {
                tx.execute("INSERT INTO methods (method_code,name,full_name,instrument_id,coefficient,multiplier,amount,is_active,notes,show_in_work,show_in_rd,show_in_sample_info,is_common) VALUES ('',?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)", postgres_compat::params![row.name,row.full_name,instrument_id,row.coefficient,row.multiplier,row.amount,row.is_active,row.notes,row.show_in_work,row.show_in_rd,row.show_in_sample_info,row.is_common])?;
                counts.created += 1;
                let id = tx.last_insert_rowid();
                tx.execute(
                    "UPDATE methods SET method_code=?1 WHERE id=?2",
                    postgres_compat::params![format!("M-{id:08}"), id],
                )?;
                id
            }
        };
        if existing.is_none() || mode == ImportMode::Upsert {
            if was_common && !row.is_common {
                tx.execute(
                    "DELETE FROM project_method_links WHERE method_id=?1",
                    [method_id],
                )?;
                tx.execute(
                    "DELETE FROM common_method_division_scopes WHERE method_id=?1",
                    [method_id],
                )?;
            } else if row.is_common {
                tx.execute("INSERT OR IGNORE INTO project_method_links(project_id,method_id) SELECT id,?1 FROM projects WHERE deleted_at IS NULL", [method_id])?;
                tx.execute(
                    "DELETE FROM common_method_division_scopes WHERE method_id=?1",
                    [method_id],
                )?;
                for division_name in &row.common_divisions {
                    let division_id =
                        id_by_name(tx, "divisions", division_name)?.ok_or_else(|| {
                            AppError::Validation(format!("分析检测显示部门不存在: {division_name}"))
                        })?;
                    tx.execute(
                        "INSERT OR IGNORE INTO common_method_division_scopes(method_id,division_id) VALUES(?1,?2)",
                        postgres_compat::params![method_id, division_id],
                    )?;
                }
            }
        }
        if existing.is_none() || mode == ImportMode::Upsert {
            tx.execute(
                "DELETE FROM method_type_links WHERE method_id=?1",
                [method_id],
            )?;
            for type_name in &row.method_types {
                let type_id = id_by_name(tx, "method_types", type_name)?
                    .ok_or_else(|| AppError::Validation(format!("检测类型不存在: {type_name}")))?;
                tx.execute("INSERT OR IGNORE INTO method_type_links (method_id,method_type_id) VALUES (?1,?2)", postgres_compat::params![method_id, type_id])?;
            }
        }
    }

    let relation_map: HashMap<&str, &RelationRow> = data
        .relations
        .iter()
        .map(|x| (x.project.as_str(), x))
        .collect();
    for row in &data.projects {
        let existing = id_by_name(tx, "projects", &row.name)?;
        let compatibility_group_id = if let Some(relation) = relation_map.get(row.name.as_str()) {
            relation
                .labs
                .first()
                .map(|lab_name| {
                    id_by_name(tx, "project_groups", lab_name)?
                        .ok_or_else(|| AppError::Validation(format!("实验室不存在: {lab_name}")))
                })
                .transpose()?
        } else {
            None
        };
        if mode == ImportMode::Upsert && compatibility_group_id.is_none() {
            return Err(AppError::Validation(format!(
                "覆盖更新中项目必须在项目关联表中至少关联一个实验室: {}",
                row.name
            )));
        }
        match (existing, mode) {
            (Some(id), ImportMode::Upsert) => {
                tx.execute(
                    "UPDATE projects SET full_name=?1,high_item=?2,sort_order=?3,is_active=?4,notes=?5,group_id=?6,project_status=?7,show_in_work=?8,show_in_rd=?9,show_in_sample_info=?10,archived_at=CASE WHEN ?7='archived' THEN COALESCE(archived_at,datetime('now','localtime')) ELSE NULL END,archived_by=CASE WHEN ?7='archived' THEN 'import' ELSE NULL END WHERE id=?11",
                    postgres_compat::params![row.full_name,row.high_item,row.sort_order,row.is_active,row.notes,compatibility_group_id,row.project_status,row.show_in_work,row.show_in_rd,row.show_in_sample_info,id],
                )?;
                trash_repo::mark_restored_on_conn(tx, "projects", id, operator)?;
                counts.updated += 1;
            }
            (Some(_), ImportMode::Skip) => counts.skipped += 1,
            (None, _) => {
                let group_id = compatibility_group_id.ok_or_else(|| {
                    AppError::Validation(format!("新增项目缺少实验室关联: {}", row.name))
                })?;
                tx.execute(
                    "INSERT INTO projects (group_id,name,full_name,high_item,sort_order,is_active,notes,method_type,project_status,show_in_work,show_in_rd,show_in_sample_info,archived_at,archived_by) VALUES (?1,?2,?3,?4,?5,?6,?7,'研发项目',?8,?9,?10,?11,CASE WHEN ?8='archived' THEN datetime('now','localtime') ELSE NULL END,CASE WHEN ?8='archived' THEN 'import' ELSE NULL END)",
                    postgres_compat::params![group_id,row.name,row.full_name,row.high_item,row.sort_order,row.is_active,row.notes,row.project_status,row.show_in_work,row.show_in_rd,row.show_in_sample_info],
                )?;
                counts.created += 1;
            }
        }
    }

    // In snapshot mode the workbook is the authoritative association set. Clear
    // every imported project's existing links first, including projects whose
    // relation rows are intentionally empty in Excel.
    if mode == ImportMode::Upsert {
        for project in &data.projects {
            let project_id = id_by_name(tx, "projects", &project.name)?
                .ok_or_else(|| AppError::Validation(format!("项目不存在: {}", project.name)))?;
            tx.execute(
                "DELETE FROM project_lab_links WHERE project_id=?1",
                [project_id],
            )?;
            tx.execute(
                "DELETE FROM project_method_links WHERE project_id=?1",
                [project_id],
            )?;
        }
    }

    for row in &data.relations {
        let project_id = id_by_name(tx, "projects", &row.project)?
            .ok_or_else(|| AppError::Validation(format!("项目不存在: {}", row.project)))?;
        if mode == ImportMode::Skip && existing_project_names.contains(&row.project) {
            counts.skipped += 1;
            continue;
        }
        if !row.labs.is_empty() {
            tx.execute(
                "DELETE FROM project_lab_links WHERE project_id=?1",
                [project_id],
            )?;
            for lab in &row.labs {
                let group_id = id_by_name(tx, "project_groups", lab)?
                    .ok_or_else(|| AppError::Validation(format!("实验室不存在: {lab}")))?;
                tx.execute(
                    "INSERT OR IGNORE INTO project_lab_links (project_id,group_id) VALUES (?1,?2)",
                    postgres_compat::params![project_id, group_id],
                )?;
            }
        }
        if !row.methods.is_empty() {
            tx.execute(
                "DELETE FROM project_method_links WHERE project_id=?1",
                [project_id],
            )?;
            for method in &row.methods {
                let (method_name, instrument_code) =
                    method_key_parts(method).unwrap_or((method, ""));
                let method_id = method_id_by_import_identity(tx, method_name, instrument_code)?
                    .ok_or_else(|| {
                        AppError::Validation(format!("检测方法不存在: {}", method_label(method)))
                    })?;
                tx.execute(
                    "INSERT OR IGNORE INTO project_method_links (project_id,method_id) VALUES (?1,?2)",
                    postgres_compat::params![project_id, method_id],
                )?;
            }
        }
        if let Some(compatibility_lab) = row.labs.first() {
            let compatibility_id = id_by_name(tx, "project_groups", compatibility_lab)?
                .ok_or_else(|| {
                    AppError::Validation(format!("实验室不存在: {compatibility_lab}"))
                })?;
            tx.execute(
                "UPDATE projects SET group_id=?1 WHERE id=?2",
                postgres_compat::params![compatibility_id, project_id],
            )?;
        }
        counts.relation_sets += 1;
    }
    tx.execute(
        "INSERT OR IGNORE INTO project_method_links(project_id,method_id) SELECT p.id,m.id FROM projects p CROSS JOIN methods m WHERE p.deleted_at IS NULL AND m.deleted_at IS NULL AND m.is_common=1",
        [],
    )?;
    if mode == ImportMode::Upsert {
        let plan = build_snapshot_delete_plan(tx, data)?;
        counts.deleted += delete_snapshot_rows(tx, &plan)?;
    }
    Ok(counts)
}

fn snapshot_candidates(
    conn: &Connection,
    sql: &str,
    imported: &HashSet<String>,
) -> Result<Vec<(i64, String)>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;
    Ok(rows
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|(_, key)| !imported.contains(key))
        .collect())
}

fn build_snapshot_delete_plan(conn: &Connection, data: &ParsedData) -> Result<SnapshotDeletePlan> {
    let department_names: HashSet<_> = data
        .departments
        .iter()
        .map(|row| row.name.clone())
        .collect();
    let lab_names: HashSet<_> = data.labs.iter().map(|row| row.name.clone()).collect();
    let type_names: HashSet<_> = data
        .method_types
        .iter()
        .map(|row| row.name.clone())
        .collect();
    let instrument_codes: HashSet<_> = data
        .instruments
        .iter()
        .map(|row| row.code.clone())
        .collect();
    let project_names: HashSet<_> = data.projects.iter().map(|row| row.name.clone()).collect();
    let method_keys: HashSet<_> = data
        .methods
        .iter()
        .map(|row| method_key(&row.name, &row.instrument_code))
        .collect();

    let mut method_stmt = conn.prepare(
        "SELECT m.id,m.name,COALESCE(i.code,'') FROM methods m LEFT JOIN instruments i ON i.id=m.instrument_id WHERE m.deleted_at IS NULL",
    )?;
    let methods = method_stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                method_key(&row.get::<_, String>(1)?, &row.get::<_, String>(2)?),
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|(_, key)| !method_keys.contains(key))
        .collect();

    Ok(SnapshotDeletePlan {
        departments: snapshot_candidates(
            conn,
            "SELECT id,name FROM divisions WHERE deleted_at IS NULL",
            &department_names,
        )?,
        labs: snapshot_candidates(
            conn,
            "SELECT id,name FROM project_groups WHERE deleted_at IS NULL",
            &lab_names,
        )?,
        method_types: snapshot_candidates(
            conn,
            "SELECT id,name FROM method_types WHERE deleted_at IS NULL",
            &type_names,
        )?,
        instruments: snapshot_candidates(
            conn,
            "SELECT id,code FROM instruments WHERE deleted_at IS NULL",
            &instrument_codes,
        )?,
        methods,
        projects: snapshot_candidates(
            conn,
            "SELECT id,name FROM projects WHERE deleted_at IS NULL",
            &project_names,
        )?,
    })
}

fn delete_snapshot_rows(tx: &Transaction<'_>, plan: &SnapshotDeletePlan) -> Result<usize> {
    fn delete_rows(tx: &Transaction<'_>, table: &str, rows: &[(i64, String)]) -> Result<usize> {
        let sql = match table {
            "projects" => "DELETE FROM projects WHERE id=?1",
            "methods" => "DELETE FROM methods WHERE id=?1",
            "instruments" => "DELETE FROM instruments WHERE id=?1",
            "method_types" => "DELETE FROM method_types WHERE id=?1",
            "project_groups" => "DELETE FROM project_groups WHERE id=?1",
            "divisions" => "DELETE FROM divisions WHERE id=?1",
            _ => return Err(AppError::Internal("不支持的主数据删除对象".into())),
        };
        let mut deleted = 0;
        for (id, name) in rows {
            tx.execute(sql, [id]).map_err(|error| {
                AppError::Validation(format!(
                    "覆盖更新无法删除未出现在 Excel 的{}「{}」，它仍被历史记录、用户或其他主数据引用；请先处理引用数据后重试：{}",
                    match table {
                        "projects" => "项目",
                        "methods" => "检测方法",
                        "instruments" => "仪器",
                        "method_types" => "检测类型",
                        "project_groups" => "实验室",
                        "divisions" => "部门",
                        _ => "数据",
                    },
                    name,
                    error
                ))
            })?;
            deleted += 1;
        }
        Ok(deleted)
    }

    // Dependency order matters. Link tables cascade from projects/methods, then
    // methods release instruments, projects release laboratories, and labs
    // release departments. A remaining business reference aborts the transaction.
    let mut deleted = 0;
    deleted += delete_rows(tx, "projects", &plan.projects)?;
    deleted += delete_rows(tx, "methods", &plan.methods)?;
    deleted += delete_rows(tx, "instruments", &plan.instruments)?;
    deleted += delete_rows(tx, "method_types", &plan.method_types)?;
    deleted += delete_rows(tx, "project_groups", &plan.labs)?;
    deleted += delete_rows(tx, "divisions", &plan.departments)?;
    Ok(deleted)
}

fn append_snapshot_delete_preview(
    plan: &SnapshotDeletePlan,
    counts: &mut ImportCounts,
    issues: &mut Vec<ImportIssue>,
) {
    for (entity_type, rows) in [
        ("项目", &plan.projects),
        ("检测方法", &plan.methods),
        ("仪器", &plan.instruments),
        ("检测类型", &plan.method_types),
        ("实验室", &plan.labs),
        ("部门", &plan.departments),
    ] {
        for (_, name) in rows {
            counts.deletes += 1;
            issues.push(ImportIssue {
                sheet: "系统现有数据".into(),
                row: 0,
                entity_type: entity_type.into(),
                name: name.clone(),
                action: "删除".into(),
                level: "warning".into(),
                message: "覆盖更新模式下未出现在 Excel 中，将从系统中永久删除".into(),
            });
        }
    }
}

fn id_by_name(conn: &Connection, table: &str, name: &str) -> Result<Option<i64>> {
    let sql = match table {
        "divisions" => "SELECT id FROM divisions WHERE name=?1 ORDER BY id LIMIT 1",
        "project_groups" => "SELECT id FROM project_groups WHERE name=?1 ORDER BY id LIMIT 1",
        "method_types" => "SELECT id FROM method_types WHERE name=?1 ORDER BY id LIMIT 1",
        "instruments" => "SELECT id FROM instruments WHERE code=?1 ORDER BY id LIMIT 1",
        "methods" => "SELECT id FROM methods WHERE method_code=?1 ORDER BY id LIMIT 1",
        "projects" => "SELECT id FROM projects WHERE name=?1 ORDER BY id LIMIT 1",
        _ => return Err(AppError::Internal("不支持的主数据表".into())),
    };
    Ok(conn.query_row(sql, [name], |row| row.get(0)).optional()?)
}

fn instrument_id_by_identity(conn: &Connection, code: &str, name: &str) -> Result<Option<i64>> {
    let by_code = id_by_name(conn, "instruments", code)?;
    let by_name = if name.trim().is_empty() {
        None
    } else {
        conn.query_row(
            "SELECT id FROM instruments WHERE name=?1 ORDER BY id LIMIT 1",
            [name],
            |row| row.get(0),
        )
        .optional()?
    };
    match (by_code, by_name) {
        (Some(code_id), Some(name_id)) if code_id != name_id => Err(AppError::Validation(format!(
            "仪器编号“{code}”与仪器名称“{name}”分别匹配到不同的已有仪器，不能自动覆盖"
        ))),
        (Some(id), _) | (_, Some(id)) => Ok(Some(id)),
        (None, None) => Ok(None),
    }
}

fn method_id_by_key(conn: &Connection, key: &str) -> Result<Option<i64>> {
    let (name, instrument_code) = method_key_parts(key)
        .ok_or_else(|| AppError::Validation(format!("方法引用格式无效: {key}")))?;
    Ok(conn.query_row(
        "SELECT m.id FROM methods m JOIN instruments i ON i.id=m.instrument_id WHERE m.name=?1 AND i.code=?2 ORDER BY m.id LIMIT 1",
        postgres_compat::params![name, instrument_code],
        |row| row.get(0),
    ).optional()?)
}

fn method_id_by_import_identity(
    conn: &Connection,
    name: &str,
    instrument_code: &str,
) -> Result<Option<i64>> {
    if !instrument_code.trim().is_empty() {
        if let Some(id) = method_id_by_key(conn, &method_key(name, instrument_code))? {
            return Ok(Some(id));
        }
        // 兼容历史上“方法已存在但未绑定仪器”的记录。只有唯一未绑定记录才自动匹配，
        // 避免同名方法在多台仪器上的统计归属被误合并。
        let mut stmt = conn.prepare(
            "SELECT id FROM methods WHERE name=?1 AND instrument_id IS NULL AND deleted_at IS NULL ORDER BY id LIMIT 2",
        )?;
        let ids = stmt
            .query_map([name], |row| row.get::<_, i64>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        return match ids.as_slice() {
            [] => Ok(None),
            [id] => Ok(Some(*id)),
            _ => Err(AppError::Validation(format!(
                "方法“{name}”存在多个未绑定仪器的历史记录，无法自动匹配"
            ))),
        };
    }

    let mut stmt = conn.prepare(
        "SELECT id FROM methods WHERE name=?1 AND deleted_at IS NULL ORDER BY id LIMIT 2",
    )?;
    let ids = stmt
        .query_map([name], |row| row.get::<_, i64>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    match ids.as_slice() {
        [] => Ok(None),
        [id] => Ok(Some(*id)),
        _ => Err(AppError::Validation(format!(
            "方法“{name}”对应多台仪器，请填写“关联仪器编号”"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Read};

    fn workbook_has_data_validations(bytes: &[u8]) -> bool {
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).expect("xlsx zip");
        let mut found = false;
        for index in 1..=12 {
            let Ok(mut entry) = archive.by_name(&format!("xl/worksheets/sheet{index}.xml")) else {
                continue;
            };
            let mut xml = String::new();
            entry.read_to_string(&mut xml).expect("read worksheet xml");
            if xml.contains("dataValidations") {
                found = true;
                break;
            }
        }
        found
    }

    #[test]
    fn generated_template_has_all_required_sheets() {
        let conn = Connection::open_test_database().expect("PostgreSQL test database");
        crate::db::test_migrations::run(&conn).expect("PostgreSQL migrations");
        let bytes = build_template().expect("template");
        assert!(
            workbook_has_data_validations(&bytes),
            "template must keep Excel dropdown validations"
        );
        let path = std::env::temp_dir().join(format!(
            "master_template_test_{}.xlsx",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&path, bytes).expect("write template");
        let mut workbook = open_workbook_auto(&path).expect("open template");
        let names: HashSet<String> = workbook.sheet_names().iter().cloned().collect();
        for sheet in [
            "使用说明",
            "字段字典",
            "部门",
            "实验室",
            "检测类型",
            "仪器",
            "检测方法",
            "通用方法",
            "研发项目",
            "项目关联",
            "预检结果",
            "填写示例（不导入）",
        ] {
            assert!(names.contains(sheet), "missing sheet {sheet}");
        }
        for sheet in [
            "部门",
            "实验室",
            "检测类型",
            "仪器",
            "检测方法",
            "通用方法",
            "研发项目",
            "项目关联",
        ] {
            let headers = workbook
                .worksheet_range(sheet)
                .expect("data sheet")
                .rows()
                .next()
                .expect("data sheet headers")
                .iter()
                .map(cell_to_string)
                .collect::<Vec<_>>();
            assert_eq!(
                headers.last().map(String::as_str),
                Some("序号"),
                "missing sequence column in {sheet}"
            );
        }
        let title = workbook
            .worksheet_range("使用说明")
            .expect("instructions sheet")
            .rows()
            .next()
            .and_then(|row| row.first())
            .map(cell_to_string)
            .unwrap_or_default();
        assert_eq!(
            title,
            format!("主数据导入/导出工作簿 v{}", env!("CARGO_PKG_VERSION"))
        );
        let relation_sheet = workbook
            .worksheet_range("项目关联")
            .expect("relation sheet");
        let headers = relation_sheet
            .rows()
            .next()
            .expect("relation headers")
            .iter()
            .map(cell_to_string)
            .collect::<Vec<_>>();
        assert_eq!(
            headers,
            vec![
                "项目简称*",
                "关联实验室",
                "关联方法名称",
                "关联仪器编号",
                "序号"
            ]
        );
        drop(workbook);
        let (empty_data, empty_issues) =
            parse_workbook_path(&path).expect("parse generated template");
        assert!(empty_data.relations.is_empty());
        assert!(empty_issues.iter().all(|issue| issue.sheet != "项目关联"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn separate_sheet_template_merges_multirow_relations() {
        let mut workbook = Workbook::new();
        add_data_sheet(
            &mut workbook,
            "部门",
            &["部门名称*", "排序", "颜色", "启用"],
            &[20.0; 4],
            &[3],
        )
        .expect("departments");
        add_data_sheet(
            &mut workbook,
            "实验室",
            &[
                "实验室名称*",
                "所属部门*",
                "排序",
                "工作量显示",
                "研发送样显示",
            ],
            &[20.0; 5],
            &[3, 4],
        )
        .expect("labs");
        add_data_sheet(
            &mut workbook,
            "检测类型",
            &["类型名称*", "排序"],
            &[20.0; 2],
            &[],
        )
        .expect("types");
        add_data_sheet(
            &mut workbook,
            "仪器",
            &["仪器编号*", "仪器名称", "仪器类型*", "启用", "备注"],
            &[20.0; 5],
            &[3],
        )
        .expect("instruments");
        add_data_sheet(
            &mut workbook,
            "检测方法",
            &[
                "方法名称*",
                "方法全称",
                "对应仪器编号*",
                "检测类型*",
                "系数",
                "倍率",
                "金额",
                "启用",
                "备注",
            ],
            &[20.0; 11],
            &[9],
        )
        .expect("methods");
        add_data_sheet(
            &mut workbook,
            "研发项目",
            &[
                "项目简称*",
                "项目全称",
                "高项",
                "项目状态",
                "排序",
                "启用",
                "备注",
            ],
            &[20.0; 7],
            &[5],
        )
        .expect("projects");
        add_data_sheet(
            &mut workbook,
            "项目关联",
            &["项目简称*", "关联实验室", "关联方法名称", "关联仪器编号"],
            &[20.0; 4],
            &[],
        )
        .expect("relations");
        add_data_sheet(&mut workbook, "预检结果", &["提示"], &[30.0], &[]).expect("preview");
        add_data_sheet(
            &mut workbook,
            "填写示例（不导入）",
            &["方法编号*", "方法名称*"],
            &[20.0; 2],
            &[],
        )
        .expect("examples");

        workbook
            .worksheet_from_name("部门")
            .unwrap()
            .write(1, 0, "测试部门")
            .unwrap();
        workbook
            .worksheet_from_name("实验室")
            .unwrap()
            .write(1, 0, "实验室01")
            .unwrap()
            .write(1, 1, "测试部门")
            .unwrap();
        workbook
            .worksheet_from_name("检测类型")
            .unwrap()
            .write(1, 0, "液相")
            .unwrap();
        workbook
            .worksheet_from_name("仪器")
            .unwrap()
            .write(1, 0, "LC-01")
            .unwrap()
            .write(1, 2, "液相")
            .unwrap();
        workbook
            .worksheet_from_name("检测方法")
            .unwrap()
            .write(1, 0, "方法01")
            .unwrap()
            .write(1, 2, "LC-01")
            .unwrap()
            .write(1, 3, "液相")
            .unwrap();
        workbook
            .worksheet_from_name("研发项目")
            .unwrap()
            .write(1, 0, "项目01")
            .unwrap()
            .write(1, 3, "进行中")
            .unwrap();
        workbook
            .worksheet_from_name("项目关联")
            .unwrap()
            .write(1, 0, "项目01")
            .unwrap()
            .write(1, 1, "实验室01")
            .unwrap();
        workbook
            .worksheet_from_name("项目关联")
            .unwrap()
            .write(2, 0, "项目01")
            .unwrap()
            .write(2, 2, "方法01")
            .unwrap()
            .write(2, 3, "LC-01")
            .unwrap();
        workbook
            .worksheet_from_name("填写示例（不导入）")
            .unwrap()
            .write(1, 0, "M-DEMO-ONLY")
            .unwrap()
            .write(1, 1, "仅演示不导入")
            .unwrap();

        let path = std::env::temp_dir().join(format!(
            "master_numbered_test_{}.xlsx",
            uuid::Uuid::new_v4()
        ));
        workbook.save(&path).expect("save workbook");
        let (data, issues) = parse_workbook_path(&path).expect("parse workbook");
        let _ = std::fs::remove_file(path);
        assert!(
            issues.iter().all(|issue| issue.level != "error"),
            "issues: {issues:?}"
        );
        assert_eq!(data.departments.len(), 1);
        assert_eq!(data.methods[0].method_types, vec!["液相"]);
        assert_eq!(data.methods.len(), 1, "示例工作表不得参与解析");
        assert_eq!(data.relations[0].labs, vec!["实验室01"]);
        assert_eq!(
            data.relations[0].methods,
            vec![method_key("方法01", "LC-01")]
        );
        assert_eq!(data.projects[0].project_status, "ongoing");
    }

    #[test]
    fn split_multi_accepts_supported_separators() {
        assert_eq!(split_multi("A；B,C、D;A"), vec!["A", "B", "C", "D"]);
    }

    #[test]
    fn multirow_relations_merge_labs_and_methods_without_duplicates() {
        let mut relations = Vec::new();
        merge_relation(
            &mut relations,
            2,
            "项目01".into(),
            "实验室01".into(),
            String::new(),
        );
        merge_relation(
            &mut relations,
            3,
            "项目01".into(),
            "实验室02".into(),
            String::new(),
        );
        merge_relation(
            &mut relations,
            4,
            "项目01".into(),
            String::new(),
            "M-001".into(),
        );
        merge_relation(
            &mut relations,
            5,
            "项目01".into(),
            "实验室01".into(),
            "M-001".into(),
        );
        merge_relation(
            &mut relations,
            6,
            "项目01".into(),
            String::new(),
            "M-002".into(),
        );

        assert_eq!(relations.len(), 1);
        assert_eq!(relations[0].labs, vec!["实验室01", "实验室02"]);
        assert_eq!(relations[0].methods, vec!["M-001", "M-002"]);
    }

    #[test]
    fn transaction_import_writes_master_data_and_links() {
        let mut conn = Connection::open_test_database().expect("PostgreSQL test database");
        crate::db::test_migrations::run(&conn).expect("PostgreSQL migrations");
        let data = ParsedData {
            departments: vec![DepartmentRow {
                row: 2,
                name: "测试部门".into(),
                sort_order: 20,
                color: "#1976d2".into(),
                is_active: true,
                show_in_work: true,
                show_in_rd: true,
                show_in_sample_info: true,
            }],
            labs: vec![LabRow {
                row: 2,
                name: "测试实验室".into(),
                department: "测试部门".into(),
                sort_order: 20,
                show_in_work: true,
                show_in_rd: true,
                show_in_sample_info: true,
            }],
            method_types: vec![MethodTypeRow {
                row: 2,
                name: "测试类型".into(),
                sort_order: 20,
            }],
            instruments: vec![InstrumentRow {
                row: 2,
                code: "INS-01".into(),
                name: "测试仪器".into(),
                instrument_type: "测试仪器类型".into(),
                is_active: true,
                notes: "".into(),
            }],
            methods: vec![MethodRow {
                row: 2,
                method_code: method_key("测试方法", "INS-01"),
                name: "测试方法".into(),
                instrument_code: "INS-01".into(),
                full_name: "测试方法全称".into(),
                method_types: vec!["测试类型".into()],
                coefficient: 1.5,
                multiplier: 2.0,
                amount: 3.0,
                is_active: true,
                notes: "".into(),
                show_in_work: true,
                show_in_rd: true,
                show_in_sample_info: true,
                is_common: false,
                common_divisions: vec![],
            }],
            projects: vec![ProjectRow {
                row: 2,
                name: "测试项目".into(),
                full_name: "测试项目全称".into(),
                high_item: Some("高项A".into()),
                sort_order: 20,
                is_active: true,
                project_status: "archived".into(),
                notes: "".into(),
                show_in_work: true,
                show_in_rd: true,
                show_in_sample_info: true,
            }],
            relations: vec![RelationRow {
                row: 2,
                project: "测试项目".into(),
                labs: vec!["测试实验室".into()],
                methods: vec![method_key("测试方法", "INS-01")],
            }],
        };
        let tx = conn.transaction().expect("transaction");
        let counts = apply_import(&tx, &data, ImportMode::Upsert).expect("apply");
        assert_eq!(counts.created, 6);
        assert_eq!(counts.relation_sets, 1);
        tx.commit().expect("commit");

        let high_item: String = conn
            .query_row(
                "SELECT high_item FROM projects WHERE name='测试项目'",
                [],
                |row| row.get(0),
            )
            .expect("high item");
        assert_eq!(high_item, "高项A");
        let project_status: String = conn
            .query_row(
                "SELECT project_status FROM projects WHERE name='测试项目'",
                [],
                |row| row.get(0),
            )
            .expect("project status");
        assert_eq!(project_status, "archived");
        let lab_links: i64 = conn.query_row("SELECT COUNT(*) FROM project_lab_links pll JOIN projects p ON p.id=pll.project_id WHERE p.name='测试项目'", [], |row| row.get(0)).expect("lab links");
        let method_links: i64 = conn.query_row("SELECT COUNT(*) FROM project_method_links pml JOIN projects p ON p.id=pml.project_id WHERE p.name='测试项目'", [], |row| row.get(0)).expect("method links");
        let type_links: i64 = conn.query_row("SELECT COUNT(*) FROM method_type_links mtl JOIN methods m ON m.id=mtl.method_id WHERE m.name='测试方法'", [], |row| row.get(0)).expect("type links");
        assert_eq!((lab_links, method_links, type_links), (1, 1, 1));
    }

    #[test]
    fn common_method_import_persists_selected_analysis_departments() {
        let mut conn = Connection::open_test_database().expect("PostgreSQL test database");
        crate::db::test_migrations::run(&conn).expect("PostgreSQL migrations");
        let data = ParsedData {
            departments: vec![
                DepartmentRow {
                    row: 2,
                    name: "通用导入部门A".into(),
                    sort_order: 1,
                    color: "#1976d2".into(),
                    is_active: true,
                    show_in_work: true,
                    show_in_rd: true,
                    show_in_sample_info: true,
                },
                DepartmentRow {
                    row: 3,
                    name: "通用导入部门B".into(),
                    sort_order: 2,
                    color: "#1976d2".into(),
                    is_active: true,
                    show_in_work: true,
                    show_in_rd: true,
                    show_in_sample_info: true,
                },
            ],
            instruments: vec![InstrumentRow {
                row: 2,
                code: "COMMON-IMPORT-INS".into(),
                name: "通用导入仪器".into(),
                instrument_type: "液相".into(),
                is_active: true,
                notes: String::new(),
            }],
            methods: vec![MethodRow {
                row: 2,
                method_code: method_key("通用导入方法", "COMMON-IMPORT-INS"),
                name: "通用导入方法".into(),
                instrument_code: "COMMON-IMPORT-INS".into(),
                full_name: String::new(),
                method_types: vec![],
                coefficient: 1.0,
                multiplier: 1.0,
                amount: 0.0,
                is_active: true,
                notes: String::new(),
                show_in_work: true,
                show_in_rd: true,
                show_in_sample_info: true,
                is_common: true,
                common_divisions: vec!["通用导入部门A".into()],
            }],
            ..ParsedData::default()
        };
        let tx = conn.transaction().expect("transaction");
        apply_import(&tx, &data, ImportMode::Upsert).expect("apply import");
        tx.commit().expect("commit");
        let scopes: Vec<String> = conn
            .prepare("SELECT d.name FROM common_method_division_scopes cmds JOIN divisions d ON d.id=cmds.division_id JOIN methods m ON m.id=cmds.method_id WHERE m.name='通用导入方法' ORDER BY d.name")
            .expect("query")
            .query_map([], |row| row.get::<_, String>(0))
            .expect("rows")
            .collect::<std::result::Result<Vec<_>, _>>()
            .expect("scope names");
        assert_eq!(scopes, vec!["通用导入部门A"]);
    }

    #[test]
    fn import_adds_method_when_legacy_unnumbered_method_exists() {
        let mut conn = Connection::open_test_database().expect("PostgreSQL test database");
        crate::db::test_migrations::run(&conn).expect("PostgreSQL migrations");
        conn.execute(
            "INSERT INTO methods(method_code,name,full_name,coefficient,multiplier,amount,is_active,notes)
             VALUES('','历史空编号方法','',1,1,0,1,'')",
            [],
        )
        .expect("legacy method");
        conn.execute(
            "INSERT INTO instruments(code,name,instrument_type,is_active,notes)
             VALUES('LC-BETA7','液相测试仪器','液相',1,'')",
            [],
        )
        .expect("instrument");
        conn.execute(
            "INSERT INTO method_types(name,sort_order) VALUES('液相测试类型',1)",
            [],
        )
        .expect("method type");

        let data = ParsedData {
            instruments: vec![InstrumentRow {
                row: 2,
                code: "LC-BETA7".into(),
                name: "液相测试仪器".into(),
                instrument_type: "液相".into(),
                is_active: true,
                notes: String::new(),
            }],
            methods: vec![MethodRow {
                row: 2,
                method_code: method_key("新导入方法", "LC-BETA7"),
                name: "新导入方法".into(),
                instrument_code: "LC-BETA7".into(),
                full_name: "新导入方法全称".into(),
                method_types: vec!["液相测试类型".into()],
                coefficient: 1.0,
                multiplier: 1.0,
                amount: 0.0,
                is_active: true,
                notes: String::new(),
                show_in_work: true,
                show_in_rd: true,
                show_in_sample_info: true,
                is_common: false,
                common_divisions: vec![],
            }],
            ..ParsedData::default()
        };
        let tx = conn.transaction().expect("transaction");
        let counts = apply_import(&tx, &data, ImportMode::Upsert).expect("apply import");
        tx.commit().expect("commit");
        assert_eq!(counts.created, 1);

        let method_code: String = conn
            .query_row(
                "SELECT method_code FROM methods WHERE name='新导入方法'",
                [],
                |row| row.get(0),
            )
            .expect("generated method code");
        assert!(method_code.starts_with("M-"));
    }

    #[test]
    fn instrument_name_match_updates_code_without_unique_error() {
        let mut conn = Connection::open_test_database().expect("PostgreSQL test database");
        crate::db::test_migrations::run(&conn).expect("PostgreSQL migrations");
        conn.execute(
            "INSERT INTO instruments(code,name,instrument_type,is_active,notes) VALUES(?1,?2,?3,1,'')",
            postgres_compat::params!["LC-OLD", "Liquid Chromatograph", "LC"],
        )
        .expect("existing instrument");

        let data = ParsedData {
            instruments: vec![InstrumentRow {
                row: 2,
                code: "LC-01".into(),
                name: "Liquid Chromatograph".into(),
                instrument_type: "LC Updated".into(),
                is_active: true,
                notes: "updated by import".into(),
            }],
            ..ParsedData::default()
        };
        let tx = conn.transaction().expect("transaction");
        let counts = apply_import(&tx, &data, ImportMode::Upsert).expect("apply import");
        assert_eq!((counts.created, counts.updated), (0, 1));
        tx.commit().expect("commit");

        let (code, kind): (String, String) = conn
            .query_row(
                "SELECT code,instrument_type FROM instruments WHERE name=?1",
                ["Liquid Chromatograph"],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("updated instrument");
        assert_eq!((code, kind), ("LC-01".into(), "LC Updated".into()));
    }

    #[test]
    fn conflicting_instrument_code_and_name_is_rejected() {
        let conn = Connection::open_test_database().expect("PostgreSQL test database");
        crate::db::test_migrations::run(&conn).expect("PostgreSQL migrations");
        conn.execute(
            "INSERT INTO instruments(code,name,instrument_type,is_active,notes) VALUES('LC-01','Liquid A','LC',1,'')",
            [],
        )
        .expect("first instrument");
        conn.execute(
            "INSERT INTO instruments(code,name,instrument_type,is_active,notes) VALUES('LC-02','Liquid B','LC',1,'')",
            [],
        )
        .expect("second instrument");

        let error = instrument_id_by_identity(&conn, "LC-01", "Liquid B")
            .expect_err("conflicting identity must be rejected");
        assert!(matches!(error, AppError::Validation(_)));
    }

    #[test]
    fn exported_workbook_round_trips_legacy_incomplete_master_data() {
        let conn = Connection::open_test_database().expect("PostgreSQL test database");
        crate::db::test_migrations::run(&conn).expect("PostgreSQL migrations");
        let suffix = &uuid::Uuid::new_v4().to_string()[..8];
        let division = format!("回环部门-{suffix}");
        let lab = format!("回环实验室-{suffix}");
        let method = format!("回环方法-{suffix}");
        let project = format!("回环项目-{suffix}");
        conn.execute(
            "INSERT INTO divisions(name,sort_order,color,is_active) VALUES(?1,0,'#1976d2',1)",
            [&division],
        )
        .expect("division");
        conn.execute(
            "INSERT INTO project_groups(name,sort_order,show_in_work,show_in_rd,division_id) VALUES(?1,0,1,1,NULL)",
            [&lab],
        )
        .expect("legacy lab");
        conn.execute(
            "INSERT INTO methods(method_code,name,full_name,instrument_id,coefficient,multiplier,amount,is_active,notes) VALUES('',?1,'',NULL,1,1,0,1,'')",
            [&method],
        )
        .expect("legacy method");
        let group_id: i64 = conn
            .query_row(
                "SELECT id FROM project_groups WHERE name=?1",
                [&lab],
                |row| row.get(0),
            )
            .expect("group id");
        conn.execute(
            "INSERT INTO projects(group_id,name,full_name,sort_order,is_active,project_status) VALUES(?1,?2,'',0,1,'ongoing')",
            postgres_compat::params![group_id, project],
        )
        .expect("legacy project");

        let bytes = build_export_workbook(&conn).expect("export workbook");
        let path =
            std::env::temp_dir().join(format!("master_roundtrip_{}.xlsx", uuid::Uuid::new_v4()));
        std::fs::write(&path, &bytes).expect("write export");
        let template_path =
            std::env::temp_dir().join(format!("master_template_{}.xlsx", uuid::Uuid::new_v4()));
        let template_bytes = build_template().expect("template workbook");
        assert!(workbook_has_data_validations(&bytes));
        assert!(workbook_has_data_validations(&template_bytes));
        std::fs::write(&template_path, template_bytes).expect("write template");
        let mut export_book = open_workbook_auto(&path).expect("open export workbook");
        let mut template_book = open_workbook_auto(&template_path).expect("open template workbook");
        assert_eq!(
            export_book.sheet_names(),
            template_book.sheet_names(),
            "export and import template must use the same workbook structure"
        );
        for sheet in [
            "部门",
            "实验室",
            "检测类型",
            "仪器",
            "检测方法",
            "通用方法",
            "研发项目",
            "项目关联",
        ] {
            let export_header = export_book
                .worksheet_range(sheet)
                .expect("export sheet")
                .rows()
                .next()
                .unwrap_or_default()
                .iter()
                .map(cell_to_string)
                .collect::<Vec<_>>();
            let template_header = template_book
                .worksheet_range(sheet)
                .expect("template sheet")
                .rows()
                .next()
                .unwrap_or_default()
                .iter()
                .map(cell_to_string)
                .collect::<Vec<_>>();
            assert_eq!(export_header, template_header, "header mismatch in {sheet}");
        }
        let (data, mut issues) = parse_workbook_path(&path).expect("parse exported workbook");
        let relation = data
            .relations
            .iter()
            .find(|item| item.project == project)
            .expect("legacy project relation");
        assert!(
            relation.labs.contains(&lab),
            "legacy projects.group_id must be exported as a lab relation"
        );
        let preview = build_preview(&conn, &data, ImportMode::Upsert, &mut issues)
            .expect("precheck exported workbook");
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(template_path);
        assert!(
            preview.valid,
            "exported workbook must be importable: {:?}",
            preview.issues
        );
        assert_eq!(preview.counts.errors, 0, "export should remain importable");
    }

    #[test]
    fn snapshot_import_deletes_master_data_missing_from_workbook() {
        let mut conn = Connection::open_test_database().expect("PostgreSQL test database");
        crate::db::test_migrations::run(&conn).expect("PostgreSQL migrations");
        conn.execute(
            "INSERT INTO divisions(name,sort_order,color,is_active) VALUES('保留部门',0,'#1976d2',1),('待删除部门',0,'#1976d2',1)",
            [],
        )
        .expect("seed divisions");
        let data = ParsedData {
            departments: vec![DepartmentRow {
                row: 2,
                name: "保留部门".into(),
                sort_order: 8,
                color: "#2e7d32".into(),
                is_active: true,
                show_in_work: true,
                show_in_rd: true,
                show_in_sample_info: true,
            }],
            ..ParsedData::default()
        };
        let tx = conn.transaction().expect("transaction");
        let counts = apply_import(&tx, &data, ImportMode::Upsert).expect("snapshot import");
        tx.commit().expect("commit");
        assert!(
            counts.deleted >= 1,
            "the missing department must be deleted"
        );
        let divisions: Vec<String> = conn
            .prepare("SELECT name FROM divisions ORDER BY name")
            .expect("query")
            .query_map([], |row| row.get(0))
            .expect("rows")
            .collect::<std::result::Result<_, _>>()
            .expect("names");
        assert_eq!(divisions, vec!["保留部门"]);
        let color: String = conn
            .query_row(
                "SELECT color FROM divisions WHERE name='保留部门'",
                [],
                |row| row.get(0),
            )
            .expect("updated row");
        assert_eq!(color, "#2e7d32");
    }

    #[test]
    fn skip_import_preserves_existing_master_data() {
        let mut conn = Connection::open_test_database().expect("PostgreSQL test database");
        crate::db::test_migrations::run(&conn).expect("PostgreSQL migrations");
        conn.execute(
            "INSERT INTO divisions(name,sort_order,color,is_active) VALUES('保留部门',1,'#1976d2',1),('旧部门',0,'#1976d2',1)",
            [],
        )
        .expect("seed divisions");
        let data = ParsedData {
            departments: vec![DepartmentRow {
                row: 2,
                name: "保留部门".into(),
                sort_order: 99,
                color: "#2e7d32".into(),
                is_active: false,
                show_in_work: false,
                show_in_rd: false,
                show_in_sample_info: false,
            }],
            ..ParsedData::default()
        };
        let tx = conn.transaction().expect("transaction");
        let counts = apply_import(&tx, &data, ImportMode::Skip).expect("skip import");
        tx.commit().expect("commit");
        assert_eq!(
            (
                counts.created,
                counts.updated,
                counts.deleted,
                counts.skipped
            ),
            (0, 0, 0, 1)
        );
        let rows: Vec<(String, i64, String)> = conn
            .prepare("SELECT name,sort_order,color FROM divisions ORDER BY name")
            .expect("query")
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .expect("rows")
            .collect::<std::result::Result<_, _>>()
            .expect("values");
        assert_eq!(
            rows,
            vec![
                ("保留部门".into(), 1, "#1976d2".into()),
                ("旧部门".into(), 0, "#1976d2".into())
            ]
        );
    }

    #[test]
    fn snapshot_precheck_blocks_deletion_of_referenced_master_data() {
        let conn = Connection::open_test_database().expect("PostgreSQL test database");
        crate::db::test_migrations::run(&conn).expect("PostgreSQL migrations");
        conn.execute(
            "INSERT INTO divisions(name,sort_order,color,is_active) VALUES('保留部门',0,'#1976d2',1),('被引用部门',0,'#1976d2',1)",
            [],
        )
        .expect("seed divisions");
        let deleted_division_id: i64 = conn
            .query_row(
                "SELECT id FROM divisions WHERE name='被引用部门'",
                [],
                |row| row.get(0),
            )
            .expect("division id");
        conn.execute(
            "INSERT INTO users(username,password,division_id) VALUES('snapshot-test-user','x',?1)",
            [deleted_division_id],
        )
        .expect("referenced user");
        let data = ParsedData {
            departments: vec![DepartmentRow {
                row: 2,
                name: "保留部门".into(),
                sort_order: 0,
                color: "#1976d2".into(),
                is_active: true,
                show_in_work: true,
                show_in_rd: true,
                show_in_sample_info: true,
            }],
            ..ParsedData::default()
        };
        let mut issues = Vec::new();
        let preview =
            build_preview(&conn, &data, ImportMode::Upsert, &mut issues).expect("preview");
        assert!(!preview.valid);
        assert!(preview
            .issues
            .iter()
            .any(|issue| issue.level == "error" && issue.name == "被引用部门"));
    }

    #[test]
    fn exported_project_relations_match_legacy_four_column_layout_without_losing_links() {
        let conn = Connection::open_test_database().expect("PostgreSQL test database");
        crate::db::test_migrations::run(&conn).expect("PostgreSQL migrations");
        let suffix = &uuid::Uuid::new_v4().to_string()[..8];
        let lab_one = format!("排序实验室甲-{suffix}");
        let lab_two = format!("排序实验室乙-{suffix}");
        let project_one = format!("排序项目甲-{suffix}");
        let project_two = format!("排序项目乙-{suffix}");
        let method = format!("排序方法-{suffix}");
        let instrument_one = format!("LC-01-{suffix}");
        let instrument_two = format!("LC-02-{suffix}");

        conn.execute(
            "INSERT INTO project_groups(name,sort_order) VALUES(?1,1)",
            [&lab_one],
        )
        .expect("first lab");
        let lab_one_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO project_groups(name,sort_order) VALUES(?1,2)",
            [&lab_two],
        )
        .expect("second lab");
        let lab_two_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO instruments(code,name,instrument_type,is_active,notes) VALUES(?1,?1,'液相',1,'')",
            [&instrument_one],
        )
        .expect("first instrument");
        let instrument_one_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO instruments(code,name,instrument_type,is_active,notes) VALUES(?1,?1,'液相',1,'')",
            [&instrument_two],
        )
        .expect("second instrument");
        let instrument_two_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO methods(method_code,name,full_name,instrument_id,coefficient,multiplier,amount,is_active,notes,is_common) VALUES('',?1,'',?2,1,1,0,1,'',0)",
            postgres_compat::params![method, instrument_one_id],
        )
        .expect("first method instance");
        let method_one_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO methods(method_code,name,full_name,instrument_id,coefficient,multiplier,amount,is_active,notes,is_common) VALUES('',?1,'',?2,1,1,0,1,'',0)",
            postgres_compat::params![method, instrument_two_id],
        )
        .expect("second method instance");
        let method_two_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO projects(group_id,name,full_name,sort_order,is_active,project_status) VALUES(?1,?2,'',1,1,'ongoing')",
            postgres_compat::params![lab_one_id, project_one],
        )
        .expect("first project");
        let project_one_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO project_method_links(project_id,method_id) VALUES(?1,?2)",
            postgres_compat::params![project_one_id, method_one_id],
        )
        .expect("first method link");
        conn.execute(
            "INSERT INTO project_method_links(project_id,method_id) VALUES(?1,?2)",
            postgres_compat::params![project_one_id, method_two_id],
        )
        .expect("second method link");
        conn.execute(
            "INSERT INTO project_lab_links(project_id,group_id) VALUES(?1,?2)",
            postgres_compat::params![project_one_id, lab_two_id],
        )
        .expect("second lab link");
        conn.execute(
            "INSERT INTO projects(group_id,name,full_name,sort_order,is_active,project_status) VALUES(?1,?2,'',2,1,'ongoing')",
            postgres_compat::params![lab_two_id, project_two],
        )
        .expect("second project");

        let path = std::env::temp_dir().join(format!(
            "master_relation_order_{}.xlsx",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(
            &path,
            build_export_workbook(&conn).expect("export workbook"),
        )
        .expect("write export");
        let mut workbook = open_workbook_auto(&path).expect("open export workbook");
        let range = workbook
            .worksheet_range("项目关联")
            .expect("project relation sheet");
        let rows = range
            .rows()
            .skip(1)
            .map(|row| row.iter().map(cell_to_string).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let first_project_rows = rows
            .iter()
            .filter(|row| row.first() == Some(&project_one))
            .collect::<Vec<_>>();
        assert_eq!(
            first_project_rows.len(),
            4,
            "every project-lab-method combination must be exported"
        );
        assert_eq!(
            first_project_rows
                .iter()
                .filter_map(|row| row.get(1))
                .cloned()
                .collect::<HashSet<_>>(),
            HashSet::from([lab_one.clone(), lab_two.clone()])
        );
        assert_eq!(
            first_project_rows
                .iter()
                .map(|row| (row[1].clone(), row[2].clone(), row[3].clone()))
                .collect::<HashSet<_>>(),
            HashSet::from([
                (lab_one.clone(), method.clone(), instrument_one.clone()),
                (lab_one.clone(), method.clone(), instrument_two.clone()),
                (lab_two.clone(), method.clone(), instrument_one.clone()),
                (lab_two.clone(), method.clone(), instrument_two.clone()),
            ])
        );
        assert_eq!(
            first_project_rows
                .iter()
                .map(|row| (row[2].clone(), row[3].clone()))
                .collect::<HashSet<_>>(),
            HashSet::from([
                (method.clone(), instrument_one.clone()),
                (method.clone(), instrument_two.clone()),
            ])
        );
        let first_project_last_row = rows
            .iter()
            .rposition(|row| row.first() == Some(&project_one))
            .expect("first project row");
        let second_project_first_row = rows
            .iter()
            .position(|row| row.first() == Some(&project_two))
            .expect("second project row");

        let (parsed, issues) = parse_workbook_path(&path).expect("parse exported workbook");
        assert!(
            issues.iter().all(|issue| issue.level != "error"),
            "{issues:?}"
        );
        let relation = parsed
            .relations
            .iter()
            .find(|row| row.project == project_one)
            .expect("parsed first project relation");
        assert_eq!(relation.labs, vec![lab_one, lab_two]);
        assert_eq!(
            relation.methods.iter().cloned().collect::<HashSet<_>>(),
            HashSet::from([
                method_key(&method, &instrument_one),
                method_key(&method, &instrument_two),
            ])
        );
        let _ = std::fs::remove_file(path);

        assert!(first_project_last_row < second_project_first_row);
    }
}
