# API Documentation

This template uses the [utoipa crate](https://crates.io/crates/utoipa) to generate swagger documentation for the 
microservice's API at compile time. When the application is started, you can access the documentation 
at http://localhost:8080/swagger-ui/index.html

## Documenting DTOs

Documenting DTOs can be done in two stages: deriving `ToSchema` on the DTO and adding it to the `components` list on `OpenApiSchemas`.

### Deriving ToSchema

`ToSchema` is a trait used by utoipa to read the fields in a DTO into an OpenAPI schema that can be used in the swagger UI.
For request DTOs, nothing needs to be done beyond adding the derive macro to the struct. For response DTOs, it is recommended
to add sample value annotations to the fields so the response looks nice in the swagger UI.

Here's how that might look using the [DTO examples in the architecture documentation](./architecture_layers.md#dtos-and-validation):

```rust
// in dto.rs

// Note the ToSchema derivation here
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlayerCreateRequest {
    pub full_name: String,
    pub username: String,
}

// Note the ToSchema derivation here
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlayerCreateResponse {
    // This annotation provides the value for this field that will
    // show up in an example response on Swagger
    #[schema(example = 3)]
    pub new_player_id: i32,
}
```

### Adding the DTO to OpenApiSchemas

Once `ToSchema` is implemented on your new DTOs, you can attach the schema to utoipa by adding it to the list of schemas
defined on the `OpenApiSchemas` empty struct at the top of `dto.rs`:

```rust
// in dto.rs

#[derive(OpenApi)]
#[openapi(components(
    schemas(
        // Add the schemas here
        PlayerCreateRequest,
        PlayerCreateResponse,
    
        // ...other schema definitions
    ),

    // ...other OpenAPI information
))]
pub struct OpenApiSchemas;
// ...dto definitions

```

## Documenting API Endpoints

API endpoints are documented using schema information defined on DTOs and [canned responses](#defining-canned-responses), 
if applicable. Each API has its own OpenAPI struct that then gets composed into the main OpenAPI definition in the
`api::swagger_main` module.

### Adding OpenAPI information to a route logic function

OpenAPI information can be added to a [request logic function](./architecture_layers.md#request-logic-function)
with the `utoipa::path` annotation. It includes information on the API group a route belongs to, request and response
DTO information, the HTTP verb used, and more. A full description of all the options on the annotation can be found
[in the utoipa documentation](https://docs.rs/utoipa/4.2.0/utoipa/attr.path.html).

With that information, let's see how we would document the "player create" endpoint defined in the above link:

<details>
<summary>Code example for documenting an endpoint</summary>

```rust
// in api/player.rs

// We define a constant at the top of the file so we can group all of this API's endpoints together in Swagger
// and rename the group all at once if need be
pub const PLAYER_API_GROUP: &str = "Players";

// ...router definition

// This path definition states the following:
//   - You can hit the route with a POST to /players
//   - It's part of the PLAYER_API_GROUP
//   - The request body is the PlayerCreateRequest schema defined in dto.rs
//   - The endpoint will respond in the following ways:
//     - With a 201 CREATED, containing the PlayerCreateResponse schema
//     - With a 400 BAD REQUEST, using the 400 validation error canned response
//     - With a 409 CONFLICT, containing a BasicError schema with the error code "username_in_use" if the
//         username is taken
//     - With a 500 INTERNAL SERVER ERROR, using the 500 error canned response
//
// The doc comment on this endpoint will get rendered as the description in the swagger UI
//
/// Create a new player in the game
#[utoipa::path(
    post,
    path = "/players",
    tag = PLAYER_API_GROUP,
    request_body = PlayerCreateRequest,
    responses(
        (status = 201, description = "Player successfully created", body = PlayerCreateResponse),
        (status = 400, response = dto::err_resps::BasicError400Validation),
        (
            status = 409,
            description = "Username is already taken",
            body = BasicError,
            example = json!({
                "error_code": "username_in_use",
                "error_description": "The given username is already taken by another player.",
                "extra_info": null,
            }),
        ),
        (status = 500, response = dto::err_resps::BasicError500),
    ),
)]
// You may include this to trace the latency of this function via the "tracing" crate. See the crate docs for more info.
#[instrument]
async fn create_player(
    // ...parameters
) -> Result<(StatusCode, Json(dto::PlayerCreateResponse)), ErrorResponse> {
    // ...route logic implementation
}
```
</details>

### Attaching OpenAPI route information to a router's API information

Each router under the `api` module has its own set of OpenAPI definitions. The routes under that API get attached
to the definitions under that router so they can be joined to the main OpenAPI spec for the microservice.

As long as your route logic functions are decorated with `utoipa::path`, they can be added to the OpenAPI schema
for the given API group via the name of the route logic function:

```rust
// in api/player.rs

// Function names for route logic functions go under the "paths" list in this annotation
#[derive(OpenApi)]
#[openapi(paths(
    create_player,
))]
pub struct PlayersApi;

// ...rest of the API definition
```

### Merging an API's data into the main OpenAPI definition

Once you have the OpenAPI spec defined for an API group, it can then be merged into the main swagger spec defined
in `api/swagger_main.rs`. The main OpenAPI spec containing the title and description of the overall API can be edited in
that file too.

Here's how you can add a new API group to the main OpenAPI schema:

```rust
// in api/swagger_main.rs

// ...main OpenAPI definition

// This function is already defined
pub fn build_documentation() -> SwaggerUi {
    let mut api_docs = TodoApi::openapi();
    api_docs.merge(dto::OpenApiSchemas::openapi());
    // The OpenAPI definition for your API group gets merged with the .merge() function:
    api_docs.merge(super::player::PlayersApi::openapi());
    
    // ...rest of the swagger UI setup
}
```

## Defining canned responses

In some cases, the same common error response may be returned across multiple different API endpoints. Rather than needing
to redefine the entire response every time that response is used, you can define a canned error response in the `dto::err_resps`
module. For these, rather than deriving `ToSchema`, you derive `ToResponse`. In doing so you include a response description
and example value that can be reused across multiple API endpoints. The response is then added to `OpenApiSchemas` like a DTO.
Note that a canned response must contain a type implementing `ToSchema` which describes the response body before `ToResponse`
can be derived. 

Here's how you might define one:

```rust
// in dto.rs

// ...dto definitions

// This submodule already exists, no need to redefine it
pub mod err_resps {
    
    // We need to derive ToResponse on this type to make it a canned response
    // The type that derives this trait must contain a schema, which is the schema used
    // for the canned response body.
    //
    // In the response annotation, we describe the canned response and provide an example response body
    #[derive(ToResponse)]
    #[response(
        description = "Conflicting data already exists in the system",
        example = json!({
            "error_code": "conflicting_data",
            "error_description": "Other data already present in the system conflicts with the new data.",
            "extra_info": null
        })
    )]
    pub struct BasicError409(BasicError);
    
    // ...other canned responses
}
```

Once the canned response is defined, we can add it to `OpenApiSchemas` just like DTO definitions:

```rust
// in dto.rs

#[derive(OpenApi)]
#[openapi(components(
    // ...other OpenAPI data
    
    responses(
        // Add the canned response here
        err_resps::BasicError409,
    
        // ...other canned responses
    ),
))]
pub struct OpenApiSchemas;
```
