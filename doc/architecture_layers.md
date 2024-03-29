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
    username: String,
    full_name: String,
}

// This struct can be consumed later, but contains all
// pertinent information about a player
pub struct Player {
    id: i32,
    username: String,
    full_name: String,
    level: i32,
    in_good_standing: bool,
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
to inject fakes during testing (TODO: ADD LINK TO TESTING DOCS HERE). As mentioned previously, the driving adapter has ownership
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
    pub trait PlayerPort {
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

TODO.