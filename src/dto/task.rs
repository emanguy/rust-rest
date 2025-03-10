use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;
use crate::domain;

/// DTO for creating a new task via the API
#[derive(Deserialize, Validate, ToSchema)]
#[cfg_attr(test, derive(Serialize))]
pub struct NewTask {
    #[validate(length(min = 1))]
    pub item_desc: String,
}

impl From<NewTask> for domain::todo::NewTask {
    fn from(value: NewTask) -> Self {
        domain::todo::NewTask {
            description: value.item_desc,
        }
    }
}

/// DTO for a returned task on the API
#[derive(Serialize, ToSchema)]
pub struct TodoTask {
    #[schema(example = 10)]
    pub id: i32,
    #[schema(example = "Something to do")]
    pub description: String,
}

impl From<domain::todo::TodoTask> for TodoTask {
    fn from(value: domain::todo::TodoTask) -> Self {
        TodoTask {
            id: value.id,
            description: value.item_desc,
        }
    }
}

/// DTO for updating a task's content via the API
#[derive(Debug, Deserialize, Validate, ToSchema)]
#[cfg_attr(test, derive(Serialize))]
pub struct UpdateTask {
    #[validate(length(min = 1))]
    pub description: String,
}

impl From<UpdateTask> for domain::todo::UpdateTask {
    fn from(value: UpdateTask) -> Self {
        domain::todo::UpdateTask {
            description: value.description,
        }
    }
}

/// DTO for a newly created task
#[derive(Serialize, ToSchema)]
pub struct InsertedTask {
    #[schema(example = 5)]
    pub id: i32,
}
