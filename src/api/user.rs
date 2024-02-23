use crate::domain::todo::driving_ports::TaskError;
use crate::domain::user::driving_ports::CreateUserError;
use crate::dto::InsertedTask;
use crate::external_connections::{ExternalConnectivity, TransactableExternalConnectivity};
use crate::routing_utils::{
    BasicErrorResponse, GenericErrorResponse, Json, ValidationErrorResponse,
};
use crate::{domain, dto, persistence, AppState, SharedData};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{ErrorResponse, IntoResponse};
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
            get(|State(app_data): AppState| async move {
                let user_service = domain::user::UserService {};
                let external_connectivity = app_data.ext_cxn.clone();

                get_users(external_connectivity, user_service).await
            }),
        )
        .route(
            "/",
            post(
                |State(app_data): AppState, Json(new_user): Json<dto::NewUser>| async move {
                    let user_service = domain::user::UserService {};
                    let external_connectivity = app_data.ext_cxn.clone();

                    create_user(new_user, external_connectivity, user_service).await
                },
            ),
        )
        .route(
            "/:user_id/tasks",
            get(
                |State(app_data): AppState, Path(user_id): Path<i32>| async move {
                    let task_service = domain::todo::TaskService {};
                    let external_connectivity = app_data.ext_cxn.clone();

                    get_tasks_for_user(user_id, external_connectivity, task_service).await
                },
            ),
        )
        .route(
            "/:user_id/tasks/:task_id",
            get(
                |State(app_data): AppState, Path(path): Path<GetTaskPath>| async move {
                    let task_service = domain::todo::TaskService {};
                    let external_connectivity = app_data.ext_cxn.clone();

                    get_task_for_user(path, external_connectivity, task_service).await
                },
            ),
        )
        .route(
            "/:user_id/tasks",
            post(
                |State(app_data): AppState,
                 Path(user_id): Path<i32>,
                 Json(new_task): Json<dto::NewTask>| async move {
                    let task_service = domain::todo::TaskService {};
                    let external_connectivity = app_data.ext_cxn.clone();

                    add_task_for_user(user_id, new_task, external_connectivity, task_service).await
                },
            ),
        )
}

/// Retrieves a list of all the users in the system.
async fn get_users(
    mut ext_cxn: impl TransactableExternalConnectivity,
    user_service: impl domain::user::driving_ports::UserPort,
) -> Result<Json<Vec<dto::TodoUser>>, ErrorResponse> {
    info!("Requested users");
    let user_reader = persistence::db_user_driven_ports::DbReadUsers {};
    let users_result = user_service.get_users(&mut ext_cxn, &user_reader).await;
    if users_result.is_err() {
        error!(
            "Could not retrieve users: {}",
            users_result.as_ref().unwrap_err()
        );
    }
    let response = users_result
        .map_err(GenericErrorResponse)?
        .into_iter()
        .map(dto::TodoUser::from)
        .collect::<Vec<_>>();

    Ok(Json(response))
}

/// Creates a user.
async fn create_user(
    new_user: dto::NewUser,
    mut ext_cxn: impl ExternalConnectivity,
    user_service: impl domain::user::driving_ports::UserPort,
) -> Result<(StatusCode, Json<dto::InsertedUser>), ErrorResponse> {
    info!("Attempt to create user: {}", new_user);
    new_user.validate().map_err(ValidationErrorResponse::from)?;

    let user_detector = persistence::db_user_driven_ports::DbDetectUser {};
    let user_writer = persistence::db_user_driven_ports::DbWriteUsers {};

    let domain_user_create = domain::user::CreateUser {
        first_name: new_user.first_name,
        last_name: new_user.last_name,
    };
    let creation_result = user_service
        .create_user(
            &domain_user_create,
            &mut ext_cxn,
            &user_writer,
            &user_detector,
        )
        .await;
    let user_id =
        match creation_result {
            Ok(id) => id,
            Err(CreateUserError::UserAlreadyExists) => {
                return Err((
                    StatusCode::CONFLICT,
                    Json(BasicErrorResponse {
                        error_code: "user_exists".to_owned(),
                        error_description:
                            "A user already exists in the system with the given information."
                                .to_owned(),
                        extra_info: None,
                    }),
                )
                    .into())
            }
            Err(CreateUserError::PortError(err)) => return Err(GenericErrorResponse(err).into()),
        };

    Ok((StatusCode::CREATED, Json(dto::InsertedUser { id: user_id })))
}

/// Handles all cases of domain errors returning [TaskError].
fn handle_todo_task_err(err: TaskError) -> ErrorResponse {
    match err {
        TaskError::UserDoesNotExist => (
            StatusCode::NOT_FOUND,
            Json(BasicErrorResponse {
                error_code: "no_matching_user".to_owned(),
                error_description: "Could not find a user matching the given information."
                    .to_owned(),
                extra_info: None,
            }),
        )
            .into_response()
            .into(),

        TaskError::PortError(err) => {
            error!("Encountered a problem fetching a task: {}", err);
            GenericErrorResponse(err).into()
        }
    }
}

/// Retrieves a set of tasks owned by a user
async fn get_tasks_for_user(
    user_id: i32,
    mut ext_cxn: impl ExternalConnectivity,
    task_service: impl domain::todo::driving_ports::TaskPort,
) -> Result<Json<Vec<dto::TodoTask>>, ErrorResponse> {
    info!("Get tasks for user {user_id}");
    // let tasks = db::get_tasks_for_user(db_cxn, user_id).await;
    let user_detect = persistence::db_user_driven_ports::DbDetectUser {};
    let task_read = persistence::db_todo_driven_ports::DbTaskReader {};

    let tasks_result = task_service
        .tasks_for_user(user_id, &mut ext_cxn, &user_detect, &task_read)
        .await;
    let tasks: Vec<dto::TodoTask> = match tasks_result {
        Ok(tasks) => tasks.into_iter().map(dto::TodoTask::from).collect(),
        Err(domain_err) => return Err(handle_todo_task_err(domain_err)),
    };

    Ok(Json(tasks))
}

#[derive(Deserialize)]
struct GetTaskPath {
    user_id: i32,
    task_id: i32,
}

/// Retrieves a specific task owned by a user
async fn get_task_for_user(
    path: GetTaskPath,
    mut ext_cxn: impl ExternalConnectivity,
    task_service: impl domain::todo::driving_ports::TaskPort,
) -> Result<Json<dto::TodoTask>, ErrorResponse> {
    info!("Get task {} for user {}", path.task_id, path.user_id);

    let user_detect = persistence::db_user_driven_ports::DbDetectUser {};
    let task_read = persistence::db_todo_driven_ports::DbTaskReader {};

    let task_result = task_service
        .user_task_by_id(
            path.user_id,
            path.task_id,
            &mut ext_cxn,
            &user_detect,
            &task_read,
        )
        .await;
    let task = match task_result {
        Ok(Some(tsk)) => tsk,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(BasicErrorResponse {
                    error_code: "no_matching_task".to_owned(),
                    error_description: "The specified task does not exist.".to_owned(),
                    extra_info: None,
                }),
            )
                .into())
        }
        Err(domain_err) => return Err(handle_todo_task_err(domain_err)),
    };

    Ok(Json(dto::TodoTask::from(task)))
}

/// Adds a new task for a user
async fn add_task_for_user(
    user_id: i32,
    new_task: dto::NewTask,
    mut ext_cxn: impl ExternalConnectivity,
    task_service: impl domain::todo::driving_ports::TaskPort,
) -> Result<(StatusCode, Json<InsertedTask>), ErrorResponse> {
    info!("Adding task for user {user_id}");
    new_task.validate().map_err(ValidationErrorResponse::from)?;

    let user_detect = persistence::db_user_driven_ports::DbDetectUser {};
    let task_write = persistence::db_todo_driven_ports::DbTaskWriter {};
    let domain_new_task = domain::todo::NewTask::from(new_task);

    let inserted_task_result = task_service
        .create_task_for_user(
            user_id,
            &domain_new_task,
            &mut ext_cxn,
            &user_detect,
            &task_write,
        )
        .await;
    let new_task_id = match inserted_task_result {
        Ok(id) => id,
        Err(domain_error) => return Err(handle_todo_task_err(domain_error)),
    };

    Ok((StatusCode::CREATED, Json(InsertedTask { id: new_task_id })))
}
