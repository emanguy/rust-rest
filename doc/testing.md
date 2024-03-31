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
of the real systems we try to interface with.

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

    // Now we can implement the driven port interface on the fake, specifically when it's wrapped
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

The `external_connections::test_util` package defines a fake `ExternalConnectivity` instance we can use for testing
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

Testing API routes is a little more interesting because it can involve deserializing the response produced from the
request logic in error cases. Typically, it is sufficient to just mock the business logic to verify values it produces
convert to expected HTTP responses.

As mentioned previously, `mockall::automock` doesn't work particularly well with traits containing async functions.
Instead, `domain::test_util` contains a composable `FakeImplementation` type that can be used to easily implement mocks
for both sync and async trait implementations.

### Defining a mock with FakeImplementation

Similarly to fake implementations, trait implementations for mocks should be done inside a synchronization primitive to
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
        // We'll need a constructor for it
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
            // to return with are varied based on response type, so keep an eye on your autocomplete.
            locked_self.new_player_response.return_value_result()
        }
    }
}
```

### Using the mock to test an API route

## Writing Integration Tests