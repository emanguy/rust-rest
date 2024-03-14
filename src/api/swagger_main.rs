use crate::dto;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(info(
    title = "Rust Todo API",
    description = "A sample to-do list API written in Rust"
))]
struct TodoApi;

/// Constructs the route on the API that renders the swagger UI and returns the OpenAPI schema.
/// Merges in OpenAPI definitions from other locations in the app, such as the [dto] package
/// and submodules of [api][crate::api]
pub fn build_documentation() -> SwaggerUi {
    let mut api_docs = TodoApi::openapi();
    api_docs.merge(dto::OpenApiSchemas::openapi());
    api_docs.merge(super::user::UsersApi::openapi());
    api_docs.merge(super::todo::TaskApi::openapi());

    SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", api_docs)
}
