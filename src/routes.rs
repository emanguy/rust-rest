use std::{ops::DerefMut};
use std::ops::Deref;

use actix_web::*;
use actix_web::body::Body;
use log::*;
use serde::Serialize;

use crate::db::{PgPool, self};

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

pub async fn get_users(pg_pool: web::Data<PgPool>) -> impl Responder {
    info!("Requested users");
    let mut db_cxn =  match pg_pool.get_ref().get() {
        Ok(cxn) => cxn,
        Err(_) => return HttpResponse::InternalServerError().body("Failed to connect to database"),
    };
    let users = db::get_users(db_cxn.deref_mut());
    match users {
        Ok(users) => HttpResponse::Ok().json(users),
        Err(db_err) => {
            error!("User retrieve failure: {}", db_err);
            HttpResponse::InternalServerError().json("Failed to retrieve users")
        },
    }
}

pub fn add_task_routes(config: &mut web::ServiceConfig) {
    config.service(get_tasks_for_user)
        .service(add_task_for_user)
        .service(update_task_for_user)
        .service(delete_task_for_user);
}

#[get("/users/{user_id}/tasks")]
pub async fn get_tasks_for_user(pg_pool: web::Data<PgPool>, web::Path(user_id): web::Path<i32>) -> impl Responder {
    info!("Get tasks for user {user_id}");
    let mut db_cxn = match pg_pool.get_ref().get() {
        Ok(cxn) => cxn,
        Err(_) => return HttpResponse::InternalServerError().body("Failed to connect to database"),
    };
    let tasks = db::get_tasks_for_user(db_cxn.deref_mut(), user_id);
    match tasks {
        Ok(tasks) => HttpResponse::Ok().json(tasks),
        Err(db_err) => {
            error!("Task retrieve failure: {}", db_err);
            HttpResponse::InternalServerError().body("Failed to retrieve tasks")
        }
    }
}

#[derive(Serialize)]
struct InsertedTask {
    id: i32,
}

#[post("/users/{user_id}/tasks")]
pub async fn add_task_for_user(pg_pool: web::Data<PgPool>, web::Path(user_id): web::Path<i32>, task_data: web::Json<db::NewTask>) -> impl Responder {
    info!("Adding task for user {user_id}");
    let mut db_cxn = match pg_pool.get_ref().get() {
        Ok(cxn) => cxn,
        Err(_) => return HttpResponse::InternalServerError().body("Failed to connect to database"),
    };
    let inserted_task = db::add_task_for_user(db_cxn.deref_mut(), user_id, task_data.deref());
    match inserted_task {
        Ok(id) => HttpResponse::Created().json(InsertedTask { id }),
        Err(db_err) => {
            error!("Insert task failure: {db_err}");
            HttpResponse::InternalServerError().body("Failed to add task.")
        },
    }
}

#[patch("/tasks/{task_id}")]
pub async fn update_task_for_user(pg_pool: web::Data<PgPool>, web::Path(task_id): web::Path<i32>, task_data: web::Json<db::UpdateTask>) -> impl Responder {
    info!("Updating task {task_id}");
    let mut db_cxn = match pg_pool.get_ref().get() {
        Ok(cxn) => cxn,
        Err(_) => return HttpResponse::InternalServerError().body("Failed to connect to database"),
    };
    let update_result = db::update_user_task(db_cxn.deref_mut(), task_id, task_data.deref());
    match update_result {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(db_err) => {
            error!("Update task failure: {db_err}");
            HttpResponse::InternalServerError().body("Failed to add task.")
        }
    }
}

#[delete("/tasks/{task_id}")]
pub async fn delete_task_for_user(pg_pool: web::Data<PgPool>, web::Path(task_id): web::Path<i32>) -> impl Responder {
    info!("Deleting task {task_id}");
    let mut db_cxn = match pg_pool.get_ref().get() {
        Ok(cxn) => cxn,
        Err(_) => return HttpResponse::InternalServerError().body("Failed to connect to database"),
    };
    let delete_result = db::delete_user_task(db_cxn.deref_mut(), task_id);
    match delete_result {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(db_err) => {
            error!("Failed to delete task: {db_err}");
            HttpResponse::InternalServerError().body("Failed to delete task.")
        }
    }
}
