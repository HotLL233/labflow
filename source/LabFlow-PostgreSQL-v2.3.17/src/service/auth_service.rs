use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::user::{LoginRequest, LoginResponse, User};
use crate::repo::user_repo;
use chrono::{Duration, Local, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const NORMAL_IDLE_MINUTES: i64 = 60;
const PUBLIC_ACCOUNT_IDLE_MINUTES: i64 = 1000;
const KEEP_SIGNED_IN_IDLE_MINUTES: i64 = 30 * 24 * 60;

/// JWT secret key — 生产环境应设置环境变量 JWT_SECRET（否则使用内置默认密钥）
fn jwt_secret() -> String {
    std::env::var("JWT_SECRET").unwrap_or_else(|_| "workload-tool-jwt-secret-v0.4.29".to_string())
}

/// JWT Claims
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: i64, // user id
    pub username: String,
    #[serde(default)]
    pub jti: String, // unique login session id
    pub is_admin: bool,
    #[serde(default)]
    pub role_id: Option<i64>,
    #[serde(default)]
    pub permissions: Vec<String>,
    pub exp: usize, // expiry
    pub iat: usize, // issued at
}

/// 生成 JWT token（同时加载用户所属角色的权限点写入 claims）
pub fn generate_token(pool: &DbPool, user: &User, keep_signed_in: bool) -> Result<String> {
    let now = Utc::now();
    // 管理员视为拥有全部权限（通配 `*`），绕过所有门控
    let permissions: Vec<String> = if user.is_admin {
        vec!["*".to_string()]
    } else {
        user_repo::get_user_permissions(pool, user.id)?
    };
    let claims = Claims {
        sub: user.id,
        username: user.username.clone(),
        jti: Uuid::new_v4().to_string(),
        is_admin: user.is_admin,
        role_id: user.role_id,
        permissions,
        exp: (if keep_signed_in {
            now + Duration::days(30)
        } else {
            now + Duration::hours(24)
        })
        .timestamp() as usize,
        iat: now.timestamp() as usize,
    };
    let secret = jwt_secret();
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(format!("Token 生成失败: {}", e)))
}

/// 验证 JWT token 并返回 Claims
pub fn verify_token(token: &str) -> Result<Claims> {
    let secret = jwt_secret();
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|e| match e.kind() {
        jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
            AppError::Validation("登录已过期，请重新登录".into())
        }
        _ => AppError::Validation("无效的登录凭证".into()),
    })
}

/// 验证密码
/// Verify both the JWT signature and the server-side login session.
/// Removing a row from user_sessions immediately revokes the corresponding token.
pub fn verify_active_token(pool: &DbPool, token: &str) -> Result<Claims> {
    let claims = verify_token(token)?;
    let conn = pool.get()?;
    let is_active: bool = conn.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM user_sessions
            WHERE user_id = ?1
              AND token = ?2
              AND datetime(expires_at) > datetime('now')
        )",
        postgres_compat::params![claims.sub, token],
        |row| row.get(0),
    )?;
    if !is_active {
        return Err(AppError::Forbidden(
            "该登录已在其他设备退出或已过期，请重新登录".into(),
        ));
    }
    // Sessions use a sliding idle timeout. JWTs only protect token integrity;
    // the server-side record decides whether this device remains online.
    let keep_signed_in = session_keep_signed_in(&conn, claims.sub, token)?;
    let idle_minutes = session_idle_minutes(&conn, claims.sub, keep_signed_in)?;
    // See verify_active_token: these persisted session values must use the
    // database server's local clock, not UTC.
    let now = Local::now();
    let expires_at = (now + Duration::minutes(idle_minutes))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    let last_seen_at = now.format("%Y-%m-%d %H:%M:%S").to_string();
    conn.execute(
        "UPDATE user_sessions SET last_seen_at=?1,expires_at=?2 WHERE user_id=?3 AND token=?4",
        postgres_compat::params![last_seen_at, expires_at, claims.sub, token],
    )?;
    Ok(claims)
}

fn session_keep_signed_in(
    conn: &postgres_compat::Connection,
    user_id: i64,
    token: &str,
) -> Result<bool> {
    let value: i64 = conn.query_row(
        "SELECT COALESCE(keep_signed_in, 0) FROM user_sessions WHERE user_id=?1 AND token=?2",
        postgres_compat::params![user_id, token],
        |row| row.get(0),
    )?;
    Ok(value != 0)
}

fn session_idle_minutes(
    conn: &postgres_compat::Connection,
    user_id: i64,
    keep_signed_in: bool,
) -> Result<i64> {
    if keep_signed_in {
        return Ok(KEEP_SIGNED_IN_IDLE_MINUTES);
    }
    let is_public: bool = conn.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM user_roles ur JOIN roles r ON r.id=ur.role_id
            WHERE ur.user_id=?1
              AND r.system_key IN ('public_account','rd_public_account','analysis_public_account')
        )",
        [user_id],
        |row| row.get(0),
    )?;
    Ok(if is_public {
        PUBLIC_ACCOUNT_IDLE_MINUTES
    } else {
        NORMAL_IDLE_MINUTES
    })
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    bcrypt::verify(password, hash).unwrap_or(false)
}

/// 哈希密码
pub fn hash_password(password: &str) -> Result<String> {
    bcrypt::hash(password, bcrypt::DEFAULT_COST)
        .map_err(|e| AppError::Internal(format!("密码哈希失败: {}", e)))
}

/// 登录：验证用户名密码，返回 token + user 信息
pub fn login(pool: &DbPool, req: &LoginRequest) -> Result<LoginResponse> {
    let user = user_repo::find_by_username(pool, &req.username)?
        .ok_or_else(|| AppError::Validation("用户名或密码错误".into()))?;

    if !user.is_active {
        return Err(AppError::Validation("该账号已被停用".into()));
    }

    if !verify_password(&req.password, &user.password) {
        return Err(AppError::Validation("用户名或密码错误".into()));
    }

    create_login_response(
        pool,
        user,
        req.keep_signed_in,
        req.device_id.as_deref(),
        req.device_name.as_deref(),
    )
}

/// Start a login session for an already authenticated user.
/// A user may retain at most two active sessions; the oldest is revoked first.
pub fn create_login_response(
    pool: &DbPool,
    user: User,
    keep_signed_in: bool,
    device_id: Option<&str>,
    device_name: Option<&str>,
) -> Result<LoginResponse> {
    let token = generate_token(pool, &user, keep_signed_in)?;
    let device_id = device_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("legacy-{}", Uuid::new_v4()));
    let device_name = device_name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("未知设备");
    // Session timestamps are stored as timezone-free text because the legacy
    // schema is shared with SQLite-compatible queries. Keep them in the same
    // local clock as PostgreSQL's datetime('now') compatibility expression.
    let now = Local::now();
    let idle_minutes = {
        let conn = pool.get()?;
        session_idle_minutes(&conn, user.id, keep_signed_in)?
    };
    let expires_at = (now + Duration::minutes(idle_minutes))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    let last_seen_at = now.format("%Y-%m-%d %H:%M:%S").to_string();
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;

    tx.execute(
        "DELETE FROM user_sessions
         WHERE user_id = ?1 AND datetime(expires_at) <= datetime('now')",
        [user.id],
    )?;
    // Re-login on the same browser/device replaces that device's token and
    // does not consume another concurrent-device slot.
    tx.execute(
        "DELETE FROM user_sessions WHERE user_id=?1 AND device_id=?2",
        postgres_compat::params![user.id, device_id],
    )?;
    tx.execute(
        "INSERT INTO user_sessions (user_id, token, expires_at, keep_signed_in, last_seen_at, device_id, device_name)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        postgres_compat::params![
            user.id,
            token,
            expires_at,
            if keep_signed_in { 1 } else { 0 },
            last_seen_at,
            device_id,
            device_name
        ],
    )?;
    tx.execute(
        "DELETE FROM user_sessions
         WHERE user_id = ?1
           AND id NOT IN (
               SELECT id FROM user_sessions
               WHERE user_id = ?1
               ORDER BY datetime(created_at) DESC, id DESC
               LIMIT 2
           )",
        [user.id],
    )?;
    tx.commit()?;

    Ok(LoginResponse { token, user })
}

/// 登出：删除会话
pub fn logout(pool: &DbPool, token: &str) -> Result<()> {
    let conn = pool.get()?;
    conn.execute("DELETE FROM user_sessions WHERE token = ?1", [token])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn third_login_revokes_the_oldest_session() {
        let pool = crate::db::init_pool("postgres-test");
        crate::db::test_migrations::run(&pool.get().unwrap()).unwrap();
        let request = LoginRequest {
            username: "admin".into(),
            password: "admin123".into(),
            keep_signed_in: false,
            device_id: Some("device-1".into()),
            device_name: Some("test-1".into()),
        };

        let first = login(&pool, &request).unwrap();
        let second = login(
            &pool,
            &LoginRequest {
                device_id: Some("device-2".into()),
                device_name: Some("test-2".into()),
                ..request
            },
        )
        .unwrap();
        let third = login(
            &pool,
            &LoginRequest {
                username: "admin".into(),
                password: "admin123".into(),
                keep_signed_in: false,
                device_id: Some("device-3".into()),
                device_name: Some("test-3".into()),
            },
        )
        .unwrap();

        assert_ne!(first.token, second.token);
        assert_ne!(second.token, third.token);
        assert!(verify_active_token(&pool, &first.token).is_err());
        assert!(verify_active_token(&pool, &second.token).is_ok());
        assert!(verify_active_token(&pool, &third.token).is_ok());

        let count: i64 = pool
            .get()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM user_sessions WHERE user_id = ?1",
                [third.user.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn repeated_login_on_same_device_replaces_only_that_session() {
        let pool = crate::db::init_pool("postgres-test");
        crate::db::test_migrations::run(&pool.get().unwrap()).unwrap();
        let request = LoginRequest {
            username: "admin".into(),
            password: "admin123".into(),
            keep_signed_in: false,
            device_id: Some("browser-a".into()),
            device_name: Some("Browser A".into()),
        };
        let first = login(&pool, &request).unwrap();
        let second = login(&pool, &request).unwrap();
        assert!(verify_active_token(&pool, &first.token).is_err());
        assert!(verify_active_token(&pool, &second.token).is_ok());
        let count: i64 = pool
            .get()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM user_sessions WHERE user_id=?1",
                [second.user.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn keep_signed_in_creates_a_persistent_session_and_long_token() {
        let pool = crate::db::init_pool("postgres-test");
        crate::db::test_migrations::run(&pool.get().unwrap()).unwrap();
        let response = login(
            &pool,
            &LoginRequest {
                username: "admin".into(),
                password: "admin123".into(),
                keep_signed_in: true,
                device_id: Some("remembered-device".into()),
                device_name: Some("Remembered Device".into()),
            },
        )
        .unwrap();

        let conn = pool.get().unwrap();
        let persisted: i64 = conn
            .query_row(
                "SELECT keep_signed_in FROM user_sessions WHERE token=?1",
                [&response.token],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(persisted, 1);

        let claims = verify_token(&response.token).unwrap();
        assert!(claims.exp as i64 >= (Utc::now() + Duration::days(29)).timestamp());
    }
}
