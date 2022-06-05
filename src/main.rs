use std::env;

use actix_web::*;
use dotenv::dotenv;
use log::*;

mod app_env;
mod db;
mod route_error;
mod routes;
mod dto;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    if dotenv().is_err() {
        println!("Starting server without .env file.");
    }
    env_logger::builder()
        .filter_level(LevelFilter::Info)
        .filter_module("sqlx", LevelFilter::Warn)
        .parse_env(app_env::LOG_LEVEL)
        .init();
    let db_url = env::var(app_env::DB_URL).expect("Could not get database URL from environment");

    let sqlx_db_connection = db::connect_sqlx(db_url.as_str()).await;

    info!("Starting server.");
    return HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(route_error::default_json_error_handler()))
            .app_data(web::Data::new(sqlx_db_connection.clone()))
            .service(routes::hello)
            .configure(routes::add_user_routes)
            .configure(routes::add_task_routes)
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await;
}
