//! Human-confirmed creation bridge. Preparation is read-only; execution goes
//! through a fixed original handler after repeating current permission checks.
use crate::{
    api,
    db::DbPool,
    service::{
        ai_business_service,
        authz_service::{self, AuthContext},
    },
};
use axum::{
    body::{to_bytes, Body},
    http::{HeaderMap, Request},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tower::Service;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreparedSubmission {
    pub target: String,
    pub path: String,
    pub body: Value,
    pub title: String,
    pub preview: Vec<Value>,
    pub issues: Vec<String>,
    // Persisted with the action, never sent to the model or browser. Preserve
    // intent rather than feeding server revision fields back as user input.
    #[serde(default)]
    pub canonical_fields: Value,
    #[serde(default)]
    pub record_route: String,
    #[serde(default)]
    pub validation_snapshot: Value,
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn id(value: &Value, key: &str) -> Result<i64, String> {
    value[key]
        .as_i64()
        .filter(|n| *n > 0)
        .ok_or_else(|| format!("请明确选择 {key}"))
}

fn line(preview: &mut Vec<Value>, label: &str, value: Value) {
    if !value.is_null() && value.as_str().is_none_or(|s| !s.trim().is_empty()) {
        let value = value
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| value.to_string());
        preview.push(json!({"label":label,"value":value}));
    }
}

fn number(value: &Value, key: &str) -> Result<f64, String> {
    let number = value[key]
        .as_f64()
        .or_else(|| value[key].as_str()?.parse().ok());
    number
        .filter(|n| n.is_finite() && *n >= 0.0)
        .ok_or_else(|| format!("当前{key}配置无效，请管理员核对"))
}

fn quantity(fields: &Value) -> Result<i64, String> {
    let quantity = match fields.get("quantity") {
        Some(value) => value.as_i64().ok_or("数量必须是整数")?,
        None => 1,
    };
    if !(1..=1_000_000).contains(&quantity) {
        return Err("数量应为1至100万的整数".into());
    }
    Ok(quantity)
}

fn metering_preview(
    preview: &mut Vec<Value>,
    quantity: i64,
    method: &Value,
    multiplier: f64,
) -> Result<(), String> {
    let coefficient = number(method, "coefficient")?;
    let unit_price = number(method, "amount")?;
    let workload = quantity as f64 * coefficient;
    let amount = quantity as f64 * unit_price * multiplier;
    if !workload.is_finite() || !amount.is_finite() {
        return Err("计量结果超出可用范围，请核对方法配置及数量".into());
    }
    line(preview, "工作量系数", json!(coefficient));
    line(preview, "工作量", json!(workload));
    line(preview, "方法单价", json!(unit_price));
    line(preview, "金额倍率", json!(multiplier));
    // Keep the existing amount and workload formulas independent.
    line(preview, "金额", json!(amount));
    Ok(())
}

fn multiplier(fields: &Value, method: &Value) -> Result<(f64, &'static str), String> {
    if fields
        .get("multiplier")
        .is_some_and(|value| !value.is_null())
    {
        let number = fields["multiplier"]
            .as_f64()
            .filter(|n| n.is_finite() && *n >= 0.0 && *n <= 1e9)
            .ok_or("金额倍率无效")?;
        Ok((number, "manual"))
    } else {
        Ok((number(method, "multiplier")?, "method"))
    }
}

fn prepare_clock(target: &str, fields: &mut Value) -> Result<(), String> {
    if matches!(target, "workload" | "auxiliary") {
        if fields
            .get("recorded_at")
            .is_some_and(|v| !v.is_null() && !v.is_string())
        {
            return Err("工作量日期时间必须是文字".into());
        }
        let recorded_at =
            crate::service::record_service::workload_recorded_at(text(fields, "recorded_at"))
                .map_err(|e| e.public_message())?;
        // Freeze the default once so confirmation does not silently move it.
        fields["recorded_at"] = json!(recorded_at);
    }
    Ok(())
}

fn copy_fields(fields: &Value, keys: &[&str]) -> Value {
    let mut result = json!({});
    for key in keys {
        if let Some(value) = fields.get(*key) {
            result[*key] = value.clone();
        }
    }
    result
}

fn preview_labels(labels: &Value) -> Vec<Value> {
    let mut result = Vec::new();
    for (key, label) in [
        ("group_id", "实验室"),
        ("project_id", "项目"),
        ("method_id", "实际方法"),
        ("sender_user_id", "实际送样人"),
        ("user_id", "实际检测人"),
        ("user_name", "实际送样人"),
        ("auxiliary_work_item_id", "辅助事项"),
        ("detection_type", "检测类型"),
        ("quantity", "数量"),
        ("recorded_at", "工作量时间"),
        ("batch_no", "批号"),
        ("main_components", "主要成分"),
        ("notes", "注意事项/备注"),
    ] {
        line(&mut result, label, labels[key].clone());
    }
    // Custom column labels have already been generated by the draft normalizer.
    if let Some(labels) = labels.as_object() {
        for (key, value) in labels {
            if ![
                "group_id",
                "project_id",
                "method_id",
                "sender_user_id",
                "user_id",
                "user_name",
                "auxiliary_work_item_id",
                "detection_type",
                "quantity",
                "recorded_at",
                "batch_no",
                "main_components",
                "notes",
                "coefficient",
                "amount",
                "multiplier",
                "type_key",
            ]
            .contains(&key.as_str())
            {
                line(&mut result, key, value.clone());
            }
        }
    }
    result
}

fn sample_required(columns: &[Value], fields: &Value) -> Result<(), String> {
    for column in columns {
        if column["is_active"].as_bool() != Some(true)
            || column["show_in_form"].as_bool() != Some(true)
            || column["is_required"].as_bool() != Some(true)
        {
            continue;
        }
        let key = column["field_key"].as_str().unwrap_or("");
        if column["data_type"].as_str() == Some("action") || key.starts_with("action_") {
            continue;
        }
        // Times and sequence/status are produced by the original creation API.
        if [
            "submitted_at",
            "detection_date",
            "seq_no",
            "status",
            "sampled_at",
            "sampled_by",
            "detected_by",
        ]
        .contains(&key)
        {
            continue;
        }
        let value = fields.get(key).or_else(|| fields["extra_fields"].get(key));
        if value.is_none_or(|value| {
            value.is_null() || value.as_str().is_some_and(|s| s.trim().is_empty())
        }) {
            let label = column["label"].as_str().unwrap_or(key);
            if column["data_type"].as_str() == Some("attachment") {
                return Err(format!("{label}为必填附件，请到原表单上传后提交"));
            }
            return Err(format!("请填写{label}"));
        }
    }
    Ok(())
}

async fn prepare_regular(
    pool: &DbPool,
    headers: &HeaderMap,
    ctx: &AuthContext,
    target: &str,
    fields: Value,
) -> Result<PreparedSubmission, String> {
    let draft_result = ai_business_service::prepare(
        pool,
        headers,
        ctx,
        &json!({"target":target,"fields":fields}),
    )
    .await?;
    let draft = &draft_result["draft"];
    let resolved = &draft["fields"];
    let labels = &draft["labels"];
    let issues: Vec<String> =
        serde_json::from_value(draft["issues"].clone()).map_err(|_| "草稿校验结果异常")?;
    if !issues.is_empty() {
        return Err(issues.join("；"));
    }
    let mut preview = preview_labels(labels);
    let (path, title, mut body) = match target {
        "rd_sample" => {
            let sender_id = id(resolved, "sender_user_id")?;
            authz_service::require_project_execution_group(
                pool,
                id(resolved, "project_id")?,
                id(resolved, "group_id")?,
            )
            .map_err(|e| e.public_message())?;
            let mut body = copy_fields(
                resolved,
                &[
                    "group_id",
                    "project_id",
                    "method_id",
                    "sender_user_id",
                    "detection_type",
                    "division_id",
                    "quantity",
                    "batch_no",
                    "notes",
                    "extra_fields",
                ],
            );
            body["user_name"] = labels["sender_user_id"].clone();
            body["sender_user_id"] = json!(sender_id);
            // The original server chooses submission time. This required JSON
            // member is deliberately not accepted as model input.
            body["recorded_at"] = json!("");
            let typed = serde_json::from_value(body.clone()).map_err(|_| "研发送样字段类型无效")?;
            api::rd_record_handler::validate_configured_fields(pool, &typed)
                .map_err(|e| e.public_message())?;
            line(&mut preview, "送样时间", json!("确认提交时由服务器生成"));
            ("/api/rd-records", "新增研发送样", body)
        }
        "sample_info" => {
            let mut meta_args = json!({"target":target,"type_key":resolved["type_key"]});
            for key in ["group_id", "project_id"] {
                if !resolved[key].is_null() {
                    meta_args[key] = resolved[key].clone();
                }
            }
            let meta = ai_business_service::metadata(pool, headers, ctx, &meta_args).await?;
            let mut body = copy_fields(
                resolved,
                &[
                    "group_id",
                    "project_id",
                    "division_id",
                    "type_key",
                    "user_name",
                    "lab_name",
                    "project_name",
                    "quantity",
                    "batch_no",
                    "main_components",
                    "notes",
                    "extra_fields",
                ],
            );
            body["detection_type"] = meta["types"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|item| item["type_key"] == resolved["type_key"])
                .map(|item| item["label"].clone())
                .ok_or("样品类型已变化，请重新选择")?;
            for key in [
                "batch_no",
                "user_name",
                "lab_name",
                "project_name",
                "main_components",
            ] {
                if body[key].is_null() {
                    body[key] = json!("");
                }
            }
            let actual_sender = body["user_name"].as_str().unwrap_or("");
            if meta["requires_actual_sender"].as_bool() == Some(true)
                && (actual_sender.trim().is_empty()
                    || actual_sender == ctx.user.username
                    || ["未知", "请填写实际送样人", "请选择送样人"].contains(&actual_sender.trim()))
            {
                return Err("请明确填写实际送样人，不能使用公共账号或占位文字".into());
            }
            let columns = crate::repo::sample_info_column_repo::list_active_by_type(
                pool,
                text(resolved, "type_key").ok_or("缺少样品类型")?,
            )
            .map_err(|e| e.public_message())?;
            let columns = columns
                .iter()
                .map(serde_json::to_value)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| "读取字段规则失败")?;
            sample_required(&columns, &body)?;
            line(&mut preview, "样品类型", body["detection_type"].clone());
            line(&mut preview, "送样时间", json!("确认提交时由服务器生成"));
            ("/api/sample-info", "新增样品信息", body)
        }
        "workload" => {
            let person_id = id(resolved, "user_id")?;
            let person = api::record_handler::resolve_record_subject(
                pool,
                ctx,
                Some(person_id),
                labels["user_id"].as_str().unwrap_or(""),
            )
            .map_err(|e| e.public_message())?;
            let method = crate::repo::method_repo::get_by_id(pool, id(resolved, "method_id")?)
                .map_err(|e| e.public_message())?;
            if resolved["method_config_revision"].as_str()
                != Some(method.method_config_revision.as_str())
            {
                return Err("方法配置已变化，请重新准备提交预览".into());
            }
            let method = serde_json::to_value(method).map_err(|_| "方法配置异常")?;
            let (multiplier, source) = multiplier(resolved, &method)?;
            let mut body = copy_fields(
                resolved,
                &[
                    "group_id",
                    "project_id",
                    "method_id",
                    "original_method_id",
                    "method_config_revision",
                    "quantity",
                    "recorded_at",
                ],
            );
            body["division_id"] = json!(authz_service::require_project_execution_group(
                pool,
                id(resolved, "project_id")?,
                id(resolved, "group_id")?
            )
            .map_err(|e| e.public_message())?);
            body["sender_user_id"] = json!(person.id);
            body["user_name"] = json!(person.username);
            body["multiplier"] = json!(multiplier);
            body["multiplier_source"] = json!(source);
            metering_preview(&mut preview, quantity(resolved)?, &method, multiplier)?;
            ("/api/records", "新增检测工作量", body)
        }
        "auxiliary" => {
            let mut body = copy_fields(
                resolved,
                &[
                    "auxiliary_work_item_id",
                    "expected_item_updated_at",
                    "user_id",
                    "quantity",
                    "recorded_at",
                    "notes",
                ],
            );
            let context = ai_business_service::get(
                api::auxiliary_handler::router(pool.clone()),
                headers,
                "/api/auxiliary/context".into(),
            )
            .await?;
            let person_id = id(resolved, "user_id")?;
            let person = context["people"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|person| person["id"].as_i64() == Some(person_id))
                .ok_or("请选择当前允许的实际工作人员")?;
            let owner = crate::repo::user_repo::find_by_id(pool, person_id)
                .map_err(|e| e.public_message())?
                .filter(|owner| {
                    owner.is_active
                        && !owner.is_public_account
                        && !owner.is_analysis_public_account
                        && !owner.is_rd_public_account
                })
                .ok_or("请选择启用的实际工作人员账号")?;
            let division_id = owner
                .primary_division_id
                .or(owner.division_id)
                .ok_or("所选人员账号未配置主归属部门，请联系管理员维护账号")?;
            if !context["divisions"].as_array().is_some_and(|items| {
                items
                    .iter()
                    .any(|division| division["id"].as_i64() == Some(division_id))
            }) {
                return Err("所选人员主归属部门未启用分析检测或不在当前授权范围内".into());
            }
            if person_id != ctx.user.id {
                let owner_ctx = AuthContext {
                    role_names: owner.role_names.clone(),
                    user: owner,
                };
                if !authz_service::work_division_allowed(pool, &owner_ctx, Some(division_id))
                    .map_err(|e| e.public_message())?
                {
                    return Err("所选人员无权在该部门录入工作量".into());
                }
            }
            body["detection_division_id"] = json!(division_id);
            line(
                &mut preview,
                "人员归属部门",
                person["division_name"].clone(),
            );
            let coefficient = number(labels, "coefficient")?;
            if !(quantity(resolved)? as f64 * coefficient).is_finite() {
                return Err("辅助工作量超出可用范围，请核对数量及系数".into());
            }
            line(&mut preview, "工作量系数", json!(coefficient));
            line(
                &mut preview,
                "辅助工作量",
                json!(quantity(resolved)? as f64 * coefficient),
            );
            line(
                &mut preview,
                "统计口径",
                json!("人员沿账号归属，不参与实验室、项目分摊和金额计算"),
            );
            ("/api/auxiliary/records", "新增辅助工作", body)
        }
        _ => return Err("不支持的业务表单".into()),
    };
    if target == "auxiliary" && body["notes"].is_null() {
        body["notes"] = json!("");
    }
    let record_route = match target {
        "rd_sample" => "/sample-records".to_string(),
        "sample_info" => format!(
            "/sample-info/entry?type={}",
            url_escape::encode_component(text(&body, "type_key").unwrap_or(""))
        ),
        "workload" => format!(
            "/entry/{}?subject_user_id={}",
            id(&body, "group_id")?,
            id(&body, "sender_user_id")?
        ),
        _ => "/auxiliary".to_string(),
    };
    Ok(PreparedSubmission {
        target: target.into(),
        path: path.into(),
        body,
        title: title.into(),
        preview,
        issues,
        canonical_fields: fields,
        record_route,
        validation_snapshot: json!({"method_config_revision":resolved["method_config_revision"]}),
    })
}

fn source_path(fields: &Value) -> Result<String, String> {
    let source_id = id(fields, "source_record_id")?;
    match text(fields, "source_type") {
        Some("rd_sample") => Ok(format!("/api/rd-records/{source_id}/sample-workload")),
        Some("sample_info") => Ok(format!("/api/sample-info/{source_id}/sample-workload")),
        _ => Err("来源类型须为研发送样或样品信息，不能指定其他接口".into()),
    }
}

fn source_router(pool: &DbPool, fields: &Value) -> Result<Router, String> {
    match text(fields, "source_type") {
        Some("rd_sample") => Ok(api::rd_record_handler::router(pool.clone())),
        Some("sample_info") => Ok(api::sample_info_handler::router(pool.clone())),
        _ => Err("不支持的工作量来源".into()),
    }
}

/// Read the original source workload dialog, including real method candidates.
/// The original GET authenticates and checks the source-specific permission.
pub async fn source_metadata(
    pool: &DbPool,
    headers: &HeaderMap,
    fields: &Value,
) -> Result<Value, String> {
    let mut path = source_path(fields)?;
    let keys: &[&str] = if text(fields, "source_type") == Some("rd_sample") {
        &["method_id"]
    } else {
        &["project_id", "group_id"]
    };
    let mut query = Vec::new();
    for key in keys {
        if fields.get(*key).is_some_and(|v| !v.is_null()) {
            query.push(format!("{key}={}", id(fields, key)?));
        }
    }
    if !query.is_empty() {
        path.push('?');
        path.push_str(&query.join("&"));
    }
    ai_business_service::get(source_router(pool, fields)?, headers, path).await
}

fn validate_source_fields(fields: &Value) -> Result<(), String> {
    let object = fields.as_object().ok_or("提交字段必须是对象")?;
    let allowed = [
        "source_type",
        "source_record_id",
        "method_id",
        "group_id",
        "project_id",
        "user_id",
        "quantity",
        "multiplier",
        "recorded_at",
        "notes",
    ];
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err("来源工作量包含不支持的字段".into());
    }
    if serde_json::to_vec(fields)
        .map_err(|_| "提交字段无效")?
        .len()
        > 16_384
    {
        return Err("提交字段过长".into());
    }
    for (key, value) in object {
        if key.ends_with("_id") && !value.is_null() {
            id(fields, key)?;
        }
        if let Some(text) = value.as_str() {
            if text.len() > 4_000 || text.contains('\0') {
                return Err(format!("{key}内容过长或无效"));
            }
        }
    }
    if fields
        .get("notes")
        .is_some_and(|v| !v.is_null() && !v.is_string())
    {
        return Err("备注必须为文字".into());
    }
    quantity(fields)?;
    Ok(())
}

async fn prepare_source(
    pool: &DbPool,
    headers: &HeaderMap,
    ctx: &AuthContext,
    mut fields: Value,
) -> Result<PreparedSubmission, String> {
    validate_source_fields(&fields)?;
    if fields["user_id"]
        .as_i64()
        .is_some_and(|id| id != ctx.user.id)
    {
        return Err(
            "来源工作量按原业务规则仅能录入当前账号本人，请使用实际检测人员账号操作".into(),
        );
    }
    // Source APIs preserve their original actual-person rules; a public account
    // must not silently become an actual detector or guess another person.
    if ctx.user.is_public_account || ctx.is_analysis_public_account() || ctx.is_rd_public_account()
    {
        return Err("来源工作量须由实际检测人员账号录入，公共账号请使用原页面核对实际人员".into());
    }
    let source = source_metadata(pool, headers, &fields).await?;
    let method_id = fields["method_id"]
        .as_i64()
        .or_else(|| {
            (text(&fields, "source_type") == Some("rd_sample"))
                .then(|| source["method_id"].as_i64())
                .flatten()
        })
        .ok_or("请先从来源工作量候选中明确选择实际计量方法，不能自动采用第一个方法")?;
    let method = source["instruments"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|item| item["method_id"].as_i64() == Some(method_id))
        .ok_or("所选方法不在该来源的当前可用计量库中，请重新选择")?;
    let quantity = quantity(&fields)?;
    let remaining = source["remaining_quantity"]
        .as_i64()
        .ok_or("无法读取来源剩余数量")?;
    if quantity > remaining {
        return Err(format!("本次数量超过来源剩余可录数量（{remaining}）"));
    }
    let (multiplier, multiplier_source) = multiplier(&fields, method)?;
    let revision =
        text(method, "method_config_revision").ok_or("方法修订信息不可用，请刷新后重试")?;
    let mut body = copy_fields(&fields, &["quantity", "recorded_at", "notes"]);
    body["quantity"] = json!(quantity);
    body["method_id"] = json!(method_id);
    body["method_config_revision"] = json!(revision);
    body["multiplier"] = json!(multiplier);
    body["multiplier_source"] = json!(multiplier_source);
    if text(&fields, "source_type") == Some("sample_info") {
        body["project_id"] = json!(id(
            &json!({"project_id":source["selected_project_id"]}),
            "project_id"
        )?);
        body["group_id"] = json!(id(
            &json!({"group_id":source["selected_group_id"]}),
            "group_id"
        )?);
        // Freeze automatic source mapping; a remapping cannot silently change
        // the accounting project between preview and confirmation.
        fields["project_id"] = body["project_id"].clone();
        fields["group_id"] = body["group_id"].clone();
    } else if fields.get("project_id").is_some_and(|v| !v.is_null())
        || fields.get("group_id").is_some_and(|v| !v.is_null())
    {
        return Err("研发送样来源的项目与实验室沿原记录，不能另行指定".into());
    }
    fields["method_id"] = json!(method_id);
    fields["quantity"] = json!(quantity);
    let mut preview = Vec::new();
    line(
        &mut preview,
        "来源",
        json!(if text(&fields, "source_type") == Some("rd_sample") {
            "研发送样"
        } else {
            "样品信息登记"
        }),
    );
    line(
        &mut preview,
        "来源业务编号",
        source["source_summary"]["business_no"].clone(),
    );
    line(&mut preview, "项目", source["project"].clone());
    line(&mut preview, "实验室", source["laboratory"].clone());
    line(&mut preview, "部门", source["department"].clone());
    line(
        &mut preview,
        "原提交方法",
        source["source_method_name"].clone(),
    );
    line(&mut preview, "实际计量方法", method["method_name"].clone());
    line(&mut preview, "仪器", method["instrument_name"].clone());
    line(&mut preview, "实际检测人", json!(ctx.user.username));
    line(&mut preview, "数量", json!(quantity));
    line(&mut preview, "来源剩余可录数量", json!(remaining));
    line(&mut preview, "工作量时间", fields["recorded_at"].clone());
    line(&mut preview, "备注", fields["notes"].clone());
    metering_preview(&mut preview, quantity, method, multiplier)?;
    let source_id = id(&fields, "source_record_id")?;
    let record_route = if text(&fields, "source_type") == Some("rd_sample") {
        format!("/sample-records?record_id={source_id}")
    } else {
        format!(
            "/sample-info/entry?type={}&record_id={source_id}",
            url_escape::encode_component(
                text(&source["source_summary"], "type_key").ok_or("来源样品类型不可用")?
            )
        )
    };
    Ok(PreparedSubmission {
        target: "workload".into(),
        path: source_path(&fields)?,
        body,
        title: "录入来源检测工作量".into(),
        preview,
        issues: vec![],
        canonical_fields: fields,
        record_route,
        validation_snapshot: json!({"source_method_id":source["source_method_id"],"source_summary":source["source_summary"]}),
    })
}

pub async fn prepare_submission(
    pool: &DbPool,
    headers: &HeaderMap,
    target: &str,
    mut fields: Value,
) -> Result<PreparedSubmission, String> {
    let ctx = authz_service::authenticate(pool, headers).map_err(|e| e.public_message())?;
    authz_service::require_permission(&ctx, "ai:use").map_err(|e| e.public_message())?;
    if !fields.is_object() {
        return Err("提交字段必须是对象".into());
    }
    prepare_clock(target, &mut fields)?;
    if target == "workload"
        && (fields.get("source_type").is_some() || fields.get("source_record_id").is_some())
    {
        prepare_source(pool, headers, &ctx, fields).await
    } else {
        prepare_regular(pool, headers, &ctx, target, fields).await
    }
}

fn fixed_router(pool: &DbPool, prepared: &PreparedSubmission) -> Result<Router, String> {
    let expected = match prepared.target.as_str() {
        "rd_sample" => "/api/rd-records".to_string(),
        "sample_info" => "/api/sample-info".to_string(),
        "auxiliary" => "/api/auxiliary/records".to_string(),
        "workload" if prepared.canonical_fields.get("source_type").is_some() => {
            source_path(&prepared.canonical_fields)?
        }
        "workload" => "/api/records".to_string(),
        _ => return Err("不支持的提交类型".into()),
    };
    if prepared.path != expected {
        return Err("待确认动作接口不匹配，请重新准备".into());
    }
    Ok(match prepared.target.as_str() {
        "rd_sample" => api::rd_record_handler::router(pool.clone()),
        "sample_info" => api::sample_info_handler::router(pool.clone()),
        "auxiliary" => api::auxiliary_handler::router(pool.clone()),
        "workload" if prepared.canonical_fields.get("source_type").is_some() => {
            source_router(pool, &prepared.canonical_fields)?
        }
        _ => api::record_handler::router(pool.clone()),
    })
}

fn unchanged(prepared: &PreparedSubmission, current: &PreparedSubmission) -> Result<(), String> {
    // A previous entry in the same confirmed batch may legitimately consume
    // part of the source. Both prepare and the original transactional POST
    // still reject any entry exceeding its current remaining capacity.
    let stable_preview = |preview: &Vec<Value>| {
        preview
            .iter()
            .filter(|line| line["label"] != "来源剩余可录数量")
            .cloned()
            .collect::<Vec<_>>()
    };
    if prepared.target != current.target
        || prepared.path != current.path
        || prepared.body != current.body
        || stable_preview(&prepared.preview) != stable_preview(&current.preview)
        || prepared.record_route != current.record_route
        || prepared.validation_snapshot != current.validation_snapshot
        || !current.issues.is_empty()
    {
        return Err("权限、人员、字段、方法配置或来源剩余数量已变化，请重新生成预览后确认".into());
    }
    Ok(())
}

fn result_route(prepared: &PreparedSubmission, record_id: i64) -> String {
    let base = &prepared.record_route;
    if prepared.canonical_fields.get("source_type").is_some() {
        return base.clone();
    }
    format!(
        "{base}{}record_id={record_id}",
        if base.contains('?') { '&' } else { '?' }
    )
}

/// Only called by the authenticated human confirmation route after the action
/// has been claimed once. An uncertain response is never automatically retried.
pub async fn execute_submission(
    pool: &DbPool,
    headers: &HeaderMap,
    prepared: &PreparedSubmission,
) -> Result<Value, String> {
    let mut router = fixed_router(pool, prepared)?;
    let current = prepare_submission(
        pool,
        headers,
        &prepared.target,
        prepared.canonical_fields.clone(),
    )
    .await?;
    unchanged(prepared, &current)?;
    let bytes = serde_json::to_vec(&current.body).map_err(|_| "提交内容无法编码")?;
    let mut request = Request::builder()
        .method("POST")
        .uri(&current.path)
        .header("content-type", "application/json")
        .body(Body::from(bytes))
        .map_err(|_| "提交请求无效")?;
    if let Some(token) = headers.get("authorization") {
        request.headers_mut().insert("authorization", token.clone());
    }
    let response = router
        .call(request)
        .await
        .map_err(|_| "UNKNOWN:原业务接口响应中断，请检查记录后处理")?;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .map_err(|_| "UNKNOWN:提交结果无法读取，请先检查原记录")?;
    let body: Value =
        serde_json::from_slice(&bytes).map_err(|_| "UNKNOWN:提交结果无法识别，请先检查原记录")?;
    let code = body["code"].as_i64();
    if !status.is_success() || code != Some(0) {
        let message = body["message"].as_str().unwrap_or("原业务接口拒绝提交");
        // Only definite validation, permission and not-found failures can be
        // called failures. Internal/database outcomes need human verification.
        if code.is_some_and(|code| [1001, 1003].contains(&code)) {
            return Err(message.to_owned());
        }
        return Err(format!("UNKNOWN:{message}；请先检查原记录，避免重复录入"));
    }
    let record_id = body["data"]["id"]
        .as_i64()
        .filter(|id| *id > 0)
        .ok_or("UNKNOWN:原业务接口未返回记录标识，请先检查原记录")?;
    Ok(
        json!({"record_id":record_id,"route":result_route(prepared,record_id),"message":"已通过原业务接口保存"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coefficient_and_amount_are_independent_and_zero_is_valid() {
        let mut preview = Vec::new();
        metering_preview(
            &mut preview,
            5,
            &json!({"coefficient":20,"amount":2}),
            350.0,
        )
        .unwrap();
        assert!(preview.contains(&json!({"label":"工作量","value":"100.0"})));
        assert!(preview.contains(&json!({"label":"金额","value":"3500.0"})));
        assert_eq!(
            multiplier(&json!({}), &json!({"multiplier":0})).unwrap(),
            (0.0, "method")
        );
        assert_eq!(
            multiplier(&json!({"multiplier":0}), &json!({"multiplier":99})).unwrap(),
            (0.0, "manual")
        );
    }

    #[test]
    fn source_routes_retain_provenance_and_reject_arbitrary_paths() {
        assert_eq!(
            source_path(&json!({"source_type":"rd_sample","source_record_id":8})).unwrap(),
            "/api/rd-records/8/sample-workload"
        );
        assert_eq!(
            source_path(&json!({"source_type":"sample_info","source_record_id":9})).unwrap(),
            "/api/sample-info/9/sample-workload"
        );
        for fields in [
            json!({"source_type":"https://example.test","source_record_id":1}),
            json!({"source_type":"rd_sample","source_record_id":-1}),
            json!({"source_type":"sample_info_sample","source_record_id":1}),
        ] {
            assert!(source_path(&fields).is_err());
        }
        assert!(validate_source_fields(
            &json!({"source_type":"rd_sample","source_record_id":1,"path":"/api/users"})
        )
        .is_err());
        assert!(validate_source_fields(&json!({"quantity":1.5})).is_err());
    }

    #[test]
    fn required_custom_and_attachment_columns_cannot_be_bypassed() {
        let column = json!({"field_key":"handling","label":"处理要求","is_active":true,"show_in_form":true,"is_required":true,"data_type":"text"});
        assert!(sample_required(&[column.clone()], &json!({"extra_fields":{}})).is_err());
        assert!(sample_required(&[column], &json!({"extra_fields":{"handling":"冷藏"}})).is_ok());
        let attachment = json!({"field_key":"files","label":"附件","is_active":true,"show_in_form":true,"is_required":true,"data_type":"attachment"});
        assert!(sample_required(&[attachment], &json!({}))
            .unwrap_err()
            .contains("原表单"));
    }

    #[test]
    fn confirmation_rejects_method_and_remaining_quantity_drift() {
        let prepared = PreparedSubmission {
            target: "workload".into(),
            path: "/api/records".into(),
            body: json!({"method_config_revision":"old"}),
            title: "测试".into(),
            preview: vec![json!({"label":"来源剩余可录数量","value":"5"})],
            issues: vec![],
            canonical_fields: json!({}),
            record_route: "/entry/1".into(),
            validation_snapshot: json!({}),
        };
        assert!(unchanged(&prepared, &prepared).is_ok());
        let mut current = prepared.clone();
        current.body["method_config_revision"] = json!("new");
        assert!(unchanged(&prepared, &current).is_err());
        let mut current = prepared.clone();
        current.preview[0]["value"] = json!("4");
        assert!(unchanged(&prepared, &current).is_ok());
    }
}
