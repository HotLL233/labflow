use axum::http::HeaderMap;

use crate::db::DbPool;
use crate::error::{AppError, Result};
use crate::models::user::User;
use crate::repo::user_repo;
use crate::service::auth_service;

pub const ROLE_SYSTEM_ADMIN: &str = "系统管理员";
pub const ROLE_ANALYST: &str = "分析检测员";
pub const ROLE_ANALYSIS_LEADER: &str = "分析检测组长";
pub const ROLE_RD_SENDER: &str = "研发送样员";
pub const ROLE_RD_LEADER: &str = "研发送样组长";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordScope {
    Own,
    AnalysisAll,
    Lab(i64),
    Global,
}

#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user: User,
    pub role_names: Vec<String>,
}

impl AuthContext {
    pub fn has_permission(&self, permission: &str) -> bool {
        self.user.is_admin
            || self
                .user
                .permissions
                .iter()
                .any(|value| value == "*" || value == permission)
    }

    pub fn is_system_admin(&self) -> bool {
        self.user.is_admin || self.role_names.iter().any(|name| name == ROLE_SYSTEM_ADMIN)
    }

    pub fn is_analysis_member(&self) -> bool {
        self.role_names
            .iter()
            .any(|name| matches!(name.as_str(), ROLE_ANALYST | ROLE_ANALYSIS_LEADER))
    }

    pub fn is_analysis_leader(&self) -> bool {
        self.role_names
            .iter()
            .any(|name| name == ROLE_ANALYSIS_LEADER)
    }

    pub fn is_rd_sender(&self) -> bool {
        self.role_names.iter().any(|name| name == ROLE_RD_SENDER)
    }

    pub fn is_rd_leader(&self) -> bool {
        self.role_names.iter().any(|name| name == ROLE_RD_LEADER)
    }

    pub fn workload_scope(&self) -> RecordScope {
        if self.is_system_admin() {
            RecordScope::Global
        } else if self.is_analysis_leader() {
            RecordScope::AnalysisAll
        } else {
            RecordScope::Own
        }
    }

    pub fn rd_scope(&self) -> Result<RecordScope> {
        if self.is_system_admin() || self.is_analysis_member() {
            return Ok(RecordScope::Global);
        }
        if self.is_rd_leader() {
            return self
                .user
                .group_id
                .map(RecordScope::Lab)
                .ok_or_else(|| AppError::Validation("研发送样组长尚未设置所属实验室".into()));
        }
        Ok(RecordScope::Own)
    }
}

fn bearer_token(headers: &HeaderMap) -> Result<&str> {
    headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or_else(|| AppError::Forbidden("未提供登录凭证".into()))
}

pub fn authenticate(pool: &DbPool, headers: &HeaderMap) -> Result<AuthContext> {
    let claims = auth_service::verify_active_token(pool, bearer_token(headers)?)?;
    let user = user_repo::find_by_id(pool, claims.sub)?
        .ok_or_else(|| AppError::Forbidden("用户不存在，请重新登录".into()))?;
    if !user.is_active {
        return Err(AppError::Forbidden("该账号已停用".into()));
    }
    if user.username != claims.username {
        return Err(AppError::Forbidden("用户名已变更，请重新登录".into()));
    }
    let role_names = if user.is_admin {
        vec![ROLE_SYSTEM_ADMIN.to_string()]
    } else {
        user.role_names.clone()
    };
    Ok(AuthContext { user, role_names })
}

pub fn require_permission(ctx: &AuthContext, permission: &str) -> Result<()> {
    if ctx.has_permission(permission) {
        Ok(())
    } else {
        Err(AppError::Forbidden(format!("缺少权限: {permission}")))
    }
}
