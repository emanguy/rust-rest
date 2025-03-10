use derive_more::Display;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;
use crate::domain;

/// DTO for a constructed user
#[derive(Serialize, ToSchema)]
#[cfg_attr(test, derive(Deserialize, PartialEq, Eq, Debug))]
pub struct TodoUser {
    #[schema(example = 4)]
    pub id: i32,
    #[schema(example = "John")]
    pub first_name: String,
    #[schema(example = "Doe")]
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
#[derive(Deserialize, Display, Validate, ToSchema)]
#[display(fmt = "{} {}", "first_name", "last_name")]
#[cfg_attr(test, derive(Serialize))]
pub struct NewUser {
    #[validate(length(max = 30))]
    pub first_name: String,
    #[validate(length(max = 50))]
    pub last_name: String,
}

/// DTO containing the ID of a user that was created via the API.
#[derive(Serialize, ToSchema)]
#[cfg_attr(test, derive(Deserialize, Debug))]
pub struct InsertedUser {
    #[schema(example = 10)]
    pub id: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    mod new_user {
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
}
