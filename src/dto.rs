use crate::domain;
use derive_more::Display;
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToResponse, ToSchema};
use validator::{Validate, ValidationErrors};

#[derive(OpenApi)]
#[openapi(components(schemas(
    TodoUser,
    NewUser,
    InsertedUser,
    NewTask,
    TodoTask,
    UpdateTask,
    InsertedTask,
)))]
pub struct OpenApiSchemas;

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
#[derive(Deserialize, Validate, ToSchema)]
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

#[derive(Serialize, ToSchema)]
pub struct InsertedTask {
    #[schema(example = 5)]
    pub id: i32,
}

/// Contains diagnostic information about an API failure
#[derive(Serialize, Debug, ToResponse)]
#[cfg_attr(test, derive(Deserialize))]
#[response(examples(
    ("Not Found" = (
        summary = "Entity could not be found (404)",
        value = json!({
            "error_code": "not_found",
            "error_description": "The requested entity could not be found.",
            "extra_info": null
        })
    )),

    ("Internal Failure" = (
        summary = "Something unexpected went wrong inside the server (500)",
        value = json!({
            "error_code": "internal_error",
            "error_description": "Could not access data to complete your request",
            "extra_info": null
        })
    )),

    ("Invalid Input" = (
        summary = "Invalid request body was passed (400)",
        value = json!({
            "error_code": "invalid_input",
            "error_description": "Submitted data was invalid.",
            "extra_info": {
                "first_name": [
                    {
                        "code": "length",
                        "message": null,
                        "params": {
                            "value": "Nameeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                            "max": 30
                        }
                    }
                ]
            }
        })
    )),

    ("Malformed JSON" = (
        summary = "Invalid JSON passed to server (400)",
        value = json!({
            "error_code": "invalid_json",
            "error_description": "The passed request body contained malformed or unreadable JSON.",
            "extra_info": "Failed to parse the request body as JSON: EOF while parsing an object at line 4 column 0"
        })
    ))
))]
pub struct BasicError {
    pub error_code: String,
    pub error_description: String,

    #[serde(skip_deserializing)]
    pub extra_info: Option<ExtraInfo>,
}

#[derive(Serialize, Debug)]
#[serde(untagged)]
pub enum ExtraInfo {
    ValidationIssues(ValidationErrors),
    Message(String),
}

#[cfg(test)]
mod dto_tests {
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
