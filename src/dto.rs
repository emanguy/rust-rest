use derive_more::Display;
use serde::{Deserialize, Serialize};
use validator::Validate;
use crate::domain;

/// DTO for a constructed user
#[derive(Serialize)]
pub struct TodoUser {
    pub id: i32,
    pub first_name: String,
    pub last_name: String,
}

impl From<domain::user::TodoUser> for TodoUser {
    fn from(value: domain::user::TodoUser) -> Self {
        TodoUser {
            id: value.id,
            first_name: value.first_name,
            last_name: value.last_name,
        }
    }
}

/// DTO for creating a new user via the API
#[derive(Deserialize, Serialize, Display, Validate)]
#[display(fmt = "{} {}", "first_name", "last_name")]
pub struct NewUser {
    #[validate(length(max = 30))]
    pub first_name: String,
    #[validate(length(max = 50))]
    pub last_name: String,
}

/// DTO containing the ID of a user that was created via the API.
#[derive(Deserialize, Serialize)]
pub struct InsertedUser {
    pub id: i32,
}

/// DTO for creating a new task via the API
#[derive(Deserialize, Serialize, Validate)]
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
#[derive(Serialize)]
pub struct TodoTask {
    id: i32,
    description: String,
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
#[derive(Deserialize, Serialize, Validate)]
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

#[cfg(test)]
mod dto_tests {
    use super::*;

    #[test]
    fn bad_user_data_gets_rejected() {
        let bad_user = NewUser {
            first_name: (0..35).map(|_| "A").collect(),
            last_name: (0..55).map(|_| "B").collect(),
        };
        let validation_result = bad_user.validate();
        assert!(validation_result.is_err());
        let validation_errors = validation_result.unwrap_err();
        let field_validations = validation_errors.field_errors();
        assert!(field_validations.contains_key("first_name"));
        assert!(field_validations.contains_key("last_name"));
    }
}

#[derive(Serialize)]
pub struct InsertedTask {
    pub id: i32,
}
