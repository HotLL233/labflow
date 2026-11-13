//! Read-only assistant tools. Every query enters the same handlers as the UI;
//! business writes remain behind the original form and its submit validation.
use crate::api;
use crate::db::DbPool;
use crate::models::ai::AiCapabilities;
use crate::service::authz_service::{self, AuthContext};
use axum::body::{to_bytes, Body};
use axum::http::{HeaderMap, Request};
use axum::Router;
use serde_json::{json, Map, Value};
use tower::Service;

type ToolResult = std::result::Result<Value, String>;

fn permission(ctx: &AuthContext, name: &str) -> std::result::Result<(), String> {
    authz_service::require_permission(ctx, name).map_err(|e| e.public_message())
}

fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({"type":"function","function":{"name":name,"description":description,
        "parameters":{"type":"object","properties":properties,"required":required,"additionalProperties":false}}})
}

fn target_schema() -> Value {
    json!({"type":"string","enum":["sample_info","rd_sample","workload","auxiliary"]})
}

fn query_domains(ctx: &AuthContext) -> Vec<&'static str> {
    let mut domains = vec![];
    if ctx.has_permission("entry:sample-info") {
        domains.push("sample_info");
    }
    if ["entry:sample", "records:rd:view-all", "records:rd:view-lab"]
        .iter()
        .any(|p| ctx.has_permission(p))
    {
        domains.push("rd_sample");
    }
    if ctx.has_permission("entry:workload") {
        domains.push("workload");
    }
    if ctx.has_permission("entry:auxiliary") {
        domains.push("auxiliary");
    }
    domains
}

pub fn tool_definitions(ctx: &AuthContext, caps: &AiCapabilities) -> Vec<Value> {
    let mut tools = Vec::new();
    if caps.help {
        tools.push(tool(
            "program_help",
            "了解本版本程序入口、工作流程和当前账号可用功能；不能据此推测未查询的业务数据。",
            json!({"topic":{"type":"string"}}),
            &[],
        ));
    }
    let domains = query_domains(ctx);
    if caps.query && !domains.is_empty() {
        tools.push(tool("query_records", "查询当前账号授权的记录。返回全范围命中总数及最多50条明细；不得把明细页当作完整汇总。日期必须是 YYYY-MM-DD。", json!({
            "domain":{"type":"string","enum":domains},"start":{"type":"string"},"end":{"type":"string"},
            "group_id":{"type":"integer"},"project_id":{"type":"integer"},
            "subject_user_id":{"type":"integer"},"record_id":{"type":"integer"},
            "type_key":{"type":"string"},"status":{"type":"string"},"keyword":{"type":"string"},
            "page":{"type":"integer","minimum":1},"page_size":{"type":"integer","minimum":1,"maximum":50}
        }), &["domain"]));
    }
    if caps.report && ctx.has_permission("stats:portal:workload") {
        tools.push(tool("workload_summary", "使用原统计页面同一全量授权集合汇总检测及辅助工作量，已含历史辅助工作去重。view=mine 默认本人，scope 需要原范围权限。返回数据不包含金额。", json!({
            "start":{"type":"string"},"end":{"type":"string"},"view":{"type":"string","enum":["mine","scope"]},
            "subject_user_id":{"type":"integer"},"group_id":{"type":"integer"},"project_id":{"type":"integer"},
            "method_id":{"type":"integer"},"kind":{"type":"string","enum":["detection","auxiliary"]}
        }), &["start","end"]));
    }
    if caps.draft
        && [
            "entry:sample-info",
            "entry:sample",
            "entry:workload",
            "entry:auxiliary",
        ]
        .iter()
        .any(|p| ctx.has_permission(p))
    {
        tools.push(tool("entry_metadata", "起草前先读取真实可用字段和主数据。用返回的ID选择实验室、项目、方法、辅助事项及实际人员；多候选不得擅自匹配。keyword只筛选名称，不改变授权范围。", json!({
            "target":target_schema(),"group_id":{"type":"integer"},"project_id":{"type":"integer"},
            "type_key":{"type":"string"},"keyword":{"type":"string"}
        }), &["target"]));
        tools.push(tool("prepare_entry_draft", "生成待用户核对的原表单草稿，绝不写入记录。必须先读取entry_metadata；不得编造ID、系数、金额、送样时间。公共账号须明确实际送样人。fields仅接受原表单可填写字段。", json!({
            "target":target_schema(),"fields":{"type":"object","properties":{
                "group_id":{"type":"integer"},"project_id":{"type":"integer"},"method_id":{"type":"integer"},
                "original_method_id":{"type":"integer"},"sender_user_id":{"type":"integer"},"user_id":{"type":"integer"},
                "auxiliary_work_item_id":{"type":"integer"},"division_id":{"type":"integer"},
                "type_key":{"type":"string"},"detection_type":{"type":"string"},"user_name":{"type":"string"},"quantity":{"type":"integer"},
                "batch_no":{"type":"string"},"main_components":{"type":"string"},"notes":{"type":"string"},
                "recorded_at":{"type":"string"},"multiplier":{"type":"number"},"extra_fields":{"type":"object"}
            },"additionalProperties":false}
        }), &["target","fields"]));
    }
    tools
}

fn text<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn validate_args(args: &Value, allowed: &[&str]) -> std::result::Result<(), String> {
    let object = args.as_object().ok_or("工具参数必须是对象")?;
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err("包含不支持的参数".into());
    }
    if serde_json::to_vec(args).map_err(|e| e.to_string())?.len() > 16_384 {
        return Err("工具参数过长".into());
    }
    for (key, value) in object {
        if key.ends_with("_id") && !value.is_null() && value.as_i64().is_none_or(|id| id <= 0) {
            return Err(format!("{key} 必须是有效ID"));
        }
        if let Some(value) = value.as_str() {
            if value.len() > 4_000 || value.contains('\0') {
                return Err(format!("{key} 内容过长或无效"));
            }
        }
    }
    Ok(())
}

fn query_uri(path: &str, args: &Value, mappings: &[(&str, &str)]) -> String {
    let parts = mappings
        .iter()
        .filter_map(|(source, target)| {
            args.get(*source).filter(|v| !v.is_null()).map(|value| {
                let value = value
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| value.to_string());
                format!("{}={}", target, url_escape::encode_component(&value))
            })
        })
        .collect::<Vec<_>>();
    if parts.is_empty() {
        path.into()
    } else {
        format!("{path}?{}", parts.join("&"))
    }
}

async fn get(mut router: Router, headers: &HeaderMap, uri: String) -> ToolResult {
    let mut request = Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .map_err(|e| e.to_string())?;
    // No model-controlled URL, method, cookie or headers can enter this bridge.
    if let Some(token) = headers.get("authorization") {
        request.headers_mut().insert("authorization", token.clone());
    }
    let response = router.call(request).await.map_err(|e| e.to_string())?;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .map_err(|_| "查询返回内容过大，请缩小范围".to_string())?;
    let body: Value =
        serde_json::from_slice(&bytes).map_err(|_| "原业务接口返回异常".to_string())?;
    if !status.is_success() || body.get("code").and_then(Value::as_i64) != Some(0) {
        return Err(body
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("没有权限或查询失败")
            .to_owned());
    }
    Ok(body.get("data").cloned().unwrap_or(Value::Null))
}

fn require_target(ctx: &AuthContext, target: &str) -> std::result::Result<(), String> {
    match target {
        "sample_info" => {
            permission(ctx, "entry:sample-info")?;
            permission(ctx, "sample-info:create")
        }
        "rd_sample" => permission(ctx, "entry:sample"),
        "workload" | "auxiliary" => {
            permission(
                ctx,
                if target == "workload" {
                    "entry:workload"
                } else {
                    "entry:auxiliary"
                },
            )?;
            if ctx.is_analysis_public_account() {
                permission(ctx, "records:work:portal-scoped")?;
            }
            Ok(())
        }
        _ => Err("不支持的业务表单".into()),
    }
}

fn keep_fields(value: &Value, fields: &[&str]) -> Value {
    Value::Object(
        fields
            .iter()
            .filter_map(|key| value.get(*key).map(|v| ((*key).into(), v.clone())))
            .collect(),
    )
}

fn candidates(items: &Value, fields: &[&str], keyword: Option<&str>) -> Value {
    let keyword = keyword.unwrap_or("").to_lowercase();
    let found = items
        .as_array()
        .into_iter()
        .flatten()
        .filter(|item| {
            item.get("is_active").and_then(Value::as_bool) != Some(false)
                && item.get("enabled").and_then(Value::as_bool) != Some(false)
                && item.get("deleted_at").is_none_or(Value::is_null)
                && (keyword.is_empty()
                    || [
                        "name",
                        "full_name",
                        "instrument_name",
                        "area_name",
                        "category",
                    ]
                    .iter()
                    .any(|key| {
                        item.get(*key)
                            .and_then(Value::as_str)
                            .is_some_and(|s| s.to_lowercase().contains(&keyword))
                    }))
        })
        .collect::<Vec<_>>();
    json!({"total":found.len(),"truncated":found.len()>100,"items":found.into_iter().take(100).map(|v|keep_fields(v,fields)).collect::<Vec<_>>()})
}

async fn metadata(
    pool: &DbPool,
    headers: &HeaderMap,
    ctx: &AuthContext,
    args: &Value,
) -> ToolResult {
    let target = text(args, "target").ok_or("请选择业务表单")?;
    require_target(ctx, target)?;
    let keyword = text(args, "keyword");
    if target == "auxiliary" {
        let context = get(
            api::auxiliary_handler::router(pool.clone()),
            headers,
            "/api/auxiliary/context".into(),
        )
        .await?;
        let items = get(
            api::auxiliary_handler::router(pool.clone()),
            headers,
            "/api/auxiliary/items".into(),
        )
        .await?;
        return Ok(
            json!({"target":target,"context":context,"items":candidates(&items,&["id","item_code","name","area_type_id","area_name","category","coefficient","updated_at"],keyword),
            "fields":["auxiliary_work_item_id","user_id","quantity","recorded_at","notes"],"rules":["事项独立配置；不关联实验室、项目或金额","实际人员来自context.people，提交仍走原校验"]}),
        );
    }
    let portal = match target {
        "rd_sample" => "rd",
        "workload" => "work",
        _ => "sample_info",
    };
    let group_uri = format!("/api/groups?portal={portal}");
    let sample_types = if target == "sample_info" {
        get(
            api::sample_info_type_handler::router(pool.clone()),
            headers,
            "/api/sample-info-types".into(),
        )
        .await?
    } else {
        Value::Null
    };
    let mut groups = get(api::group_handler::router(pool.clone()), headers, group_uri).await?;
    if let Some(items) = groups.as_array_mut() {
        items.retain(|item| {
            let Some(id) = item["id"].as_i64() else {
                return false;
            };
            if target == "rd_sample" {
                api::rd_record_handler::can_access_entry_group(ctx, id)
                    && authz_service::rd_group_allowed(pool, ctx, id).unwrap_or(false)
            } else if target == "workload" {
                authz_service::work_group_allowed(pool, ctx, id).unwrap_or(false)
            } else {
                (ctx.is_system_admin()
                    || ctx.is_analysis_leader()
                    || ctx.user.group_id.is_none_or(|own| own == id))
                    && sample_types
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|item| item["type_key"].as_str())
                        .filter(|key| {
                            text(args, "type_key").is_none_or(|selected| selected == *key)
                        })
                        .any(|key| {
                            authz_service::sample_info_scope_allows(
                                pool,
                                ctx,
                                item["division_id"].as_i64(),
                                key,
                                None,
                            )
                            .is_ok_and(|allowed| allowed != Some(false))
                        })
            }
        });
    }
    let group_id = args.get("group_id").and_then(Value::as_i64);
    if group_id.is_some_and(|id| {
        !groups
            .as_array()
            .is_some_and(|items| items.iter().any(|g| g["id"].as_i64() == Some(id)))
    }) {
        return Err("实验室不在当前账号可用范围内".into());
    }
    let mut project_args = json!({"portal":portal,"active_only":true});
    if let Some(id) = group_id {
        project_args["group_id"] = json!(id);
    }
    let mut projects = get(
        api::project_handler::router(pool.clone()),
        headers,
        query_uri(
            "/api/projects",
            &project_args,
            &[
                ("portal", "portal"),
                ("active_only", "active_only"),
                ("group_id", "group_id"),
            ],
        ),
    )
    .await?;
    if target == "rd_sample" {
        if let Some(items) = projects.as_array_mut() {
            items.retain(|p| {
                p["id"].as_i64().is_some_and(|id| {
                    authz_service::rd_project_party_allowed(pool, ctx, id).unwrap_or(false)
                })
            });
        }
    }
    if let Some(items) = projects.as_array_mut() {
        items.retain(|project| {
            project["lab_ids"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|lab| {
                    groups
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|group| group["id"] == *lab)
                })
        });
    }
    let project_id = args.get("project_id").and_then(Value::as_i64);
    if project_id.is_some_and(|id| {
        !projects
            .as_array()
            .is_some_and(|items| items.iter().any(|p| p["id"].as_i64() == Some(id)))
    }) {
        return Err("项目不在当前表单可用范围内".into());
    }
    let selected_groups = group_id
        .map(|id| {
            json!(groups
                .as_array()
                .into_iter()
                .flatten()
                .filter(|v| v["id"].as_i64() == Some(id))
                .collect::<Vec<_>>())
        })
        .unwrap_or(groups);
    let selected_projects = project_id
        .map(|id| {
            json!(projects
                .as_array()
                .into_iter()
                .flatten()
                .filter(|v| v["id"].as_i64() == Some(id))
                .collect::<Vec<_>>())
        })
        .unwrap_or(projects);
    let mut result = json!({"target":target,"groups":candidates(&selected_groups,&["id","name","division_id"],None),"projects":candidates(&selected_projects,&["id","name","lab_ids","method_ids"],None)});
    if target == "sample_info" {
        let mut types = sample_types;
        if !ctx.is_system_admin() {
            if let Some(items) = types.as_array_mut() {
                items.retain(|t| {
                    let key = t["type_key"].as_str().unwrap_or("");
                    selected_groups
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|group| {
                            authz_service::sample_info_scope_allows(
                                pool,
                                ctx,
                                group["division_id"].as_i64(),
                                key,
                                None,
                            )
                            .is_ok_and(|allowed| allowed != Some(false))
                        })
                        || authz_service::sample_info_scope_allows(pool, ctx, None, key, None)
                            .is_ok_and(|allowed| allowed != Some(false))
                });
            }
        }
        let type_key = text(args, "type_key");
        if type_key.is_some_and(|key| {
            !types
                .as_array()
                .is_some_and(|items| items.iter().any(|t| t["type_key"].as_str() == Some(key)))
        }) {
            return Err("样品类型不在当前可用范围内".into());
        }
        let columns = get(
            api::sample_info_column_handler::router(pool.clone()),
            headers,
            query_uri(
                "/api/sample-info/columns/active",
                args,
                &[("type_key", "type_key")],
            ),
        )
        .await?;
        result["types"] = types;
        result["columns"] = columns;
        result["requires_actual_sender"] = json!(
            ctx.user.require_actual_submitter
                || ctx.is_rd_public_account()
                || ctx.is_analysis_public_account()
                || ctx.user.is_public_account
        );
        result["rules"] = json!([
            "送样时间在正式提交时由服务器生成，不可起草",
            "公共账号不能将账号名作为实际送样人；未提供则留空待用户填写",
            "仅当前类型启用的字段可起草，必填规则仍由原表单校验"
        ]);
    } else {
        if group_id.is_none() {
            result["hint"] = json!("先明确实验室，再获取项目、方法和实际人员候选");
            return Ok(result);
        }
        let methods = if target == "workload" && project_id.is_some() {
            get(
                api::record_handler::router(pool.clone()),
                headers,
                query_uri(
                    "/api/records/method-options",
                    args,
                    &[("group_id", "group_id"), ("project_id", "project_id")],
                ),
            )
            .await?
        } else {
            let method_args = json!({"portal":portal,"group_id":group_id});
            get(
                api::method_handler::router(pool.clone()),
                headers,
                query_uri(
                    "/api/methods",
                    &method_args,
                    &[("portal", "portal"), ("group_id", "group_id")],
                ),
            )
            .await?
        };
        result["methods"] = candidates(
            &methods,
            &[
                "id",
                "method_code",
                "name",
                "full_name",
                "coefficient",
                "amount",
                "multiplier",
                "instrument_id",
                "instrument_name",
                "type_names",
                "method_config_revision",
            ],
            keyword,
        );
        let people = if target == "rd_sample" {
            if ctx.is_rd_public_account() || ctx.has_permission("records:rd:select-sender") {
                api::user_handler::rd_entry_people(pool, headers, group_id.unwrap())
                    .map_err(|e| e.public_message())?
            } else {
                vec![ctx.user.clone()]
            }
        } else if ctx.is_analysis_public_account() {
            crate::repo::user_repo::analysis_public_account_scope(pool, ctx.user.id)
                .map_err(|e| e.public_message())?
                .accounts
                .into_iter()
                .map(|person| {
                    crate::repo::user_repo::find_by_id(pool, person.id)
                        .map_err(|e| e.public_message())
                        .and_then(|p| p.ok_or("人员不存在".into()))
                })
                .collect::<std::result::Result<Vec<_>, _>>()?
        } else if ctx.can_view_workload_scope()
            || ctx.has_permission("records:work:select-detector")
        {
            api::user_handler::work_entry_people(pool, headers, group_id.unwrap())
                .map_err(|e| e.public_message())?
        } else {
            vec![ctx.user.clone()]
        };
        result["people"] = json!(people
            .iter()
            .map(|p| json!({"id":p.id,"username":p.username,"group_id":p.group_id}))
            .collect::<Vec<_>>());
        if target == "rd_sample" {
            result["columns"] = get(
                api::rd_record_column_handler::router(pool.clone()),
                headers,
                "/api/rd-record-columns".into(),
            )
            .await?;
        }
    }
    Ok(result)
}

async fn records(pool: &DbPool, headers: &HeaderMap, args: &Value) -> ToolResult {
    validate_args(
        args,
        &[
            "domain",
            "start",
            "end",
            "group_id",
            "project_id",
            "subject_user_id",
            "record_id",
            "type_key",
            "status",
            "keyword",
            "page",
            "page_size",
        ],
    )?;
    if args["record_id"].is_null() {
        require_dates(args)?;
    }
    let domain = text(args, "domain").ok_or("请选择记录类型")?;
    let ctx = authz_service::authenticate(pool, headers).map_err(|e| e.public_message())?;
    if !query_domains(&ctx).contains(&domain) {
        return Err("当前账号无此类记录查看入口".into());
    }
    let supported = match domain {
        "rd_sample" => vec![
            "domain",
            "start",
            "end",
            "group_id",
            "project_id",
            "record_id",
            "page",
            "page_size",
        ],
        "workload" => vec![
            "domain",
            "start",
            "end",
            "group_id",
            "project_id",
            "subject_user_id",
            "record_id",
            "page",
            "page_size",
        ],
        "sample_info" => vec![
            "domain",
            "start",
            "end",
            "group_id",
            "project_id",
            "record_id",
            "type_key",
            "status",
            "keyword",
            "page",
            "page_size",
        ],
        _ => vec![
            "domain",
            "start",
            "end",
            "subject_user_id",
            "keyword",
            "page",
            "page_size",
        ],
    };
    validate_args(args, &supported)?;
    let mut query = args.clone();
    query["page_size"] = json!(args["page_size"].as_i64().unwrap_or(20).clamp(1, 50));
    query["page"] = json!(args["page"].as_i64().unwrap_or(1).clamp(1, 100_000));
    let common = [
        ("start", "start"),
        ("end", "end"),
        ("group_id", "group_id"),
        ("project_id", "project_id"),
        ("record_id", "record_id"),
        ("page", "page"),
        ("page_size", "page_size"),
    ];
    let data = match text(args, "domain") {
        Some("rd_sample") => {
            get(
                api::rd_record_handler::router(pool.clone()),
                headers,
                query_uri("/api/rd-records", &query, &common),
            )
            .await?
        }
        Some("sample_info") => {
            let mut mappings = common.to_vec();
            mappings.extend([
                ("type_key", "type_key"),
                ("status", "status"),
                ("keyword", "keyword"),
            ]);
            get(
                api::sample_info_handler::router(pool.clone()),
                headers,
                query_uri("/api/sample-info", &query, &mappings),
            )
            .await?
        }
        Some("workload") => {
            let mut mappings = common.to_vec();
            mappings.push(("subject_user_id", "subject_user_id"));
            get(
                api::record_handler::router(pool.clone()),
                headers,
                query_uri("/api/records", &query, &mappings),
            )
            .await?
        }
        Some("auxiliary") => {
            get(
                api::auxiliary_handler::router(pool.clone()),
                headers,
                query_uri(
                    "/api/auxiliary/records",
                    &query,
                    &[
                        ("start", "start"),
                        ("end", "end"),
                        ("subject_user_id", "user_id"),
                        ("keyword", "keyword"),
                        ("page", "page"),
                        ("page_size", "page_size"),
                    ],
                ),
            )
            .await?
        }
        _ => return Err("不支持的记录类型".into()),
    };
    let safe = [
        "id",
        "business_no",
        "recorded_at",
        "submitted_at",
        "user_name",
        "lab_name",
        "group_name",
        "project_name",
        "method_name",
        "method_type",
        "type_key",
        "detection_type",
        "status",
        "quantity",
        "coefficient",
        "workload",
        "batch_no",
        "main_components",
        "notes",
        "sampled_by",
        "sampled_at",
        "auxiliary_work_name",
        "category",
        "area_name",
        "origin",
        "recorded_quantity",
        "workload_recorded",
    ];
    let items = data["items"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|item| keep_fields(item, &safe))
        .collect::<Vec<_>>();
    Ok(
        json!({"domain":args["domain"],"filters":args,"total":data["total"],"page":data["page"],"page_size":data["page_size"],"items":items,"detail_is_partial":data["total"].as_i64().unwrap_or(0)>items.len() as i64,"notice":"total为原权限范围内全部命中数量；items为一页明细，禁止据此计算全量工作量"}),
    )
}

fn find(items: &Value, id: i64) -> Option<&Value> {
    items
        .as_array()?
        .iter()
        .find(|v| v["id"].as_i64() == Some(id))
}
fn required_id(fields: &Value, key: &str) -> std::result::Result<i64, String> {
    fields[key]
        .as_i64()
        .filter(|id| *id > 0)
        .ok_or_else(|| format!("请先明确选择 {key}"))
}

fn require_dates(args: &Value) -> std::result::Result<(), String> {
    let parse = |key| {
        text(args, key)
            .ok_or_else(|| "请先明确查询开始和结束日期".to_string())
            .and_then(|value| {
                chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
                    .map_err(|_| "查询日期必须为 YYYY-MM-DD".to_string())
            })
    };
    let start = parse("start")?;
    let end = parse("end")?;
    if start > end {
        return Err("开始日期不能晚于结束日期".into());
    }
    Ok(())
}

async fn prepare(
    pool: &DbPool,
    headers: &HeaderMap,
    ctx: &AuthContext,
    args: &Value,
) -> ToolResult {
    validate_args(args, &["target", "fields"])?;
    let target = text(args, "target").ok_or("请选择业务表单")?;
    require_target(ctx, target)?;
    let mut fields = args.get("fields").cloned().ok_or("缺少草稿字段")?;
    let allowed = match target {
        "sample_info" => vec![
            "group_id",
            "project_id",
            "division_id",
            "type_key",
            "user_name",
            "quantity",
            "batch_no",
            "main_components",
            "notes",
            "extra_fields",
        ],
        "rd_sample" => vec![
            "group_id",
            "project_id",
            "method_id",
            "sender_user_id",
            "detection_type",
            "quantity",
            "batch_no",
            "notes",
            "extra_fields",
        ],
        "workload" => vec![
            "group_id",
            "project_id",
            "method_id",
            "original_method_id",
            "user_id",
            "quantity",
            "recorded_at",
            "multiplier",
        ],
        "auxiliary" => vec![
            "auxiliary_work_item_id",
            "user_id",
            "quantity",
            "recorded_at",
            "notes",
        ],
        _ => return Err("不支持的业务表单".into()),
    };
    validate_args(&fields, &allowed)?;
    if fields.get("quantity").is_some_and(|v| v.as_i64().is_none()) {
        return Err("数量必须是整数".into());
    }
    let quantity = fields["quantity"].as_i64().unwrap_or(1);
    if !(1..=1_000_000).contains(&quantity) {
        return Err("数量应为1至100万的整数".into());
    }
    fields["quantity"] = json!(quantity);
    if fields["extra_fields"].as_object().is_some_and(|extra| {
        extra
            .values()
            .any(|value| value.is_object() || value.is_array())
    }) {
        return Err("自定义字段只能填写原表单的单值内容".into());
    }
    let mut metadata_args = json!({"target":target});
    for key in ["group_id", "project_id", "type_key"] {
        if !fields[key].is_null() {
            metadata_args[key] = fields[key].clone();
        }
    }
    let meta = metadata(pool, headers, ctx, &metadata_args).await?;
    let mut labels = Map::new();
    let mut issues = Vec::new();
    labels.insert("quantity".into(), json!(quantity.to_string()));
    let title = match target {
        "sample_info" => "样品信息登记草稿",
        "rd_sample" => "研发送样草稿",
        "workload" => "工作量录入草稿",
        _ => "辅助工作草稿",
    };
    let route = if target == "auxiliary" {
        let id = required_id(&fields, "auxiliary_work_item_id")?;
        let all_items = get(
            api::auxiliary_handler::router(pool.clone()),
            headers,
            "/api/auxiliary/items".into(),
        )
        .await?;
        let item = find(&all_items, id)
            .filter(|item| item["enabled"].as_bool() == Some(true) && item["deleted_at"].is_null())
            .ok_or("辅助事项未匹配或已停用，请缩小关键词重新查询")?;
        labels.insert("auxiliary_work_item_id".into(), item["name"].clone());
        labels.insert("coefficient".into(), item["coefficient"].clone());
        fields["expected_item_updated_at"] = item["updated_at"].clone();
        let person_id = fields["user_id"].as_i64().unwrap_or(ctx.user.id);
        let person =
            find(&meta["context"]["people"], person_id).ok_or("请选择原录入页允许的实际人员")?;
        fields["user_id"] = json!(person_id);
        labels.insert("user_id".into(), person["username"].clone());
        "/auxiliary".into()
    } else {
        let group_id = if target == "sample_info" {
            fields["group_id"].as_i64().unwrap_or(0)
        } else {
            required_id(&fields, "group_id")?
        };
        let empty = Value::Null;
        let group = if group_id > 0 {
            find(&meta["groups"]["items"], group_id).ok_or("实验室未匹配，请重新选择")?
        } else {
            &empty
        };
        if group_id > 0 {
            labels.insert("group_id".into(), group["name"].clone());
        }
        let project_id = if target == "sample_info" {
            fields["project_id"].as_i64().unwrap_or(0)
        } else {
            required_id(&fields, "project_id")?
        };
        let project = if project_id > 0 {
            find(&meta["projects"]["items"], project_id).ok_or("项目未匹配，请重新选择")?
        } else {
            &empty
        };
        if group_id > 0
            && project_id > 0
            && !project["lab_ids"]
                .as_array()
                .is_some_and(|ids| ids.iter().any(|id| id.as_i64() == Some(group_id)))
        {
            return Err("项目未关联所选实验室".into());
        }
        if project_id > 0 {
            labels.insert("project_id".into(), project["name"].clone());
        }
        if target == "sample_info" {
            let type_key = text(&fields, "type_key")
                .ok_or("请选择样品类型")?
                .to_owned();
            if let Some(false) = authz_service::sample_info_scope_allows(
                pool,
                ctx,
                group["division_id"]
                    .as_i64()
                    .or(fields["division_id"].as_i64()),
                &type_key,
                None,
            )
            .map_err(|e| e.public_message())?
            {
                return Err("当前角色无权登记该部门及样品类型组合".into());
            }
            if group_id > 0 {
                fields["division_id"] = group["division_id"].clone();
                fields["lab_name"] = group["name"].clone();
            }
            if project_id > 0 {
                fields["project_name"] = project["name"].clone();
            }
            let mut payload = fields.clone();
            payload["detection_type"] = meta["types"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|item| item["type_key"].as_str() == Some(type_key.as_str()))
                .map(|item| item["label"].clone())
                .unwrap_or(json!(&type_key));
            let mut body: crate::models::sample_info::SampleInfoCreate =
                serde_json::from_value(payload).map_err(|_| "样品字段类型无效".to_string())?;
            api::sample_info_handler::validate_sample_info_bindings(pool, ctx, &mut body, false)
                .map_err(|e| e.public_message())?;
            let sender = text(&fields, "user_name").unwrap_or("");
            if meta["requires_actual_sender"].as_bool() == Some(true)
                && (sender.is_empty() || sender == ctx.user.username)
            {
                fields["user_name"] = json!("");
                issues.push("请填写并核对实际送样人，不能使用公共账号名称".to_string());
            } else if sender.is_empty() {
                fields["user_name"] = json!(ctx.user.username);
            }
            let column_keys = meta["columns"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|c| c["show_in_form"].as_bool() != Some(false))
                .filter_map(|c| c["field_key"].as_str())
                .collect::<Vec<_>>();
            if fields["extra_fields"]
                .as_object()
                .is_some_and(|extra| extra.keys().any(|key| !column_keys.contains(&key.as_str())))
            {
                return Err("草稿包含当前样品类型未启用的自定义字段".into());
            }
            format!(
                "/sample-info/entry?type={}",
                url_escape::encode_component(&type_key)
            )
        } else {
            let method_id = required_id(&fields, "method_id")?;
            // Candidates are bounded for the model; validation checks the entire original
            // endpoint result so a chosen stable ID beyond the displayed slice still works.
            let methods = if target == "workload" {
                get(
                    api::record_handler::router(pool.clone()),
                    headers,
                    query_uri(
                        "/api/records/method-options",
                        &fields,
                        &[("group_id", "group_id"), ("project_id", "project_id")],
                    ),
                )
                .await?
            } else {
                get(
                    api::method_handler::router(pool.clone()),
                    headers,
                    format!("/api/methods?portal=rd&group_id={group_id}"),
                )
                .await?
            };
            let method = find(&methods, method_id).ok_or("方法不在原表单可用候选内")?;
            if method["is_active"].as_bool() == Some(false) {
                return Err("方法已停用".into());
            }
            labels.insert("method_id".into(), method["name"].clone());
            labels.insert("coefficient".into(), method["coefficient"].clone());
            labels.insert("amount".into(), method["amount"].clone());
            fields["method_config_revision"] = method["method_config_revision"].clone();
            if target == "rd_sample" {
                if !project["method_ids"]
                    .as_array()
                    .is_some_and(|ids| ids.iter().any(|id| id.as_i64() == Some(method_id)))
                {
                    return Err("送样方法未关联所选项目".into());
                }
                let types = method["type_names"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>();
                let detection_type = text(&fields, "detection_type")
                    .map(str::to_owned)
                    .or_else(|| (types.len() == 1).then(|| types[0].to_owned()))
                    .ok_or("方法关联多个类型，请明确选择 detection_type")?;
                let conn = pool.get().map_err(|_| "查询失败，请稍后重试".to_string())?;
                crate::repo::rd_record_repo::validate_submission_selection(
                    &conn,
                    project_id,
                    method_id,
                    &detection_type,
                )
                .map_err(|e| e.public_message())?;
                crate::repo::method_repo::ensure_method_visible_for_group(
                    pool, method_id, group_id, "rd",
                )
                .map_err(|e| e.public_message())?;
                fields["detection_type"] = json!(detection_type);
                let public = ctx.is_rd_public_account();
                let person_id = fields["sender_user_id"]
                    .as_i64()
                    .or((!public).then_some(ctx.user.id));
                if let Some(person_id) = person_id {
                    let person =
                        find(&meta["people"], person_id).ok_or("请选择原表单允许的实际送样人员")?;
                    fields["sender_user_id"] = json!(person_id);
                    labels.insert("sender_user_id".into(), person["username"].clone());
                } else {
                    issues.push("请在原表单选择实际送样人员".into());
                }
                fields["method_name"] = method["name"].clone();
                fields["project_name"] = project["name"].clone();
                fields["division_id"] = group["division_id"].clone();
                format!("/sample/{group_id}")
            } else {
                let original = fields["original_method_id"].as_i64().unwrap_or(method_id);
                if !project["method_ids"]
                    .as_array()
                    .is_some_and(|ids| ids.iter().any(|id| id.as_i64() == Some(original)))
                {
                    return Err("请选择项目原关联方法作为原表单录入行".into());
                }
                fields["original_method_id"] = json!(original);
                let person_id = fields["user_id"].as_i64().unwrap_or(ctx.user.id);
                let person =
                    find(&meta["people"], person_id).ok_or("请选择原录入页允许的实际检测人员")?;
                fields["user_id"] = json!(person_id);
                labels.insert("user_id".into(), person["username"].clone());
                if let Some(multiplier) = fields.get("multiplier") {
                    if multiplier
                        .as_f64()
                        .is_none_or(|n| !n.is_finite() || n < 0.0 || n > 1e9)
                    {
                        return Err("金额倍率无效".into());
                    }
                }
                fields["method_config_revision"] = method["method_config_revision"].clone();
                format!("/entry/{group_id}?subject_user_id={person_id}")
            }
        }
    };
    for key in [
        "recorded_at",
        "batch_no",
        "main_components",
        "notes",
        "user_name",
        "type_key",
        "multiplier",
        "detection_type",
    ] {
        if let Some(value) = fields.get(key) {
            labels.insert(key.into(), value.clone());
        }
    }
    if let Some(recorded) = text(&fields, "recorded_at") {
        if crate::service::record_service::workload_recorded_at(Some(recorded)).is_err() {
            return Err("工作量日期时间格式无效，请使用原表单日期时间格式".into());
        }
    }
    let labels = labels
        .into_iter()
        .map(|(key, value)| {
            (
                key,
                Value::String(
                    value
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| value.to_string()),
                ),
            )
        })
        .collect::<Map<_, _>>();
    Ok(
        json!({"draft":{"id":uuid::Uuid::new_v4().to_string(),"account_id":ctx.user.id,"target":target,"title":title,"route":route,"fields":fields,"labels":labels,"issues":issues,"created_at":chrono::Utc::now().to_rfc3339()},"notice":"尚未提交；需在原表单核对、添加并使用原提交按钮"}),
    )
}

fn program_guides(allowed: impl Fn(&str) -> bool, is_admin: bool) -> Vec<Value> {
    let mut guides = Vec::new();
    let mut add = |name: &str, path: &str, steps: &str, rule: &str| {
        guides.push(json!({"name":name,"path":path,"steps":steps,
            "basis":format!("v{} · {}原页面与业务规则",env!("CARGO_PKG_VERSION"),name),"rule":rule}));
    };
    let rd_records = ["entry:sample", "records:rd:view-all", "records:rd:view-lab"]
        .iter()
        .any(|p| allowed(p));
    if allowed("entry:sample") {
        add("研发送样", "/sample", "选择可用实验室→项目和提交方法→数量、批号及必填信息→原提交按钮；提交后从送样记录跟进。",
            "公共账号按原规则选择实际送样人；提交方法不因检测人的计量方法调整而改变；新送样时间由服务器生成。");
    }
    if rd_records {
        add(
            "研发送样记录",
            "/sample-records",
            "按日期、人员、实验室、项目和状态查询；从原记录查看进度及来源关联。",
            "只显示原权限范围；查历史需调整日期，当前页明细不能代表全量统计。",
        );
        if allowed("sample:collect") {
            add(
                "研发送样取样",
                "/sample-records",
                "找到待取样记录→核对样品→点击该行取样；以服务端返回的状态为准。",
                "按钮还受记录状态和范围限制；取样后对应站内送样提醒自动已读，AI不能代为取样。",
            );
        }
        if allowed("sample:return") {
            add(
                "研发送样驳回",
                "/sample-records",
                "找到可退回的记录→退回送样→填写原因→原确认退回按钮。",
                "需有退回资格且状态允许；原记录与退回原因保留，AI不能代为退回。",
            );
        }
        if allowed("records:rd:return-confirm") || allowed("records:rd:return-delegate") {
            add(
                "研发送样退回修改",
                "/sample-records",
                "查看退回原因→确认退回并修改→核对新生成的待修改记录→按原页面重新提交。",
                "原退回记录保留；是否可确认或代办由当前记录归属及原权限决定。",
            );
        }
        if allowed("sample:record-workload") {
            add("研发送样来源工作量", "/sample-records", "从已符合录入条件的原记录点击录入工作量→搜索实际计量方法→核对人员、数量、日期与倍率→原确认按钮。",
                "原提交方法与实际计量方法分别保留；剩余可录数量、方法有效性及历史快照沿原校验。");
        }
    }
    if allowed("entry:sample-info") {
        add("样品信息登记", "/sample-info", if allowed("sample-info:create") {
                "选择检测类型→按当前字段填写样品、数量、实际送样人和附件→原提交按钮；可使用本人的原页面草稿。"
                } else { "选择有权查看的检测类型→在原记录列表按日期与状态查询。" },
            "字段与必填项以当前类型配置为准；公共用途账号的实际送样人不能用账号名兜底；AI不回填送样提交时间。");
        if allowed("sample-info:collect") {
            add(
                "样品信息取样",
                "/sample-info",
                "进入检测类型→找到待取样记录→核对后点击取样；记录进入原待检测流程。",
                "按记录状态和原范围复核；AI不能操作取样或确认检测完成。",
            );
        }
        if allowed("sample-info:return") || allowed("sample-info:return-confirm") {
            add("样品信息退回处理", "/sample-info", "进入原检测类型查看退回原因；有退回资格者填写原因退回，有确认资格的提交人确认后在待修改记录中修改并重新提交。",
                "各操作分别要求原资格及记录状态；旧记录与来源保留，AI草稿不能代替退回修改接口。");
        }
        if allowed("sample-info:record-workload") {
            add("样品信息来源工作量", "/sample-info", "从原记录点击录入工作量→按关键词搜索实际计量项目和方法→核对人员、数量、业务时间和倍率→原确认。",
                "自由填写的送样描述不等于主数据身份，必须选择真实有效方法；来源容量和计量快照沿原规则。");
        }
    }
    if allowed("entry:workload") {
        add("手动检测工作量", "/workload", "选择可用实验室及原项目方法行→需要时按关键词改选实际计量方法→填写数量、业务时间和倍率→原录入按钮。",
            "方法配置提供系数、金额倍率与单价；历史记录沿保存快照，不因主数据调整自动重算；账号操作人与实际检测人分别保留。");
    }
    if allowed("entry:auxiliary") || allowed("records:auxiliary:manage") {
        add("独立辅助工作", "/auxiliary", "选择原允许的实际人员和业务日期→区域→辅助事项→填写数量、备注→原录入按钮；编辑删除按原记录权限操作。",
            "区域和事项独立配置，人员沿账号归属；辅助不分摊实验室或项目，不计算金额；历史镜像与独立记录按原逻辑去重。");
    }
    if is_admin && allowed("manage:auxiliary") {
        add(
            "辅助工作配置",
            "/manage/auxiliary",
            "打开配置页的区域管理维护区域，再逐项维护名称、所属区域、系数、启停和排序。",
            "辅助配置不复用检测类型；系统识别字段用于迁移和回导，修改事项应保留原身份。",
        );
    }
    for (perm, name, path) in [
        ("stats:portal:workload", "分析检测统计", "/stats"),
        ("stats:portal:rd", "研发送样统计", "/sample/stats"),
        (
            "stats:portal:sample-info",
            "样品信息统计",
            "/sample-info/stats",
        ),
    ] {
        if allowed(perm) {
            add(name, path, "先核对业务日期和人员范围→按原筛选查看图表、汇总与明细；导出使用原页面入口。",
                "服务端按完整授权集合统计，非当前页求和；本人、范围和导出资格分别沿原权限，辅助不计金额和实验室分摊。");
        }
    }
    add("个人通知", "/notifications", "从头像旁通知入口或个人通知页查看未读→点击消息跳到有权访问的原记录；可按页面按钮标记已读。",
        "取样会使对应送样提醒自动已读；已读不表示样品检测完成，外部群消息不会随站内已读被自动撤回。");
    for (perm, name, path, steps, rule) in [
        ("manage:methods", "检测方法配置", "/manage/methods", "维护方法的名称、仪器绑定、系数、金额倍率及其他现有字段；实际变化以后续原录入校验为准。", "同名不等于同一方法；稳定方法ID识别原方法，历史快照不会被配置保存自动重算。"),
        ("manage:master-import", "主数据导入导出", "/manage/master-import", "导出当前主数据作基础→保留现有方法和辅助事项识别字段→修改→先预览差异及阻止项→确认后按所选模式导入。", "新增条目识别字段留空由系统生成；跳过已有不会修改旧系数；整批覆盖可能涉及缺项删除，必须核对引用和预览；导入不重算历史。"),
        ("manage:backup", "数据备份", "/manage/backup", "核对数据库或全量模式→立即备份→检查备份结果并下载；云端接入和自动备份在原备份设置维护；恢复先在原专用入口核对文件与版本。", "全量与数据库备份范围不同；恢复属于原受控操作，AI不备份、不恢复；AI私密服务配置未纳入业务备份，换主机须另行配置。"),
        ("manage:roles", "角色权限管理", "/manage/roles", "在角色页核对角色用途、业务身份、权限点和数据范围；按原可用操作创建或修改模板及角色，再由用户管理关联账号。", "实际授权以服务端权限与范围为准，不能按角色中文名推断；使用AI资格与原业务权限取交集，普通角色不因此获得密钥管理。"),
        ("manage:users", "用户与人员归属", "/manage/users", "维护账号启停、归属、可工作范围和所关联角色；公共用途与实际业务人员沿原角色配置和选择规则处理。", "账号操作人、实际送样人和实际检测人是不同身份；修改账号角色不会授权AI越过原范围。"),
        ("manage:notifications", "通知配置", "/manage/notifications", "在原通知中心配置事件、模板、渠道和目标人员；核对字段与渠道结果。", "AI仅解释操作，不发群消息、不修改模板；钉钉精确@依赖原账号绑定和渠道配置。"),
        ("manage:audit", "审计日志", "/manage/audit", "按模块、操作、人员、业务编号和日期查询→打开原审计详情追溯；AI调用与真实业务提交分别记录。", "AI审计不默认记录密钥、原始录音及完整对话正文；AI工具没有创建业务记录的能力。"),
        ("manage:log-maintenance", "日志与维护", "/manage/log-maintenance", "在原日志维护页核对留存、归档及维护设置，执行前阅读原页面的影响和确认提示。", "维护与归档保留原授权和并发限制；AI不执行数据库命令或清理操作。"),
    ] {
        if allowed(perm) { add(name, path, steps, rule); }
    }
    if allowed("help:view") {
        add("程序帮助", "/help", "打开现有操作文档，结合当前页面字段和错误提示核对；内置说明未覆盖的问题先查原帮助或联系管理员。",
            "内置指引按本版本维护，不代表模型了解任何未来新增功能，也不自动读取附件或全库配置。");
    }
    if is_admin {
        add("AI服务配置", "/manage/ai", "配置模型服务→获取或手填模型→保存→分别进行hi、流式和无副作用工具测试→启用能力；语音服务单独配置。",
            "地址可达不等于模型可用；模型需通过hi才可对话、工具测试才可业务辅助；密钥仅服务端维护，不隐式切换供应商。");
    }
    guides
}

pub async fn execute_tool(
    pool: DbPool,
    headers: HeaderMap,
    name: &str,
    args: Value,
    caps: &AiCapabilities,
) -> ToolResult {
    let ctx = authz_service::authenticate(&pool, &headers).map_err(|e| e.public_message())?;
    permission(&ctx, "ai:use")?;
    if !tool_definitions(&ctx, caps)
        .iter()
        .any(|tool| tool["function"]["name"].as_str() == Some(name))
    {
        return Err("当前账号或AI配置未开放此能力".into());
    }
    match name {
        "program_help" => {
            validate_args(&args, &["topic"])?;
            let targets = ["sample_info", "rd_sample", "workload", "auxiliary"]
                .into_iter()
                .filter(|target| require_target(&ctx, target).is_ok())
                .collect::<Vec<_>>();
            Ok(
                json!({"version":env!("CARGO_PKG_VERSION"),"topic":text(&args,"topic"),
                "knowledge_kind":"本版本内置操作指引，非任意未来功能或私密配置的自动理解",
                "workflows":program_guides(|p| ctx.has_permission(p),ctx.user.is_admin),
                "available_entry_targets":targets,
                "data_notice":"指引仅提供原权限可达入口，最终按钮还受范围和状态限制。未调用业务查询工具不能声称已查看真实业务；字段、系数和主数据以entry_metadata及原表单为准。AI草稿只带入原表单，必须复核后按原按钮提交。"}),
            )
        }
        "query_records" => records(&pool, &headers, &args).await,
        "entry_metadata" => {
            validate_args(
                &args,
                &["target", "group_id", "project_id", "type_key", "keyword"],
            )?;
            metadata(&pool, &headers, &ctx, &args).await
        }
        "prepare_entry_draft" => prepare(&pool, &headers, &ctx, &args).await,
        "workload_summary" => {
            validate_args(
                &args,
                &[
                    "start",
                    "end",
                    "view",
                    "subject_user_id",
                    "group_id",
                    "project_id",
                    "method_id",
                    "kind",
                ],
            )?;
            require_dates(&args)?;
            let mut query = args.clone();
            query["page_size"] = json!(1);
            query["include_people"] = json!(true);
            let pairs = [
                ("start", "start"),
                ("end", "end"),
                ("view", "view"),
                ("subject_user_id", "subject_user_id"),
                ("group_id", "group_id"),
                ("project_id", "project_id"),
                ("method_id", "method_id"),
                ("kind", "kind"),
                ("page_size", "page_size"),
                ("include_people", "include_people"),
            ];
            let report = get(
                api::workload_report_handler::router(pool),
                &headers,
                query_uri("/api/stats/workload-report", &query, &pairs),
            )
            .await?;
            let mut output = keep_fields(
                &report,
                &[
                    "scope",
                    "summary",
                    "people",
                    "matrix",
                    "breakdowns",
                    "trend",
                ],
            );
            output["filters"] = args;
            output["notice"]=json!("使用原统计全量授权集合及历史辅助工作去重，非当前页求和；使用已确认日期，默认本人。辅助工作不计金额。");
            Ok(output)
        }
        _ => Err("未知工具".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn program_guides_filter_entries_and_actions_by_actual_permissions() {
        let guides = program_guides(|p| matches!(p, "entry:sample" | "sample:collect"), false);
        let names: Vec<_> = guides.iter().filter_map(|v| v["name"].as_str()).collect();
        assert!(names.contains(&"研发送样"));
        assert!(names.contains(&"研发送样取样"));
        assert!(names.contains(&"个人通知"));
        assert!(!names.contains(&"研发送样驳回"));
        assert!(!names.contains(&"样品信息登记"));
        assert!(guides
            .iter()
            .all(|v| !v["path"].as_str().unwrap().starts_with("/manage")));
    }

    #[test]
    fn program_guides_cover_authorized_management_without_private_configuration() {
        let guides = program_guides(|_| true, true);
        for path in [
            "/manage/master-import",
            "/manage/backup",
            "/manage/roles",
            "/manage/audit",
            "/manage/ai",
            "/manage/auxiliary",
        ] {
            assert!(guides.iter().any(|v| v["path"] == path), "missing {path}");
        }
        for guide in &guides {
            let path = guide["path"].as_str().unwrap();
            assert!(path.starts_with('/') && !path.starts_with("//") && !path.contains("http"));
            assert!(guide["basis"]
                .as_str()
                .unwrap()
                .contains(env!("CARGO_PKG_VERSION")));
            assert!(!guide["steps"].as_str().unwrap().is_empty());
            assert!(!guide["rule"].as_str().unwrap().is_empty());
        }
        let ordinary = program_guides(|p| p == "manage:backup", false);
        assert!(ordinary.iter().any(|v| v["path"] == "/manage/backup"));
        assert!(!ordinary
            .iter()
            .any(|v| v["path"] == "/manage/roles" || v["path"] == "/manage/ai"));
        assert!(guides
            .iter()
            .any(|v| v["name"] == "独立辅助工作"
                && v["rule"].as_str().unwrap().contains("不计算金额")));
    }

    #[test]
    fn tools_reject_write_fields_and_encode_queries() {
        assert!(validate_args(&json!({"coefficient":28}), &["quantity"]).is_err());
        assert!(validate_args(&json!({"group_id":-1}), &["group_id"]).is_err());
        let uri = query_uri(
            "/api/sample-info",
            &json!({"keyword":"a&scope=statistics"}),
            &[("keyword", "keyword")],
        );
        assert!(!uri.contains("&scope="));
        assert!(uri.starts_with("/api/sample-info?keyword="));
        let selected = keep_fields(
            &json!({"id":1,"password":"secret","notes":"ok"}),
            &["id", "notes"],
        );
        assert!(selected.get("password").is_none());
    }
}
