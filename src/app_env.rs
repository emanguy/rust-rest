/// URL for accessing the PostrgeSQL database (should contain a schema name in the path)
pub const DB_URL: &str = "DATABASE_URL";
/// Log level configuration for the application. For formatting info, see [env_logger's documentation](https://docs.rs/env_logger/latest/env_logger/#enabling-logging)
pub const LOG_LEVEL: &str = "LOG_LEVEL";

/// OpenTelemetry span export URL. Should be http://localhost:4317 by default, as the service should
/// have an OpenTelemetry collector sidecar which directs metrics to the correct place
pub const OTEL_SPAN_EXPORT_URL: &str = "OTEL_SPAN_EXPORT_URL";
/// OpenTelemetry metrics export URL. Should be http://localhost:4317 by default, as the service should
/// have an OpenTelemetry collector sidecar which directs metrics to the correct place
pub const OTEL_METRIC_EXPORT_URL: &str = "OTEL_METRIC_EXPORT_URL";

#[cfg(test)]
pub mod test {
    /// URL for accessing the PostgreSQL database during integration tests (should not contain a schema name in the path)
    pub const TEST_DB_URL: &str = "TEST_DB_URL";
}
