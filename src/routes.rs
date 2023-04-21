use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::ErrorResponse;

use axum::routing::{delete, get, patch, post};
use axum::Router;
use log::*;
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::entity::{TodoTask, TodoUser};
use crate::routing_utils::{DbErrorResponse, Json, ValidationErrorResponse};
use crate::{db, dto, AppState, SharedData};

/// Sample endpoint that can be used to show the API is responsive.
pub async fn hello() -> &'static str {
    info!("Hello");
    "Hello world!"
}

/// Builds a router for all the user routes
pub fn user_routes() -> Router<Arc<SharedData>> {
    Router::new()
        .route("/", get(get_users))
        .route("/", post(create_user))
        .route("/:user_id/tasks", get(get_tasks_for_user))
        .route("/:user_id/tasks/:task_id", get(get_task_for_user))
        .route("/:user_id/tasks", post(add_task_for_user))
}

/// Retrieves a list of all the users in the system.
async fn get_users(State(app_data): AppState) -> Result<Json<Vec<TodoUser>>, ErrorResponse> {
    info!("Requested users");
    let db_cxn = &app_data.db;
    let users_result = db::get_users(db_cxn).await;
    if users_result.is_err() {
        error!(
            "Could not retrieve users: {}",
            users_result.as_ref().unwrap_err()
        );
    }
    let response = users_result.map_err(DbErrorResponse::from)?;
    Ok(Json(response))
}

/// Creates a user.
async fn create_user(
    State(app_data): AppState,
    Json(user_to_create): Json<dto::NewUser>,
) -> Result<(StatusCode, Json<dto::InsertedUser>), ErrorResponse> {
    info!("Attempt to create user: {}", user_to_create);
    user_to_create
        .validate()
        .map_err(ValidationErrorResponse::from)?;

    let db_cxn = &app_data.db;
    let creation_result = db::create_user(db_cxn, &user_to_create).await;
    if creation_result.is_err() {
        error!(
            "User create failure: {}",
            creation_result.as_ref().unwrap_err()
        );
    }
    Ok((
        StatusCode::CREATED,
        Json(
            creation_result
                .map_err(DbErrorResponse::from)
                .map(|id| dto::InsertedUser { id })?,
        ),
    ))
}

/// Adds routes under "/tasks" and routes for user-owned tasks to the application router
pub fn task_routes() -> Router<Arc<SharedData>> {
    Router::new()
        .route("/tasks/:task_id", patch(update_task))
        .route("/tasks/:task_id", delete(delete_task))
}

/// Retrieves a set of tasks owned by a user
async fn get_tasks_for_user(
    State(app_state): AppState,
    Path(user_id): Path<i32>,
) -> Result<Json<Vec<TodoTask>>, ErrorResponse> {
    info!("Get tasks for user {user_id}");
    let db_cxn = &app_state.db;
    let tasks = db::get_tasks_for_user(db_cxn, user_id).await;
    if tasks.is_err() {
        let err = tasks.as_ref().unwrap_err();
        error!("Failed to get user {user_id}'s tasks: {}", err)
    }
    Ok(Json(tasks.map_err(DbErrorResponse::from)?))
}

#[derive(Deserialize)]
struct GetTaskPath {
    user_id: i32,
    task_id: i32,
}

/// Retrieves a specific task owned by a user
async fn get_task_for_user(
    State(app_state): AppState,
    Path(path): Path<GetTaskPath>,
) -> Result<Json<TodoTask>, ErrorResponse> {
    info!("Get task {} for user {}", path.task_id, path.user_id);
    let db_cxn = &app_state.db;
    let task = db::get_task_for_user(db_cxn, path.user_id, path.task_id).await;
    if let Err(ref contained_err) = task {
        // We don't want to log an error for the "no results" case
        match contained_err {
            &db::DbError::NoResults => {}
            _ => error!(
                "Failed to get task {} for user {}: {}",
                path.task_id, path.user_id, contained_err
            ),
        }
    }

    Ok(Json(task.map_err(DbErrorResponse::from)?))
}

#[derive(Serialize)]
struct InsertedTask {
    id: i32,
}

/// Adds a new task for a user
async fn add_task_for_user(
    State(app_state): AppState,
    Path(user_id): Path<i32>,
    Json(task_data): Json<dto::NewTask>,
) -> Result<(StatusCode, Json<InsertedTask>), ErrorResponse> {
    info!("Adding task for user {user_id}");
    task_data
        .validate()
        .map_err(ValidationErrorResponse::from)?;

    let db_cxn = &app_state.db;
    let inserted_task = db::add_task_for_user(db_cxn, user_id, &task_data).await;
    if inserted_task.is_err() {
        error!(
            "Failed to add task for user {user_id}: {}",
            inserted_task.as_ref().unwrap_err()
        );
    }

    Ok((
        StatusCode::CREATED,
        Json(
            inserted_task
                .map_err(DbErrorResponse::from)
                .map(|id| InsertedTask { id })?,
        ),
    ))
}

/// Updates the content of a task
async fn update_task(
    State(app_state): AppState,
    Path(task_id): Path<i32>,
    Json(task_data): Json<dto::UpdateTask>,
) -> Result<StatusCode, ErrorResponse> {
    info!("Updating task {task_id}");
    task_data
        .validate()
        .map_err(ValidationErrorResponse::from)?;

    let db_cxn = &app_state.db;
    let update_result = db::update_user_task(db_cxn, task_id, &task_data).await;
    match update_result {
        Ok(_) => Ok(StatusCode::OK),
        Err(db_err) => {
            error!("Update task failure: {db_err}");
            Err(DbErrorResponse::from(db_err).into())
        }
    }
}

/// Deletes a task
async fn delete_task(
    State(app_state): AppState,
    Path(task_id): Path<i32>,
) -> Result<StatusCode, ErrorResponse> {
    info!("Deleting task {task_id}");
    let db_cxn = &app_state.db;
    let delete_result = db::delete_user_task(db_cxn, task_id).await;
    match delete_result {
        Ok(_) => Ok(StatusCode::OK),
        Err(db_err) => {
            error!("Failed to delete task: {db_err}");
            Err(DbErrorResponse::from(db_err).into())
        }
    }
}
