use std::time::Duration;

use thiserror::Error;

use crate::{dto, entity};
use sqlx::{postgres::PgPoolOptions, PgExecutor, Row};

/// Describes errors that may happen at the database layer
#[derive(Debug, Error)]
pub enum DbError {
    #[error("Failed to execute query: {0}")]
    QueryFailure(#[source] sqlx::Error),
    #[error("No results were returned.")]
    NoResults,
}

impl DbError {
    /// Converts a sqlx error to a [DbError]
    fn generic(pg_err: sqlx::Error) -> DbError {
        match pg_err {
            sqlx::Error::RowNotFound => Self::NoResults,
            _ => Self::QueryFailure(pg_err),
        }
    }
}

/// Connects to a PostgreSQL database with the given `db_url`, returning a connection pool for accessing it
pub async fn connect_sqlx(db_url: &str) -> sqlx::PgPool {
    PgPoolOptions::new()
        .acquire_timeout(Duration::from_secs(2))
        .idle_timeout(Duration::from_secs(30))
        .max_connections(16)
        .connect(db_url)
        .await
        .expect("Could not connect to the database")
}

/// Retrieves all users in the system
pub async fn get_users(conn: impl PgExecutor<'_>) -> Result<Vec<entity::TodoUser>, DbError> {
    let fetched_users = sqlx::query_as("SELECT * FROM todo_user")
        .fetch_all(conn)
        .await
        .map_err(DbError::generic)?;

    Ok(fetched_users)
}

/// Creates a new user
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

/// Retrieves tasks owned by the given user
pub async fn get_tasks_for_user(
    conn: impl PgExecutor<'_>,
    user_id: i32,
) -> Result<Vec<entity::TodoTask>, DbError> {
    let fetched_tasks = sqlx::query_as("SELECT * FROM todo_item WHERE user_id = $1")
        .bind(user_id)
        .fetch_all(conn)
        .await
        .map_err(DbError::generic)?;

    Ok(fetched_tasks)
}

/// Gets a specific task owned by a user
pub async fn get_task_for_user(
    conn: impl PgExecutor<'_>,
    user_id: i32,
    task_id: i32,
) -> Result<entity::TodoTask, DbError> {
    let fetched_task = sqlx::query_as("SELECT * FROM todo_item WHERE user_id = $1 AND id = $2")
        .bind(user_id)
        .bind(task_id)
        .fetch_one(conn)
        .await
        .map_err(DbError::generic)?;

    Ok(fetched_task)
}

/// Adds a new task for a user
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

/// Updates a task
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

/// Deletes a task for a user
pub async fn delete_user_task(conn: impl PgExecutor<'_>, task_id: i32) -> Result<(), DbError> {
    sqlx::query("DELETE FROM todo_item WHERE id = $1")
        .bind(task_id)
        .execute(conn)
        .await
        .map_err(DbError::generic)?;

    Ok(())
}
