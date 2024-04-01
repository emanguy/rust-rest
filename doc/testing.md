# Testing

Since the template uses Hexagonal Architecture, it is relatively easy to test both the business logic and routing
logic in isolation. This is achieved by providing fakes for the driven ports in the case of business logic, or mocking
the driving port in the case of HTTP routing. This template also provides support for writing integration tests against
a real database.

The examples in this document use hexagonal architecture terms and provide tests for the "player API" for a
hypothetical video game backend described in [the Architecture documentation](./architecture_layers.md). It is recommended
that you familiarize yourself with the examples provided there before reading through the testing examples here.

## Unit Testing Business Logic

Business logic is most easily tested by defining fakes for the driven ports the logic communicates with. We'll start with
a sample implementation of just the player detection fake, then use that assuming `domain::player::driven_ports::PlayerWriter`
is implemented on it to write a whole test for the `new_player()` function defined on the player driving port.

### Implementing a fake

To implement a fake, define a struct which implements the driven port trait and mimics the behavior of the real
thing. It is recommended that you define the trait implementation on a synchronization primitive containing your
struct, as implementing a fake typically requires mutable access to the struct and driven port implementations
are usually passed as immutable references. Using a synchronization primitive will allow for safe interior mutability
via an immutable reference.

Normally, `mockall::automock` could be used to generate mocks for traits, but at the time of writing it doesn't work
well for traits containing async methods. Besides, fakes make tests easier to write because they mimic the behavior
of the real systems the business logic tries to interface with.

The `domain::test_util` module defines a re-usable `Connectivity` enum that can be composed in fakes to simulate
connectivity failure. When `Connectivity::Disconnected` is used, it is expected that any function on the fake will
return an error instead of performing typical fake behavior.

This is how `domain::player::driven_ports::PlayerWriter` might be implemented as a fake:

<details>
<summary>Fake implementation example</summary>

```rust
// in domain/player.rs

// ...business logic/domain objects/port definitions

// Re-usable test utilities are defined under a test_util submodule below the module where relevant traits are defined
#[cfg(test)]
pub mod test_util {
    use std::sync::RwLock;
    use super::*;

    pub struct InMemoryPlayerPersistence {
        // This will determine if we're actively "connected" to the thing on the other end of the port
        pub connectivity: Connectivity,

        // We'll store the created players in this vector
        pub players: Vec<Player>,
    }

    impl InMemoryPlayerPersistence {
        // Define a constructor for the fake
        pub fn new() -> InMemoryPlayerPersistence {
            Self {
                connectivity: Connectivity::Connected,
                players: Vec::new(),
            }
        }

        // It can also be handy to provide a constructor which wraps the type inside the synchronization
        // primitive
        pub fn new_locked() -> RwLock<InMemoryPlayerPersistence> {
            RwLock::new(Self::new())
        }
    }

    // Now we can implement the driven port trait on the fake, specifically when it's wrapped
    // in the synchronization primitive
    impl driven_ports::PlayerDetector for RwLock<InMemoryPlayerPersistence> {
        async fn player_with_username_exists(
            &self,
            username: &str,
            _ext_cxn: &mut impl ExternalConnectivity
        ) -> Result<bool, anyhow::Error> {
            // First, we need to acquire a lock on the fake
            let self_locked = self.read().unwrap();

            // Next, blow up in the event the port is in a disconnected state
            self_locked.connectivity.blow_up_if_disconnected()?;

            // Next, implement the fake's logic
            let matching_username_exists = self_locked.players.iter().any(|player| player.username == username);
            Ok(matching_username_exists)
        }
    }
    
    // We can then implement PlayerWriter and other driven ports on InMemoryPlayerPersistence down here
}
```

</details>

### Using the fake in a test

With the fake defined, we can use it to fake the functionality of driven ports in business logic tests. We'll define a
simple happy path test and error test for PlayerService this way.

The `external_connections::test_util` module defines a fake `ExternalConnectivity` instance we can use for testing
on top of the fake we just implemented.

<details>
<summary>Code example for testing business logic with a fake</summary>

```rust
// in domain/player.rs

// ...business logic/domain objects/port definitions

#[cfg(test)]
mod tests {
    // As much as possible, try to keep your imports at the top level of the test module and inherit
    // them into submodules
    use super::*;
    use speculoos::prelude::*;
    use std::sync::RwLock;

    // Keeping short test names is easier if you create a test submodule
    // for every function you want to test
    mod player_service_new_player {
        use super::*;
        
        // It can help to have factory methods for common sets of data
        fn player_create_default() -> PlayerCreate {
            PlayerCreate {
                full_name: "John Smith".to_owned(),
                username: "jsmith22".to_owned(),
            }
        }

        #[tokio::test]
        async fn happy_path() {
            // First, we need to define our fakes and the service to test against
            let in_memory_players = test_util::InMemoryPlayerPersistence::new_locked();
            let mut ext_cxn = external_connectivity::test_util::FakeExternalConnectivity::new();
            let svc = PlayerService;

            // Now we can define our input to the business logic
            let new_player = player_create_default();

            // Now let's invoke the business logic!
            let player_create_result = svc.new_player(&new_player, &mut ext_cxn, &in_memory_players, &in_memory_players).await;

            // With Speculoos, we can chain some assertions together to verify we got what we expected (successful creation, ID 1)
            assert_that!(player_create_result).is_ok().is_equal_to(1);
        }

        #[tokio::test]
        async fn returns_port_error_on_port_fail() {
            // Next, we'll do a test where the port is disconnected.
            let mut raw_players = test_util::InMemoryPlayerPersistence::new();
            // After creating the fake, we can set its connectivity property to "disconnected" to force a failure
            raw_players.connectivity = Connectivity::Disconnected;

            let in_memory_players = RwLock::new(raw_players);
            let mut ext_cxn = external_connectivity::test_util::FakeExternalConnectivity::new();
            let new_player = player_create_default();
            let svc = PlayerService;
            
            // Now invoke the business logic and assert
            let player_create_result = svc.new_player(&new_player, &mut ext_cxn, &in_memory_players, &in_memory_players).await;
            
            assert_that!(player_create_result)
                .is_err()
                .matches(|err| {
                    // You can use the matches! macro to pattern match against the error value
                    matches!(err, driving_ports::PlayerCreateError::PortError(_))
                });
        }
        
        // ...more tests
    }
    
    // ...more test submodules
}

// ...implementation of test_util module
```

</details>

## Unit Testing API Routes

Testing API routes is a little more complex because it can involve deserializing the response produced from the
request logic in error cases. Typically, it is sufficient to just mock the business logic to verify values it produces
convert to expected HTTP responses.

As mentioned previously, `mockall::automock` doesn't work particularly well with traits containing async functions.
Instead, `domain::test_util` contains a composable `FakeImplementation` type that can be used to easily implement mocks
for both sync and async trait implementations.

### Defining a mock with FakeImplementation

Similar to fake implementations, trait implementations for mocks should be done inside a synchronization primitive to
allow for interior mutability while using immutable references to pass around the driving port.

Mocks are implemented by composing together `FakeConnectivity` instances to set return values on specific functions. It
does this by utilizing generics to both record inputs and fake outputs of a function. Here's how that can be done, 
implementing `domain::player::driving_ports::PlayerPort` as a mock:

```rust
// in domain/player.rs

// ...rest of the module
#[cfg(test)]
pub mod test_util {
    use std::sync::Mutex;
    use super::*;

    // ...other test utility definitions

    pub struct MockPlayerService {
        // Define one FakeConnectivity instance for every function on the trait you're mocking
        //
        // The first generic is the type containing arguments you want to capture (usually a tuple or basic data type
        // but it must implement Clone)
        //
        // The second generic is the return type of the function you're mocking, which also must do one of 3 things:
        // 1. Implement the Clone trait
        // 2. Be a Result which returns cloneable types for both the Ok and Err variants
        // 3. Be a result which returns a cloneable type for the Ok variant but an anyhow::Error for the error
        //
        // These traits can be conditionally derived on a type specifically during tests via #[cfg_attr(test, derive(...))]
        pub new_player_response: FakeImplementation<PlayerCreate, Result<i32, PlayerCreateError>>,
    }

    impl MockPlayerService {
        // We'll need a constructor for the mock
        pub fn new() -> MockPlayerService {
            Self {
                new_player_response: FakeImplementation::new(),
            }
        }

        // It's also useful to have a builder constructor, so you can configure the mocks and wrap the mock in a mutex in one
        // function call
        pub fn build_locked(builder: impl FnOnce(&mut MockPlayerService)) -> Mutex<MockPlayerService> {
            let mut instance = Self::new();
            // The builder function here will allow developers to set mock return types before wrapping the mock in a lock
            builder(&mut instance);
            
            Mutex::new(instance)
        }
    }
    
    // Now, let's implement the mock for PlayerService
    //
    // With fakes, we'll typically have cases where we'll either be reading the data inside the fake or
    // actually writing new data to the fake. In that case, an RwLock makes more sense since it has "read" and "write" 
    // locking modes. For mocks, we tend to be writing into the mock on just about every call to save arguments, so 
    // Mutexes make more sense for mocks.
    impl driving_ports::PlayerPort for Mutex<MockPlayerService> {
        async fn new_player(
            &self,
            player_create: &PlayerCreate,
            _: &mut impl ExternalConnectivity,
            _: &impl driven_ports::PlayerDetector,
            _: &impl driven_ports::PlayerWriter
        ) -> Result<i32, driving_ports::PlayerCreateError> {
            // First, lock the sync primitive
            let mut locked_self = self.lock().unwrap();
            
            // Next, use FakeImplementation to record the call
            locked_self.new_player_response.save_arguments(player_create.clone());
            
            // Then, use FakeImplementation to return the mock result. The available functions
            // to return with are varied based on return type, so keep an eye on your autocomplete.
            locked_self.new_player_response.return_value_result()
        }
    }
}
```

### Using the mock to test an API route

Now that we have a mock for the player service, we can force specific results from the business logic to verify every
response from the endpoint. On happy path tests, you can easily extract the raw value from the result returned from the
function.

For tests verifying error responses, there are a number of different data structures that could have been
transformed into `ErrorResponse` for the `Err` variant of the request logic. To make it easy to verify the body of these
error results, the `api::test_util` module provides the `deserialize_body()` helper function, which takes a raw `axum::body::Body`,
turns it into a set of bytes, then deserializes it from JSON into a DTO.

Speaking of DTOs, we'll need to make sure response DTOs we interact with can be deserialized. We'll walk through router
testing by following these 3 steps:

1. Derive `serde::Deserialize` on response DTOs specifically during tests
2. Write a happy path test to verify the behavior of the endpoint
3. Write a test verifying the `409 Conlfict` response, checking the error body with the `deserialize_body()` helper function.

#### Making response DTOs deserializable in tests

It's a good idea to only make response DTOs deserializable in tests. This makes it so developers don't accidentally try to
use a response DTO for a request instead! However, response DTOs may need to be deserialized back into Rust structs during
tests so they can be asserted against. This can be done via the `cfg_attr()` macro, building off of 
[this example in the architecture documentation](architecture_layers.md#dtos-and-validation):

```rust
// in dto.rs

// ...request implementation

#[derive(Serialize)]
// This cfg_attr line will only implement Deserialize if we're running cargo tests.
#[cfg_attr(test, derive(Deserialize))]
#[serde(rename_all = "camelCase")]
pub struct PlayerCreateResponse {
    pub new_player_id: i32,
}
```

#### Writing the tests

With the DTOs configured, we can now implement tests against the router logic to verify that we receive the expected
HTTP responses based on return values from the business logic. The first test will be a simple happy path test, while
the second one will verify the `409 Conflict` response defined in [this example in the microservice documentation](./architecture_layers.md#request-logic-function).

Here's how we'd write those tests:

<details>
<summary>Code example for testing router logic</summary>

```rust
// in api/player.rs

// ...route implementation

#[cfg(test)]
mod tests {
    use super::*;

    // Similar to the domain logic tests, it can be helpful to organize your test functions
    // by grouping route tests in separate test submodules
    mod create_player {
        use super::*;

        // The first test verifies the success path, producing a 201 and returning an ID of the new player
        #[tokio::test]
        async fn happy_path() {
            // First, we need to set up our mocks. We'll make the player service mock return a successful result to
            // the driving adapter.
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();
            // Using the builder function we defined earlier, we can set mock responses via the mutable reference in the
            // closure before the mock service gets wrapped in a mutex - no locking necessary!
            let player_service = domain::player::test_util::MockPlayerService::build_locked(|svc| {
                // We can use the FakeImplementation property to set the mock return value
                svc.new_player_response.set_returned_result(Ok(5));
            });

            // Now we just need to set up the DTO and invoke the route logic function
            let new_player_info = dto::PlayerCreateRequest {
                full_name: "John Smith".to_owned(),
                username: "jsmith22".to_owned(),
            };
            let (status_code, Json(api_response)) = create_player(new_player_info, &mut ext_cxn, &player_service)
                .await
                .expect("Didn't get a successful response from the HTTP route!");

            // Now we can verify we got the right status code and ID from the business logic
            assert_eq!(StatusCode::CREATED, status_code);
            assert_eq!(5, api_response.new_player_id);

            // The mock also captured passed arguments, so we can verify those too if we want
            let locked_player_service = player_service.lock().unwrap();
            let service_calls = locked_player_service.new_player_response.calls();

            assert_eq!(1, service_calls.len());
            assert_eq!("John Smith", service_calls[0].full_name);
            assert_eq!("jsmith22", service_calls[0].username);
        }

        // The second test verifies we return a 409 conflict with appropriate error code if the
        // requested username is taken
        #[tokio::test]
        async fn responds_409_if_username_is_taken() {
            // Again, set up the mocks. This time we'll return an expected domain error to trigger the 409
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();
            let player_service = domain::player::test_util::MockPlayerService::build_locked(|svc| {
                svc.new_player_response.set_returned_result(Err(domain::player::driving_ports::PlayerCreateError::UsernameTaken));
            });
            
            // Create the request DTO and trigger route logic
            let new_player_info = dto::PlayerCreateRequest {
                full_name: "John Smith".to_owned(),
                username: "jsmith22".to_owned(),
            };
            let error_response = create_player(new_player_info, &mut ext_cxn, &player_service)
                .await
                .expect_err("Should have received an unsuccessful result from the endpoint");
            
            // Now we need to convert error_response into something we can assert against.
            // We know it's supposed to return a dto::BasicError, so we'll first need to convert
            // the ErrorResponse into a response, then test the status code and deserialize the body.
            let (parts, resp_body) = error_response.into_response().into_parts();
            
            assert_eq!(StatusCode::CONFLICT, parts.status);
            
            let body: dto::BasicError = deserialize_body(resp_body).await;
            // Since the error_code on the body is supposed to be read by API consumers, we should
            // verify the appropriate error code was sent.
            assert_eq!("username_in_use", body.error_code);
        }
    }
}
```

</details>

## Writing Integration Tests

Integration tests are written very similarly to API Route tests, but they actually test the whole app running against
an active Postgres database. 

### Running and marking integration tests

Because other systems are involved with integration tests, these tests are disabled by default and can be enabled
via a Cargo feature flag called "integration_test". To run both the unit and integration tests, you'll need to start
the database and run cargo tests, enabling the integration_test feature:

1. `docker-compose up -d`
2. `cargo test --features integration_test`

Integration tests go under the `integration_test` module. In that module, integration tests can be defined like normal
tests, but with a `cfg_attr` annotation to exclude them from a normal `cargo test` run without the integration_test feature:

```rust
// in integration_test/player_api.rs

#[tokio::test]
// This cfg_attr annotation excludes the test unless the integration_test feature is enabled
#[cfg_attr(not(feature = "integration_test"), ignore)]
async fn sample_test() {
    // ...test logic
}
```

### Implementing the integration test

The `integration_test::test_util` module defines a utility function, `prepare_application()`, which prepares an Axum 
application and a standalone schema for the active unit test, which is derived from the default schema by turning it 
into a template and copying it. Using these utilities, you can inject test data into the database and attach necessary 
routes to the Axum app to perform the integration test. It also provides an active database connection if you wish to
inject test data during the test.

You can create requests to the Axum application using `axum::http::Request::builder()`. The
`deserialize_body()` helper from `api::test_util` can be used to read API responses just like API tests, and
the same module provides a `dto_to_body()` helper to help pass DTOs into the request builder for requests.

With all that being said, here's how we can write a happy path integration test for the [player create endpoint](./architecture_layers.md#the-router-function):

<details>
<summary>Code example for implementing an integration test</summary>

```rust
// in integration_test/player_api.rs

#[tokio::test]
#[cfg_attr(not(feature = "integration_test"), ignore)]
async fn can_create_player() {
    // First off, let's use our router function from the API module to attach the player routes to a test router
    let router = Router::new().nest("/players", api::player::player_routes());
    // Next, let's use the integration test utilities to scaffold the app and a database connection.
    // We won't need the database connection here, so we can just ignore it. Preparing the application
    // starts us with a fresh schema copied from the "postgres" schema so tests won't interfere with one another.
    let (mut app, _) = test_util::prepare_application(router).await;
    
    // Now, let's create a request (assuming we already conditionally cause the DTO to implement serde::Serialize during tests)
    let request = Request::builder()
        .method(Method::POST)
        .uri("/players")
        .header(header::CONTENT_TYPE, "application/json")
        .body(dto_to_body(
            &dto::PlayerCreateRequest {
                full_name: "John Smith".to_owned(),
                username: "jsmith22".to_owned(),
            },
        )).unwrap();
    
    // With the request in hand, we can pass it to the Axum app and get a response
    let response = app.call(request).await.unwrap();
    
    // Now that we have the response we can verify the received status code and check the received response DTO.
    let (res_parts, res_body) = response.into_parts();
    
    assert_eq!(StatusCode::CREATED, res_parts.status);
    
    // There's no guaranteed ID, so we can verify the response by just checking that the new player ID is not 0.
    let parsed_body: dto::PlayerCreateResponse = deserialize_body(res_body).await;
    assert!(parsed_body.new_player_id > 0);
}
```

</details>