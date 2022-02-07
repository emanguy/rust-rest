use std::{ops::DerefMut};

use actix_web::*;
use log::*;


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

#[get("/users/{user_id}/tasks")]
pub async fn get_tasks_for_user(pg_pool: web::Data<PgPool>, web::Path(user_id): web::Path<i32>) -> impl Responder {
    info!("Get tasks for user {}", user_id);
    let mut db_cxn = match pg_pool.get_ref().get() {
        Ok(cxn) => cxn,
        Err(_) => return HttpResponse::InternalServerError().body("Failed to connect to database"),
    };
    let tasks = db::get_tasks_for_user(db_cxn.deref_mut(), user_id);
    match tasks {
        Ok(tasks) => HttpResponse::Ok().json(tasks),
        Err(db_err) => {
            error!("Task retrieve failure: {}", db_err);
            HttpResponse::InternalServerError().json("Failed to retrieve tasks")
        }
    }
}
