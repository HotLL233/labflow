use crate::db::DbPool;
use crate::models::user::LoginRequest;
use crate::models::ApiResponse;
use crate::service::auth_service;
use axum::{
    extract::{Json, State},
    routing::post,
    Router,
};
use serde::Serialize;

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
}

pub fn router(pool: DbPool) -> Router {
    Router::new()
        .route("/api/auth/login", post(login))
        .with_state(pool)
}

/// 兼容旧客户端的登录入口，统一走 `auth_service::login`（bcrypt 校验 + 停用校验 + 失败限流）。
/// v2.3.19：移除与配置文件口令做明文比对的旁路。该旁路绕过 bcrypt 和账号停用校验，
/// 且管理员改密后配置文件里的旧口令依然可以登录，属于无法通过改密撤销的后门。
async fn login(
    State(pool): State<DbPool>,
    Json(body): Json<LoginRequest>,
) -> Json<ApiResponse<LoginResponse>> {
    match auth_service::login(&pool, &body) {
        Ok(response) => Json(ApiResponse::ok(LoginResponse {
            token: response.token,
        })),
        Err(error) => Json(ApiResponse {
            code: error.code(),
            message: error.public_message(),
            data: None,
        }),
    }
}
