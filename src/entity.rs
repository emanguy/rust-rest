use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// DB entity describing a user
#[derive(Debug, Deserialize, Serialize, FromRow, PartialEq, Eq)]
pub struct TodoUser {
    pub id: i32,
    pub first_name: String,
    pub last_name: String,
}

/// DB entity describing a user's task
#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct TodoTask {
    pub id: i32,
    pub user_id: i32,
    pub item_desc: String,
}
