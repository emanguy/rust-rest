use actix_web::*;
use log::*;
use dotenv::dotenv;


mod routes;
mod db;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().expect("Failed to initialize dotenv");
    env_logger::builder()
        .filter_level(LevelFilter::Info)
        .filter_module("sqlx", LevelFilter::Warn)
        .init();
    // let db_url = env::var("DATABASE_URL").expect("Could not get database URL from environment");    

    let sqlx_db_connection = db::connect_sqlx().await;

    info!("Starting server.");
    return HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(sqlx_db_connection.clone()))
            .service(routes::hello)
            .service(routes::echo)
            .route("/hey", web::get().to(routes::manual_hello))
            .route("/users", web::get().to(routes::get_users))
            .route("/users", web::post().to(routes::create_user))
            .configure(routes::add_task_routes)

    })
        .bind("0.0.0.0:8080")?
        .run()
        .await;
}
