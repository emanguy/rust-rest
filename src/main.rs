use axum::Router;
use axum::extract::State;
use dotenv::dotenv;
use std::env;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::*;

mod api;
mod app_env;
mod db;
mod domain;
mod dto;
mod logging;
mod persistence;
mod routing_utils;

mod external_connections;
#[cfg(test)]
mod integration_test;

/// Global data store which is shared among HTTP routes
pub struct SharedData {
    pub ext_cxn: persistence::ExternalConnectivity,
}

/// Type alias for the extractor used to get access to the global app state
type AppState = State<Arc<SharedData>>;

#[tokio::main]
async fn main() {
    if dotenv().is_err() {
        println!("Starting server without .env file.");
    }
    let span_url = env::var(app_env::OTEL_SPAN_EXPORT_URL)
        .unwrap_or_else(|_| "http://localhost:4317".to_owned());
    let metric_url = env::var(app_env::OTEL_METRIC_EXPORT_URL)
        .unwrap_or_else(|_| "http://localhost:4317".to_owned());
    logging::setup_logging_and_tracing(
        logging::init_env_filter(),
        Some(logging::init_exporters(&span_url, &metric_url)),
    );
    let db_url = env::var(app_env::DB_URL).expect("Could not get database URL from environment");

    let sqlx_db_connection = db::connect_sqlx(&db_url).await;
    let ext_cxn = persistence::ExternalConnectivity::new(sqlx_db_connection);

    let router = Router::new()
        .nest("/users", api::user::user_routes())
        .nest("/tasks", api::todo::task_routes())
        .nest("/tracing-demo", api::tracing_prop::tracing_routes())
        .merge(api::swagger_main::build_documentation())
        .with_state(Arc::new(SharedData { ext_cxn }));
    let router = logging::attach_tracing_http(router);

    info!("Starting server.");
    let network_listener = match TcpListener::bind(&"0.0.0.0:8080").await {
        Ok(listener) => listener,
        Err(bind_err) => panic!("Could not listen on requested port! {}", bind_err),
    };
    axum::serve(network_listener, router.into_make_service())
        .await
        .unwrap();
}
