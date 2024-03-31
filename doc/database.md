# Database

This microservice template is not prescriptive about how you manage your database or migrate its schema. 
For convenience, a database setup script is provided in the [postgres-scripts](../postgres-scripts) folder
for you to populate the database as you see fit. It is recommended that you set up a separate database migration
solution such as [liquibase](https://www.liquibase.com/) for managing migrations on your database during deployment.

## Executing a database transaction across business logic

According to Hexagonal Architecture principles, the domain/business logic should be built agnostic of the external
technologies it interfaces with. That's the responsibility of the adapters that help the hexagon interface with the
outside world. In order to support this, a `with_transaction` function is provided to initiate database transactions
across business logic invocations at the driving adapter level.

Here's an example which builds off of the [request logic function example](./architecture_layers.md#request-logic-function)
to create a new player in a database transaction:

```rust
// in api/player.rs
use routing_utils::Json;
use axum::ErrorResponse;
use external_connections::{TransactableExternalConnectivity, with_transaction};

async fn create_player(
    new_player: dto::PlayerCreateRequest,
    
    // If you want to perform database transactions, you need to use TransactableExternalConnectivity
    // rather than just ExternalConnectivity to make sure you can initiate database transactions.
    //
    // Because of the implementation of with_transaction, you only need an immutable borrow of ext_cxn
    ext_cxn: &impl TransactableExternalConnectivity,
    
    player_service: &impl domain::player::driving_ports::PlayerPort,
) -> Result<(StatusCode, Json(dto::PlayerCreateResponse)), ErrorResponse> {
    // ...validation and DTO creation
    
    // You can just wrap the call to the service inside with_transaction!
    // The lambda you provide gives you a new version of ext_cxn which is inside the database transaction
    //
    // with_transaction will also wrap errors returned from the domain in a custom error type that reports
    // issues with committing the transaction, so you'll need to handle for that too
    let player_create_result = with_transaction(ext_cxn, |tx_cxn| async {
        // tx_cxn is a mutable reference to an ExternalConnectivity instance with an initiated
        // database transaction. The transaction will be committed after the end of the lambda
        // if the lambda does not return Result::Err.
        player_service.new_player(&player_create_domain, &mut *tx_cxn, &player_detector, &player_writer).await
        
        // Any business logic you want to run in the same database transaction can be added here. Just keep using tx_cxn.
    }).await;
    
    // ...error handling and response
}
```