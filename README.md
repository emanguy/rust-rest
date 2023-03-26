# Rust REST Server

Test of building a REST API using Axum with the SQLX PostgreSQL connection pool. Start the server with `cargo run`. Run tests with `cargo test`.

_Note that the server requires a PostgreSQL database to run properly. A docker compose file is provided to start the database at your convenience._

## Benchmark

Did a quick load test on the server using `oha` running for 5 minutes:

```
❯ oha -z 5m http://localhost:8080/users/1/tasks
Summary:
  Success rate:	1.0000
  Total:	300.0010 secs
  Slowest:	0.6709 secs
  Fastest:	0.0054 secs
  Average:	0.0324 secs
  Requests/sec:	1543.8113

  Total data:	63.60 MiB
  Size/request:	144 B
  Size/sec:	217.10 KiB

Response time histogram:
  0.014 [1972]   |
  0.022 [16050]  |■■
  0.030 [169591] |■■■■■■■■■■■■■■■■■■■■■■■■■
  0.039 [212188] |■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■
  0.047 [46930]  |■■■■■■■
  0.055 [11667]  |■
  0.064 [3211]   |
  0.072 [952]    |
  0.080 [289]    |
  0.089 [49]     |
  0.097 [246]    |

Latency distribution:
  10% in 0.0245 secs
  25% in 0.0279 secs
  50% in 0.0317 secs
  75% in 0.0353 secs
  90% in 0.0409 secs
  95% in 0.0451 secs
  99% in 0.0556 secs

Details (average, fastest, slowest):
  DNS+dialup:	0.0012 secs, 0.0007 secs, 0.0014 secs
  DNS-lookup:	0.0000 secs, 0.0000 secs, 0.0001 secs

Status code distribution:
  [200] 463145 responses
```

Most frequently, the server responds in about 39ms and it was able to process 463k requests in 5 minutes. Needless to say Rust web servers are FAST.

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
