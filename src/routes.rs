use actix_web::*;
use log::*;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use validator::Validate;

use crate::{db, dto, route_error::BasicError};

// For a majority of the endpoints I don't use this macro because incomplete handlers generate imprecise error squiggles in the IDE.
/// Sample endpoint that can be used to show the API is responsive.
#[get("/")]
pub async fn hello() -> impl Responder {
    info!("Hello");
    HttpResponse::Ok().body("Hello world!")
}

/// Attaches routes under "/users" to the application router.
pub fn add_user_routes(config: &mut web::ServiceConfig) {
    config
        .route("/users", web::get().to(get_users))
        .route("/users", web::post().to(create_user));
}

/// Retrieves a list of all the users in the system.
async fn get_users(pg_pool: web::Data<PgPool>) -> Result<HttpResponse, BasicError> {
    info!("Requested users");
    let db_cxn = pg_pool.get_ref();
    let users_result = db::get_users(db_cxn).await;
    if users_result.is_err() {
        error!(
            "Could not retrieve users: {}",
            users_result.as_ref().unwrap_err()
        );
    }
    Ok(HttpResponse::Ok().json(users_result.map_err(BasicError::from_db)?))
}

/// Creates a user.
async fn create_user(
    pg_pool: web::Data<PgPool>,
    user_to_create: web::Json<dto::NewUser>,
) -> Result<HttpResponse, BasicError> {
    info!("Attempt to create user: {}", user_to_create);
    user_to_create
        .validate()
        .map_err(BasicError::from_validate)?;

    let db_cxn = pg_pool.get_ref();
    let creation_result = db::create_user(db_cxn, &user_to_create.into_inner()).await;
    if creation_result.is_err() {
        error!(
            "User create failure: {}",
            creation_result.as_ref().unwrap_err()
        );
    }
    Ok(HttpResponse::Created().json(
        creation_result
            .map_err(BasicError::from_db)
            .map(|id| dto::InsertedUser { id })?,
    ))
}

/// Adds routes under "/tasks" and routes for user-owned tasks to the application router
pub fn add_task_routes(config: &mut web::ServiceConfig) {
    config
        .route("/users/{user_id}/tasks", web::get().to(get_tasks_for_user))
        .route(
            "/users/{user_id}/tasks/{task_id}",
            web::get().to(get_task_for_user),
        )
        .route("/users/{user_id}/tasks", web::post().to(add_task_for_user))
        .route("/tasks/{task_id}", web::patch().to(update_task_for_user))
        .route("/tasks/{task_id}", web::delete().to(delete_task_for_user));
}

/// Retrieves a set of tasks owned by a user
async fn get_tasks_for_user(
    pg_pool: web::Data<PgPool>,
    user_id: web::Path<i32>,
) -> Result<HttpResponse, BasicError> {
    info!("Get tasks for user {user_id}");
    let db_cxn = pg_pool.get_ref();
    let unwrapped_user_id = user_id.into_inner();
    let tasks = db::get_tasks_for_user(db_cxn, unwrapped_user_id).await;
    if tasks.is_err() {
        let err = tasks.as_ref().unwrap_err();
        error!("Failed to get user {unwrapped_user_id}'s tasks: {}", err)
    }
    Ok(HttpResponse::Ok().json(tasks.map_err(BasicError::from_db)?))
}

#[derive(Deserialize)]
struct GetTaskPath {
    user_id: i32,
    task_id: i32,
}

/// Retrieves a specific task owned by a user
async fn get_task_for_user(
    pg_pool: web::Data<PgPool>,
    path: web::Path<GetTaskPath>,
) -> Result<HttpResponse, BasicError> {
    info!("Get task {} for user {}", path.task_id, path.user_id);
    let db_cxn = pg_pool.get_ref();
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
    Ok(HttpResponse::Ok().json(task.map_err(BasicError::from_db)?))
}

#[derive(Serialize)]
struct InsertedTask {
    id: i32,
}

/// Adds a new task for a user
async fn add_task_for_user(
    pg_pool: web::Data<PgPool>,
    user_id: web::Path<i32>,
    task_data: web::Json<dto::NewTask>,
) -> Result<HttpResponse, BasicError> {
    info!("Adding task for user {user_id}");
    task_data.validate().map_err(BasicError::from_validate)?;

    let db_cxn = pg_pool.get_ref();
    let unwrapped_user_id = user_id.into_inner();
    let inserted_task =
        db::add_task_for_user(db_cxn, unwrapped_user_id, &task_data.into_inner()).await;
    if inserted_task.is_err() {
        error!(
            "Failed to add task for user {unwrapped_user_id}: {}",
            inserted_task.as_ref().unwrap_err()
        );
    }
    Ok(HttpResponse::Created().json(
        inserted_task
            .map_err(BasicError::from_db)
            .map(|id| InsertedTask { id })?,
    ))
}

/// Updates the content of a task
async fn update_task_for_user(
    pg_pool: web::Data<PgPool>,
    task_id: web::Path<i32>,
    task_data: web::Json<dto::UpdateTask>,
) -> Result<HttpResponse, BasicError> {
    info!("Updating task {task_id}");
    task_data.validate().map_err(BasicError::from_validate)?;

    let db_cxn = pg_pool.get_ref();
    let update_result =
        db::update_user_task(db_cxn, task_id.into_inner(), &task_data.into_inner()).await;
    match update_result {
        Ok(_) => Ok(HttpResponse::Ok().finish()),
        Err(db_err) => {
            error!("Update task failure: {db_err}");
            Err(BasicError::from_db(db_err))
        }
    }
}

/// Deletes a task
async fn delete_task_for_user(
    pg_pool: web::Data<PgPool>,
    task_id: web::Path<i32>,
) -> Result<HttpResponse, BasicError> {
    info!("Deleting task {task_id}");
    let db_cxn = pg_pool.get_ref();
    let delete_result = db::delete_user_task(db_cxn, task_id.into_inner()).await;
    match delete_result {
        Ok(_) => Ok(HttpResponse::Ok().finish()),
        Err(db_err) => {
            error!("Failed to delete task: {db_err}");
            Err(BasicError::from_db(db_err))
        }
    }
}
