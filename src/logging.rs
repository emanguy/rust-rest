use crate::app_env;
use axum::Router;
use axum::body::Body;
use axum::http::{Request, Response};
use opentelemetry::trace::TracerProvider;
use opentelemetry::{KeyValue, global};
use opentelemetry_http::HeaderExtractor;
use opentelemetry_otlp::{MetricExporter, SpanExporter, WithExportConfig};
use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::Tracer;
use opentelemetry_sdk::{Resource, runtime};
use std::time::Duration;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::level_filters::LevelFilter;
use tracing::{Span, debug, debug_span, field};
use tracing_opentelemetry::{MetricsLayer, OpenTelemetryLayer, OpenTelemetrySpanExt};
use tracing_subscriber::{EnvFilter, prelude::*, registry};

/// The name of the service as it should appear in OpenTelemetry collectors
const SERVICE_NAME: &str = "sample-rest";

/// Struct containing OpenTelemetry primitives which export data to a tracing server
pub struct OtelExporters {
    pub tracer: Tracer,
    pub meter: SdkMeterProvider,
}

/// Attaches a tracing middleware layer to the given router.
pub fn attach_tracing_http<T>(router: Router<T>) -> Router<T>
where
    T: Clone + Send + Sync + 'static,
{
    router.layer(
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
}

/// Instantiates OpenTelemetry exporters which run in the background and send tracing/logging/metrics
/// data to an opentelemetry-compatible gRPC endpoint (typically http://localhost:4317 with a standard
/// sidecar setup)
pub fn init_exporters(otlp_traces_endpoint: &str, otlp_metrics_endpoint: &str) -> OtelExporters {
    let span_export = SpanExporter::builder()
        .with_tonic()
        .with_endpoint(otlp_traces_endpoint)
        .build()
        .expect("failed to build span exporter");
    let meter_export = MetricExporter::builder()
        .with_tonic()
        .with_endpoint(otlp_metrics_endpoint)
        .build()
        .expect("failed to build meter exporter");

    let metrics_reader = PeriodicReader::builder(meter_export, runtime::Tokio).build();

    let trace_provider = opentelemetry_sdk::trace::TracerProvider::builder()
        .with_batch_exporter(span_export, runtime::Tokio)
        .with_resource(Resource::new([KeyValue::new("service.name", SERVICE_NAME)]))
        .build()
        .tracer(SERVICE_NAME);
    let meter_provider = SdkMeterProvider::builder()
        .with_reader(metrics_reader)
        .with_resource(Resource::new([KeyValue::new("service.name", SERVICE_NAME)]))
        .build();

    OtelExporters {
        tracer: trace_provider,
        meter: meter_provider,
    }
}

/// Constructs a filter which uses [app_env::LOG_LEVEL] to configure per-module logging. Filters
/// to the "info" level by default.
pub fn init_env_filter() -> EnvFilter {
    EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .with_env_var(app_env::LOG_LEVEL)
        .from_env()
        .expect("building the logging filter failed")
}

/// Sets up the global logging and tracing sinks. All logs and metrics at the "debug" level and above
/// will automatically be sent to OpenTelemetry sinks if [otel_exporters] is provided. [env_filter] is
/// applied specifically to the JSON logger printing to stdout. Though the logger is set up with the
/// "tracing" crate, it also provides a bridge for libraries still using "log" for logging.
pub fn setup_logging_and_tracing(env_filter: EnvFilter, otel_exporters: Option<OtelExporters>) {
    global::set_text_map_propagator(TraceContextPropagator::new());

    if let Some(exporters) = otel_exporters {
        registry()
            .with(LevelFilter::DEBUG)
            .with(OpenTelemetryLayer::new(exporters.tracer))
            .with(MetricsLayer::new(exporters.meter))
            .with(
                tracing_subscriber::fmt::layer()
                    .json()
                    .with_filter(env_filter),
            )
            .init();
    } else {
        registry()
            .with(LevelFilter::DEBUG)
            .with(
                tracing_subscriber::fmt::layer()
                    .json()
                    .with_filter(env_filter),
            )
            .init();
    }
}
