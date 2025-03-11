# Logging

In this microservice template, logging is handled via the [Tracing crate](https://docs.rs/tracing/latest/tracing/).
Logs are aggregated and sent to an OpenTelemetry sink of your choosing - a Jaeger instance is included in the `docker-compose.yml`
file of this repository so you can view both logs and traces. You may log information
at the levels provided by the facade by invoking the provided macros
`debug!()`, `trace!()`, `info!()`, `warn!()`, or `error!()`.

## OpenTelemetry

Code for setting up OpenTelemetry can be found in the [logging.rs](../src/logging.rs) file. This template uses
the environment variables `OTEL_SPAN_EXPORT_URL` and `OTEL_METRIC_EXPORT_URL` to determine where to send logs, traces, and metrics.
These variables are not required, and the default behavior is documented in [app_env.rs](../src/app_env.rs).

Since OpenTelemetry is already integrated into the app, you can take advantage of the ability to record the latency
of your app via "tracing spans", which are just a hierarchical representation of how long different sections of your
code take to run. The [main README](../README.md) explains how to access the Jaeger instance which is set up to capture
your tracing data for local development. OpenTelemetry also comes with support for recording metrics, which are generally
just data points which you can aggregate in dashboards via OpenTelemetry sinks such as Grafana.

The easiest way to record a span is to use the span macros that come with tracing, such as `info_span!` or `debug_span!`.
You can also just decorate a function with the `#[instrument]` macro, which will capture the function arguments:

```rust
// Note that you can skip capturing any arguments you choose
#[instrument(skip(unimportant_or_sensitive_param))]
async fn my_cool_function(important_param: i32, unimportant_or_sensitive_param: &str) {
    // Do something cool...
}
```

More information on creating spans can be found in the [tracing crate documentation](https://docs.rs/tracing/latest/tracing/index.html#events-1).

### Turning off sending tracing data

If you don't want to send tracing or metrics data to an OpenTelemetry sink for whatever reason, you can disable this
functionality by passing `Option::None` to the "otel_exporters" parameter in the call to`setup_logging_and_tracing()` in 
the [main() function](../src/main.rs).

## Filtering Log Messages

The app is set up to accept the logging configuration passed to the `LOG_LEVEL`
environment variable. Formatting details can be found in the [tracing_subscriber](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html)
documentation.

By default, the logger is set up to only allow the info level or higher, or warn
or higher specifically for logs coming from the `sqlx` crate. These defaults
should be able to be overridden via `LOG_LEVEL`, though. Traces will always capture logs
at the debug level or above, though. They are not affected by `LOG_LEVEL`.