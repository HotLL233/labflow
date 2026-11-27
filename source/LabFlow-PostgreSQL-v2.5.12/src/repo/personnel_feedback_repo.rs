use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::personnel_feedback::{
    PersonnelFeedback, PersonnelFeedbackCreate, PersonnelFeedbackUpdate,
};
use crate::repo::{audit_repo, trash_repo};

const SELECT: &str = "SELECT f.id,f.notice_no,f.lab_id,g.name,f.responsible_user_id,COALESCE(f.responsible_name_snapshot,''),f.change_type,f.person_name,f.effective_at,f.notes,f.status,f.created_by_user_id,f.created_by_username,f.created_at,f.updated_at FROM personnel_change_feedbacks f JOIN project_groups g ON g.id=f.lab_id";

fn map_feedback(row: &postgres_compat::Row) -> postgres_compat::Result<PersonnelFeedback> {
    Ok(PersonnelFeedback {
        id: row.get(0)?,
        notice_no: row.get(1)?,
        lab_id: row.get(2)?,
        lab_name: row.get(3)?,
        responsible_user_id: row.get(4)?,
        responsible_name: row.get(5)?,
        change_type: row.get(6)?,
        person_name: row.get(7)?,
        effective_at: row.get(8)?,
        notes: row.get(9)?,
        status: row.get(10)?,
        project_ids: vec![],
        project_names: vec![],
        created_by_user_id: row.get(11)?,
        created_by_username: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

fn hydrate(
    conn: &postgres_compat::Connection,
    mut value: PersonnelFeedback,
) -> Result<PersonnelFeedback> {
    let mut stmt = conn.prepare("SELECT p.id,p.name FROM personnel_change_feedback_projects link JOIN projects p ON p.id=link.project_id WHERE link.feedback_id=?1 ORDER BY p.name,p.id")?;
    let rows = stmt.query_map([value.id], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (id, name) = row?;
        value.project_ids.push(id);
        value.project_names.push(name);
    }
    Ok(value)
}

fn get_on_conn(conn: &postgres_compat::Connection, id: i64) -> Result<PersonnelFeedback> {
    let value = conn
        .query_row(
            &format!("{SELECT} WHERE f.id=?1 AND f.deleted_at IS NULL"),
            [id],
            map_feedback,
        )
        .map_err(|error| match error {
            postgres_compat::Error::QueryReturnedNoRows => AppError::NotFound(
                "\u{4eba}\u{5458}\u{53d8}\u{52a8}\u{53cd}\u{9988}\u{4e0d}\u{5b58}\u{5728}".into(),
            ),
            _ => error.into(),
        })?;
    hydrate(conn, value)
}

fn set_projects(
    conn: &postgres_compat::Connection,
    feedback_id: i64,
    project_ids: &[i64],
) -> Result<()> {
    conn.execute(
        "DELETE FROM personnel_change_feedback_projects WHERE feedback_id=?1",
        [feedback_id],
    )?;
    for project_id in project_ids {
        let active: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1 AND deleted_at IS NULL)",
            [project_id],
            |row| row.get(0),
        )?;
        if !active {
            return Err(AppError::Validation(format!("\u{9879}\u{76ee} {project_id} \u{4e0d}\u{5b58}\u{5728}\u{6216}\u{5df2}\u{5220}\u{9664}")));
        }
        conn.execute("INSERT INTO personnel_change_feedback_projects(feedback_id,project_id) VALUES(?1,?2) ON CONFLICT DO NOTHING", postgres_compat::params![feedback_id, project_id])?;
    }
    Ok(())
}

fn responsible_name(conn: &postgres_compat::Connection, user_id: Option<i64>) -> Result<String> {
    match user_id {
        None => Ok(String::new()),
        Some(id) => conn
            .query_row(
                "SELECT username FROM users WHERE id=?1 AND deleted_at IS NULL",
                [id],
                |row| row.get(0),
            )
            .map_err(|_| {
                AppError::Validation("\u{8d1f}\u{8d23}\u{4eba}\u{4e0d}\u{5b58}\u{5728}".into())
            }),
    }
}

fn valid_type(value: &str) -> bool {
    matches!(
        value,
        "\u{65b0}\u{4eba}\u{5458}\u{52a0}\u{5165}"
            | "\u{4eba}\u{5458}\u{79bb}\u{5f00}"
            | "\u{5c97}\u{4f4d}\u{8c03}\u{6574}"
            | "\u{4fe1}\u{606f}\u{66f4}\u{65b0}"
    )
}

pub fn list(pool: &DbPool, lab_id: Option<i64>, global: bool) -> Result<Vec<PersonnelFeedback>> {
    let conn = pool.get()?;
    let (sql, params): (String, Vec<i64>) = if global {
        (
            format!("{SELECT} WHERE f.deleted_at IS NULL ORDER BY f.created_at DESC,f.id DESC"),
            vec![],
        )
    } else {
        let lab = lab_id.ok_or_else(|| {
            AppError::Forbidden(
                "\u{672a}\u{8bbe}\u{7f6e}\u{6240}\u{5c5e}\u{5b9e}\u{9a8c}\u{5ba4}".into(),
            )
        })?;
        (format!("{SELECT} WHERE f.deleted_at IS NULL AND f.lab_id=?1 ORDER BY f.created_at DESC,f.id DESC"), vec![lab])
    };
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(postgres_compat::params_from_iter(params), map_feedback)?;
    rows.map(|row| hydrate(&conn, row?)).collect()
}

pub fn create(
    pool: &DbPool,
    body: &PersonnelFeedbackCreate,
    actor_id: i64,
    actor: &str,
) -> Result<PersonnelFeedback> {
    if !valid_type(&body.change_type)
        || body.person_name.trim().is_empty()
        || body.effective_at.trim().is_empty()
    {
        return Err(AppError::Validation("\u{8bf7}\u{5b8c}\u{6574}\u{5b8c}\u{4eba}\u{3001}\u{53d8}\u{52a8}\u{7c7b}\u{578b}\u{548c}\u{751f}\u{6548}\u{65f6}\u{95f4}".into()));
    }
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let lab_exists: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM project_groups WHERE id=?1 AND deleted_at IS NULL)",
        [body.lab_id],
        |row| row.get(0),
    )?;
    if !lab_exists {
        return Err(AppError::Validation(
            "\u{5b9e}\u{9a8c}\u{5ba4}\u{4e0d}\u{5b58}\u{5728}".into(),
        ));
    }
    let responsible = responsible_name(&tx, body.responsible_user_id)?;
    tx.execute("INSERT INTO personnel_change_feedbacks(notice_no,lab_id,responsible_user_id,responsible_name_snapshot,change_type,person_name,effective_at,notes,created_by_user_id,created_by_username) VALUES('',?1,?2,?3,?4,?5,?6,?7,?8,?9)", postgres_compat::params![body.lab_id,body.responsible_user_id,responsible,body.change_type.trim(),body.person_name.trim(),body.effective_at.trim(),body.notes.trim(),actor_id,actor])?;
    let id = tx.last_insert_rowid();
    tx.execute(
        "UPDATE personnel_change_feedbacks SET notice_no=CONCAT('HR-', LPAD(CAST(?1 AS TEXT), 6, '0')) WHERE id=?1",
        [id],
    )?;
    set_projects(&tx, id, &body.project_ids)?;
    let record = get_on_conn(&tx, id)?;
    let after = serde_json::to_value(&record).unwrap_or_default();
    audit_repo::log_structured_actor_on_conn(
        &tx,
        "create",
        "personnel_change_feedbacks",
        Some(id),
        actor_id,
        actor,
        "\u{65b0}\u{589e}\u{4eba}\u{5458}\u{53d8}\u{52a8}\u{53cd}\u{9988}",
        "help_feedback",
        &record.notice_no,
        None,
        Some(&after),
        "application",
    )?;
    tx.commit()?;
    Ok(record)
}

pub fn update(
    pool: &DbPool,
    id: i64,
    body: &PersonnelFeedbackUpdate,
    actor_id: i64,
    actor: &str,
) -> Result<PersonnelFeedback> {
    let mut conn = pool.get()?;
    let before_record = get_on_conn(&conn, id)?;
    if before_record.status != "\u{6709}\u{6548}" {
        return Err(AppError::Validation("\u{5df2}\u{64a4}\u{56de}\u{53cd}\u{7684}\u{901a}\u{77e5}\u{4e0d}\u{80fd}\u{7f16}\u{8f91}".into()));
    }
    let before = serde_json::to_value(&before_record).unwrap_or_default();
    let tx = conn.transaction()?;
    if let Some(change_type) = &body.change_type {
        if !valid_type(change_type) {
            return Err(AppError::Validation(
                "\u{65e0}\u{6548}\u{7684}\u{53d8}\u{52a8}\u{7c7b}\u{578b}".into(),
            ));
        }
        tx.execute("UPDATE personnel_change_feedbacks SET change_type=?1,updated_at=to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS') WHERE id=?2",postgres_compat::params![change_type.trim(),id])?;
    }
    if let Some(person_name) = &body.person_name {
        if person_name.trim().is_empty() {
            return Err(AppError::Validation(
                "\u{4eba}\u{5458}\u{59d3}\u{540d}\u{4e0d}\u{80fd}\u{4e3a}\u{7a7a}".into(),
            ));
        }
        tx.execute("UPDATE personnel_change_feedbacks SET person_name=?1,updated_at=to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS') WHERE id=?2",postgres_compat::params![person_name.trim(),id])?;
    }
    if let Some(effective_at) = &body.effective_at {
        if effective_at.trim().is_empty() {
            return Err(AppError::Validation(
                "\u{751f}\u{6548}\u{65f6}\u{95f4}\u{4e0d}\u{80fd}\u{4e3a}\u{7a7a}".into(),
            ));
        }
        tx.execute("UPDATE personnel_change_feedbacks SET effective_at=?1,updated_at=to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS') WHERE id=?2",postgres_compat::params![effective_at.trim(),id])?;
    }
    if let Some(notes) = &body.notes {
        tx.execute("UPDATE personnel_change_feedbacks SET notes=?1,updated_at=to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS') WHERE id=?2",postgres_compat::params![notes.trim(),id])?;
    }
    if let Some(responsible_id) = body.responsible_user_id {
        let name = responsible_name(&tx, responsible_id)?;
        tx.execute("UPDATE personnel_change_feedbacks SET responsible_user_id=?1,responsible_name_snapshot=?2,updated_at=to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS') WHERE id=?3",postgres_compat::params![responsible_id,name,id])?;
    }
    if let Some(project_ids) = &body.project_ids {
        set_projects(&tx, id, project_ids)?;
        tx.execute("UPDATE personnel_change_feedbacks SET updated_at=to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS') WHERE id=?1", [id])?;
    }
    let record = get_on_conn(&tx, id)?;
    let after = serde_json::to_value(&record).unwrap_or_default();
    audit_repo::log_structured_actor_on_conn(
        &tx,
        "update",
        "personnel_change_feedbacks",
        Some(id),
        actor_id,
        actor,
        "\u{4fee}\u{6539}\u{4eba}\u{5458}\u{53d8}\u{52a8}\u{53cd}\u{9988}",
        "help_feedback",
        &record.notice_no,
        Some(&before),
        Some(&after),
        "application",
    )?;
    tx.commit()?;
    Ok(record)
}

pub fn withdraw(pool: &DbPool, id: i64, actor_id: i64, actor: &str) -> Result<PersonnelFeedback> {
    let mut conn = pool.get()?;
    let before_record = get_on_conn(&conn, id)?;
    let before = serde_json::to_value(&before_record).unwrap_or_default();
    let tx = conn.transaction()?;
    tx.execute("UPDATE personnel_change_feedbacks SET status='\u{5df2}\u{64a4}\u{56de}',updated_at=to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS') WHERE id=?1",[id])?;
    let record = get_on_conn(&tx, id)?;
    let after = serde_json::to_value(&record).unwrap_or_default();
    audit_repo::log_structured_actor_on_conn(
        &tx,
        "withdraw",
        "personnel_change_feedbacks",
        Some(id),
        actor_id,
        actor,
        "\u{64a4}\u{56de}\u{4eba}\u{5458}\u{53d8}\u{52a8}\u{53cd}\u{9988}",
        "help_feedback",
        &record.notice_no,
        Some(&before),
        Some(&after),
        "application",
    )?;
    tx.commit()?;
    Ok(record)
}

pub fn delete(pool: &DbPool, id: i64, actor_id: i64, actor: &str, reason: &str) -> Result<()> {
    let mut conn = pool.get()?;
    let record = get_on_conn(&conn, id)?;
    let before = serde_json::to_value(&record).unwrap_or_default();
    let tx = conn.transaction()?;
    tx.execute("UPDATE personnel_change_feedbacks SET deleted_at=to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS'),updated_at=to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS') WHERE id=?1", [id])?;
    trash_repo::move_to_trash_on_conn(
        &tx,
        "人员变动反馈",
        "personnel_change_feedbacks",
        id,
        "feedback",
        "help_feedback",
        &record.notice_no,
        "",
        &before,
        reason,
        actor,
        Some(record.created_by_user_id),
        Some(record.lab_id),
        "撤回或删除不影响用户、角色、主数据和统计",
        true,
    )?;
    let after = serde_json::json!({"deleted_at":"now","data":before});
    audit_repo::log_structured_actor_on_conn(
        &tx,
        "delete",
        "personnel_change_feedbacks",
        Some(id),
        actor_id,
        actor,
        "删除人员变动反馈",
        "help_feedback",
        &record.notice_no,
        Some(&before),
        Some(&after),
        "application",
    )?;
    tx.commit()?;
    Ok(())
}
