use super::*;
use crate::{repo::user_repo, service::auth_service};
use axum::body::{to_bytes, Body};
use serde_json::{json, Value};
use tower::Service;

const ADMIN_PATH: &str = "/api/admin/workload-api-links";
const FIRST_MONTH: &str = "2026-01";

fn fixture() -> (DbPool, String, String) {
    let pool = crate::db::init_pool("postgres-test");
    let conn = pool.get().unwrap();
    crate::db::test_migrations::run(&conn).unwrap();
    conn.execute_batch(
        "INSERT INTO divisions(id,name) VALUES(710001,'API scope A'),(710002,'API scope B');
         INSERT INTO roles(id,name) VALUES(710001,'API test wildcard');
         INSERT INTO role_permissions(role_id,permission_key) VALUES(710001,'*');
         INSERT INTO users(id,username,password,division_id,role_id) VALUES(710001,'api-detector','unused',710001,710001);",
    ).unwrap();
    drop(conn);
    let login = |name| {
        let user = user_repo::find_by_username(&pool, name).unwrap().unwrap();
        auth_service::create_login_response(&pool, user, false, None, None)
            .unwrap()
            .token
    };
    (pool.clone(), login("admin"), login("api-detector"))
}

fn config() -> Value {
    json!({
        "name": "公司运营测试", "kind": "both", "departments": {"mode":"selected", "ids":[710001]},
        "from_month": FIRST_MONTH, "through_month": "2026-03", "expires_on": null, "enabled": true
    })
}

async fn call(
    app: &mut Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let response = app
        .call(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn data(response: (StatusCode, Value)) -> Value {
    assert_eq!(response.0, StatusCode::OK);
    assert_eq!(response.1["code"], 0, "{}", response.1["message"]);
    response.1["data"].clone()
}

#[tokio::test]
async fn admin_only_credentials_are_hashed_and_disable_expiry_rotation_are_enforced() {
    let (pool, admin, wildcard_user) = fixture();
    let mut app = router(pool.clone());
    let manifest = data(
        call(
            &mut app,
            "GET",
            "/api/lab/integration/v1/manifest",
            None,
            Value::Null,
        )
        .await,
    );
    assert_eq!(manifest["workloadRecordsPath"], ENDPOINT);
    let read_path = format!("{ENDPOINT}?month={FIRST_MONTH}");
    for token in [None, Some(admin.as_str())] {
        assert_eq!(
            call(&mut app, "GET", &read_path, token, Value::Null)
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
    }
    let denied = call(&mut app, "POST", ADMIN_PATH, Some(&wildcard_user), config()).await;
    assert_eq!(denied.1["code"], 1003);
    for departments in [
        json!({"mode":"selected", "ids":[]}),
        json!({"mode":"selected", "ids":[719999]}),
    ] {
        let mut invalid = config();
        invalid["departments"] = departments;
        assert_eq!(
            call(&mut app, "POST", ADMIN_PATH, Some(&admin), invalid)
                .await
                .1["code"],
            1001
        );
    }
    let mut invalid = config();
    invalid["from_month"] = json!("2026-13");
    assert_eq!(
        call(&mut app, "POST", ADMIN_PATH, Some(&admin), invalid)
            .await
            .1["code"],
        1001
    );

    let issued = data(call(&mut app, "POST", ADMIN_PATH, Some(&admin), config()).await);
    let token = issued["token"].as_str().unwrap();
    let id = issued["link"]["id"].as_i64().unwrap();
    let detail = format!("{ADMIN_PATH}/{id}");
    let machine_on_management = call(&mut app, "GET", ADMIN_PATH, Some(token), Value::Null)
        .await
        .1;
    assert_ne!(machine_on_management["code"], 0);
    assert!(machine_on_management["data"].is_null());
    let list = data(call(&mut app, "GET", ADMIN_PATH, Some(&admin), Value::Null).await);
    assert_eq!(list["links"].as_array().unwrap().len(), 1);
    assert!(!list.to_string().contains(token) && list["links"][0].get("token").is_none());
    let conn = pool.get().unwrap();
    let stored: String = conn
        .query_row(
            "SELECT token_digest FROM workload_api_links WHERE id=?1",
            [id],
            |row| row.get(0),
        )
        .unwrap();
    assert!(stored != token && stored == digest(token));
    let page = data(call(&mut app, "GET", &read_path, Some(token), Value::Null).await);
    assert_eq!(page["totalCount"], 0);
    assert_eq!(page["items"], json!([]));
    assert_eq!(
        call(&mut app, "POST", &read_path, Some(token), Value::Null)
            .await
            .0,
        StatusCode::METHOD_NOT_ALLOWED
    );

    let rotated = data(
        call(
            &mut app,
            "POST",
            &format!("{detail}/rotate"),
            Some(&admin),
            json!({"revision": issued["link"]["revision"]}),
        )
        .await,
    );
    let new_token = rotated["token"].as_str().unwrap();
    assert!(new_token != token);
    assert_eq!(
        call(&mut app, "GET", &read_path, Some(token), Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let revision = rotated["link"]["revision"].clone();
    let mut disabled = config();
    disabled["enabled"] = json!(false);
    assert_eq!(
        call(
            &mut app,
            "PUT",
            &detail,
            Some(&admin),
            json!({"revision":1,"config":disabled})
        )
        .await
        .1["code"],
        1001
    );
    let disabled = data(
        call(
            &mut app,
            "PUT",
            &detail,
            Some(&admin),
            json!({"revision":revision,"config":disabled}),
        )
        .await,
    );
    assert_eq!(
        call(&mut app, "GET", &read_path, Some(new_token), Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );

    let mut expired = disabled["config"].clone();
    expired["enabled"] = json!(true);
    expired["expires_on"] = json!("2020-01-01");
    conn.execute(
        "UPDATE workload_api_links SET config=?1 WHERE id=?2",
        postgres_compat::params![expired.to_string(), id],
    )
    .unwrap();
    assert_eq!(
        call(&mut app, "GET", &read_path, Some(new_token), Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let audited: String = conn.query_row("SELECT string_agg(coalesce(before_json,'')||coalesce(after_json,''),' ') FROM audit_log WHERE table_name='workload_api_links'", [], |row| row.get(0)).unwrap();
    assert!(!audited.contains(token) && !audited.contains(new_token));
}

#[tokio::test]
async fn delete_requires_admin_and_current_revision_and_revokes_only_that_link() {
    let (pool, admin, wildcard_user) = fixture();
    let conn = pool.get().unwrap();
    conn.execute_batch(
        "INSERT INTO auxiliary_work_records(user_id,user_name_snapshot,auxiliary_work_name_snapshot,quantity,coefficient_snapshot,recorded_at,detection_division_id,status)
           VALUES(710001,'api-detector','first',2,1,'2026-01-05T12:00:00',710001,'有效'),
                 (710001,'api-detector','second',3,1,'2026-01-06T12:00:00',710001,'有效');",
    )
    .unwrap();
    let mut app = router(pool.clone());
    let issued = data(call(&mut app, "POST", ADMIN_PATH, Some(&admin), config()).await);
    let token = issued["token"].as_str().unwrap();
    let id = issued["link"]["id"].as_i64().unwrap();
    let revision = issued["link"]["revision"].as_i64().unwrap();
    let detail = format!("{ADMIN_PATH}/{id}");
    let original_delete = format!("{detail}?revision={revision}");
    let denied = call(
        &mut app,
        "DELETE",
        &original_delete,
        Some(&wildcard_user),
        Value::Null,
    )
    .await;
    assert_eq!(denied.1["code"], 1003);

    let mut renamed = config();
    renamed["name"] = json!("已调整名称的接入");
    let current = data(
        call(
            &mut app,
            "PUT",
            &detail,
            Some(&admin),
            json!({"revision": revision, "config": renamed}),
        )
        .await,
    );
    let stale = call(
        &mut app,
        "DELETE",
        &original_delete,
        Some(&admin),
        Value::Null,
    )
    .await;
    assert_eq!(stale.1["code"], 1001);
    let other = data(call(&mut app, "POST", ADMIN_PATH, Some(&admin), config()).await);
    let other_token = other["token"].as_str().unwrap();
    let first_path = format!("{ENDPOINT}?month={FIRST_MONTH}&limit=1");
    let continuation = |page: Value| {
        assert_eq!(page["totalCount"], 2);
        assert_eq!(page["hasMore"], true);
        format!(
            "{first_path}&snapshot={}&cursor={}",
            page["snapshot"].as_str().unwrap(),
            page["nextCursor"]
        )
    };
    let next_path = continuation(data(
        call(&mut app, "GET", &first_path, Some(token), Value::Null).await,
    ));
    let other_next = continuation(data(
        call(&mut app, "GET", &first_path, Some(other_token), Value::Null).await,
    ));
    let delete_path = format!("{detail}?revision={}", current["revision"]);
    data(call(&mut app, "DELETE", &delete_path, Some(&admin), Value::Null).await);
    let list = data(call(&mut app, "GET", ADMIN_PATH, Some(&admin), Value::Null).await);
    assert_eq!(list["links"], json!([other["link"]]));
    for path in [&first_path, &next_path] {
        assert_eq!(
            call(&mut app, "GET", path, Some(token), Value::Null)
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
    }
    let remaining = data(call(&mut app, "GET", &other_next, Some(other_token), Value::Null).await);
    assert_eq!(remaining["totalCount"], 2);
    assert_eq!(remaining["items"][0]["workload"], 3.0);
    let fresh = data(call(&mut app, "GET", &first_path, Some(other_token), Value::Null).await);
    assert_eq!(fresh["totalCount"], 2);

    let (actor, detail, before, after): (String, String, String, Option<String>) = conn
        .query_row(
            "SELECT operator_username_snapshot,detail,before_json,after_json FROM audit_log
             WHERE table_name='workload_api_links' AND action='delete' AND record_id=?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(actor, "admin");
    assert!(detail.contains("已调整名称的接入"));
    assert!(!detail.contains(token) && !detail.contains(&digest(token)));
    assert_eq!(
        serde_json::from_str::<Value>(&before).unwrap(),
        current["config"]
    );
    assert_eq!(after, None);
}

#[tokio::test]
async fn scoped_month_pages_keep_step_totals_and_auxiliary_deduplication() {
    let (pool, admin, _) = fixture();
    let conn = pool.get().unwrap();
    conn.execute_batch(
        "INSERT INTO project_groups(id,name,division_id) VALUES(710001,'Integration lab',710001);
         INSERT INTO projects(id,group_id,name) VALUES(710001,710001,'Integration project');
         INSERT INTO methods(id,name,method_code,coefficient,amount) VALUES(710001,'Integration method','TEST-WORKLOAD-API',99,123);
         INSERT INTO step_detections(id,method_id,quantity,unit_price,multiplier,created_by_user_id) VALUES(710001,710001,6,123,1,710001);
         INSERT INTO detection_steps(id,detection_id,name_snapshot,coefficient_snapshot,sort_order)
           VALUES(710001,710001,'first',1,0),(710002,710001,'second',2,1),(710003,710001,'third',3,2);
         INSERT INTO detection_quantity_groups(id,detection_id,name,quantity) VALUES(710001,710001,'All samples',6);
         INSERT INTO work_records(id,business_no,project_id,group_id,method_id,user_name,subject_user_id,created_by_user_id,
             quantity,coefficient_snapshot,recorded_at,detection_division_id,ownership_status)
           SELECT ids,'fixture-'||ids,710001,710001,710001,'legacy name',710001,(SELECT id FROM users WHERE username='admin'),
              quantity,coefficient,recorded_at,department,ownership FROM (VALUES
              (711001,6,1,'2026-01-02T12:00:00',710001,'confirmed'),(711002,6,2,'2026-01-03T12:00:00',710001,'confirmed'),
              (711003,6,3,'2026-01-31T23:59:59.500',710001,'confirmed'),(711004,2,3,'2026-01-31T23:59:59.900',710001,'confirmed'),
              (711005,4,2,'2026-01-15T12:00:00',710001,'confirmed'),(711006,99,1,'2026-01-15T12:00:00',710002,'confirmed'),
              (711007,99,1,'2026-01-15T12:00:00',NULL,'confirmed'),(711008,99,1,'2026-01-15T12:00:00',710001,'pending_confirmation'),
              (711009,99,1,'2026-02-01T00:00:00',710001,'confirmed')
           ) AS fixture(ids,quantity,coefficient,recorded_at,department,ownership);
         INSERT INTO step_work_entries(work_record_id,detection_id,group_id,step_id,rework)
           VALUES(711001,710001,710001,710001,FALSE),(711002,710001,710001,710002,FALSE),
                 (711003,710001,710001,710003,FALSE),(711004,710001,710001,710003,TRUE);
         INSERT INTO auxiliary_work_records(id,user_id,user_name_snapshot,auxiliary_work_name_snapshot,quantity,coefficient_snapshot,recorded_at,detection_division_id,status)
           VALUES(711001,710001,'old name','independent zero',2,0,'2026-01-16T12:00:00',710001,'有效'),
                 (711005,710001,'old name','linked mirror',4,2,'2026-01-15T12:00:00',710001,'有效'),
                 (711010,710001,'old name','voided',100,1,'2026-01-15T12:00:00',710001,'作废');
         UPDATE work_records SET source_type='auxiliary_work',source_record_id=711005 WHERE id=711005;",
    ).unwrap();
    let mut app = router(pool.clone());
    let issued = data(call(&mut app, "POST", ADMIN_PATH, Some(&admin), config()).await);
    let token = issued["token"].as_str().unwrap();
    let read_path = format!("{ENDPOINT}?month={FIRST_MONTH}");
    let first = data(
        call(
            &mut app,
            "GET",
            &format!("{read_path}&limit=1"),
            Some(token),
            Value::Null,
        )
        .await,
    );
    assert_eq!(first["totalCount"], 6);
    assert_eq!(
        first["totals"],
        json!({"detectionQuantity":6,"detectionWorkload":42.0,"auxiliaryWorkload":8.0})
    );
    assert_eq!(first["items"][0]["id"], "auxiliary_record:711001");
    assert_eq!(first["items"][0]["workload"], 0.0);
    assert_eq!(first["items"][0]["projectId"], Value::Null);
    let next_path = format!(
        "{read_path}&snapshot={}&cursor={}",
        first["snapshot"].as_str().unwrap(),
        first["nextCursor"]
    );
    let mut outside = config();
    outside["departments"] = json!({"mode":"selected","ids":[710002]});
    let other = data(call(&mut app, "POST", ADMIN_PATH, Some(&admin), outside).await);
    assert_eq!(
        call(
            &mut app,
            "GET",
            &next_path,
            other["token"].as_str(),
            Value::Null
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(
            &mut app,
            "GET",
            &format!("{ENDPOINT}?month=2026-04"),
            Some(token),
            Value::Null
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &mut app,
            "GET",
            &format!("{read_path}&kind=auxiliary"),
            Some(token),
            Value::Null
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );

    conn.execute(
        "UPDATE work_records SET deleted_at='2026-02-01T13:00:00' WHERE id IN(711003,711005)",
        [],
    )
    .unwrap();
    let rest = data(call(&mut app, "GET", &next_path, Some(token), Value::Null).await);
    assert_eq!(rest["totals"], first["totals"]);
    assert_eq!(rest["hasMore"], false);
    let rows = rest["items"].as_array().unwrap();
    assert_eq!(rows.len(), 5);
    assert_eq!(rows[2]["detectionQuantity"], 6);
    assert_eq!(rows[3]["detectionQuantity"], 0);
    assert_eq!(rows[3]["employeeId"], "710001");
    assert_eq!(rows[3]["employeeName"], "api-detector");
    assert!(rows[3].get("amount").is_none() && rows[3].get("createdBy").is_none());
    let fresh = data(call(&mut app, "GET", &read_path, Some(token), Value::Null).await);
    assert_eq!(fresh["totalCount"], 4);
    assert_eq!(
        fresh["totals"],
        json!({"detectionQuantity":0,"detectionWorkload":24.0,"auxiliaryWorkload":0.0})
    );
    let old_page = format!(
        "{read_path}&snapshot={}&cursor=1",
        fresh["snapshot"].as_str().unwrap()
    );
    let mut auxiliary_only = config();
    auxiliary_only["kind"] = json!("auxiliary");
    let path = format!("{ADMIN_PATH}/{}", issued["link"]["id"]);
    data(
        call(
            &mut app,
            "PUT",
            &path,
            Some(&admin),
            json!({"config":auxiliary_only,"revision":issued["link"]["revision"]}),
        )
        .await,
    );
    assert_eq!(
        call(&mut app, "GET", &old_page, Some(token), Value::Null)
            .await
            .0,
        StatusCode::CONFLICT
    );
    let fresh = data(call(&mut app, "GET", &read_path, Some(token), Value::Null).await);
    assert_eq!(fresh["totalCount"], 1);
    assert_eq!(fresh["items"][0]["kind"], "auxiliary");
}
