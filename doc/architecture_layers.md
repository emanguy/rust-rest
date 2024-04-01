# Architecture Layers

This microservice template is built using [Hexagonal Architecture](https://medium.com/ssense-tech/hexagonal-architecture-there-are-always-two-sides-to-every-story-bc0780ed7d9c).
**Please read that overview before moving forward, terminology from it will be used in this document.**

The app is built using the standard hexagon layout:
* The HTTP-based driving adapters are implemented in the [API Package](../src/api/mod.rs)
* The domain logic is implemented in the [Domain Package](../src/domain/mod.rs)
* The driven adapters (just SQL connections for now) are implemented in the [Persistence package](../src/persistence/mod.rs)

## Layer Interactions

### Domain layer
The domain layer should define all its interactions with the other layers. This means defining submodules for some set of
functionality called `driving_ports` and `driven_ports`, including the return types and error types that are either received
from or passed to adapters interacting with the domain.

Functions able to be called from driving adapters should be exposed through the driving port and implemented in the 
domain. Other functions can be defined to support the domain logic, but they don't need to be included on the driving port
unless they need to be called from a driving adapter.

For testability, driving ports should accept driven port implementations so ownership plays nicely with your code (basically,
ownership of everything trickles down from the driving adapter).

### Driving adapters (HTTP routing)
Driving adapters should handle protocol-specific details for triggering logic in the domain. This means accepting and responding
with DTOs (Data Transfer Objects), and converting these DTOs into domain types before invoking domain functionality through
the driving port. Conversely, when a domain type or domain error is passed back to a driving adapter, they should be converted
into DTOs before they go over the wire. The HTTP response code should be determined based on the semantics of either a successful
operation or the specific domain error received from the driving port.

Driving adapters have a "special" method of interaction with driven ports on the other side of the hexagon - only driving
adapters may start database transactions, as the domain must be ignorant of underlying technologies it interacts with.
This also supports propagating the same database transaction across multiple calls into the domain. 

This is all facilitated through the **ExternalConnectivity** trait, which provides accessors for driven adapters to contact 
external systems. It also provides an abstraction over SQLX's [Transaction](https://docs.rs/sqlx/latest/sqlx/struct.Transaction.html) 
and [PoolConnection](https://docs.rs/sqlx/latest/sqlx/pool/struct.PoolConnection.html) types so database
connection code can be written in a way that's agnostic of whether the app communicates with the database in the middle 
of a transaction or via a standard database connection.

### Driven adapters (database and other external systems)
Driven adapters should handle protocol-specific details to help facilitate the domain's communication with external systems.
They should implement the driven port interface defined by the domain and convert domain types into DTOs (Data Transfer Objects)
before sending data over the wire. Acquiring the connection to the outside world can be done by accepting something implementing
the **ExternalConnectivity** trait.

The driving adapter should then translate the DTO it receives back into a domain type, or if an error is received it should
either be translated into a predefined domain error or be returned as a catch-all "port error". This way the domain
can respond appropriately to errors without having knowledge of the underlying implementation.

### TL;DR

![A diagram depicting the interaction between layers of the application as described in previous paragraphs.](img/hexagonal_arch_diagram.png "Too Long - Didn't Read Diagram")

## Implementation of each layer

To demonstrate how to write out each layer, we'll walk through writing the domain logic and the ports associated with
it. Let's imagine we're defining an API where we're registering players for a game. The incoming DTO will have some
basic validation, but the user may not have the same username as another player.

### Implementing the domain layer

Ports should have a narrow focus and **shouldn't grow to more than about 8 to 10 available functions**. If you go beyond that,
it's likely that the port is doing too much, and it should be decomposed into more focused ports. As a reminder, a **driven port**
allows the domain logic to communicate with external systems. It will need an `ExternalConnectivity` instance in order to do so.
Additionally, a **driving port** will expose the domain's functionality to driving adapters in higher layers.

#### Driven Port

Let's start with the driven port. We'll need to define our inputs and outputs for our business logic, including any
expected errors. Each set of ports will be defined in their own submodule.

<details>
<summary>Code example for driven port definition</summary>

```rust
// Pretend this file is domain/player.rs

// Domain objects are defined at the module level
// Structs containing data for specific operations should be defined
// separately from structs that contain a full set of data.
pub struct PlayerCreate {
    pub username: String,
    pub full_name: String,
}

// This struct can be consumed later, but contains all
// pertinent information about a player
pub struct Player {
    pub id: i32,
    pub username: String,
    pub full_name: String,
    pub level: i32,
    pub in_good_standing: bool,
}

// All driven port traits are defined in this submodule
pub mod driven_ports {
    use super::*;

    // Making trait implementations require the Sync trait makes
    // tons of ownership errors go away
    // PlayerDetector detects the presence of users
    pub trait PlayerDetector: Sync {
        // If the only expected error is a port error, just return anyhow::Error in your result. It can be refactored
        // later if need be.
        async fn player_with_username_exists(
            &self,
            username: &str,
            ext_cxn: &mut impl ExternalConnectivity,
        ) -> Result<bool, anyhow::Error>;
    }

    // PlayerWriter writes new player data to an external system
    pub trait PlayerWriter: Sync {
        // order of parameters is:
        // 1. Necessary data
        // 2. External connectivity implementation
        async fn new_player(
            &self,
            creation_data: &PlayerCreate,
            ext_cxn: &mut impl ExternalConnectivity,
        ) -> Result<i32, anyhow::Error>;
    }

    // Here's another example of a driven port
    // PlayerReader retrieves player data
    pub trait PlayerReader: Sync {
        async fn all_players(&self, ext_cxn: &mut impl ExternalConnectivity) -> Result<Vec<Player>, anyhow::Error>;
        async fn player_by_username(&self, username: &str, ext_cxn: &mut impl ExternalConnectivity) -> Result<Option<Player>, anyhow::Error>;
    }
}
```
</details>

#### Driving Port

Once we have the driven port interface defined, we can use the driven ports as parameters in our driving port, allowing us
to [inject fakes during testing](./testing.md#unit-testing-business-logic). As mentioned previously, the driving adapter has ownership
of the connections the code makes to the outside world, so we need to pass something implementing `ExternalConnectivity` into
the driving port. We'll need to define the set of errors that can be produced from the driving port functions, too.

Here's how we can define the driving port with a function for creating the new player:

<details>
<summary>Driving port code example</summary>

```rust
// This is in another section of domain/player.rs

// We'll define a module to group the driving port definition here
pub mod driving_ports {
    use super::*;
    use thiserror::Error;
    
    // Now that we have specific error cases we want to call out, we should define
    // an error specific to player creation. You may share these domain errors across multiple
    // functions, or compose re-usable error data structures across multiple error enums
    // to make your error definitions DRY (don't repeat yourself).
    
    // We'll use the "thiserror" crate to make the error variants display human-readable
    // error messages if need be.
    #[derive(Debug, Error)]
    pub enum PlayerCreateError {
        // Because we can only validate if the username was taken by hitting the database,
        // we have to check username usage as part of the business logic
        #[error("The given username was already taken")]
        UsernameTaken,
        
        // "Transparent" makes this error variant just display the error message from the wrapped error
        // "From" on the inner error adds an implementation of From<anyhow::Error> for PlayerCreateError
        //
        // Port errors encapsulate any other error that gets returned other than the ones we expect. These
        // might be database-specific errors or connectivity errors.
        #[transparent]
        PortError(#[from] anyhow::Error),
    }
    
    // This trait is what the driving adapter will invoke to trigger the business logic
    // we write in this file
    //
    // Again, requiring Sync on the implementer gets rid of a ton of lifetime issues
    pub trait PlayerPort: Sync {
        // The conventional order of parameters in driving port functions is:
        //   1. Actual imports
        //   2. The external connections implementation to pass to lower layers
        //   3. The driven port implementations, so this function can invoke them without the implementation
        //        needing to own instances of the trait implementations or access them through Arcs
        //
        // Passing the driven ports this way will also allow callers to know exactly what kinds of operations a piece
        // of business logic intends to invoke.
        async fn new_player(
            &self,
            player_create: &PlayerCreate, 
            ext_cxn: &mut impl ExternalConnectivity, 
            p_detect: &impl driven_ports::PlayerDetector, 
            p_write: &impl driven_ports::PlayerWriter
        ) -> Result<i32, PlayerCreateError>; // The driven port function will just return the ID of the new player or our error set
    }
}
```
</details>

#### Business logic (driving port implementation)

The driving port implementation is the entrypoint from outside the system for our business logic. We can call into passed 
driven ports to access data from outside the system if need be, so technically we can actually write and test our business 
logic without starting the server or connecting to any external systems/writing out database migrations.

The driving adapter is defined as an empty struct and should not have any fields (therefore, it holds no state). This empty
struct can later be used to pass the real adapter when the app starts or a mock when testing the HTTP router. (Also, since
we use an empty struct the port implementations we define consume no memory, and thus is equivalent to passing around a single
pointer for a bundle of functions! Gotta love monomorphic generics.)

Keep in mind that your business logic pretty much just consists of a bunch of functions floating around in the "domain"
module. **Adding functions to the driving port isn't necessary unless you need business logic to be called from a driving adapter.**

Now, let's look at how we'd implement the business logic on the other side of the driving port:

<details>
<summary>Code example for business logic</summary>

```rust
// This example would also be part of domain/player.rs

// We define the other end of the driving port as an empty struct.
// This will let us swap out the real implementation in tests.
pub struct PlayerService;

// Here's our impl block for the driving port logic
impl driving_ports::PlayerPort for PlayerService {
    
    // And now we can implement the logic for creating the new player
    async fn new_player(
        &self,
        player_create: &PlayerCreate,
        ext_cxn: &mut impl ExternalConnectivity,
        p_detect: &impl driven_ports::PlayerDetector,
        p_write: &impl driven_ports::PlayerWriter,
    ) -> Result<i32, driving_ports::PlayerCreateError> {
        // We can just use the logger here to say what it is we're doing
        info!("Attempting to create a new user with the username {}.", player_create.username);
        
        // First, we need to check for our error condition - is the username taken?
        //
        // In order to re-use our borrowed external connectivity implementation, we'll need to re-borrow the reference
        // to pass it into driven ports (this creates a new mutable reference to the data pointed to by our mutable reference
        // and doesn't violate any ownership rules. In Rust, & is Copy while &mut is not. That's what the "&mut *" is.)
        let username_taken = p_detect.player_with_username_exists(&player_create.username, &mut *ext_cxn)
            .await
            // Since we have an anyhow::Error here, we'll add some context on what we were doing for debuggability.
            // Then we've got the good ole question mark operator to auto-handle our PortError case.
            //
            // Remember how we have From<anyhow::Error> implemented on PlayerCreateError? The question mark will auto-transform
            // the anyhow::Error into a PlayerCreateError::PortError due to that implementation.
            .context("checking username existence during player creation")?;
        
        // Now return the expected error, if need be.
        if username_taken {
            warn!("Oops, the username {} was already taken.", player_create.username);
            
            return Err(driving_ports::PlayerCreateError::UsernameTaken);
        }
        
        // Our check is passed! Let's create the new player.
        Ok(p_write.new_player(player_create, &mut *ext_cxn).await?)
    }
}
```
</details>

With that, the core of the hexagon is complete! Now we just need to be able to access the database and to expose our
business logic via the HTTP server.

### Implementing the driven adapter (database)

To implement the driven adapter, we'll need to create a couple of empty structs and use them to implement the `PlayerDetector`
and `PlayerWriter` structs. From the `ExternalConnectivity` struct, we can acquire a connection to the database and use it
with `sqlx` to retrieve data from the database. Remember that we need to convert domain types into DTOs before sending them
to the database, and only respond to the domain with domain types and errors. This enforces a strict boundary between
the business logic and the actual implementation detail of connecting to the database as an external data source. This
decoupling allows for easy swapping of underlying implementations if need be.

Note that the `persistence` package includes utilities to transform anything that implements the `Debug` and `Display` traits
into an `anyhow::Error` called `anyhowify()`. There are also utilities for extracting the ID of inserted data, such as the
`NewId` struct. Similarly, `Count` stores the output of the `count()` SQL function.

The really neat thing about using SQLx for queries is that it will automatically verify type compatibility between your
DTOs and the database schema during build time automatically. Just another layer of correctness for your API.

Here's how we can implement those driven adapters:

<details>
<summary>Implementation of the driven adapters</summary>

```rust
// This example takes place in persistence/db_player_driven_ports.rs
use sqlx::query_as;

// Define a struct for the implementation of the driven adapter
pub struct DbPlayerWriter;

impl domain::player::driven_ports::PlayerWriter for DbPlayerWriter {
    async fn new_player(&self, creation_data: &domain::player::PlayerCreate, ext_cxn: &mut impl ExternalConnectivity) -> Result<i32, anyhow::Error> {
        // Acquire a database connection
        let mut cxn = ext_cxn.database_cxn().await.map_err(super::anyhowify)?;
        
        // Make the database query. In this case, NewId is our re-usable DTO
        // used for acquiring the id of the information added into the database
        let new_id = query_as!(
            super::NewId,
            "INSERT INTO players(username, full_name) VALUES ($1, $2) RETURNING players.id",
            creation_data.username,
            creation_data.full_name,
        )
            .fetch_one(cxn.borrow_connection())
            .await
            .context("trying to insert a new player into the database")?;
        
        // Convert the DTO into the domain type and return (typically this is a more complex data structure,
        // but we only need the ID here)
        Ok(new_id.id)
    }
}

// This struct implements the other driven adapter
pub struct DbPlayerDetector;

impl domain::player::driven_ports::PlayerDetector for DbPlayerDetector {
    async fn player_with_username_exists(&self, username: &str, ext_cxn: &mut impl ExternalConnectivity) -> Result<bool, anyhow::Error> {
        // Acquire a database connection
        let mut cxn = ext_cxn.database_cxn().await.map_err(super::anyhowify)?;
        
        // Make the query
        let username_count = query_as!(
            super::Count,
            "SELECT count(*) FROM players WHERE username = $1",
            username
        )
            .fetch_one(cxn.borrow_connection())
            .await
            .context("detecting existing players with a given username")?;
        
        // Return the result
        Ok(username_count.count() > 0)
    }
}
```
</details>

### Implementing the driving adapter (HTTP Routing)

With the business logic and driven adapters in place, we can now implement an HTTP-based driving adapter to trigger
everything end-to-end. The driven adapter is implemented in 2 parts - the request extractor function and the request logic
itself. This separation is in place to make it easy to mock out the business logic in tests. We'll first look at the code
for the request logic, then integrate it into the Axum router with the request extractor function.

#### Request Logic

The request logic starts by taking in any required information, then an implementation of both `ExternalConnectivity` and
the driving adapter.

##### DTOs and Validation

In order to get that required information, we'll need to define a DTO for our request body. Request bodies only need to
implement [serde](https://serde.rs/)'s `Deserialize` trait, while responses need to implement `Serialize`. Let's define our request and response
bodies. We can also use the `rename_all` piece of the serde annotation to make the data structure accept and output fields
in camel case, which is typical for JS/TS code which would consume the API.

```rust
// in dto.rs
use serde::{Serialize, Deserialize};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerCreateRequest {
    pub full_name: String,
    pub username: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerCreateResponse {
    pub new_player_id: i32,
}
```

Now, recall that one of our requirements for creating the player was that there should be some basic validation on the
incoming data. That can be done by deriving [validator::Validate](https://docs.rs/validator/latest/validator/index.html)
on our request. Let's add some validation which requires the full name to be 1-256 characters, and the username to be
1-32 characters in length.

```rust
// in dto.rs
use serde::{Serialize, Deserialize};
use validator::Validate;

// We're now also deriving Validate so we get the .validate() function on our DTO
#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct PlayerCreateRequest {
    // The full name should be 1-256 characters in length
    #[validate(length(min = 1, max = 256))]
    full_name: String,
    
    // The username can only be 1-32 characters in length
    #[validate(length(min = 1, max = 32))]
    username: String,
}

// ...response DTO
```

It can also be useful to implement the `From` trait on domain types so we can convert DTOs into domain types in a single line
of code. Let's do that while we're here:

```rust
// in dto.rs

// ...definition of PlayerCreateRequest

impl From<PlayerCreateRequest> for domain::player::PlayerCreate {
    fn from(value: PlayerCreateRequest) -> Self {
        Self {
            full_name: value.full_name,
            username: value.username,
        } 
    }
}

// ...response DTO
```

##### Request logic function

Now that we have DTOs for our request, let's actually implement the request function. The conventional order of parameters
for a request function is:

1. DTOs and required data
2. ExternalConnectivity implementation
3. Driving Port for the business logic

Because the business logic is mocked out in tests, we can actually create instances of the driven adapters inside the logic
of the function as they won't ever be invoked.

Many responses are common across various endpoints on the microservice, so canned responses are available in the `routing_utils` package.
We'll use some of these for generic 500 errors and validation errors. Otherwise, [axum-compatible response types](https://docs.rs/axum/0.7.5/axum/response/index.html#building-responses) 
should be returned from the routing logic function.

We'll also need to interpret the result from the business logic and turn it into an appropriate HTTP response, complete with
appropriate status codes and DTOs. **NOTE: be sure to use the `routing_utils` version of the `Json` type, as it is customized to
return the common error data structure defined in this template when used for extracting JSON in the request.** 
Here's how the logic would be implemented:

<details>
<summary>Code example for implementing the request logic function</summary>

```rust
// In api/player.rs
use crate::router_util::ValidationErrorResponse;

async fn create_player(
    new_player: dto::PlayerCreateRequest,
    ext_cxn: &mut impl ExternalConnectivity,
    player_service: &impl domain::player::driving_ports::PlayerPort,
) -> Result<(StatusCode, Json(dto::PlayerCreateResponse)), ErrorResponse> {
    info!("Creating a new player with the username {}.", new_player.username);

    // First, we need to validate the incoming request.
    // On an error, we can use the pre-built routing_utils::ValidationErrorResponse type
    // to report the error to the user
    new_player.validate().map_err(ValidationErrorResponse::from)?;

    // Now that we have a valid payload, we can convert it into a domain type to pass through the driving port.
    let player_create_domain = domain::player::PlayerCreate::from(new_player);

    // Next, we can create instances of the driven adapters to pass to the business logic and
    // attempt to create the player.
    let player_detector = persistence::db_player_driven_ports::DbPlayerDetector;
    let player_writer = persistence::db_player_driven_ports::DbPlayerWriter;

    // We have to reborrow the ExternalConnectivity instance to retain ownership of it, as mutable references
    // don't implement the Copy trait.
    //
    // Notice that you can immediately tell from the invocation that new_player both detects the existence of a player
    // and writes a player to an external system. Beyond ownership and testing benefits, passing the driven adapters here
    // provides a level of transparency to the operations you perform.
    let player_create_result = player_service.new_player(&player_create_domain, &mut *ext_cxn, &player_detector, &player_writer).await;

    // We now need to handle any domain errors that cropped up, or retrieve the result
    let new_player_id = match player_create_result {
        Ok(id) => id,
        // If the username was already taken, we should return a 409 Conflict with an appropriate error message.
        // The error_code field here is used to differentiate between unique errors in a class of errors, such
        // as determining what part of a URL was missing for a 404 error
        Err(PlayerCreateError::UsernameTaken) => {
            warn!("Username {} was already in use.", player_create_domain.username);

            // A tuple of (StatusCode, Json) can be converted into a response, and the into() on the end converts it
            // into an ErrorResponse
            return Err((
                StatusCode::CONFLICT,

                // BasicError is the error DTO type used throughout this template
                Json(dto::BasicError {
                    // The error code is a differentiator that consumers can match on to differentiate different errors
                    // produced under the same HTTP status code
                    error_code: "username_in_use".to_owned(),

                    // The error description is a human-readable error that can be presented to users via consumers of the API
                    error_description: format!(r#"The username "{}" is already in use. Please choose another."#, player_create_domain.username),

                    // Extra info is used to provide contextual information in some cases, such as the set of failed
                    // validations produced by a DTO's validate() function. It is intended to be extended, so feel free!
                    extra_info: None,
                })
            ).into());
        },

        // This is the "unexpected error" case. We'll just use routing_utils::GenericErrorResponse to report it.
        Err(PlayerCreateError::PortError(cause)) => {
            error!("An unexpected error went wrong creating the player {}. Error: {}", player_create_domain.username, cause);

            // GenericErrorResponse can also be converted into ErrorResponse
            return Err(GenericErrorResponse(cause).into());
        },
    };

    // Now that all the errors are handled, we can create our response DTO and provide a response to the user
    let response_body = dto::PlayerCreateResponse {
        new_player_id,
    };

    Ok((StotusCode::CREATED, Json(response_body)))
}
```

</details>

#### Request Extractor

Now that the business logic is in place, we can set up a function on an Axum router by defining a function which
creates an attachable router and setting it up to be invoked on certain HTTP routes.

##### The router function

Each group of APIs can be defined piecemeal and exposed via a central function which attaches the defined routes to a
router, which can then be attached to the main Axum app. Here's how we can define that router and attach it:

```rust
// In api/player.rs
use axum::Router;
// This import is important!!! This version of the JSON extractor uses BasicError
// for an error when a caller passes invalid JSON rather than Axum's error type
use routing_util::Json;

// The generic type in the return type defines the expected application
// state that we're allowed to extract from the main Axum app.
// This will allow us to get access to the shared ExternalConnectivity instance
pub fn player_routes() -> Router<Arc<SharedData>> {
    Router::new()
    // Off of the router, we define a new route which will invoke our route logic
    // You may assume that when the router is attached to the main app, a prefix will
    // be added for this group of routes
        .route(
            "/",
            // HTTP verb functions specify which HTTP verb will invoke the following function
            //
            // We're using some axum extractors here to get the application state for the
            // ExternalConnectivity instance and the request body, which we're getting via
            // the Json extractor
            post(|State(app_data): AppState, 
                  Json(player_create): Json<dto::PlayerCreateRequest>| async move {
                // In order to invoke the route logic, we need ExternalConnectivity and
                // the business logic instance. We'll create both, then invoke the route logic.
                let player_service = domain::player::PlayerService;
                let mut ext_cxn = app_data.ext_cxn.clone();
                
                // Remember to await the call. You'll get a nasty error otherwise.
                create_player(player_create, &mut ext_cxn, &player_service).await
            })
        )
}

// ...route logic function we defined earlier
```

##### Attaching to the main Axum app

With the router implemented, all that's left to do is attach it to the main axum app!

```rust
// in main.rs

#[tokio::main]
async fn main() {
    // ...setup in the main function
    
    let router = Router::new()
        // This nest() call attaches the player router to the app. Now we're ready to serve!
        .nest("/players", api::player::player_routes())
        .with_state(Arc::new(SharedData { ext_cxn }));
    
    // ...axum app starts listening
}
```
