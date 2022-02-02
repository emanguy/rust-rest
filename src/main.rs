use actix_web::*;
use log::*;

mod routes;
mod db;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::builder().filter_level(LevelFilter::Info).init();
    info!("Starting server.");
    HttpServer::new(|| {
        App::new()
            .service(routes::hello)
            .service(routes::echo)
            .route("/hey", web::get().to(routes::manual_hello))
    })
        .bind("0.0.0.0:8080")?
        .run()
        .await
}
