use std::env;
use std::sync::Arc;

use axum::extract::State;
use axum::routing::get;
use axum::Router;
use dotenv::dotenv;
use log::*;
use sqlx::PgPool;

mod api;
mod app_env;
mod db;
mod domain;
mod dto;
// mod entity;
mod persistence;
// mod routes;
mod routing_utils;

mod external_connections;
#[cfg(test)]
mod integration_test;

/// Configures the logging system for the application. Pulls configuration from the [LOG_LEVEL](app_env::LOG_LEVEL)
/// environment variable. Sets log level to "INFO" for all modules and sqlx to "WARN" by default.
pub fn configure_logger() {
    env_logger::builder()
        .filter_level(LevelFilter::Info)
        .filter_module("sqlx", LevelFilter::Warn)
        .parse_env(app_env::LOG_LEVEL)
        .init();
}

pub struct SharedData {
    pub ext_cxn: persistence::ExternalConnectivity,
}

type AppState = State<Arc<SharedData>>;

#[tokio::main]
async fn main() {
    if dotenv().is_err() {
        println!("Starting server without .env file.");
    }
    configure_logger();
    let db_url = env::var(app_env::DB_URL).expect("Could not get database URL from environment");

    let sqlx_db_connection = db::connect_sqlx(&db_url).await;
    let ext_cxn = persistence::ExternalConnectivity::new(sqlx_db_connection);
    
    let router = Router::new()
        .nest("/users", api::user::user_routes())
        .nest("/tasks", api::todo::task_routes())
        .with_state(Arc::new(SharedData {
            ext_cxn,
        }));

    info!("Starting server.");
    axum::Server::bind(&"0.0.0.0:8080".parse().unwrap())
        .serve(router.into_make_service())
        .await
        .unwrap();
}
