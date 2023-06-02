use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{OpenApi, ToSchema};

#[derive(OpenApi)]
#[openapi(components(schemas(TodoUser, TodoTask)))]
pub struct SystemEntities;

/// DB entity describing a user
#[derive(Debug, Deserialize, Serialize, FromRow, PartialEq, Eq, ToSchema)]
#[schema(example = json!({
    "id": 1,
    "first_name": "John",
    "last_name": "Doe"
}))]
pub struct TodoUser {
    pub id: i32,
    pub first_name: String,
    pub last_name: String,
}

/// DB entity describing a user's task
#[derive(Debug, Deserialize, Serialize, FromRow, ToSchema)]
#[schema(example = json!({
    "id": 1,
    "user_id": 2,
    "item_desc": "Something to get done today"
}))]
pub struct TodoTask {
    pub id: i32,
    pub user_id: i32,
    pub item_desc: String,
}
