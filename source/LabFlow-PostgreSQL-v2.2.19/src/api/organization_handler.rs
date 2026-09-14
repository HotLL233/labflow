//! Organization governance HTTP endpoints were retired in v2.2.0.
//! Organization tables remain available to the authorization layer for data scopes.
use crate::db::DbPool;
use axum::Router;

pub fn router(pool: DbPool) -> Router {
    Router::new().with_state(pool)
}
