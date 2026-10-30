//! PostgreSQL test migration entrypoint retained for database integration tests.
//! Tests use a dedicated PostgreSQL schema and never create a file or in-memory database.

use crate::error::Result;

pub fn run(connection: &postgres_compat::Connection) -> Result<()> {
    crate::db::postgres_migrations::run(connection, "admin123")
}
