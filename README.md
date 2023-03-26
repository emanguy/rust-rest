# Rust REST Server

Test of building a REST API using Axum with the SQLX PostgreSQL connection pool. Start the server with `cargo run`. Run tests with `cargo test`.

_Note that the server requires a PostgreSQL database to run properly. A docker compose file is provided to start the database at your convenience._

## Benchmark

Did a quick load test on the server using `wrk` using 12 threads with 400 open requests for 30 seconds. These are the results:

```
Running 30s test @ http://localhost:8082/users
  12 threads and 400 connections
  Thread Stats   Avg      Stdev     Max   +/- Stdev
    Latency    75.34ms   69.52ms   1.09s    99.05%
    Req/Sec   149.41     35.94   270.00     67.53%
  51805 requests in 30.04s, 13.19MB read
  Socket errors: connect 157, read 171, write 0, timeout 0
Requests/sec:   1724.63
Transfer/sec:    449.68KB
```

Crazy that a majority of the requests responded in <100ms. When using my normal rest client with no load the server replies in <10ms.

Additionally, a majority of the connection drop errors just happened because the OS couldn't keep up. There were some infrequent "too many open files" errors.

## Integration tests

Provided on this repo is a framework for integration testing. By default, the integration tests are skipped via the `#[cfg_attr()]` declaration which requires the `integration_test` feature to be enabled.

To run the tests with the integration tests, run the following:

```bash
# Create a postgres database to test against
docker-compose up -d
# Run all tests, including integration tests
cargo test --features integration_test
```

### Marking a test as an integration test

Tests can be marked as integration tests by conditionally adding the "ignore" attribute to a test function based on the presence of the "integration_test" feature:

```rust
#[test]
#[cfg_attr(not(feature = "integration_test"), ignore)]
fn my_test() {
  // ...
}
```
