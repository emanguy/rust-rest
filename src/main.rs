use actix_web::*;
use log::*;
use dotenv::dotenv;


mod routes;
mod db;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    env_logger::builder().filter_level(LevelFilter::Info).init();
    // let db_url = env::var("DATABASE_URL").expect("Could not get database URL from environment");    

    let db_connection = db::connect();

    info!("Starting server.");
    return HttpServer::new(move || {
        App::new()
            .data(db_connection.clone())
            .service(routes::hello)
            .service(routes::echo)
            .route("/hey", web::get().to(routes::manual_hello))
            .route("/users", web::get().to(routes::get_users))
            .service(routes::get_tasks_for_user)
    })
        .bind("0.0.0.0:8082")?
        .run()
        .await;
}
