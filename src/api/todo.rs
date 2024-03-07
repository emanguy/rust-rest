use std::sync::Arc;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{ErrorResponse, IntoResponse, Response};
use axum::Router;
use axum::routing::{delete, patch};
use log::{error, info};
use validator::Validate;
use crate::{AppState, domain, dto, persistence, SharedData};
use crate::dto::UpdateTask;
use crate::external_connections::ExternalConnectivity;
use crate::routing_utils::{GenericErrorResponse, Json, ValidationErrorResponse};

/// Adds routes under "/tasks" and routes for user-owned tasks to the application router
pub fn task_routes() -> Router<Arc<SharedData>> {
    Router::new()
        .route("/tasks/:task_id", patch(|State(app_state): AppState, Path(task_id): Path<i32>, Json(update): Json<UpdateTask>| async move {
            let mut ext_cxn = app_state.ext_cxn.clone();
            let task_service = domain::todo::TaskService{};
            
            update_task(task_id, update, &mut ext_cxn, &task_service).await
        }))
        .route("/tasks/:task_id", delete(|State(app_state): AppState, Path(task_id): Path<i32>| async move {
            let mut ext_cxn = app_state.ext_cxn.clone();
            let task_service = domain::todo::TaskService{};
            
            delete_task(task_id, &mut ext_cxn, &task_service).await
        }))
}

/// Updates the content of a task
async fn update_task(
    task_id: i32,
    task_data: dto::UpdateTask,
    ext_cxn: &mut impl ExternalConnectivity,
    task_service: &impl domain::todo::driving_ports::TaskPort,
) -> Result<StatusCode, ErrorResponse> {
    info!("Updating task {task_id}");
    task_data
        .validate()
        .map_err(ValidationErrorResponse::from)?;

    let domain_update = domain::todo::UpdateTask::from(task_data);
    let task_writer = persistence::db_todo_driven_ports::DbTaskWriter{};
    
    let update_result = task_service.update_task(task_id, &domain_update, &mut *ext_cxn, &task_writer).await;
    match update_result {
        Ok(_) => Ok(StatusCode::OK),
        Err(db_err) => {
            error!("Update task failure: {db_err}");
            Err(GenericErrorResponse(db_err).into())
        }
    }
}

/// Deletes a task
async fn delete_task(
    task_id: i32,
    ext_cxn: &mut impl ExternalConnectivity,
    task_service: &impl domain::todo::driving_ports::TaskPort,
) -> Result<StatusCode, Response> {
    info!("Deleting task {task_id}");
    let task_write = persistence::db_todo_driven_ports::DbTaskWriter{};
    
    let delete_result = task_service.delete_task(task_id, &mut *ext_cxn, &task_write).await;
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
    use crate::{domain, external_connections};
    use speculoos::prelude::*;
    use anyhow::anyhow;
    use std::sync::Mutex;
    use hyper::body;
    use crate::routing_utils::BasicErrorResponse;

    mod update_task {
        use super::*;

        #[tokio::test]
        async fn happy_path() {
            let mut task_service_raw = domain::todo::test_util::MockTaskService::new();
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();

            task_service_raw.update_task_result.set_returned_anyhow(Ok(()));
            let task_service = Mutex::new(task_service_raw);

            let update_task_response = update_task(2, UpdateTask {
                description: "Something to do".to_owned(),
            }, &mut ext_cxn, &task_service).await;
            assert_that!(update_task_response).is_ok_containing(StatusCode::OK);

            let locked_task_service = task_service.lock().expect("task service mutex poisoned");
            assert!(
                matches!(locked_task_service.update_task_result.calls(), [
                    (2, domain::todo::UpdateTask {
                        description,
                    })
                ] if description == "Something to do")
            )
        }

        #[tokio::test]
        async fn returns_500_on_failed_update() {
            let mut task_service_raw = domain::todo::test_util::MockTaskService::new();
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();

            task_service_raw.update_task_result.set_returned_anyhow(Err(anyhow!("Something went wrong!")));
            let task_service = Mutex::new(task_service_raw);

            let update_task_response = update_task(2, UpdateTask {
                description: "Something to do".to_owned(),
            }, &mut ext_cxn, &task_service).await;
            let real_response = update_task_response.into_response();
            
            assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, real_response.status());
            
            let body_bytes_result = body::to_bytes(real_response.into_body()).await;
            let Ok(body_bytes) = body_bytes_result else {
                panic!("Could not extract body: {:#?}", body_bytes_result);
            };
            
            let deserialized_body_result: Result<BasicErrorResponse, _> = serde_json::from_slice(&body_bytes);
            assert_that!(deserialized_body_result).is_ok().matches(|body| {
                body.error_code == "internal_error"
            });
        }
        
        #[tokio::test]
        async fn returns_400_on_bad_input() {
            let task_service = domain::todo::test_util::MockTaskService::new_locked();
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();
            
            let update_task_response = update_task(5, UpdateTask {
                description: String::new(),
            }, &mut ext_cxn, &task_service).await;
            let real_response = update_task_response.into_response();
            
            assert_eq!(StatusCode::BAD_REQUEST, real_response.status());
            
            let body_bytes_result = body::to_bytes(real_response.into_body()).await;
            let Ok(body_bytes) = body_bytes_result else { 
                panic!("Could not extract HTTP body bytes: {:#?}", body_bytes_result);
            };
            
            let deserialized_body_result: Result<BasicErrorResponse, _> = serde_json::from_slice(&body_bytes);
            assert_that!(deserialized_body_result).is_ok().matches(|response_body| {
                response_body.error_code == "invalid_input"
            });
        }
    }
    
    mod delete_task {
        use super::*;
        
        #[tokio::test]
        async fn happy_path() {
            let mut task_service_raw = domain::todo::test_util::MockTaskService::new();
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();
            
            task_service_raw.delete_task_result.set_returned_anyhow(Ok(()));
            let task_service = Mutex::new(task_service_raw);
            
            let delete_task_result = delete_task(5, &mut ext_cxn, &task_service).await;
            let Ok(status) = delete_task_result else {
                panic!("Didn't receive expected response: {:#?}", delete_task_result);
            };
            
            
        }
    }
}
