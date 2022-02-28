use core::task;
use std::{time::Duration, fmt::Display, error::Error};
use std::borrow::Borrow;

use serde::{Serialize, Deserialize};
use postgres::{Config, NoTls, Row, GenericClient};
use postgres::types::ToSql;
use r2d2_postgres::{PostgresConnectionManager, r2d2::Pool};
use sqlx::Pool;
use sqlx::postgres::PgPoolOptions;

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

#[derive(Serialize)]
pub struct TodoTask {
    pub id: i32,
    pub user_id: i32,
    pub item_desc: String,
}

impl From<&Row> for TodoTask {
    fn from(row: &Row) -> Self {
        TodoTask {
            id: row.get("id"),
            user_id: row.get("user_id"),
            item_desc: row.get("item_desc"),
        }
    }
}

#[derive(Deserialize)]
pub struct NewTask {
    pub item_desc: String,
}

impl NewTask {
    pub fn as_columns<'task>(&'task self) -> Vec<&'task (dyn ToSql + Sync)> {
        vec![&self.item_desc]
    }
}

#[derive(Deserialize)]
pub struct UpdateTask {
    pub item_desc: String,
}

impl UpdateTask {
    pub fn as_columns<'task>(&'task self) -> Vec<&'task (dyn ToSql + Sync)> {
        vec![&self.item_desc]
    }
}

pub type PgPool = Pool<PostgresConnectionManager<NoTls>>;

#[derive(Debug)]
pub enum DbError {
    QueryFailure(postgres::Error)
}

impl DbError {
    fn generic(pg_err: postgres::Error) -> DbError {
        Self::QueryFailure(pg_err)
    }
}

impl Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            &Self::QueryFailure(ref pg_err) => write!(f, "Failed to execute query: {}", pg_err)
        }
    }
}

impl Error for DbError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            &Self::QueryFailure(ref pg_err) => Some(pg_err)
        }
    }
}


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

pub fn connect_sqlx() -> sqlx::PgPool {
    PgPoolOptions::new()
        .connect_timeout(Duration::from_secs(2))
        .max_connections(16);
}

pub fn get_users(conn: &mut impl GenericClient) -> Result<Vec<TodoUser>, DbError> {
    let fetched_users = conn.query("SELECT * FROM todo_user", &[])
        .map_err(DbError::generic)?
        .iter()
        .map(TodoUser::from)
        .collect::<Vec<TodoUser>>();

    Ok(fetched_users)
}

pub fn get_tasks_for_user(conn: &mut impl GenericClient, user_id: i32) -> Result<Vec<TodoTask>, DbError> {
    let fetched_tasks = conn.query("SELECT * FROM todo_item WHERE user_id = $1", &[&user_id])
        .map_err(DbError::generic)?
        .iter()
        .map(TodoTask::from)
        .collect::<Vec<TodoTask>>();

    Ok(fetched_tasks)
}

pub fn add_task_for_user(conn: &mut impl GenericClient, user_id: i32, new_task: &NewTask) -> Result<i32, DbError> {
    let mut columns: Vec<&(dyn ToSql + Sync)> = vec!(&user_id);
    columns.append(&mut new_task.as_columns());
    let inserted_task: i32 = conn.query_one("INSERT INTO todo_item(user_id, item_desc) VALUES ($1, $2) RETURNING id;", columns.as_slice())
        .map_err(DbError::generic)?
        .borrow()
        .get(0);

    Ok(inserted_task)
}

pub fn update_user_task(conn: &mut impl GenericClient, task_id: i32, task_update: &UpdateTask) -> Result<(), DbError> {
    let mut update_content = task_update.as_columns();
    update_content.push(&task_id);
    conn.execute("UPDATE todo_item SET item_desc = $1 WHERE id = $2;", update_content.as_slice())
        .map_err(DbError::generic)?;

    Ok(())
}

pub fn delete_user_task(conn: &mut impl GenericClient, task_id: i32) -> Result<(), DbError> {
    conn.execute("DELETE FROM todo_item WHERE id = $1", &[&task_id])
        .map_err(DbError::generic)?;

    Ok(())
}