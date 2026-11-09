use super::*;
use crate::models::notification::{
    DeliveryQuery, InboxQuery, NotificationChannelInput, NotificationRuleInput,
};

fn test_pool() -> DbPool {
    let pool = crate::db::init_pool("notification-integrity-tests");
    crate::db::test_migrations::run(&pool.get().unwrap()).unwrap();
    pool
}

fn new_user(pool: &DbPool, name: &str) -> i64 {
    let conn = pool.get().unwrap();
    conn.execute(
        "INSERT INTO users(username,password,is_active) VALUES(?1,'unused',1)",
        [name],
    )
    .unwrap();
    conn.last_insert_rowid()
}

fn sample(pool: &DbPool, sender: &str, creator: &str, batch: &str) -> SampleInfoResponse {
    let body = serde_json::from_value(serde_json::json!({
        "batch_no":batch,"user_name":sender,"lab_name":"integrity-lab","project_name":"integrity-project",
        "main_components":"component","detection_type":"ICP","type_key":"icp","quantity":1
    })).unwrap();
    crate::repo::sample_info_repo::create(pool, &body, creator).unwrap()
}

fn rule_input(value: serde_json::Value) -> NotificationRuleInput {
    serde_json::from_value(value).unwrap()
}

#[test]
fn inbox_pagination_finds_old_messages_and_keeps_unread_search_and_owner_scopes() {
    let pool = test_pool();
    let owner = new_user(&pool, "inbox-owner");
    let other = new_user(&pool, "inbox-other");
    let conn = pool.get().unwrap();
    let mut first_event = 0;
    for index in 1..=125 {
        let record = sample(&pool, "inbox-owner", "inbox-owner", &format!("B-{index}"));
        let event: i64 = conn.query_row("SELECT id FROM notification_events WHERE resource_type='sample_info' AND resource_id=?1", [record.id], |row|row.get(0)).unwrap();
        if index == 1 {
            first_event = event;
        }
        notification_repo::create_in_app(
            &pool,
            event,
            &[owner],
            if index == 1 {
                "needle oldest message"
            } else {
                "ordinary message"
            },
            "private owner content",
            "",
        )
        .unwrap();
        if index % 2 == 0 {
            conn.execute(
                "UPDATE in_app_notifications SET is_read=1 WHERE event_id=?1 AND user_id=?2",
                postgres_compat::params![event, owner],
            )
            .unwrap();
        }
    }
    notification_repo::create_in_app(
        &pool,
        first_event,
        &[other],
        "needle other account",
        "other private content",
        "",
    )
    .unwrap();
    assert_eq!(notification_repo::inbox(&pool, owner).unwrap().len(), 100);
    let first = notification_repo::inbox_page(&pool, owner, &InboxQuery::default()).unwrap();
    assert_eq!(
        (first.total, first.unread_count, first.items.len()),
        (125, 63, 50)
    );
    let oldest = notification_repo::inbox_page(
        &pool,
        owner,
        &InboxQuery {
            page: Some(3),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(oldest.items.len(), 25);
    assert!(oldest
        .items
        .iter()
        .any(|item| item.title == "needle oldest message"));
    let selected = notification_repo::inbox_page(
        &pool,
        owner,
        &InboxQuery {
            only_unread: Some(true),
            keyword: Some("needle".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        (selected.total, selected.unread_count, selected.items.len()),
        (1, 63, 1)
    );
    assert_eq!(selected.items[0].title, "needle oldest message");
    assert!(!selected.items[0].is_read);
    let peer = notification_repo::inbox_page(&pool, other, &InboxQuery::default()).unwrap();
    assert_eq!(peer.total, 1);
    assert_eq!(peer.items[0].body, "other private content");
    assert!(notification_repo::inbox_page(
        &pool,
        owner,
        &InboxQuery {
            keyword: Some("x".repeat(101)),
            ..Default::default()
        }
    )
    .is_err());
}

#[test]
fn delivery_pages_filter_before_pagination_and_redact_legacy_errors() {
    let pool = test_pool();
    let conn = pool.get().unwrap();
    let mut channels = Vec::new();
    for index in 0..3 {
        channels.push(notification_repo::create_channel(&pool,&serde_json::from_value::<NotificationChannelInput>(json!({
            "name":format!("delivery-channel-{index}"),"webhook_url":"https://example.invalid/robot"
        })).unwrap()).unwrap());
    }
    let mut events = Vec::new();
    for index in 0..2 {
        let source = sample(&pool, "admin", "admin", &format!("delivery-{index}"));
        let event:i64=conn.query_row("SELECT id FROM notification_events WHERE resource_type='sample_info' AND resource_id=?1",[source.id],|row|row.get(0)).unwrap();
        events.push(event);
    }
    for (event, dates) in [
        (
            events[0],
            [
                "2026-11-01 23:59:59",
                "2026-11-02 00:00:00",
                "2026-11-02T23:59:59",
            ],
        ),
        (
            events[1],
            [
                "2026-11-02 00:00:00",
                "2026-11-03 00:00:00",
                "2026-11-02 12:00:00",
            ],
        ),
    ] {
        for (channel, date) in channels.iter().zip(dates) {
            conn.execute("INSERT INTO notification_deliveries(event_id,channel_id,status,attempts,response_summary,last_error,created_at) VALUES(?1,?2,'failed',3,'secret=legacy-response-private','https://example.invalid/robot?access_token=legacy-token-private&sign=legacy-sign-private',?3)",postgres_compat::params![event,channel,date]).unwrap();
        }
    }
    let mut query = DeliveryQuery {
        page: Some(1),
        page_size: Some(1),
        event_id: Some(events[0]),
        start_date: Some("2026-11-02".into()),
        end_date: Some("2026-11-02".into()),
    };
    let first = notification_repo::deliveries_page(&pool, &query).unwrap();
    assert_eq!((first.total, first.items.len()), (2, 1));
    query.page = Some(2);
    let second = notification_repo::deliveries_page(&pool, &query).unwrap();
    assert_ne!(first.items[0].id, second.items[0].id);
    for item in first.items.iter().chain(second.items.iter()) {
        assert_eq!(item.event_id, events[0]);
        assert!(item.created_at.starts_with("2026-11-02"));
        assert!(!item.response_summary.contains("legacy-response-private"));
        assert!(!item.last_error.contains("legacy-token-private"));
        assert!(!item.last_error.contains("legacy-sign-private"));
    }
    query.event_id = None;
    assert_eq!(
        notification_repo::deliveries_page(&pool, &query)
            .unwrap()
            .total,
        4
    );
    query.start_date = Some("2026-02-30".into());
    assert!(notification_repo::deliveries_page(&pool, &query).is_err());
    query.start_date = Some("2026-11-03".into());
    assert!(notification_repo::deliveries_page(&pool, &query).is_err());
    query.start_date = None;
    query.end_date = None;
    query.event_id = Some(-1);
    assert!(notification_repo::deliveries_page(&pool, &query).is_err());
}

#[test]
fn matching_rules_use_target_type_and_all_lab_scopes_instead_of_legacy_columns() {
    let pool = test_pool();
    let conn = pool.get().unwrap();
    conn.execute(
        "INSERT INTO project_groups(name) VALUES('priority-lab-a')",
        [],
    )
    .unwrap();
    let lab_a = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO project_groups(name) VALUES('priority-lab-b')",
        [],
    )
    .unwrap();
    let lab_b = conn.last_insert_rowid();
    let broad = notification_repo::create_rule(
        &pool,
        &rule_input(json!({"name":"broad","targets":[{"target_kind":"sample_info"}]})),
    )
    .unwrap();
    let lab=notification_repo::create_rule(&pool,&rule_input(json!({"name":"lab fallback","group_ids":[lab_a],"targets":[{"target_kind":"sample_info"}]}))).unwrap();
    let typed=notification_repo::create_rule(&pool,&rule_input(json!({"name":"typed multiple labs","group_ids":[lab_a,lab_b],"targets":[{"target_kind":"sample_info","sample_info_type_key":"icp"}]}))).unwrap();
    let specific=notification_repo::create_rule(&pool,&rule_input(json!({"name":"typed project","group_ids":[lab_a],"project_name":"P1","targets":[{"target_kind":"sample_info","sample_info_type_key":"icp"}]}))).unwrap();
    let same_priority=notification_repo::create_rule(&pool,&rule_input(json!({"name":"typed project second channel","group_ids":[lab_a],"project_name":"P1","targets":[{"target_kind":"sample_info","sample_info_type_key":"icp"}]}))).unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT sample_info_type_key FROM notification_rules WHERE id=?1",
            [typed],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        ""
    );
    let matches = |group, project, type_key| {
        notification_repo::matching_rules(
            &pool,
            EVENT_SAMPLE_SUBMITTED,
            "sample_info",
            group,
            project,
            "ICP",
            "",
            type_key,
        )
        .unwrap()
        .into_iter()
        .map(|rule| rule.id)
        .collect::<Vec<_>>()
    };
    assert_eq!(
        matches(Some(lab_a), "P1", "icp"),
        vec![specific, same_priority]
    );
    assert_eq!(matches(Some(lab_b), "P2", "icp"), vec![typed]);
    assert_eq!(matches(Some(lab_a), "P1", "other-type"), vec![lab]);
    assert_eq!(matches(None, "P2", "other-type"), vec![broad]);
}

#[test]
fn rejection_without_rules_reaches_stable_owners_but_not_unrelated_disabled_or_public_accounts() {
    let pool = test_pool();
    let conn = pool.get().unwrap();
    let sender = new_user(&pool, "rejection-sender");
    let creator = new_user(&pool, "rejection-creator");
    let unrelated = new_user(&pool, "rejection-unrelated");
    let disabled = new_user(&pool, "rejection-disabled");
    let public_rd = new_user(&pool, "rejection-public-rd");
    let public_analysis = new_user(&pool, "rejection-public-analysis");
    conn.execute(
        "INSERT INTO roles(name,is_system) VALUES('rejection-own-sender',0)",
        [],
    )
    .unwrap();
    let role = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO role_permissions(role_id,permission_key) VALUES(?1,'entry:sample')",
        [role],
    )
    .unwrap();
    for person in [sender, creator, unrelated, disabled] {
        conn.execute(
            "INSERT INTO user_roles(user_id,role_id) VALUES(?1,?2)",
            postgres_compat::params![person, role],
        )
        .unwrap();
    }
    conn.execute("UPDATE users SET is_active=0 WHERE id=?1", [disabled])
        .unwrap();
    for (person, key) in [
        (public_rd, "rd_public_account"),
        (public_analysis, "analysis_public_account"),
    ] {
        conn.execute(
            "INSERT INTO user_roles(user_id,role_id) SELECT ?1,id FROM roles WHERE system_key=?2",
            postgres_compat::params![person, key],
        )
        .unwrap();
    }
    conn.execute(
        "INSERT INTO project_groups(name) VALUES('rejection-source-lab')",
        [],
    )
    .unwrap();
    let group = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO projects(name,group_id) VALUES('rejection-source-project',?1)",
        [group],
    )
    .unwrap();
    let project = conn.last_insert_rowid();
    let body=serde_json::from_value(json!({"project_id":project,"user_name":"rejection-sender","sender_user_id":sender,"quantity":1,"recorded_at":"2026-11-02T10:00:00"})).unwrap();
    let rd =
        crate::repo::rd_record_repo::create(&pool, &body, None, None, creator, "rejection-creator")
            .unwrap();
    let info = sample(&pool, "rejection-sender", "rejection-creator", "rejection");
    conn.execute(
        "UPDATE users SET username='rejection-renamed-sender' WHERE id=?1",
        [sender],
    )
    .unwrap();
    let reason = format!(
        "第一行：批号不完整\n{}\n最后一行：请核对后重新提交",
        "必须保持完整的原因。".repeat(60)
    );
    crate::repo::rd_record_repo::return_record(&pool, rd.id, &reason, "admin").unwrap();
    crate::repo::sample_info_repo::return_record(&pool, info.id, &reason, "admin").unwrap();
    assert!(notification_repo::rejection_recipient_ids(
        &pool,
        "rd_work_record_rejected",
        rd.id,
        Some(&[unrelated, disabled, public_rd, public_analysis])
    )
    .unwrap()
    .is_empty());
    assert!(notification_repo::rejection_recipient_ids(
        &pool,
        "sample_info_rejected",
        info.id,
        Some(&[unrelated, disabled, public_rd, public_analysis])
    )
    .unwrap()
    .is_empty());
    let mut sent = 0;
    assert_eq!(
        process_pending_with_sender(&pool, 20, &mut |_, _, _, _, _| {
            sent += 1;
            Ok("ok".into())
        })
        .unwrap(),
        4
    );
    assert_eq!(sent, 0);
    let recipients:Vec<(String,i64)>=conn.prepare("SELECT e.resource_type,n.user_id FROM in_app_notifications n JOIN notification_events e ON e.id=n.event_id ORDER BY e.resource_type,n.user_id").unwrap().query_map([],|row|Ok((row.get(0)?,row.get(1)?))).unwrap().collect::<postgres_compat::Result<_>>().unwrap();
    assert_eq!(recipients.len(), 4);
    for source in ["rd_work_record_rejected", "sample_info_rejected"] {
        assert!(recipients.contains(&(source.into(), sender)));
        assert!(recipients.contains(&(source.into(), creator)));
    }
    let bodies: Vec<String> = conn
        .prepare("SELECT body FROM in_app_notifications ORDER BY id")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<postgres_compat::Result<_>>()
        .unwrap();
    assert!(
        bodies.iter().all(|body| body.contains(&reason)),
        "站内驳回原因不得截断或遗漏"
    );

    let bad_leader = new_user(&pool, "rejection-leader-without-lab");
    conn.execute(
        "INSERT INTO roles(name,is_system) VALUES('rejection-leader-without-lab',0)",
        [],
    )
    .unwrap();
    let leader_role = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO role_permissions(role_id,permission_key) VALUES(?1,'records:rd:view-lab')",
        [leader_role],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO user_roles(user_id,role_id) VALUES(?1,?2)",
        postgres_compat::params![bad_leader, leader_role],
    )
    .unwrap();
    assert_eq!(
        notification_repo::rejection_recipient_ids(
            &pool,
            "rd_work_record_rejected",
            rd.id,
            Some(&[bad_leader, sender])
        )
        .unwrap(),
        vec![sender]
    );
    notification_repo::create_rule(&pool,&rule_input(json!({"name":"skip invalid leader candidate","targets":[{"target_kind":"rd_work_record_rejected"}],"recipient_user_ids":[bad_leader,sender]}))).unwrap();
    let second_rd =
        crate::repo::rd_record_repo::create(&pool, &body, None, None, creator, "rejection-creator")
            .unwrap();
    crate::repo::rd_record_repo::return_record(&pool, second_rd.id, &reason, "admin").unwrap();
    assert_eq!(
        process_pending_with_sender(&pool, 20, &mut |_, _, _, _, _| panic!(
            "no group channel configured"
        ))
        .unwrap(),
        2
    );
    let second_recipients:Vec<i64>=conn.prepare("SELECT n.user_id FROM in_app_notifications n JOIN notification_events e ON e.id=n.event_id WHERE e.resource_type='rd_work_record_rejected' AND e.resource_id=?1 ORDER BY n.user_id").unwrap().query_map([second_rd.id],|row|row.get(0)).unwrap().collect::<postgres_compat::Result<_>>().unwrap();
    assert_eq!(second_recipients.len(), 2);
    assert!(second_recipients.contains(&sender));
    assert!(second_recipients.contains(&creator));
    assert!(!second_recipients.contains(&bad_leader));
    conn.execute(
        "UPDATE rd_work_records SET subject_user_id=?1,created_by_user_id=?2 WHERE id=?3",
        postgres_compat::params![disabled, public_rd, rd.id],
    )
    .unwrap();
    conn.execute(
        "UPDATE sample_info_records SET business_user_id=?1,created_by_user_id=?2 WHERE id=?3",
        postgres_compat::params![disabled, public_analysis, info.id],
    )
    .unwrap();
    assert!(notification_repo::rejection_recipient_ids(
        &pool,
        "rd_work_record_rejected",
        rd.id,
        None
    )
    .unwrap()
    .is_empty());
    assert!(notification_repo::rejection_recipient_ids(
        &pool,
        "sample_info_rejected",
        info.id,
        None
    )
    .unwrap()
    .is_empty());
}

#[test]
fn deleted_returned_or_sampled_sources_cancel_pending_group_notifications() {
    let pool = test_pool();
    let conn = pool.get().unwrap();
    let admin: i64 = conn
        .query_row("SELECT id FROM users WHERE username='admin'", [], |row| {
            row.get(0)
        })
        .unwrap();
    let channel = notification_repo::create_channel(
        &pool,
        &serde_json::from_value(
            json!({"name":"never send stale","webhook_url":"https://example.invalid/robot"}),
        )
        .unwrap(),
    )
    .unwrap();
    notification_repo::create_rule(&pool,&rule_input(json!({"name":"all sources","channel_id":channel,"targets":[{"target_kind":"sample_info"},{"target_kind":"rd_work_record"}]}))).unwrap();
    conn.execute(
        "INSERT INTO project_groups(name) VALUES('stale-source-lab')",
        [],
    )
    .unwrap();
    let group = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO projects(name,group_id) VALUES('stale-source-project',?1)",
        [group],
    )
    .unwrap();
    let project = conn.last_insert_rowid();
    for (index, state) in ["deleted", "returned", "sampled"].into_iter().enumerate() {
        let info = sample(&pool, "admin", "admin", &format!("stale-{index}"));
        let body=serde_json::from_value(json!({"project_id":project,"user_name":"admin","sender_user_id":admin,"quantity":1,"recorded_at":"2026-11-02T11:00:00"})).unwrap();
        let rd =
            crate::repo::rd_record_repo::create(&pool, &body, None, None, admin, "admin").unwrap();
        for (table, id) in [("sample_info_records", info.id), ("rd_work_records", rd.id)] {
            let assignment = match state {
                "deleted" => "deleted_at='2026-11-02 12:00:00'",
                "returned" => "status='已退回'",
                _ => "sampled_at='2026-11-02 12:00:00'",
            };
            conn.execute(
                &format!("UPDATE {table} SET {assignment} WHERE id=?1"),
                [id],
            )
            .unwrap();
        }
    }
    let mut sends = 0;
    assert_eq!(
        process_pending_with_sender(&pool, 20, &mut |_, _, _, _, _| {
            sends += 1;
            Ok("ok".into())
        })
        .unwrap(),
        6
    );
    assert_eq!(sends, 0);
    assert_eq!(conn.query_row("SELECT COUNT(*) FROM notification_events WHERE status='cancelled' AND status_reason<>''",[],|row|row.get::<_,i64>(0)).unwrap(),6);
    assert_eq!(conn.query_row("SELECT (SELECT COUNT(*) FROM notification_deliveries)+(SELECT COUNT(*) FROM in_app_notifications)",[],|row|row.get::<_,i64>(0)).unwrap(),0);
}

#[test]
fn a_matching_rule_with_no_authorized_recipient_or_channel_is_skipped() {
    let pool = test_pool();
    let ordinary = new_user(&pool, "no-analysis-permission");
    sample(
        &pool,
        "no-analysis-permission",
        "no-analysis-permission",
        "skip-empty",
    );
    notification_repo::create_rule(&pool,&rule_input(json!({"name":"empty effective audience","recipient_user_ids":[ordinary],"targets":[{"target_kind":"sample_info"}]}))).unwrap();
    let mut sends = 0;
    assert_eq!(
        process_pending_with_sender(&pool, 1, &mut |_, _, _, _, _| {
            sends += 1;
            Ok("ok".into())
        })
        .unwrap(),
        1
    );
    assert_eq!(sends, 0);
    let conn = pool.get().unwrap();
    let state: (String, String) = conn
        .query_row(
            "SELECT status,status_reason FROM notification_events",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(state.0, "skipped");
    assert!(state.1.contains("没有有效"));
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM in_app_notifications", [], |row| row
            .get::<_, i64>(
            0
        ))
        .unwrap(),
        0
    );
}

#[test]
fn permanent_processing_errors_stop_after_three_failures_and_explicit_retry_resets_them() {
    let pool = test_pool();
    let conn = pool.get().unwrap();
    let admin: i64 = conn
        .query_row("SELECT id FROM users WHERE username='admin'", [], |row| {
            row.get(0)
        })
        .unwrap();
    let source = sample(&pool, "admin", "admin", "processing-error");
    let event:i64=conn.query_row("SELECT id FROM notification_events WHERE resource_type='sample_info' AND resource_id=?1",[source.id],|row|row.get(0)).unwrap();
    notification_repo::create_rule(&pool,&rule_input(json!({"name":"error handling","recipient_user_ids":[admin],"targets":[{"target_kind":"sample_info","sample_info_type_key":"icp"}]}))).unwrap();
    notification_repo::save_template(&pool, "sample_info:icp", "template", "", &json!([])).unwrap();
    conn.execute("UPDATE notification_templates SET field_config_json='invalid-json' WHERE event_key='sample_info:icp'",[]).unwrap();
    let mut sends = 0;
    for attempt in 1..=3 {
        conn.execute(
            "UPDATE notification_events SET next_attempt_at=NULL WHERE id=?1",
            [event],
        )
        .unwrap();
        assert_eq!(
            process_pending_with_sender(&pool, 1, &mut |_, _, _, _, _| {
                sends += 1;
                Ok("ok".into())
            })
            .unwrap(),
            1
        );
        let state:(String,i64,String)=conn.query_row("SELECT status,processing_failures,status_reason FROM notification_events WHERE id=?1",[event],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).unwrap();
        assert_eq!(state.0, if attempt == 3 { "failed" } else { "retry" });
        assert_eq!(state.1, attempt);
        assert!(!state.2.is_empty());
    }
    assert_eq!(
        process_pending_with_sender(&pool, 1, &mut |_, _, _, _, _| {
            sends += 1;
            Ok("ok".into())
        })
        .unwrap(),
        0
    );
    assert_eq!(sends, 0);
    conn.execute("UPDATE notification_templates SET field_config_json='[]' WHERE event_key='sample_info:icp'",[]).unwrap();
    notification_repo::retry_event(&pool, event, "fixed template", admin, "admin").unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT processing_failures FROM notification_events WHERE id=?1",
            [event],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        process_pending_with_sender(&pool, 1, &mut |_, _, _, _, _| {
            sends += 1;
            Ok("ok".into())
        })
        .unwrap(),
        1
    );
    assert_eq!(
        conn.query_row(
            "SELECT status FROM notification_events WHERE id=?1",
            [event],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        "sent"
    );
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM in_app_notifications WHERE event_id=?1 AND user_id=?2",
            postgres_compat::params![event, admin],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    assert!(
        notification_repo::retry_event(&pool, event, "do not duplicate", admin, "admin").is_err()
    );
}

#[test]
fn notification_error_redaction_removes_url_json_and_bearer_credentials() {
    let text = r#"request https://example.invalid/robot?access_token=url-private&sign=signed-private returned {"secret":"secret-private","password":"password-private","refresh_token":"refresh-private","Authorization":"Bearer bearer-private"}"#;
    let redacted = redact_notification_error(text);
    for secret in [
        "url-private",
        "signed-private",
        "secret-private",
        "password-private",
        "refresh-private",
        "bearer-private",
    ] {
        assert!(!redacted.contains(secret), "credential leaked: {secret}");
    }
    assert!(redacted.contains("example.invalid/robot"));
    assert_eq!(
        redact_notification_error("a regular delivery failed"),
        "a regular delivery failed"
    );
}
