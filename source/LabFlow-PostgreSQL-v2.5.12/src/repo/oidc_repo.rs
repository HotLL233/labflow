use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::user::User;
use crate::repo::audit_repo;
use postgres_compat::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub(crate) fn new_secret() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

pub(crate) fn invite_digest(secret: &str) -> Result<String> {
    let secret = secret.trim();
    if secret.len() != 64 || !secret.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(AppError::Validation("统一登录邀请码无效".into()));
    }
    Ok(format!("{:x}", Sha256::digest(secret.as_bytes())))
}

pub(crate) fn lock_user(conn: &Connection, user_id: i64) -> Result<()> {
    conn.query_row(
        "SELECT id FROM users WHERE id=?1 FOR UPDATE",
        [user_id],
        |_| Ok(()),
    )
    .optional()?
    .ok_or_else(|| AppError::NotFound("人员不存在".into()))
}

fn require_eligible(conn: &Connection, user_id: i64) -> Result<()> {
    if conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM labflow_oidc_eligible_users WHERE id=?1)",
        [user_id],
        |row| row.get(0),
    )? {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "仅可绑定已启用且已配置角色的个人账号".into(),
        ))
    }
}

pub(crate) fn find_user(conn: &Connection, issuer: &str, subject: &str) -> Result<Option<i64>> {
    Ok(conn.query_row(
        "SELECT b.user_id FROM labflow_oidc_bindings b JOIN labflow_oidc_eligible_users u ON u.id=b.user_id
         WHERE b.issuer=?1 AND b.subject=?2 AND b.confirmed",
        params![issuer, subject], |row| row.get(0),
    ).optional()?)
}

fn binding_error(error: postgres_compat::Error) -> AppError {
    if let postgres_compat::Error::Database(message) = &error {
        if message.contains("oidc_employee_unique") || message.contains("oidc_identity_unique") {
            return AppError::Conflict("工号或中心身份已关联其他人员，请人工核对".into());
        }
    }
    error.into()
}

#[derive(Serialize)]
pub(crate) struct Binding {
    user_id: i64,
    username: String,
    employee_no: String,
    issuer: String,
    subject: Option<String>,
    status: String,
}

pub(crate) fn list(pool: &DbPool) -> Result<Vec<Binding>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT b.user_id,u.username,b.employee_no,b.issuer,b.subject,
                CASE WHEN b.confirmed THEN 'confirmed' WHEN b.subject IS NOT NULL THEN 'pending_review'
                     WHEN b.invite_expires_at>CURRENT_TIMESTAMP THEN 'invited'
                     WHEN b.invite_expires_at IS NOT NULL THEN 'expired' ELSE 'revoked' END
         FROM labflow_oidc_bindings b JOIN users u ON u.id=b.user_id ORDER BY b.user_id",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(Binding {
                user_id: row.get(0)?,
                username: row.get(1)?,
                employee_no: row.get(2)?,
                issuer: row.get(3)?,
                subject: row.get(4)?,
                status: row.get(5)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub(crate) fn invite(
    pool: &DbPool,
    actor: &User,
    id: i64,
    issuer: &str,
    employee_no: &str,
    reason: &str,
) -> Result<String> {
    let employee_no = employee_no.trim().to_ascii_uppercase();
    if employee_no.is_empty()
        || employee_no.len() > 64
        || !employee_no
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
    {
        return Err(AppError::Validation(
            "请填写已核对的工号（字母、数字、短横线、下划线或点）".into(),
        ));
    }
    let reason = audit_reason(reason)?;
    let invite = new_secret();
    let digest = invite_digest(&invite)?;
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    lock_user(&tx, id)?;
    require_eligible(&tx, id)?;
    if tx.execute(
        "INSERT INTO labflow_oidc_bindings (user_id,employee_no,issuer,invite_digest,invite_expires_at)
         VALUES (?1,?2,?3,?4,CURRENT_TIMESTAMP+INTERVAL '24 hours')
         ON CONFLICT (user_id) DO UPDATE SET employee_no=EXCLUDED.employee_no,issuer=EXCLUDED.issuer,
            invite_digest=EXCLUDED.invite_digest,invite_expires_at=EXCLUDED.invite_expires_at
         WHERE labflow_oidc_bindings.subject IS NULL",
        params![id, employee_no, issuer, digest],
    ).map_err(binding_error)? == 0 {
        return Err(AppError::Conflict("已有候选或确认绑定，请核对后先撤销".into()));
    }
    tx.execute("DELETE FROM user_sessions WHERE user_id=?1", [id])?;
    audit(
        &tx,
        "oidc_invite",
        id,
        actor,
        &format!("预选工号 {employee_no}；{reason}"),
    )?;
    tx.commit()?;
    Ok(invite)
}

pub(crate) fn stage(pool: &DbPool, issuer: &str, subject: &str, digest: &str) -> Result<()> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    let id = tx
        .query_row(
            "SELECT user_id FROM labflow_oidc_bindings WHERE invite_digest=?1 AND issuer=?2",
            params![digest, issuer],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .ok_or_else(|| AppError::Forbidden("邀请码无效或已被使用".into()))?;
    lock_user(&tx, id)?;
    require_eligible(&tx, id)?;
    if tx.execute(
        "UPDATE labflow_oidc_bindings SET subject=?1,invite_digest=NULL,invite_expires_at=NULL
         WHERE user_id=?2 AND invite_digest=?3 AND issuer=?4 AND invite_expires_at>CURRENT_TIMESTAMP",
        params![subject, id, digest, issuer],
    ).map_err(binding_error)? != 1 {
        return Err(AppError::Forbidden("邀请码已过期或已被使用".into()));
    }
    tx.execute(
        "INSERT INTO audit_log(action,table_name,record_id,user_name,detail,module,created_at)
         VALUES ('oidc_candidate','labflow_oidc_bindings',?1,'oidc-unbound',?2,'shared',datetime('now','localtime'))",
        params![id, format!("中心候选身份 {issuer} / {subject:?}，尚未确认或签发业务会话")],
    )?;
    tx.commit()?;
    Ok(())
}

pub(crate) fn confirm(
    pool: &DbPool,
    actor: &User,
    id: i64,
    issuer: &str,
    subject: &str,
    reason: &str,
) -> Result<()> {
    let reason = audit_reason(reason)?;
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    lock_user(&tx, id)?;
    require_eligible(&tx, id)?;
    if tx.execute(
        "UPDATE labflow_oidc_bindings SET confirmed=TRUE
         WHERE user_id=?1 AND issuer=?2 AND subject=?3 AND NOT confirmed",
        params![id, issuer, subject],
    )? != 1
    {
        return Err(AppError::Conflict(
            "候选身份不一致或已处理，请重新核对发证方和身份标识".into(),
        ));
    }
    audit(
        &tx,
        "oidc_confirm",
        id,
        actor,
        &format!("确认 {issuer} / {subject:?}；{reason}"),
    )?;
    tx.commit()?;
    Ok(())
}

pub(crate) fn revoke(pool: &DbPool, actor: &User, id: i64, reason: &str) -> Result<()> {
    let reason = audit_reason(reason)?;
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    lock_user(&tx, id)?;
    if tx.execute(
        "UPDATE labflow_oidc_bindings SET subject=NULL,confirmed=FALSE,invite_digest=NULL,invite_expires_at=NULL
         WHERE user_id=?1 AND (subject IS NOT NULL OR invite_digest IS NOT NULL)", [id],
    )? != 1 {
        return Err(AppError::Conflict("未找到可撤销的绑定或邀请".into()));
    }
    tx.execute("DELETE FROM user_sessions WHERE user_id=?1", [id])?;
    audit(&tx, "oidc_revoke", id, actor, reason)?;
    tx.commit()?;
    Ok(())
}

fn audit_reason(reason: &str) -> Result<&str> {
    let reason = reason.trim();
    if reason.is_empty() || reason.len() > 512 {
        return Err(AppError::Validation(
            "请填写核对依据或撤销原因（最多 512 字节）".into(),
        ));
    }
    Ok(reason)
}

fn audit(conn: &Connection, action: &str, id: i64, actor: &User, detail: &str) -> Result<()> {
    audit_repo::log_actor_on_conn(
        conn,
        action,
        "labflow_oidc_bindings",
        Some(id),
        actor.id,
        &actor.username,
        detail,
        "shared",
    )
}
