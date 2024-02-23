use std::sync::Arc;
use axum::http::StatusCode;
use axum::response::ErrorResponse;
use axum::Router;
use axum::routing::{delete, patch};
use log::info;
use validator::Validate;
use crate::{domain, dto, SharedData};
use crate::external_connections::ExternalConnectivity;
use crate::routing_utils::ValidationErrorResponse;

/// Adds routes under "/tasks" and routes for user-owned tasks to the application router
pub fn task_routes() -> Router<Arc<SharedData>> {
    Router::new()
        .route("/tasks/:task_id", patch(update_task))
        .route("/tasks/:task_id", delete(delete_task))
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

    // let update_result = task_service.;
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
    task_id: i32,
    mut ext_cxn: impl ExternalConnectivity,
    task_service: impl domain::todo::driving_ports::TaskPort,
) -> Result<StatusCode, ErrorResponse> {
    info!("Deleting task {task_id}");
    // let delete_result = task_service.;
    match delete_result {
        Ok(_) => Ok(StatusCode::OK),
        Err(db_err) => {
            error!("Failed to delete task: {db_err}");
            Err(DbErrorResponse::from(db_err).into())
        }
    }
}
