//! This module exists to show OpenTelemetry propagation across services (even though
//! it's just sending a message to its own endpoint)
//!
//! Do not use this as a reference for Hexagonal Architecture as described by the docs.

use crate::external_connections::ExternalConnectivity;
use crate::routing_utils::GenericErrorResponse;
use crate::{AppState, SharedData, dto};
use anyhow::anyhow;
use axum::Router;
use axum::extract::State;
use axum::response::ErrorResponse;
use axum::routing::*;
use std::sync::Arc;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(paths(trace_demo, trace_demo_part2))]
/// Defines the OpenAPI documentation for the tracing demo API
pub struct TracingApi;
/// Constant used to group tracing endpoints in OpenAPI documentation
pub const TRACE_API_GROUP: &str = "Tracing";

/// Creates a router for endpoints under the "/tracing-demo" group of APIs
pub fn tracing_routes() -> Router<Arc<SharedData>> {
    Router::new()
        .route(
            "/",
            get(async |State(app_state): AppState| trace_demo(&app_state.ext_cxn.clone()).await),
        )
        .route("/part2", get(trace_demo_part2))
}

#[utoipa::path(
    get,
    path = "/tracing-demo",
    tag = TRACE_API_GROUP,
    responses(
        (status = 200, description = "Received message from self"),
        (status = 500, response = dto::err_resps::BasicError500)
    ),
)]
#[tracing::instrument(skip_all, ret)]
/// Sends an HTTP request to this server to emit a "cross server" trace
async fn trace_demo(ext_cxn: &impl ExternalConnectivity) -> Result<String, ErrorResponse> {
    let http_client = ext_cxn.http_client();
    let server2_msg: String = http_client
        .get("http://localhost:8080/tracing-demo/part2")
        .send()
        .await
        .map_err(|err| GenericErrorResponse(anyhow!(err)))?
        .text()
        .await
        .map_err(|err| GenericErrorResponse(anyhow!(err)))?;

    Ok(format!("Got message: {server2_msg}"))
}

#[utoipa::path(
    get,
    path = "/tracing-demo/part2",
    tag = TRACE_API_GROUP,
    responses(
        (status = 200, description = "Sent sample payload"),
    ),
)]
#[tracing::instrument(ret)]
/// Receives the "cross server" trace and sends a string back
async fn trace_demo_part2() -> &'static str {
    "Hello, Rust server!"
}
