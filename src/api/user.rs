use crate::entity::{TodoTask, TodoUser};
use crate::external_connections::TransactableExternalConnectivity;
use crate::routing_utils::{DbErrorResponse, Json, ValidationErrorResponse};
use crate::{db, domain, dto, persistence, AppState, SharedData};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::ErrorResponse;
use axum::routing::{get, post};
use axum::Router;
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use validator::Validate;

/// Builds a router for all the user routes
pub fn user_routes() -> Router<Arc<SharedData>> {
    Router::new()
        .route(
            "/",
            get(|State(app_data): AppState| async {
                let user_service = domain::user::UserService {};
                let user_reader = persistence::db_user_driven_ports::DbReadUsers {};
                let external_connectivity = app_data.ext_cxn.clone();

                get_users(external_connectivity, user_service, user_reader).await
            }),
        )
        .route("/", post(create_user))
        .route("/:user_id/tasks", get(get_tasks_for_user))
        .route("/:user_id/tasks/:task_id", get(get_task_for_user))
        .route("/:user_id/tasks", post(add_task_for_user))
}

/// Retrieves a list of all the users in the system.
async fn get_users(
    mut ext_cxn: impl TransactableExternalConnectivity,
    user_service: impl domain::user::driving_ports::UserPort,
    user_reader: impl domain::user::driven_ports::UserReader,
) -> Result<Json<Vec<TodoUser>>, ErrorResponse> {
    info!("Requested users");
    let users_result = user_service.get_users(&mut ext_cxn, &user_reader).await;
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

    let db_cxn = &app_data.ext_cxn;
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

/// Retrieves a set of tasks owned by a user
async fn get_tasks_for_user(
    State(app_state): AppState,
    Path(user_id): Path<i32>,
) -> Result<Json<Vec<TodoTask>>, ErrorResponse> {
    info!("Get tasks for user {user_id}");
    let db_cxn = &app_state.ext_cxn;
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
    let db_cxn = &app_state.ext_cxn;
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

    let db_cxn = &app_state.ext_cxn;
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
