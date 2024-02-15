use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use crate::domain;



/// DB entity describing a user's task
#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct TodoTask {
    pub id: i32,
    pub user_id: i32,
    pub item_desc: String,
}
