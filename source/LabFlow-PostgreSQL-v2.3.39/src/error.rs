use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    Database(#[from] postgres_compat::Error), // code 2001
    #[error("连接池错误: {0}")]
    Pool(#[from] r2d2::Error), // code 2001
    #[error("Excel错误: {0}")]
    Xlsx(#[from] rust_xlsxwriter::XlsxError), // code 2001
    #[error("未找到: {0}")]
    NotFound(String), // code 2001
    #[error("验证失败: {0}")]
    Validation(String), // code 1001
    #[error("冲突: {0}")]
    Conflict(String), // code 1001
    #[error("无权限: {0}")]
    Forbidden(String), // code 1003
    #[error("内部错误: {0}")]
    Internal(String), // code 5000
}

/// Business error codes
impl AppError {
    pub fn code(&self) -> i32 {
        match self {
            AppError::Validation(_) | AppError::Conflict(_) => 1001, // 参数/业务校验错误
            AppError::NotFound(_) => 2001,                           // 数据不存在
            AppError::Forbidden(_) => 1003,                          // 无权限
            AppError::Database(_) | AppError::Pool(_) | AppError::Xlsx(_) => 2001,
            AppError::Internal(_) => 5000, // 服务器内部错误
        }
    }

    /// 对外可见的错误文案。
    /// v2.3.19：数据库、连接池、Excel 与内部错误的原始信息可能包含 SQL 片段、
    /// 表结构或服务器绝对路径，只写日志，对外统一返回泛化文案。
    pub fn public_message(&self) -> String {
        match self {
            AppError::NotFound(msg) => msg.clone(),
            AppError::Validation(msg) => msg.clone(),
            AppError::Conflict(msg) => msg.clone(),
            AppError::Forbidden(msg) => msg.clone(),
            AppError::Database(_)
            | AppError::Pool(_)
            | AppError::Xlsx(_)
            | AppError::Internal(_) => "操作失败，请稍后重试；如持续出现请联系管理员。".to_string(),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // Log Internal errors with full detail for diagnostics
        if let AppError::Internal(ref msg) = self {
            tracing::error!(error_code = self.code(), detail = %msg, "AppError::Internal");
        }
        if let AppError::Database(ref e) = self {
            tracing::error!(error_code = self.code(), detail = %e, "AppError::Database");
        }
        if let AppError::Forbidden(ref msg) = self {
            tracing::warn!(error_code = self.code(), detail = %msg, "AppError::Forbidden");
        }

        let code = self.code();
        let message = self.public_message();
        let body = json!({ "code": code, "message": message, "data": null });
        (StatusCode::OK, Json(body)).into_response()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
