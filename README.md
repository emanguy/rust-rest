# Rust REST Server

Test of building a REST API using Actix with the R2D2 Postgres connection pool. Start the server with `cargo run`.

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

## SqlX

I tried switching to SqlX and while it is nicer to work with queries are slightly slower, on the order of about 50-70ms per request under heavy load and 5-8 under light. It's probably an acceptable slowdown and it's also async.
