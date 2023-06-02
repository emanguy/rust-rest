use std::env;
use std::sync::Arc;

use axum::extract::State;
use axum::routing::get;
use axum::Router;
use dotenv::dotenv;
use log::*;
use sqlx::PgPool;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

mod app_env;
mod db;
mod dto;
mod entity;
mod routes;
mod routing_utils;

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
    pub db: PgPool,
}

type AppState = State<Arc<SharedData>>;

#[derive(OpenApi)]
#[openapi(
    components(
        schemas(routing_utils::ExtraInfo, routing_utils::ValidationErrorSchema),
        responses(routing_utils::BasicErrorResponse)
    ),
    info(
        title = "Rust Todo API",
        description = "A simple to-do list API written in Rust"
    )
)]
pub struct TodoApi;

fn build_documentation() -> SwaggerUi {
    let mut api_docs = TodoApi::openapi();
    api_docs.merge(routes::UsersApi::openapi());
    api_docs.merge(routes::TasksApi::openapi());
    api_docs.merge(entity::SystemEntities::openapi());
    api_docs.merge(dto::DtoEntities::openapi());

    SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", api_docs)
}

#[tokio::main]
async fn main() {
    if dotenv().is_err() {
        println!("Starting server without .env file.");
    }
    configure_logger();
    let db_url = env::var(app_env::DB_URL).expect("Could not get database URL from environment");

    let sqlx_db_connection = db::connect_sqlx(&db_url).await;
    let documentation = build_documentation();

    let router = Router::new()
        .route("/hello", get(routes::hello))
        .merge(documentation)
        .merge(routes::user_routes())
        .merge(routes::task_routes())
        .with_state(Arc::new(SharedData {
            db: sqlx_db_connection,
        }));

    info!("Starting server.");
    axum::Server::bind(&"0.0.0.0:8080".parse().unwrap())
        .serve(router.into_make_service())
        .await
        .unwrap();
}
