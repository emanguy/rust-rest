use std::time::Duration;

use thiserror::Error;

use serde::Serialize;

use crate::dto;
use sqlx::postgres::PgPoolOptions;
use sqlx::{FromRow, PgExecutor, Row};

#[derive(Debug, Serialize, FromRow)]
pub struct TodoUser {
    pub id: i32,
    pub first_name: String,
    pub last_name: String,
}

#[derive(Debug, Serialize, FromRow)]
pub struct TodoTask {
    pub id: i32,
    pub user_id: i32,
    pub item_desc: String,
}

#[derive(Debug, Error)]
pub enum DbError {
    #[error("Failed to execute query: {0}")]
    QueryFailure(#[source] sqlx::Error),
    #[error("No results were returned.")]
    NoResults,
}

impl DbError {
    fn generic(pg_err: sqlx::Error) -> DbError {
        match pg_err {
            sqlx::Error::RowNotFound => Self::NoResults,
            _ => Self::QueryFailure(pg_err),
        }
    }
}

pub async fn connect_sqlx(db_url: &str) -> sqlx::PgPool {
    PgPoolOptions::new()
        .connect_timeout(Duration::from_secs(2))
        .idle_timeout(Duration::from_secs(30))
        .max_connections(16)
        .connect(db_url)
        .await
        .expect("Could not connect to the database")
}

pub async fn get_users(conn: impl PgExecutor<'_>) -> Result<Vec<TodoUser>, DbError> {
    let fetched_users = sqlx::query_as("SELECT * FROM todo_user")
        .fetch_all(conn)
        .await
        .map_err(DbError::generic)?;

    Ok(fetched_users)
}

pub async fn create_user(conn: impl PgExecutor<'_>, user: &dto::NewUser) -> Result<i32, DbError> {
    let created_id: i32 =
        sqlx::query("INSERT INTO todo_user(first_name, last_name) VALUES ($1, $2) RETURNING id")
            .bind(&user.first_name)
            .bind(&user.last_name)
            .fetch_one(conn)
            .await
            .map_err(DbError::generic)?
            .get(0);

    Ok(created_id)
}

pub async fn get_tasks_for_user(
    conn: impl PgExecutor<'_>,
    user_id: i32,
) -> Result<Vec<TodoTask>, DbError> {
    let fetched_tasks = sqlx::query_as("SELECT * FROM todo_item WHERE user_id = $1")
        .bind(user_id)
        .fetch_all(conn)
        .await
        .map_err(DbError::generic)?;

    Ok(fetched_tasks)
}

pub async fn get_task_for_user(
    conn: impl PgExecutor<'_>,
    user_id: i32,
    task_id: i32,
) -> Result<TodoTask, DbError> {
    let fetched_task = sqlx::query_as("SELECT * FROM todo_item WHERE user_id = $1 AND id = $2")
        .bind(user_id)
        .bind(task_id)
        .fetch_one(conn)
        .await
        .map_err(DbError::generic)?;

    Ok(fetched_task)
}

pub async fn add_task_for_user(
    conn: impl PgExecutor<'_>,
    user_id: i32,
    new_task: &dto::NewTask,
) -> Result<i32, DbError> {
    let inserted_task: i32 =
        sqlx::query("INSERT INTO todo_item(user_id, item_desc) VALUES ($1, $2) RETURNING id;")
            .bind(user_id)
            .bind(new_task.item_desc.clone())
            .fetch_one(conn)
            .await
            .map_err(DbError::generic)?
            .get(0);

    Ok(inserted_task)
}

pub async fn update_user_task(
    conn: impl PgExecutor<'_>,
    task_id: i32,
    task_update: &dto::UpdateTask,
) -> Result<(), DbError> {
    sqlx::query("UPDATE todo_item SET item_desc = $1 WHERE id = $2;")
        .bind(task_id)
        .bind(task_update.item_desc.clone())
        .execute(conn)
        .await
        .map_err(DbError::generic)?;

    Ok(())
}

pub async fn delete_user_task(conn: impl PgExecutor<'_>, task_id: i32) -> Result<(), DbError> {
    sqlx::query("DELETE FROM todo_item WHERE id = $1")
        .bind(task_id)
        .execute(conn)
        .await
        .map_err(DbError::generic)?;

    Ok(())
}
