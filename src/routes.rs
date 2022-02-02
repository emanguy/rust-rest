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
    let users = db::get_users(&mut db_cxn);

    HttpResponse::Ok().json(users)
}
