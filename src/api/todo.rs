use crate::external_connections::ExternalConnectivity;
use crate::routing_utils::{GenericErrorResponse, Json, ValidationErrorResponse};
use crate::{AppState, SharedData, domain, dto, persistence};
use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{ErrorResponse, IntoResponse, Response};
use axum::routing::patch;
use std::sync::Arc;
use tracing::*;
use utoipa::OpenApi;
use validator::Validate;

#[derive(OpenApi)]
#[openapi(paths(update_task, delete_task,))]
/// Defines the OpenAPI documentation for the tasks API
pub struct TaskApi;
/// Constant used to group task endpoints in OpenAPI documentation
pub const TASK_API_GROUP: &str = "Tasks";

/// Creates a router for endpoints under the "/tasks" group of APIs
pub fn task_routes() -> Router<Arc<SharedData>> {
    Router::new().route(
        "/:task_id",
        patch(
            async |State(app_state): AppState,
                   Path(task_id): Path<i32>,
                   Json(update): Json<dto::task::UpdateTask>| {
                let mut ext_cxn = app_state.ext_cxn.clone();
                let task_service = domain::todo::TaskService;

                update_task(task_id, update, &mut ext_cxn, &task_service).await
            },
        )
        .delete(
            async |State(app_state): AppState, Path(task_id): Path<i32>| {
                let mut ext_cxn = app_state.ext_cxn.clone();
                let task_service = domain::todo::TaskService;

                delete_task(task_id, &mut ext_cxn, &task_service).await
            },
        ),
    )
}

/// Updates the content of a task
#[utoipa::path(
    patch,
    path = "/tasks/{task_id}",
    tag = TASK_API_GROUP,
    params(
        ("task_id" = i32, Path, description = "The ID of the task to update"),
    ),
    request_body = UpdateTask,
    responses(
        (status = 200, description = "Task successfully updated"),
        (status = 400, response = dto::err_resps::BasicError400Validation),
        (status = 500, response = dto::err_resps::BasicError500),
    ),
)]
#[instrument(skip(ext_cxn, task_service))]
async fn update_task(
    task_id: i32,
    task_data: dto::task::UpdateTask,
    ext_cxn: &mut impl ExternalConnectivity,
    task_service: &impl domain::todo::driving_ports::TaskPort,
) -> Result<StatusCode, ErrorResponse> {
    info!("Updating task {task_id}");
    task_data
        .validate()
        .map_err(ValidationErrorResponse::from)?;

    let domain_update = domain::todo::UpdateTask::from(task_data);
    let task_writer = persistence::db_todo_driven_ports::DbTaskWriter;

    let update_result = task_service
        .update_task(task_id, &domain_update, &mut *ext_cxn, &task_writer)
        .await;
    match update_result {
        Ok(_) => Ok(StatusCode::OK),
        Err(db_err) => {
            error!("Update task failure: {db_err}");
            Err(GenericErrorResponse(db_err).into())
        }
    }
}

/// Deletes a task
#[utoipa::path(
    delete,
    path = "/tasks/{task_id}",
    tag = TASK_API_GROUP,
    params(
        ("task_id" = i32, Path, description = "The ID of the task to delete")
    ),
    responses(
        (status = 200, description = "Task successfully deleted"),
        (status = 500, response = dto::err_resps::BasicError500),
    ),
)]
#[instrument(skip(ext_cxn, task_service))]
async fn delete_task(
    task_id: i32,
    ext_cxn: &mut impl ExternalConnectivity,
    task_service: &impl domain::todo::driving_ports::TaskPort,
) -> Result<StatusCode, Response> {
    info!("Deleting task {task_id}");
    let task_write = persistence::db_todo_driven_ports::DbTaskWriter;

    let delete_result = task_service
        .delete_task(task_id, &mut *ext_cxn, &task_write)
        .await;
    match delete_result {
        Ok(_) => Ok(StatusCode::OK),
        Err(db_err) => {
            error!("Failed to delete task: {db_err}");
            Err(GenericErrorResponse(db_err).into_response())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{domain, dto, external_connections};
    use anyhow::anyhow;
    use speculoos::prelude::*;
    use std::sync::Mutex;

    mod update_task {
        use super::*;
        use crate::api::test_util::deserialize_body;

        #[tokio::test]
        async fn happy_path() {
            let mut task_service_raw = domain::todo::test_util::MockTaskService::new();
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();

            task_service_raw
                .update_task_result
                .set_returned_anyhow(Ok(()));
            let task_service = Mutex::new(task_service_raw);

            let update_task_response = update_task(
                2,
                dto::task::UpdateTask {
                    description: "Something to do".to_owned(),
                },
                &mut ext_cxn,
                &task_service,
            )
            .await;
            assert_that!(update_task_response).is_ok_containing(StatusCode::OK);

            let locked_task_service = task_service.lock().expect("task service mutex poisoned");
            assert!(matches!(locked_task_service.update_task_result.calls(), [
                    (2, domain::todo::UpdateTask {
                        description,
                    })
                ] if description == "Something to do"))
        }

        #[tokio::test]
        async fn returns_500_on_failed_update() {
            let mut task_service_raw = domain::todo::test_util::MockTaskService::new();
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();

            task_service_raw
                .update_task_result
                .set_returned_anyhow(Err(anyhow!("Something went wrong!")));
            let task_service = Mutex::new(task_service_raw);

            let update_task_response = update_task(
                2,
                dto::task::UpdateTask {
                    description: "Something to do".to_owned(),
                },
                &mut ext_cxn,
                &task_service,
            )
            .await;
            let real_response = update_task_response.into_response();

            assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, real_response.status());

            let deserialized_body: dto::BasicError =
                deserialize_body(real_response.into_body()).await;
            assert_eq!("internal_error", deserialized_body.error_code);
        }

        #[tokio::test]
        async fn returns_400_on_bad_input() {
            let task_service = domain::todo::test_util::MockTaskService::new_locked();
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();

            let update_task_response = update_task(
                5,
                dto::task::UpdateTask {
                    description: String::new(),
                },
                &mut ext_cxn,
                &task_service,
            )
            .await;
            let real_response = update_task_response.into_response();

            assert_eq!(StatusCode::BAD_REQUEST, real_response.status());

            let deserialized_body: dto::BasicError =
                deserialize_body(real_response.into_body()).await;
            assert_eq!("invalid_input", deserialized_body.error_code);
        }
    }

    mod delete_task {
        use super::*;
        use crate::api::test_util::deserialize_body;

        #[tokio::test]
        async fn happy_path() {
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();
            let task_service = domain::todo::test_util::MockTaskService::build_locked(|svc| {
                svc.delete_task_result.set_returned_anyhow(Ok(()));
            });

            // Verify we got the expected response
            let delete_task_result = delete_task(5, &mut ext_cxn, &task_service).await;
            let Ok(status) = delete_task_result else {
                panic!(
                    "Didn't receive expected response: {:#?}",
                    delete_task_result
                );
            };

            assert_eq!(StatusCode::OK, status);

            // Verify the service was called with the right params
            let locked_service = task_service.lock().unwrap();
            let calls = locked_service.delete_task_result.calls();
            assert_eq!(1, calls.len());
            assert_eq!(5, calls[0]);
        }

        #[tokio::test]
        async fn returns_500_when_service_blows_up() {
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();
            let task_service = domain::todo::test_util::MockTaskService::build_locked(|svc| {
                svc.delete_task_result
                    .set_returned_anyhow(Err(anyhow!("Whoopsie daisy!")));
            });

            // Verify we got the expected response
            let delete_task_result = delete_task(5, &mut ext_cxn, &task_service).await;
            let response = delete_task_result.into_response();

            assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, response.status());

            let deserialized_body: dto::BasicError = deserialize_body(response.into_body()).await;
            assert_eq!("internal_error", deserialized_body.error_code);
        }
    }
}
