use actix_web::*;
use log::*;

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
