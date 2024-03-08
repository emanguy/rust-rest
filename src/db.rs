use std::time::Duration;

use thiserror::Error;

use crate::{dto};
use sqlx::{postgres::PgPoolOptions, PgExecutor, Row};

/// Connects to a PostgreSQL database with the given `db_url`, returning a connection pool for accessing it
pub async fn connect_sqlx(db_url: &str) -> sqlx::PgPool {
    PgPoolOptions::new()
        .acquire_timeout(Duration::from_secs(2))
        .idle_timeout(Duration::from_secs(30))
        .max_connections(32)
        .min_connections(4)
        .connect(db_url)
        .await
        .expect("Could not connect to the database")
}
