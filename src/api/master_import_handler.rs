use axum::{
    body::Body,
    extract::{Multipart, State},
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

use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::ApiResponse;
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
    pub skipped: usize,
    pub relation_sets: usize,
    pub message: String,
}

#[derive(Debug, Clone)]
struct DepartmentRow {
    row: usize,
    name: String,
    sort_order: i64,
    color: String,
    is_active: bool,
}

#[derive(Debug, Clone)]
struct LabRow {
    row: usize,
    name: String,
    department: String,
    sort_order: i64,
    show_in_work: bool,
    show_in_rd: bool,
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

fn method_key(name: &str, instrument_code: &str) -> String {
    format!("{}\u{1f}{}", name.trim(), instrument_code.trim())
}

fn method_key_parts(key: &str) -> Option<(&str, &str)> {
    key.split_once('\u{1f}')
}

fn method_label(key: &str) -> String {
    method_key_parts(key)
        .map(|(name, instrument)| format!("{name} [{instrument}]"))
        .unwrap_or_else(|| key.to_string())
}

#[derive(Debug, Clone, Default)]
struct ApplyCounts {
    created: usize,
    updated: usize,
    skipped: usize,
    relation_sets: usize,
}

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/master-import/template", get(download_template))
        .route("/api/master-import/precheck", post(precheck))
        .route("/api/master-import/execute", post(execute))
        .with_state(pool)
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
            "attachment; filename=master_data_import_v0.4.103.xlsx; filename*=UTF-8''%E4%B8%BB%E6%95%B0%E6%8D%AE%E4%B8%80%E9%94%AE%E5%AF%BC%E5%85%A5%E6%A8%A1%E6%9D%BF_v0.4.103.xlsx",
        )
        .body(Body::from(bytes))
        .map_err(|e| AppError::Internal(format!("构建模板响应失败: {e}")))?;
    Ok(response)
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
    let applied = apply_import(&tx, &data, mode)?;
    tx.execute(
        "INSERT INTO audit_log (action, table_name, user_id, user_name, detail) VALUES ('import','master_data',?1,?2,?3)",
        postgres_compat::params![
            ctx.user.id,
            ctx.user.username,
            format!(
            "主数据一键导入：新增{}，更新{}，跳过{}，关联{}",
            applied.created, applied.updated, applied.skipped, applied.relation_sets
            )
        ],
    )?;
    tx.commit()?;

    let message = format!(
        "导入完成：新增 {}，更新 {}，跳过 {}，项目关联 {}",
        applied.created, applied.updated, applied.skipped, applied.relation_sets
    );
    Ok(Json(ApiResponse::ok(MasterImportResult {
        success: true,
        created: applied.created,
        updated: applied.updated,
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

fn build_simplified_template(conn: &Connection) -> Result<Vec<u8>> {
    let mut workbook = Workbook::new();
    let title = Format::new()
        .set_bold()
        .set_font_size(18)
        .set_font_color(Color::RGB(0x0B6E69));
    let wrap = Format::new().set_text_wrap().set_align(FormatAlign::Top);
    let section = Format::new()
        .set_bold()
        .set_font_color(Color::White)
        .set_background_color(Color::RGB(0x1976D2))
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center);
    {
        let ws = workbook.add_worksheet();
        ws.set_name("使用说明")?;
        ws.set_column_width(0, 22)?;
        ws.set_column_width(1, 90)?;
        ws.merge_range(
            0,
            0,
            0,
            1,
            "主数据一键导入模板 v0.4.94-trace-preview",
            &title,
        )?;
        let rows = [
            ("需要填写", "只填写“主数据”和“项目关联”两个工作表。其他工作表用于查看规则、程序现有数据和填写示例，不参与导入。"),
            ("导入顺序", "系统按部门 → 实验室 → 检测类型 → 检测方法 → 研发项目 → 项目关联的顺序处理，同一模板中新建的数据可以互相引用。"),
            ("必填规则", "带 * 的字段必填。每类对象只填写与其有关的列，无关列保持空白。详细规则见“填写规则”。"),
            ("项目关联", "每个项目填写一行。关联实验室、关联方法允许填写多个名称，用中文分号、英文分号、顿号、逗号或单元格内换行分隔。"),
            ("项目状态", "填写“进行中”或“已归档”，留空默认为“进行中”。已归档项目保留历史数据，但不会出现在前台录入项目中。"),
            ("启用字段", "启用、工作量显示、研发送样显示统一填写“是”或“否”，留空时采用填写规则中的默认值。"),
            ("引用数据", "“数据字典”来自下载模板时程序内的现有数据。可以引用现有名称，也可以引用本模板“主数据”中新增的名称。"),
            ("导入策略", "覆盖更新会更新同名数据及关联；跳过已有只新增不存在的数据。正式导入前必须先执行预检。"),
            ("安全机制", "正式导入使用单个数据库事务，任何一项写入失败都会整批回滚，不会留下半套数据。"),
        ];
        for (idx, (name, desc)) in rows.iter().enumerate() {
            ws.write_with_format((idx + 2) as u32, 0, *name, &section)?;
            ws.write_with_format((idx + 2) as u32, 1, *desc, &wrap)?;
        }
    }
    add_data_sheet(
        &mut workbook,
        "主数据",
        &[
            "对象类型*",
            "名称*",
            "所属部门",
            "全称",
            "检测类型",
            "高项",
            "项目状态",
            "系数",
            "倍率",
            "金额",
            "排序",
            "启用",
            "工作量显示",
            "研发送样显示",
            "颜色",
            "备注",
        ],
        &[
            14.0, 24.0, 20.0, 34.0, 26.0, 18.0, 14.0, 10.0, 10.0, 12.0, 10.0, 10.0, 14.0, 14.0,
            12.0, 36.0,
        ],
        &[11, 12, 13],
    )?;
    add_data_sheet(
        &mut workbook,
        "项目关联",
        &["项目简称*", "关联实验室", "关联方法编号"],
        &[26.0, 30.0, 30.0],
        &[],
    )?;

    {
        let ws = workbook.add_worksheet();
        ws.set_name("填写规则")?;
        let headers = ["字段", "适用对象", "必填", "填写规则", "默认值", "示例"];
        write_headers(ws, &headers)?;
        let rules = [
            [
                "对象类型*",
                "全部",
                "是",
                "仅填写：部门、实验室、检测类型、检测方法、研发项目",
                "无",
                "检测方法",
            ],
            [
                "名称*",
                "全部",
                "是",
                "对象的唯一简称；同名数据按导入策略处理",
                "无",
                "液相含量测定",
            ],
            [
                "所属部门",
                "实验室",
                "是",
                "填写程序现有部门或本模板中新建部门的名称",
                "无",
                "研究院",
            ],
            [
                "全称",
                "检测方法、研发项目",
                "否",
                "对象完整名称",
                "空",
                "高效液相色谱法含量测定",
            ],
            [
                "检测类型",
                "检测方法",
                "是",
                "填写一个或多个检测类型；多个值用分号、顿号、逗号或换行分隔",
                "无",
                "液相；含量",
            ],
            [
                "高项",
                "研发项目",
                "否",
                "项目关联的高项文本",
                "空",
                "重点项目",
            ],
            [
                "项目状态",
                "研发项目",
                "否",
                "仅允许：进行中、已归档",
                "进行中",
                "进行中",
            ],
            ["系数", "检测方法", "否", "大于等于0的数字", "1", "1.5"],
            ["倍率", "检测方法", "否", "大于等于0的数字", "1", "2"],
            ["金额", "检测方法", "否", "大于等于0的数字", "0", "50"],
            ["排序", "全部", "否", "整数，数值越小显示越靠前", "0", "10"],
            [
                "启用",
                "部门、检测方法、研发项目",
                "否",
                "仅允许：是、否",
                "是",
                "是",
            ],
            [
                "工作量显示",
                "实验室",
                "否",
                "是否显示在分析检测入口，仅允许：是、否",
                "是",
                "是",
            ],
            [
                "研发送样显示",
                "实验室",
                "否",
                "是否显示在研发送样入口，仅允许：是、否",
                "是",
                "是",
            ],
            [
                "颜色",
                "部门",
                "否",
                "十六进制颜色，格式为 #RRGGBB",
                "#1976d2",
                "#2E7D32",
            ],
            [
                "备注",
                "检测方法、研发项目",
                "否",
                "补充说明，可留空",
                "空",
                "常规检测方法",
            ],
            [
                "项目简称*",
                "项目关联",
                "是",
                "必须与主数据或程序现有项目名称完全一致",
                "无",
                "项目A",
            ],
            [
                "关联实验室",
                "项目关联",
                "否",
                "每行选择一个实验室，同一项目可填写多行",
                "空",
                "实验室401",
            ],
            [
                "关联方法编号",
                "项目关联",
                "否",
                "每行选择一个方法编号，同一项目可填写多行",
                "空",
                "M-LC01-001",
            ],
        ];
        for (r, row) in rules.iter().enumerate() {
            for (c, value) in row.iter().enumerate() {
                ws.write_with_format((r + 1) as u32, c as u16, *value, &wrap)?;
            }
        }
        for (col, width) in [20.0, 28.0, 10.0, 72.0, 20.0, 34.0].iter().enumerate() {
            ws.set_column_width(col as u16, *width)?;
        }
        ws.set_freeze_panes(1, 0)?;
    }

    write_example_sheets(&mut workbook)?;
    write_data_dictionary(&mut workbook, conn)?;
    Ok(workbook.save_to_buffer()?)
}

fn write_example_sheets(workbook: &mut Workbook) -> std::result::Result<(), XlsxError> {
    let wrap = Format::new().set_text_wrap().set_align(FormatAlign::Top);
    let ws = workbook.add_worksheet();
    ws.set_name("示例数据")?;
    let headers = [
        "对象类型*",
        "名称*",
        "所属部门",
        "全称",
        "检测类型",
        "高项",
        "项目状态",
        "系数",
        "倍率",
        "金额",
        "排序",
        "启用",
        "工作量显示",
        "研发送样显示",
        "颜色",
        "备注",
    ];
    write_headers(ws, &headers)?;
    let rows = [
        [
            "部门",
            "示例研究院",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "10",
            "是",
            "",
            "",
            "#1976D2",
            "部门示例",
        ],
        [
            "实验室",
            "示例实验室401",
            "示例研究院",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "10",
            "",
            "是",
            "是",
            "",
            "实验室示例",
        ],
        [
            "检测类型",
            "示例液相",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "10",
            "",
            "",
            "",
            "",
            "类型示例",
        ],
        [
            "检测方法",
            "示例液相含量",
            "",
            "示例高效液相色谱含量测定",
            "示例液相",
            "",
            "",
            "1.5",
            "1",
            "50",
            "10",
            "是",
            "",
            "",
            "",
            "方法示例",
        ],
        [
            "研发项目",
            "示例项目A",
            "",
            "示例研发项目A全称",
            "",
            "重点项目",
            "进行中",
            "",
            "",
            "",
            "10",
            "是",
            "",
            "",
            "",
            "项目示例",
        ],
        [
            "研发项目",
            "示例项目B",
            "",
            "示例研发项目B全称",
            "",
            "一般项目",
            "已归档",
            "",
            "",
            "",
            "20",
            "是",
            "",
            "",
            "",
            "归档项目示例",
        ],
    ];
    for (r, row) in rows.iter().enumerate() {
        for (c, value) in row.iter().enumerate() {
            ws.write_with_format((r + 1) as u32, c as u16, *value, &wrap)?;
        }
    }
    for (col, width) in [
        14.0, 24.0, 20.0, 34.0, 26.0, 18.0, 14.0, 10.0, 10.0, 12.0, 10.0, 10.0, 14.0, 14.0, 12.0,
        30.0,
    ]
    .iter()
    .enumerate()
    {
        ws.set_column_width(col as u16, *width)?;
    }

    let ws = workbook.add_worksheet();
    ws.set_name("关联示例")?;
    write_headers(ws, &["项目简称*", "关联实验室", "关联方法编号"])?;
    let rows = [
        [
            "示例项目A",
            "示例实验室401",
            "示例液相含量",
            "示例实验室401",
        ],
        [
            "示例项目B",
            "示例实验室401；示例实验室402",
            "示例液相含量；示例水分测定",
            "示例实验室401",
        ],
    ];
    for (r, row) in rows.iter().enumerate() {
        for (c, value) in row.iter().enumerate() {
            ws.write_with_format((r + 1) as u32, c as u16, *value, &wrap)?;
        }
    }
    for (col, width) in [26.0, 42.0, 56.0, 24.0].iter().enumerate() {
        ws.set_column_width(col as u16, *width)?;
    }
    Ok(())
}

fn write_data_dictionary(workbook: &mut Workbook, conn: &Connection) -> Result<()> {
    let ws = workbook.add_worksheet();
    ws.set_name("数据字典")?;
    write_headers(ws, &["分类", "程序现有数据", "用途"])?;
    let specs = [
        (
            "部门",
            "SELECT name FROM divisions WHERE is_active=1 ORDER BY sort_order,id",
            "实验室的所属部门可引用",
        ),
        (
            "实验室",
            "SELECT name FROM project_groups WHERE name!='研发项目' ORDER BY sort_order,id",
            "项目关联可引用",
        ),
        (
            "检测类型",
            "SELECT name FROM method_types ORDER BY sort_order,id",
            "检测方法的检测类型可引用",
        ),
        (
            "检测方法",
            "SELECT name FROM methods WHERE is_active=1 ORDER BY id",
            "项目关联方法可引用",
        ),
        (
            "研发项目",
            "SELECT name FROM projects WHERE is_active=1 ORDER BY sort_order,id",
            "项目关联的项目简称可引用",
        ),
    ];
    let wrap = Format::new().set_text_wrap().set_align(FormatAlign::Top);
    let mut row = 1u32;
    for (category, sql, usage) in specs {
        let mut stmt = conn.prepare(sql)?;
        let values = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if values.is_empty() {
            ws.write_with_format(row, 0, category, &wrap)?;
            ws.write_with_format(row, 1, "（暂无程序现有数据）", &wrap)?;
            ws.write_with_format(row, 2, usage, &wrap)?;
            row += 1;
        } else {
            for value in values {
                ws.write_with_format(row, 0, category, &wrap)?;
                ws.write_with_format(row, 1, value, &wrap)?;
                ws.write_with_format(row, 2, usage, &wrap)?;
                row += 1;
            }
        }
    }
    ws.set_column_width(0, 18)?;
    ws.set_column_width(1, 50)?;
    ws.set_column_width(2, 48)?;
    ws.set_freeze_panes(1, 0)?;
    ws.autofilter(0, 0, row.saturating_sub(1), 2)?;
    Ok(())
}

fn build_template() -> std::result::Result<Vec<u8>, XlsxError> {
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
        ws.merge_range(0, 0, 0, 1, "主数据一键导入模板 v0.4.103", &title)?;
        let rows = [
            ("导入顺序", "部门 → 实验室 → 检测类型 → 仪器 → 检测方法 → 研发项目 → 项目关联。系统会按该顺序自动处理。"),
            ("填写要求", "只填写部门、实验室、检测类型、仪器、检测方法、研发项目、项目关联七个工作表。带 * 的字段必填；不要修改工作表名称和表头。"),
            ("直接引用", "带下拉箭头的单元格直接引用前面的元数据表。请先完成元数据表，再填写项目关联。"),
            ("方法实例", "方法名称可以相同，但一条方法只绑定一台仪器；同名方法绑定不同仪器时属于不同方法实例。内部编号由程序自动生成。"),
            ("多行关联", "同一项目可以填写多行。实验室可单独一行；方法必须同时填写关联方法名称和关联仪器编号，系统会按项目自动合并。"),
            ("导入策略", "覆盖更新：部门、实验室、类型和项目按名称更新；仪器按仪器编号更新；方法按“方法名称 + 仪器编号”更新。"),
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
        &["部门名称*", "排序", "颜色", "启用"],
        &[24.0, 10.0, 14.0, 10.0],
        &[3],
    )?;
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
        &[24.0, 24.0, 10.0, 14.0, 14.0],
        &[3, 4],
    )?;
    add_data_sheet(
        &mut workbook,
        "检测类型",
        &["类型名称*", "排序"],
        &[24.0, 10.0],
        &[],
    )?;
    add_data_sheet(
        &mut workbook,
        "仪器",
        &["仪器编号*", "仪器名称", "仪器类型*", "启用", "备注"],
        &[20.0, 30.0, 20.0, 10.0, 36.0],
        &[3],
    )?;
    add_data_sheet(
        &mut workbook,
        "检测方法",
        &[
            "方法名称*",
            "方法全称",
            "对应仪器编号*",
            "检测类型1*",
            "检测类型2",
            "检测类型3",
            "系数",
            "倍率",
            "金额",
            "启用",
            "备注",
        ],
        &[
            28.0, 38.0, 22.0, 22.0, 22.0, 22.0, 10.0, 10.0, 12.0, 10.0, 36.0,
        ],
        &[9],
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
        ],
        &[26.0, 38.0, 20.0, 14.0, 10.0, 10.0, 36.0],
        &[5],
    )?;
    let relation_headers = ["项目简称*", "关联实验室", "关联方法名称", "关联仪器编号"];
    add_data_sheet(
        &mut workbook,
        "项目关联",
        &relation_headers,
        &[26.0, 28.0, 30.0, 22.0],
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
    add_named_list_validation(&mut workbook, "检测方法", &[3, 4, 5], "MethodTypeOptions")?;
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
            ["7", "项目关联", "项目001｜液相实验室｜", "实验室单独占一行；同一项目可继续填写下一行"],
            ["8", "项目关联", "项目001｜｜含量测定-A｜LC-01", "方法名称和仪器编号必须同时填写"],
            ["9", "项目关联", "项目001｜｜含量测定-A｜LC-02", "同名方法通过仪器编号区分"],
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

    workbook.save_to_buffer()
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
        return Err(AppError::Validation("旧版“主数据”合并模板已停用，请重新下载 v0.4.101 模板，分别填写仪器、检测方法和项目关联".into()));
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
            "项目关联表仍是旧版横向结构，请重新下载 v0.4.101 模板并按项目多行填写".into(),
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
            name,
            sort_order,
            color,
            is_active,
        });
    }

    let labs = read_sheet_rows(&workbook.worksheet_range("实验室").map_err(workbook_error)?)?;
    for row in labs {
        let name = required(&row, "实验室名称*", "实验室", "实验室", &mut issues);
        let department = required(&row, "所属部门*", "实验室", "实验室", &mut issues);
        if name.is_empty() || department.is_empty() {
            continue;
        }
        data.labs.push(LabRow {
            row: row.row,
            sort_order: parse_i64(&row, "排序", 0, "实验室", &name, &mut issues),
            show_in_work: parse_bool(&row, "工作量显示", true, "实验室", &name, &mut issues),
            show_in_rd: parse_bool(&row, "研发送样显示", true, "实验室", &name, &mut issues),
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

    let methods = read_sheet_rows(
        &workbook
            .worksheet_range("检测方法")
            .map_err(workbook_error)?,
    )?;
    for row in methods {
        let name = required(&row, "方法名称*", "检测方法", "检测方法", &mut issues);
        let instrument_code = required(&row, "对应仪器编号*", "检测方法", "检测方法", &mut issues);
        if name.contains("@[") || name.contains('@') {
            push_error(
                &mut issues,
                "检测方法",
                row.row,
                "检测方法",
                &name,
                "方法名称不得包含仪器识别字符，请在“对应仪器编号”列绑定仪器",
            );
        }
        let method_types = collect_numbered_values(&row, &["检测类型*", "检测类型"], "检测类型", 3);
        if method_types.is_empty() {
            push_error(
                &mut issues,
                "检测方法",
                row.row,
                "检测方法",
                &name,
                "至少选择一个检测类型",
            );
        }
        if name.is_empty() || instrument_code.is_empty() || method_types.is_empty() {
            continue;
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
            name,
            method_types,
        });
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
        if method_name.is_empty() != method_instrument.is_empty() {
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
    for relation in &data.relations {
        if relation.labs.is_empty() {
            push_error(
                &mut issues,
                "项目关联",
                relation.row,
                "项目关联",
                &relation.project,
                "该项目至少需要关联一个实验室",
            );
        }
        if relation.methods.is_empty() {
            push_error(
                &mut issues,
                "项目关联",
                relation.row,
                "项目关联",
                &relation.project,
                "该项目至少需要关联一个检测方法",
            );
        }
    }

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

fn parse_compact_workbook<R: std::io::Read + std::io::Seek>(
    workbook: &mut calamine::Sheets<R>,
) -> Result<(ParsedData, Vec<ImportIssue>)> {
    let mut data = ParsedData::default();
    let mut issues = Vec::new();

    let main_rows = read_sheet_rows(&workbook.worksheet_range("主数据").map_err(workbook_error)?)?;
    for row in main_rows {
        let entity = required_any(
            &row,
            &["对象类型*", "对象类型", "类型"],
            "主数据",
            "主数据",
            &mut issues,
        );
        let name = required_any(
            &row,
            &["名称*", "名称", "简称*"],
            "主数据",
            &entity,
            &mut issues,
        );
        if entity.is_empty() || name.is_empty() {
            continue;
        }
        if entity.contains("部门") && !entity.contains("实验") {
            let color = value_any(&row, &["颜色"]);
            data.departments.push(DepartmentRow {
                row: row.row,
                name: name.clone(),
                sort_order: parse_i64(&row, "排序", 0, "主数据", &entity, &mut issues),
                color: if color.is_empty() {
                    "#1976d2".into()
                } else {
                    color
                },
                is_active: parse_bool(&row, "启用", true, "主数据", &entity, &mut issues),
            });
        } else if entity.contains("实验室") {
            let department = required_any(
                &row,
                &["所属部门*", "所属部门"],
                "主数据",
                "实验室",
                &mut issues,
            );
            if department.is_empty() {
                continue;
            }
            data.labs.push(LabRow {
                row: row.row,
                name,
                department,
                sort_order: parse_i64(&row, "排序", 0, "主数据", "实验室", &mut issues),
                show_in_work: parse_bool(&row, "工作量显示", true, "主数据", "实验室", &mut issues),
                show_in_rd: parse_bool(&row, "研发送样显示", true, "主数据", "实验室", &mut issues),
            });
        } else if entity.contains("检测类型") || entity.contains("方法类型") {
            data.method_types.push(MethodTypeRow {
                row: row.row,
                name,
                sort_order: parse_i64(&row, "排序", 0, "主数据", "检测类型", &mut issues),
            });
        } else if entity.contains("方法") {
            let method_types = split_multi(&required_any(
                &row,
                &["检测类型*", "检测类型"],
                "主数据",
                "检测方法",
                &mut issues,
            ));
            if method_types.is_empty() {
                continue;
            }
            data.methods.push(MethodRow {
                row: row.row,
                method_code: method_key(
                    &name,
                    &value_any(&row, &["对应仪器编号*", "对应仪器编号", "仪器编号"]),
                ),
                instrument_code: value_any(&row, &["对应仪器编号*", "对应仪器编号", "仪器编号"]),
                name,
                full_name: value_any(&row, &["全称", "方法全称"]),
                method_types,
                coefficient: parse_f64(&row, "系数", 1.0, "主数据", "检测方法", &mut issues),
                multiplier: parse_f64(&row, "倍率", 1.0, "主数据", "检测方法", &mut issues),
                amount: parse_f64(&row, "金额", 0.0, "主数据", "检测方法", &mut issues),
                is_active: parse_bool(&row, "启用", true, "主数据", "检测方法", &mut issues),
                notes: value_any(&row, &["备注"]),
            });
        } else if entity.contains("项目") {
            let high_item = value_any(&row, &["高项"]);
            data.projects.push(ProjectRow {
                row: row.row,
                name: name.clone(),
                full_name: value_any(&row, &["全称", "项目全称"]),
                high_item: if high_item.is_empty() {
                    None
                } else {
                    Some(high_item)
                },
                sort_order: parse_i64(&row, "排序", 0, "主数据", "研发项目", &mut issues),
                is_active: parse_bool(&row, "启用", true, "主数据", "研发项目", &mut issues),
                project_status: parse_project_status(
                    &row,
                    "项目状态",
                    "主数据",
                    &name,
                    &mut issues,
                ),
                notes: value_any(&row, &["备注"]),
            });
        } else {
            push_error(
                &mut issues,
                "主数据",
                row.row,
                "主数据",
                &name,
                "对象类型仅支持部门、实验室、检测类型、检测方法、研发项目",
            );
        }
    }

    let relation_rows = read_sheet_rows(
        &workbook
            .worksheet_range("项目关联")
            .map_err(workbook_error)?,
    )?;
    for row in relation_rows {
        let project = required_any(
            &row,
            &["项目简称*", "项目简称", "项目名称*", "项目名称"],
            "项目关联",
            "项目关联",
            &mut issues,
        );
        let labs = split_multi(&required_any(
            &row,
            &["关联实验室*", "关联实验室"],
            "项目关联",
            "项目关联",
            &mut issues,
        ));
        let methods = split_multi(&required_any(
            &row,
            &["关联方法*", "关联方法", "关联检测方法*", "关联检测方法"],
            "项目关联",
            "项目关联",
            &mut issues,
        ));
        if project.is_empty() || labs.is_empty() || methods.is_empty() {
            continue;
        }
        data.relations.push(RelationRow {
            row: row.row,
            project,
            labs,
            methods,
        });
    }

    check_duplicates(
        &data.departments,
        |x| (&x.name, x.row),
        "主数据",
        "部门",
        &mut issues,
    );
    check_duplicates(
        &data.labs,
        |x| (&x.name, x.row),
        "主数据",
        "实验室",
        &mut issues,
    );
    check_duplicates(
        &data.method_types,
        |x| (&x.name, x.row),
        "主数据",
        "检测类型",
        &mut issues,
    );
    check_duplicates(
        &data.methods,
        |x| (&x.name, x.row),
        "主数据",
        "检测方法",
        &mut issues,
    );
    check_duplicates(
        &data.projects,
        |x| (&x.name, x.row),
        "主数据",
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

fn workbook_error(error: calamine::Error) -> AppError {
    AppError::Validation(format!("读取工作表失败: {error}"))
}

fn cell_to_string(cell: &DataType) -> String {
    match cell {
        DataType::String(value) | DataType::DateTimeIso(value) | DataType::DurationIso(value) => {
            value.trim().to_string()
        }
        DataType::Float(value) | DataType::DateTime(value) | DataType::Duration(value) => {
            if value.fract() == 0.0 {
                format!("{}", *value as i64)
            } else {
                value.to_string()
            }
        }
        DataType::Int(value) => value.to_string(),
        DataType::Bool(value) => {
            if *value {
                "是".into()
            } else {
                "否".into()
            }
        }
        DataType::Empty | DataType::Error(_) => String::new(),
    }
}

fn value(row: &SheetRow, header: &str) -> String {
    row.values
        .get(header)
        .map(|x| x.trim().to_string())
        .unwrap_or_default()
}

fn value_any(row: &SheetRow, headers: &[&str]) -> String {
    headers
        .iter()
        .find_map(|header| {
            let v = value(row, header);
            if v.is_empty() {
                None
            } else {
                Some(v)
            }
        })
        .unwrap_or_default()
}

fn required_any(
    row: &SheetRow,
    headers: &[&str],
    sheet: &str,
    entity_type: &str,
    issues: &mut Vec<ImportIssue>,
) -> String {
    let result = value_any(row, headers);
    if result.is_empty() {
        push_error(
            issues,
            sheet,
            row.row,
            entity_type,
            "",
            &format!("必填字段 {:?} 不能为空", headers),
        );
    }
    result
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

fn split_multi(raw: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    raw.split([';', '；', '、', ',', '，', '\n'])
        .map(str::trim)
        .filter(|x| !x.is_empty())
        .filter(|x| seen.insert((*x).to_string()))
        .map(ToString::to_string)
        .collect()
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

fn is_hex_color(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].chars().all(|x| x.is_ascii_hexdigit())
}

fn push_error(
    issues: &mut Vec<ImportIssue>,
    sheet: &str,
    row: usize,
    entity_type: &str,
    name: &str,
    message: &str,
) {
    issues.push(ImportIssue {
        sheet: sheet.into(),
        row,
        entity_type: entity_type.into(),
        name: name.into(),
        action: "阻止导入".into(),
        level: "error".into(),
        message: message.into(),
    });
}

fn check_duplicates<T, F>(
    items: &[T],
    key: F,
    sheet: &str,
    entity_type: &str,
    issues: &mut Vec<ImportIssue>,
) where
    F: Fn(&T) -> (&String, usize),
{
    let mut seen = HashMap::<String, usize>::new();
    for item in items {
        let (name, row) = key(item);
        if let Some(first_row) = seen.insert(name.clone(), row) {
            push_error(
                issues,
                sheet,
                row,
                entity_type,
                name,
                &format!("模板内名称重复，首次出现在第 {first_row} 行"),
            );
        }
    }
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
    let existing_methods = load_names(conn, "SELECT m.name || char(31) || i.code FROM methods m JOIN instruments i ON i.id=m.instrument_id")?;
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
        if !existing_departments.contains(&row.department)
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
        if !existing_instruments.contains(&row.instrument_code)
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
            if !existing_methods.contains(method) && !imported_methods.contains(method) {
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
    append_actions(
        issues,
        "检测方法",
        "检测方法",
        data.methods.iter().map(|x| (x.row, &x.method_code)),
        &existing_methods,
        mode,
        &mut counts,
    );
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

fn apply_import(tx: &Transaction<'_>, data: &ParsedData, mode: ImportMode) -> Result<ApplyCounts> {
    let mut counts = ApplyCounts::default();
    let existing_project_names = load_names(tx, "SELECT name FROM projects")?;
    for row in &data.departments {
        let existing = id_by_name(tx, "divisions", &row.name)?;
        match (existing, mode) {
            (Some(id), ImportMode::Upsert) => {
                tx.execute("UPDATE divisions SET sort_order=?1,color=?2,is_active=?3,deleted_at=NULL WHERE id=?4", postgres_compat::params![row.sort_order, row.color, row.is_active, id])?;
                counts.updated += 1;
            }
            (Some(_), ImportMode::Skip) => counts.skipped += 1,
            (None, _) => {
                tx.execute(
                    "INSERT INTO divisions (name,sort_order,color,is_active) VALUES (?1,?2,?3,?4)",
                    postgres_compat::params![row.name, row.sort_order, row.color, row.is_active],
                )?;
                counts.created += 1;
            }
        }
    }
    for row in &data.labs {
        let division_id = id_by_name(tx, "divisions", &row.department)?
            .ok_or_else(|| AppError::Validation(format!("部门不存在: {}", row.department)))?;
        let existing = id_by_name(tx, "project_groups", &row.name)?;
        match (existing, mode) {
            (Some(id), ImportMode::Upsert) => {
                tx.execute("UPDATE project_groups SET sort_order=?1,show_in_work=?2,show_in_rd=?3,division_id=?4,deleted_at=NULL WHERE id=?5", postgres_compat::params![row.sort_order, row.show_in_work, row.show_in_rd, division_id, id])?;
                counts.updated += 1;
            }
            (Some(_), ImportMode::Skip) => counts.skipped += 1,
            (None, _) => {
                tx.execute("INSERT INTO project_groups (name,sort_order,show_in_work,show_in_rd,division_id) VALUES (?1,?2,?3,?4,?5)", postgres_compat::params![row.name, row.sort_order, row.show_in_work, row.show_in_rd, division_id])?;
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
        let instrument_id = id_by_name(tx, "instruments", &row.instrument_code)?
            .ok_or_else(|| AppError::Validation(format!("仪器不存在: {}", row.instrument_code)))?;
        let existing = method_id_by_key(tx, &row.method_code)?;
        let method_id = match (existing, mode) {
            (Some(id), ImportMode::Upsert) => {
                tx.execute("UPDATE methods SET name=?1,full_name=?2,instrument_id=?3,coefficient=?4,multiplier=?5,amount=?6,is_active=?7,notes=?8 WHERE id=?9", postgres_compat::params![row.name,row.full_name,instrument_id,row.coefficient,row.multiplier,row.amount,row.is_active,row.notes,id])?;
                counts.updated += 1;
                id
            }
            (Some(id), ImportMode::Skip) => {
                counts.skipped += 1;
                id
            }
            (None, _) => {
                tx.execute("INSERT INTO methods (method_code,name,full_name,instrument_id,coefficient,multiplier,amount,is_active,notes) VALUES ('',?1,?2,?3,?4,?5,?6,?7,?8)", postgres_compat::params![row.name,row.full_name,instrument_id,row.coefficient,row.multiplier,row.amount,row.is_active,row.notes])?;
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
            let lab_name = &relation.labs[0];
            Some(
                id_by_name(tx, "project_groups", lab_name)?
                    .ok_or_else(|| AppError::Validation(format!("实验室不存在: {lab_name}")))?,
            )
        } else {
            None
        };
        match (existing, mode) {
            (Some(id), ImportMode::Upsert) => {
                tx.execute(
                    "UPDATE projects SET full_name=?1,high_item=?2,sort_order=?3,is_active=?4,notes=?5,group_id=COALESCE(?6,group_id),project_status=?7,archived_at=CASE WHEN ?7='archived' THEN COALESCE(archived_at,datetime('now','localtime')) ELSE NULL END,archived_by=CASE WHEN ?7='archived' THEN 'import' ELSE NULL END WHERE id=?8",
                    postgres_compat::params![row.full_name, row.high_item, row.sort_order, row.is_active, row.notes, compatibility_group_id, row.project_status, id],
                )?;
                counts.updated += 1;
            }
            (Some(_), ImportMode::Skip) => counts.skipped += 1,
            (None, _) => {
                let group_id = compatibility_group_id.ok_or_else(|| {
                    AppError::Validation(format!("新增项目缺少实验室关联: {}", row.name))
                })?;
                tx.execute(
                    "INSERT INTO projects (group_id,name,full_name,high_item,sort_order,is_active,notes,method_type,project_status,archived_at,archived_by) VALUES (?1,?2,?3,?4,?5,?6,?7,'研发项目',?8,CASE WHEN ?8='archived' THEN datetime('now','localtime') ELSE NULL END,CASE WHEN ?8='archived' THEN 'import' ELSE NULL END)",
                    postgres_compat::params![group_id, row.name, row.full_name, row.high_item, row.sort_order, row.is_active, row.notes, row.project_status],
                )?;
                counts.created += 1;
            }
        }
    }

    for row in &data.relations {
        let project_id = id_by_name(tx, "projects", &row.project)?
            .ok_or_else(|| AppError::Validation(format!("项目不存在: {}", row.project)))?;
        if mode == ImportMode::Skip && existing_project_names.contains(&row.project) {
            counts.skipped += 1;
            continue;
        }
        tx.execute(
            "DELETE FROM project_lab_links WHERE project_id=?1",
            [project_id],
        )?;
        tx.execute(
            "DELETE FROM project_method_links WHERE project_id=?1",
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
        for method in &row.methods {
            let method_id = method_id_by_key(tx, method)?.ok_or_else(|| {
                AppError::Validation(format!("检测方法不存在: {}", method_label(method)))
            })?;
            tx.execute(
                "INSERT OR IGNORE INTO project_method_links (project_id,method_id) VALUES (?1,?2)",
                postgres_compat::params![project_id, method_id],
            )?;
        }
        let compatibility_lab = &row.labs[0];
        let compatibility_id = id_by_name(tx, "project_groups", compatibility_lab)?
            .ok_or_else(|| AppError::Validation(format!("实验室不存在: {compatibility_lab}")))?;
        tx.execute(
            "UPDATE projects SET group_id=?1 WHERE id=?2",
            postgres_compat::params![compatibility_id, project_id],
        )?;
        counts.relation_sets += 1;
    }
    Ok(counts)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_template_has_all_required_sheets() {
        let conn = Connection::open_test_database().expect("PostgreSQL test database");
        crate::db::test_migrations::run(&conn).expect("PostgreSQL migrations");
        let bytes = build_template().expect("template");
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
            "研发项目",
            "项目关联",
            "预检结果",
            "填写示例（不导入）",
        ] {
            assert!(names.contains(sheet), "missing sheet {sheet}");
        }
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
            vec!["项目简称*", "关联实验室", "关联方法名称", "关联仪器编号"]
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
                "检测类型1*",
                "检测类型2",
                "检测类型3",
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
            }],
            labs: vec![LabRow {
                row: 2,
                name: "测试实验室".into(),
                department: "测试部门".into(),
                sort_order: 20,
                show_in_work: true,
                show_in_rd: true,
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
        let mut conn = Connection::open_test_database().expect("PostgreSQL test database");
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
}
