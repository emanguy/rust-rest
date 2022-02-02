use std::time::Duration;

use log::*;
use serde::Serialize;
use postgres::{Config, NoTls, Row};
use r2d2_postgres::{PostgresConnectionManager, r2d2::{Pool, PooledConnection}};

#[derive(Debug, Serialize)]
pub struct TodoUser {
    pub id: i32,
    pub first_name: String,
    pub last_name: String,
}

impl From<&Row> for TodoUser {
    fn from(row: &Row) -> Self {
        TodoUser {
            id: row.get("id"),
            first_name: row.get("first_name"),
            last_name: row.get("last_name"),
        }
    }
}

pub struct NewUser {
    pub first_name: String,
    pub last_name: String,
}

pub type PgPool = Pool<PostgresConnectionManager<NoTls>>;
pub type PgClient = PooledConnection<PostgresConnectionManager<NoTls>>;

pub fn connect() -> PgPool {
    let mut pg_config = Config::new();
    pg_config
        .host("127.0.0.1")
        .port(5432)
        .user("postgres")
        .password("sample123");
    let cxn_manager = PostgresConnectionManager::new(pg_config, NoTls);
    Pool::builder()
        .max_size(20)
        .connection_timeout(Duration::from_secs(2))
        .build(cxn_manager).expect("Failed to build connection pool")
}

pub fn get_users(conn: &mut PgClient) -> Vec<TodoUser> {
    let fetched_users = match conn.query("SELECT * FROM todo_user", &[]) {
        Ok(rows) => rows.iter().map(TodoUser::from).collect::<Vec<TodoUser>>(),
        Err(_) => {
            error!("Failed to select on users");
            return Vec::new();
        }
    };

    fetched_users
}