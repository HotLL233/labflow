use crate::config::AppConfig;
use crate::db::DbPool;
use crate::models::ApiResponse;
use crate::repo::user_repo;
use crate::service::auth_service;
use axum::{
    extract::{Json, State},
    routing::post,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    pub device_id: Option<String>,
    pub device_name: Option<String>,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
}

pub fn router(config: Arc<AppConfig>, pool: DbPool) -> Router {
    let state = Arc::new(AuthState { config, pool });

    Router::new()
        .route("/api/auth/login", post(login))
        .with_state(state)
}

#[derive(Clone)]
struct AuthState {
    config: Arc<AppConfig>,
    pool: DbPool,
}

async fn login(
    State(state): State<Arc<AuthState>>,
    Json(body): Json<LoginRequest>,
) -> Json<ApiResponse<LoginResponse>> {
    if body.username == state.config.admin_user && body.password == state.config.admin_pass {
        // v0.4.45: admin 也走 JWT 生成（修 tok_admin_* 伪 token 导致所有 PUT/POST 鉴权失败）
        let user = match user_repo::find_by_username(&state.pool, &body.username) {
            Ok(Some(u)) => u,
            _ => {
                return Json(ApiResponse {
                    code: 5000,
                    message: "admin 用户数据异常".into(),
                    data: None,
                })
            }
        };
        match auth_service::create_login_response(
            &state.pool,
            user,
            body.device_id.as_deref(),
            body.device_name.as_deref(),
        ) {
            Ok(response) => Json(ApiResponse::ok(LoginResponse {
                token: response.token,
            })),
            Err(e) => Json(ApiResponse {
                code: 5000,
                message: format!("Token 生成失败: {}", e),
                data: None,
            }),
        }
    } else {
        Json(ApiResponse {
            code: 403,
            message: "账号或密码错误".into(),
            data: None,
        })
    }
}
