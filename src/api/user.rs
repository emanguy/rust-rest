use crate::domain::todo::driving_ports::TaskError;
use crate::domain::user::driving_ports::CreateUserError;
use crate::external_connections::ExternalConnectivity;
use crate::routing_utils::{GenericErrorResponse, Json, ValidationErrorResponse};
use crate::{domain, dto, persistence, AppState, SharedData};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::ErrorResponse;
use axum::routing::get;
use axum::Router;
use log::{error, info};
use serde::Deserialize;
use std::sync::Arc;
use utoipa::OpenApi;
use validator::Validate;

#[derive(OpenApi)]
#[openapi(paths(
    get_users,
    create_user,
    get_tasks_for_user,
    get_task_for_user,
    add_task_for_user,
))]
pub struct UsersApi;

pub const USER_API_GROUP: &str = "Users";

/// Builds a router for all the user routes
pub fn user_routes() -> Router<Arc<SharedData>> {
    Router::new()
        .route(
            "/",
            get(|State(app_data): AppState| async move {
                let user_service = domain::user::UserService {};
                let mut external_connectivity = app_data.ext_cxn.clone();

                get_users(&mut external_connectivity, &user_service).await
            })
            .post(
                |State(app_data): AppState, Json(new_user): Json<dto::NewUser>| async move {
                    let user_service = domain::user::UserService {};
                    let mut external_connectivity = app_data.ext_cxn.clone();

                    create_user(new_user, &mut external_connectivity, &user_service).await
                },
            ),
        )
        .route(
            "/:user_id/tasks",
            get(
                |State(app_data): AppState, Path(user_id): Path<i32>| async move {
                    let task_service = domain::todo::TaskService {};
                    let mut external_connectivity = app_data.ext_cxn.clone();

                    get_tasks_for_user(user_id, &mut external_connectivity, &task_service).await
                },
            )
            .post(
                |State(app_data): AppState,
                 Path(user_id): Path<i32>,
                 Json(new_task): Json<dto::NewTask>| async move {
                    let task_service = domain::todo::TaskService {};
                    let mut external_connectivity = app_data.ext_cxn.clone();

                    add_task_for_user(user_id, new_task, &mut external_connectivity, &task_service)
                        .await
                },
            ),
        )
        .route(
            "/:user_id/tasks/:task_id",
            get(
                |State(app_data): AppState, Path(path): Path<GetTaskPath>| async move {
                    let task_service = domain::todo::TaskService {};
                    let mut external_connectivity = app_data.ext_cxn.clone();

                    get_task_for_user(path, &mut external_connectivity, &task_service).await
                },
            ),
        )
}

/// Retrieves a list of all the users in the system.
#[utoipa::path(
    get,
    path = "/users",
    tag = USER_API_GROUP,
    responses(
        (status = 200, description = "User list successfully retrieved", body = Vec<TodoUser>),
        (status = 500, response = dto::err_resps::BasicError500)
    ),
)]
async fn get_users(
    ext_cxn: &mut impl ExternalConnectivity,
    user_service: &impl domain::user::driving_ports::UserPort,
) -> Result<Json<Vec<dto::TodoUser>>, ErrorResponse> {
    info!("Requested users");
    let user_reader = persistence::db_user_driven_ports::DbReadUsers {};
    let users_result = user_service.get_users(&mut *ext_cxn, &user_reader).await;
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
#[utoipa::path(
    post,
    path = "/users",
    tag = USER_API_GROUP,
    request_body = NewUser,
    responses(
        (status = 201, description = "User successfully created", body = InsertedUser),
        (status = 400, response = dto::err_resps::BasicError400Validation),
        (
            status = 409,
            description = "User with matching data already exists (error code `user_exists`)", 
            body = BasicError,
            example = json!({
                "error_code": "user_exists",
                "error_description": "A user with the same information already exists.",
                "extra_info": null,
            }),
        ),
        (status = 500, response = dto::err_resps::BasicError500)
    )
)]
async fn create_user(
    new_user: dto::NewUser,
    ext_cxn: &mut impl ExternalConnectivity,
    user_service: &impl domain::user::driving_ports::UserPort,
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
            &mut *ext_cxn,
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
                    Json(dto::BasicError {
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
            Json(dto::BasicError {
                error_code: "no_matching_user".to_owned(),
                error_description: "Could not find a user matching the given information."
                    .to_owned(),
                extra_info: None,
            }),
        )
            .into(),

        TaskError::PortError(err) => {
            error!("Encountered a problem fetching a task: {}", err);
            GenericErrorResponse(err).into()
        }
    }
}

/// Retrieves a set of tasks owned by a user
#[utoipa::path(
    get,
    path = "/users/{user_id}/tasks",
    tag = super::todo::TASK_API_GROUP,
    params(
        ("user_id" = i32, Path, description = "Which user to look up tasks for")
    ),
    responses(
        (status = 200, description = "Task list successfully retrieved", body = Vec<TodoTask>),
        (
            status = 404,
            description = "The requested user does not exist in the system (error code `no_matching_user`)",
            body = BasicError,
            example = json!({
                "error_code": "no_matching_user",
                "error_description": "No user exists in the system with the given id",
                "extra_info": null,
            })
        ),
        (status = 500, response = dto::err_resps::BasicError500)
    ),
)]
async fn get_tasks_for_user(
    user_id: i32,
    ext_cxn: &mut impl ExternalConnectivity,
    task_service: &impl domain::todo::driving_ports::TaskPort,
) -> Result<Json<Vec<dto::TodoTask>>, ErrorResponse> {
    info!("Get tasks for user {user_id}");
    // let tasks = db::get_tasks_for_user(db_cxn, user_id).await;
    let user_detect = persistence::db_user_driven_ports::DbDetectUser {};
    let task_read = persistence::db_todo_driven_ports::DbTaskReader {};

    let tasks_result = task_service
        .tasks_for_user(user_id, &mut *ext_cxn, &user_detect, &task_read)
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
#[utoipa::path(
    get,
    path = "/users/{user_id}/tasks/{task_id}",
    tag = super::todo::TASK_API_GROUP,
    params(
        ("user_id" = i32, Path, description = "The user ID to retrieve a task from"),
        ("task_id" = i32, Path, description = "The task ID to retrieve from the user"),
    ),
    responses(
        (status = 200, description = "Task successfully retrieved", body = TodoTask),
        (
            status = 404,
            description = "Specified user or task does not exist",
            body = BasicError,
            examples(
                ("No user" = (
                    summary = "User does not exist (error code no_matching_user)",
                    value = json!({
                        "error_code": "no_matching_user",
                        "error_description": "There is no user in the system with the given ID.",
                        "extra_info": null,
                    })
                )),

                ("No task" = (
                    summary = "Task does not exist (error code no_matching_task)",
                    value = json!({
                        "error_code": "no_matching_task",
                        "error_description": "The given user does not have a task with the given ID.",
                        "extra_info": null,
                    })
                ))
            )
        ),
        (status = 500, response = dto::err_resps::BasicError500),
    )
)]
async fn get_task_for_user(
    path: GetTaskPath,
    ext_cxn: &mut impl ExternalConnectivity,
    task_service: &impl domain::todo::driving_ports::TaskPort,
) -> Result<Json<dto::TodoTask>, ErrorResponse> {
    info!("Get task {} for user {}", path.task_id, path.user_id);

    let user_detect = persistence::db_user_driven_ports::DbDetectUser {};
    let task_read = persistence::db_todo_driven_ports::DbTaskReader {};

    let task_result = task_service
        .user_task_by_id(
            path.user_id,
            path.task_id,
            &mut *ext_cxn,
            &user_detect,
            &task_read,
        )
        .await;
    let task = match task_result {
        Ok(Some(tsk)) => tsk,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(dto::BasicError {
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
#[utoipa::path(
    post,
    path = "/users/{user_id}/tasks",
    tag = super::todo::TASK_API_GROUP,
    params(
        ("user_id" = i32, Path, description = "The user to add a task for")
    ),
    request_body = NewTask,
    responses(
        (status = 201, description = "Task successfully created", body = InsertedTask),
        (status = 400, response = dto::err_resps::BasicError400Validation),
        (
            status = 404,
            description = "Specified user does not exist (error code `no_matching_user`)",
            body = BasicError,
            example = json!({
                "error_code": "no_matching_user",
                "error_description": "No user in the system matches the given ID.",
                "extra_info": null,
            })
        ),
        (status = 500, response = dto::err_resps::BasicError500),
    ),
)]
async fn add_task_for_user(
    user_id: i32,
    new_task: dto::NewTask,
    ext_cxn: &mut impl ExternalConnectivity,
    task_service: &impl domain::todo::driving_ports::TaskPort,
) -> Result<(StatusCode, Json<dto::InsertedTask>), ErrorResponse> {
    info!("Adding task for user {user_id}");
    new_task.validate().map_err(ValidationErrorResponse::from)?;

    let user_detect = persistence::db_user_driven_ports::DbDetectUser {};
    let task_write = persistence::db_todo_driven_ports::DbTaskWriter {};
    let domain_new_task = domain::todo::NewTask::from(new_task);

    let inserted_task_result = task_service
        .create_task_for_user(
            user_id,
            &domain_new_task,
            &mut *ext_cxn,
            &user_detect,
            &task_write,
        )
        .await;
    let new_task_id = match inserted_task_result {
        Ok(id) => id,
        Err(domain_error) => return Err(handle_todo_task_err(domain_error)),
    };

    Ok((
        StatusCode::CREATED,
        Json(dto::InsertedTask { id: new_task_id }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::test_util::deserialize_body;
    use crate::api::user::get_users;
    use crate::{domain, external_connections};
    use anyhow::anyhow;
    use axum::response::IntoResponse;
    use speculoos::prelude::*;

    mod get_users {
        use super::*;

        #[tokio::test]
        async fn happy_path() {
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();
            let user_port = domain::user::test_util::MockUserService::build_locked(|svc| {
                svc.get_users_response.set_returned_anyhow(Ok(vec![
                    domain::user::TodoUser {
                        id: 1,
                        first_name: "John".to_owned(),
                        last_name: "Doe".to_owned(),
                    },
                    domain::user::TodoUser {
                        id: 2,
                        first_name: "Jane".to_owned(),
                        last_name: "Doe".to_owned(),
                    },
                ]));
            });

            let endpoint_result = get_users(&mut ext_cxn, &user_port).await;
            assert_that!(endpoint_result)
                .is_ok()
                .matches(|Json(user_list)| {
                    matches!(user_list.as_slice(), [
                        dto::TodoUser {
                            id: 1,
                            first_name: f1,
                            last_name: l1,
                        },
                        dto::TodoUser {
                            id: 2,
                            first_name: f2,
                            last_name: l2,
                        }
                    ] if f1 == "John" &&
                         f2 == "Jane" &&
                         l1 == "Doe" &&
                         l2 == "Doe"
                    )
                });
        }

        #[tokio::test]
        async fn returns_500_when_service_blows_up() {
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();
            let user_service = domain::user::test_util::MockUserService::build_locked(|svc| {
                // Configure what the service will return
                svc.get_users_response
                    .set_returned_anyhow(Err(anyhow!("Whoopsy daisy")));
            });

            // Execute endpoint, get response
            let response_result = get_users(&mut ext_cxn, &user_service).await;
            let (req_parts, response_body) = response_result.into_response().into_parts();

            // Verify status code
            assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, req_parts.status);

            // Extract raw bytes from HTTP body
            let deserialized_body: dto::BasicError = deserialize_body(response_body).await;
            // Verify error code is correct
            assert_eq!("internal_error", deserialized_body.error_code);
        }
    }

    mod create_user {
        use super::*;

        fn create_user_payload() -> dto::NewUser {
            dto::NewUser {
                first_name: "John".to_owned(),
                last_name: "Doe".to_owned(),
            }
        }

        #[tokio::test]
        async fn happy_path() {
            let user = create_user_payload();

            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();
            let user_service = domain::user::test_util::MockUserService::build_locked(|svc| {
                svc.create_user_response.set_returned_result(Ok(10));
            });

            let create_user_result = create_user(user, &mut ext_cxn, &user_service).await;
            let Ok((status, Json(inserted_user))) = create_user_result else {
                panic!(
                    "Could not read response from router: {:#?}",
                    create_user_result
                );
            };

            assert_eq!(StatusCode::CREATED, status);
            assert_eq!(10, inserted_user.id);
        }

        #[tokio::test]
        async fn responds_409_on_already_existing_user() {
            let user = create_user_payload();

            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();
            let user_service = domain::user::test_util::MockUserService::build_locked(|svc| {
                svc.create_user_response
                    .set_returned_result(Err(CreateUserError::UserAlreadyExists));
            });

            let response = create_user(user, &mut ext_cxn, &user_service)
                .await
                .into_response();
            let (resp_parts, resp_body) = response.into_parts();

            assert_eq!(StatusCode::CONFLICT, resp_parts.status);

            let deserialized_body: dto::BasicError = deserialize_body(resp_body).await;
            assert_eq!("user_exists", deserialized_body.error_code);
        }

        #[tokio::test]
        async fn responds_500_on_port_error() {
            let payload = create_user_payload();

            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();
            let user_service = domain::user::test_util::MockUserService::build_locked(|svc| {
                svc.create_user_response
                    .set_returned_result(Err(CreateUserError::PortError(anyhow!(
                        "Whoopsie daisy"
                    ))));
            });

            let response = create_user(payload, &mut ext_cxn, &user_service)
                .await
                .into_response();
            let (resp_parts, resp_body) = response.into_parts();

            assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, resp_parts.status);

            let deserialized_body: dto::BasicError = deserialize_body(resp_body).await;
            assert_eq!("internal_error", deserialized_body.error_code);
        }
    }

    mod handle_todo_task_err {
        use super::*;

        #[tokio::test]
        async fn converts_missing_user_to_not_found() {
            let produced_response =
                Err::<(), _>(handle_todo_task_err(TaskError::UserDoesNotExist)).into_response();
            let (res_parts, res_body) = produced_response.into_parts();

            assert_eq!(StatusCode::NOT_FOUND, res_parts.status);

            let deserialized_body: dto::BasicError = deserialize_body(res_body).await;
            assert_eq!("no_matching_user", deserialized_body.error_code);
        }

        #[tokio::test]
        async fn converts_port_error_to_500() {
            let produced_response = Err::<(), _>(handle_todo_task_err(TaskError::PortError(
                anyhow!("Whoopsie daisy"),
            )))
            .into_response();
            let (res_parts, res_body) = produced_response.into_parts();

            assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, res_parts.status);

            let deserialized_body: dto::BasicError = deserialize_body(res_body).await;
            assert_eq!("internal_error", deserialized_body.error_code);
        }
    }

    mod get_tasks_for_user {
        use super::*;

        #[tokio::test]
        async fn happy_path() {
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();
            let task_service = domain::todo::test_util::MockTaskService::build_locked(|svc| {
                svc.tasks_for_user_result.set_returned_result(Ok(vec![
                    domain::todo::TodoTask {
                        id: 3,
                        owner_user_id: 2,
                        item_desc: "Something to do".to_owned(),
                    },
                    domain::todo::TodoTask {
                        id: 10,
                        owner_user_id: 2,
                        item_desc: "Another thing to do".to_owned(),
                    },
                ]));
            });

            let Json(tasks) = get_tasks_for_user(2, &mut ext_cxn, &task_service)
                .await
                .unwrap_or_else(|err| {
                    panic!("Didn't get the expected response! Error: {:#?}", err);
                });

            assert!(matches!(tasks.as_slice(), [
                dto::TodoTask{
                    id: 3,
                    description: d1,
                },
                dto::TodoTask {
                    id: 10,
                    description: d2,
                }
            ] if d1 == "Something to do" &&
                 d2 == "Another thing to do"
            ))
        }

        #[tokio::test]
        async fn returns_404_on_user_not_found() {
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();
            let task_service = domain::todo::test_util::MockTaskService::build_locked(|svc| {
                svc.tasks_for_user_result
                    .set_returned_result(Err(TaskError::UserDoesNotExist));
            });

            let response = get_tasks_for_user(2, &mut ext_cxn, &task_service)
                .await
                .into_response();
            let (parts, body) = response.into_parts();

            assert_eq!(StatusCode::NOT_FOUND, parts.status);

            let body: dto::BasicError = deserialize_body(body).await;
            assert_eq!("no_matching_user", body.error_code);
        }
    }

    mod get_task_for_user {
        use super::*;

        fn path_variables() -> GetTaskPath {
            GetTaskPath {
                user_id: 2,
                task_id: 10,
            }
        }

        #[tokio::test]
        async fn happy_path() {
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();
            let path_vars = path_variables();
            let task_service = domain::todo::test_util::MockTaskService::build_locked(|svc| {
                svc.user_task_by_id_result
                    .set_returned_result(Ok(Some(domain::todo::TodoTask {
                        id: path_vars.task_id,
                        owner_user_id: path_vars.user_id,
                        item_desc: "Something to do".to_owned(),
                    })));
            });

            let Json(task) = get_task_for_user(path_vars, &mut ext_cxn, &task_service)
                .await
                .unwrap_or_else(|err| {
                    panic!("Didn't get expected response, instead got this: {:#?}", err);
                });

            assert!(matches!(task,
                dto::TodoTask {
                    id: 10,
                    description
                } if description == "Something to do",
            ));
        }

        #[tokio::test]
        async fn gives_appropriate_404_on_no_user() {
            let path_vars = path_variables();
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();
            let task_service = domain::todo::test_util::MockTaskService::build_locked(|svc| {
                svc.user_task_by_id_result
                    .set_returned_result(Err(TaskError::UserDoesNotExist));
            });

            let response = get_task_for_user(path_vars, &mut ext_cxn, &task_service)
                .await
                .into_response();
            let (parts, body) = response.into_parts();

            assert_eq!(StatusCode::NOT_FOUND, parts.status);

            let deserialized_body: dto::BasicError = deserialize_body(body).await;
            assert_eq!("no_matching_user", deserialized_body.error_code);
        }

        #[tokio::test]
        async fn gives_appropriate_404_on_no_task() {
            let path_vars = path_variables();
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();
            let task_service = domain::todo::test_util::MockTaskService::build_locked(|svc| {
                svc.user_task_by_id_result.set_returned_result(Ok(None));
            });

            let response = get_task_for_user(path_vars, &mut ext_cxn, &task_service)
                .await
                .into_response();
            let (parts, body) = response.into_parts();

            assert_eq!(StatusCode::NOT_FOUND, parts.status);

            let deserialized_body: dto::BasicError = deserialize_body(body).await;
            assert_eq!("no_matching_task", deserialized_body.error_code);
        }
    }

    mod add_task_for_user {
        use super::*;

        fn new_task_payload() -> dto::NewTask {
            dto::NewTask {
                item_desc: "Something to do".to_owned(),
            }
        }
        #[tokio::test]
        async fn happy_path() {
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();
            let task_service = domain::todo::test_util::MockTaskService::build_locked(|svc| {
                svc.create_task_for_user_result.set_returned_result(Ok(10));
            });

            let (status, Json(new_task_info)) =
                add_task_for_user(3, new_task_payload(), &mut ext_cxn, &task_service)
                    .await
                    .unwrap_or_else(|err| {
                        panic!("Didn't get a successful response: {:#?}", err);
                    });

            assert_eq!(StatusCode::CREATED, status);
            assert_eq!(10, new_task_info.id);
        }

        #[tokio::test]
        async fn gives_appropriate_404_on_no_user() {
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();
            let task_service = domain::todo::test_util::MockTaskService::build_locked(|svc| {
                svc.create_task_for_user_result
                    .set_returned_result(Err(TaskError::UserDoesNotExist));
            });

            let response = add_task_for_user(10, new_task_payload(), &mut ext_cxn, &task_service)
                .await
                .into_response();
            let (parts, body) = response.into_parts();

            assert_eq!(StatusCode::NOT_FOUND, parts.status);

            let deserialized_body: dto::BasicError = deserialize_body(body).await;
            assert_eq!("no_matching_user", deserialized_body.error_code);
        }
    }
}
