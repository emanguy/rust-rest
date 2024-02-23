use std::sync::Arc;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::ErrorResponse;
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
            let ext_cxn = app_state.ext_cxn.clone();
            let task_service = domain::todo::TaskService{};
            
            update_task(task_id, update, ext_cxn, task_service).await
        }))
        .route("/tasks/:task_id", delete(|State(app_state): AppState, Path(task_id): Path<i32>| async move {
            let ext_cxn = app_state.ext_cxn.clone();
            let task_service = domain::todo::TaskService{};
            
            delete_task(task_id, ext_cxn, task_service).await
        }))
}

/// Updates the content of a task
async fn update_task(
    task_id: i32,
    task_data: dto::UpdateTask,
    mut ext_cxn: impl ExternalConnectivity,
    task_service: impl domain::todo::driving_ports::TaskPort,
) -> Result<StatusCode, ErrorResponse> {
    info!("Updating task {task_id}");
    task_data
        .validate()
        .map_err(ValidationErrorResponse::from)?;

    let domain_update = domain::todo::UpdateTask::from(task_data);
    let task_writer = persistence::db_todo_driven_ports::DbTaskWriter{};
    
    let update_result = task_service.update_task(task_id, &domain_update, &mut ext_cxn, &task_writer).await;
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
    mut ext_cxn: impl ExternalConnectivity,
    task_service: impl domain::todo::driving_ports::TaskPort,
) -> Result<StatusCode, ErrorResponse> {
    info!("Deleting task {task_id}");
    let task_write = persistence::db_todo_driven_ports::DbTaskWriter{};
    
    let delete_result = task_service.delete_task(task_id, &mut ext_cxn, &task_write).await;
    match delete_result {
        Ok(_) => Ok(StatusCode::OK),
        Err(db_err) => {
            error!("Failed to delete task: {db_err}");
            Err(GenericErrorResponse(db_err).into())
        }
    }
}


