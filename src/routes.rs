use actix_web::*;
use log::*;
use serde::{Serialize, Deserialize};
use sqlx::PgPool;

use crate::db::{self, DbError};

#[get("/")]
pub async fn hello() -> impl Responder {
    info!("Hello");
    HttpResponse::Ok().body("Hello world!")
}

#[post("/echo")]
pub async fn echo(request_body: String) -> impl Responder {
    info!("Echo. Got body {}", &request_body);
    HttpResponse::Ok().body(request_body)
}

pub async fn manual_hello() -> impl Responder {
    info!("Manual hello");
    HttpResponse::Ok().body("Hey there!")
}


pub fn add_user_routes(config: &mut web::ServiceConfig) {
    config
        .route("/users", web::get().to(get_users))
        .route("/users", web::post().to(create_user));
}

pub async fn get_users(pg_pool: web::Data<PgPool>) -> impl Responder {
    info!("Requested users");
    let db_cxn = pg_pool.get_ref();
    let users = db::get_users(db_cxn).await;
    match users {
        Ok(users) => HttpResponse::Ok().json(users),
        Err(db_err) => {
            error!("User retrieve failure: {}", db_err);
            HttpResponse::InternalServerError().json("Failed to retrieve users")
        },
    }
}

pub async fn create_user(pg_pool: web::Data<PgPool>, user_to_create: web::Json<db::NewUser>) -> impl Responder {
    info!("Attempt to create user: {}", user_to_create);
    let db_cxn = pg_pool.get_ref();
    let creation_result = db::create_user(db_cxn, &user_to_create.into_inner()).await;
    match creation_result {
        Ok(user_id) => HttpResponse::Ok().json(user_id),
        Err(db_err) => {
            error!("User create failure: {}", db_err);
            HttpResponse::InternalServerError().json("Failed to create user")
        }
    }
}

// Testing adding a "controller" to the app
pub fn add_task_routes(config: &mut web::ServiceConfig) {
    config.service(get_tasks_for_user)
        .service(get_task_for_user)
        .service(add_task_for_user)
        .service(update_task_for_user)
        .service(delete_task_for_user);
}

#[get("/users/{user_id}/tasks")]
pub async fn get_tasks_for_user(pg_pool: web::Data<PgPool>, user_id: web::Path<i32>) -> impl Responder {
    info!("Get tasks for user {user_id}");
    let db_cxn = pg_pool.get_ref();
    let tasks = db::get_tasks_for_user(db_cxn, user_id.into_inner()).await;
    match tasks {
        Ok(tasks) => HttpResponse::Ok().json(tasks),
        Err(db_err) => {
            error!("Task retrieve failure: {}", db_err);
            HttpResponse::InternalServerError().body("Failed to retrieve tasks")
        }
    }
}

#[derive(Deserialize)]
pub struct GetTaskPath {
    user_id: i32,
    task_id: i32,
}

#[get("/users/{user_id}/tasks/{task_id}")]
pub async fn get_task_for_user(pg_pool: web::Data<PgPool>, path: web::Path<GetTaskPath>) -> impl Responder {
    info!("Get task {} for user {}", path.task_id, path.user_id);
    let db_cxn = pg_pool.get_ref();
    let task = db::get_task_for_user(db_cxn, path.user_id, path.task_id).await;
    match task {
        Ok(task) => HttpResponse::Ok().json(task),
        Err(db_err) => {
            
            match db_err {
                DbError::NoResults => {
                    warn!("Task not found.");
                    HttpResponse::NotFound().body("Task not found.")
                },
                _ => {
                    error!("Task retrieve failure: {}", db_err);
                    HttpResponse::InternalServerError().body("Failed to retrieve task")
                },
            }
        }
    }
}

#[derive(Serialize)]
struct InsertedTask {
    id: i32,
}

#[post("/users/{user_id}/tasks")]
pub async fn add_task_for_user(pg_pool: web::Data<PgPool>, user_id: web::Path<i32>, task_data: web::Json<db::NewTask>) -> impl Responder {
    info!("Adding task for user {user_id}");
    let db_cxn = pg_pool.get_ref();
    let inserted_task = db::add_task_for_user(db_cxn, user_id.into_inner(), &task_data.into_inner()).await;
    match inserted_task {
        Ok(id) => HttpResponse::Created().json(InsertedTask { id }),
        Err(db_err) => {
            error!("Insert task failure: {db_err}");
            HttpResponse::InternalServerError().body("Failed to add task.")
        },
    }
}

#[patch("/tasks/{task_id}")]
pub async fn update_task_for_user(pg_pool: web::Data<PgPool>, task_id: web::Path<i32>, task_data: web::Json<db::UpdateTask>) -> impl Responder {
    info!("Updating task {task_id}");
    let db_cxn = pg_pool.get_ref();
    let update_result = db::update_user_task(db_cxn, task_id.into_inner(), &task_data.into_inner()).await;
    match update_result {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(db_err) => {
            error!("Update task failure: {db_err}");
            HttpResponse::InternalServerError().body("Failed to add task.")
        }
    }
}

#[delete("/tasks/{task_id}")]
pub async fn delete_task_for_user(pg_pool: web::Data<PgPool>, task_id: web::Path<i32>) -> impl Responder {
    info!("Deleting task {task_id}");
    let db_cxn = pg_pool.get_ref();
    let delete_result = db::delete_user_task(db_cxn, task_id.into_inner()).await;
    match delete_result {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(db_err) => {
            error!("Failed to delete task: {db_err}");
            HttpResponse::InternalServerError().body("Failed to delete task.")
        }
    }
}
