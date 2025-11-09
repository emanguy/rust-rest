use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, Response};
use dotenv::dotenv;
use opentelemetry::global;
use opentelemetry_http::HeaderExtractor;
use std::env;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::*;
use tracing_opentelemetry::OpenTelemetrySpanExt;

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
        .layer(
            ServiceBuilder::new().layer(
                TraceLayer::new_for_http()
                    .make_span_with(|request: &Request<Body>| {
                        let req_span = debug_span!(
                            "request",
                            method = &request.method().as_str(),
                            path = request.uri().path(),
                            response_status = field::Empty,
                        );

                        req_span.set_parent(global::get_text_map_propagator(|propagator| {
                            propagator.extract(&HeaderExtractor(request.headers()))
                        }));

                        req_span
                    })
                    .on_response(
                        |response: &Response<Body>, _latency: Duration, span: &Span| {
                            span.record("response_status", field::display(response.status()));
                            debug!("request processing complete");
                        },
                    ),
            ),
        )
        .with_state(Arc::new(SharedData { ext_cxn }));

    info!("Starting server.");
    let network_listener = match TcpListener::bind(&"0.0.0.0:8080").await {
        Ok(listener) => listener,
        Err(bind_err) => panic!("Could not listen on requested port! {}", bind_err),
    };
    axum::serve(network_listener, router.into_make_service())
        .await
        .unwrap();
}
