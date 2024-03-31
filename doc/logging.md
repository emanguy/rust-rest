# Logging

In this microservice template, logging is handled via the standard [log facade](https://crates.io/crates/log),
with [env_logger](https://crates.io/crates/env_logger) as the underlying implementation. You may log information
at the levels provided by the facade, simply by invoking the provided macros
`debug!()`, `info!()`, `warn!()`, or `error!()`.

## Filtering Log Messages

The app is set up to accept the logging configuration passed to the `LOG_LEVEL`
environment variable. Formatting details can be found in the [env_logger documentation](https://docs.rs/env_logger/0.11.3/env_logger/#enabling-logging).

By default, the logger is set up to only allow the info level or higher, or warn
or higher specifically for logs coming from the `sqlx` crate. These defaults
should be able to be overridden via `LOG_LEVEL`, though.