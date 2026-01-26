use crate::domain;
use derive_more::Display;
use serde::{Deserialize, Serialize};
use utoipa::openapi::{RefOr, Schema};
use utoipa::{openapi, OpenApi, ToSchema};
use validator::{Validate, ValidationErrors};

#[derive(OpenApi)]
#[openapi(components(
    schemas(
        TodoUser,
        NewUser,
        InsertedUser,
        NewTask,
        TodoTask,
        UpdateTask,
        InsertedTask,
        BasicError,
        ExtraInfo,
        ValidationErrorSchema,
    ),
    responses(
        err_resps::BasicError400Validation,
        err_resps::BasicError404,
        err_resps::BasicError500,
    ),
))]
/// Captures OpenAPI schemas and canned responses defined in the DTO module
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
#[display("{} {}", first_name, last_name)]
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

/// DTO for a newly created task
#[derive(Serialize, ToSchema)]
pub struct InsertedTask {
    #[schema(example = 5)]
    pub id: i32,
}

/// Contains diagnostic information about an API failure
#[derive(Serialize, Debug, ToSchema)]
#[cfg_attr(test, derive(Deserialize))]
pub struct BasicError {
    /// A sentinel value that can be used to differentiate between different causes of a non-2XX
    /// HTTP response code
    pub error_code: String,
    /// A human-readable error message suitable for showing to users
    pub error_description: String,

    /// Additional contextual information, such as what validations failed on a request DTO
    #[serde(skip_deserializing)]
    pub extra_info: Option<ExtraInfo>,
}

/// Contains a set of generic OpenAPI error responses based on [BasicError] that can
/// be easily reused in other requests
pub mod err_resps {
    use crate::dto::BasicError;
    use utoipa::ToResponse;

    #[derive(ToResponse)]
    #[response(
        description = "Invalid request body was passed",
        example = json!({
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
    )]
    pub struct BasicError400Validation(BasicError);

    #[derive(ToResponse)]
    #[response(
        description = "Entity could not be found",
        example = json!({
            "error_code": "not_found",
            "error_description": "The requested entity could not be found.",
            "extra_info": null
        })
    )]
    pub struct BasicError404(BasicError);

    #[derive(ToResponse)]
    #[response(
        description = "Something unexpected went wrong inside the server",
        example = json!({
            "error_code": "internal_error",
            "error_description": "Could not access data to complete your request",
            "extra_info": null
        })
    )]
    pub struct BasicError500(BasicError);
}

/// Extra contextual information which explains why an API error occurred
#[derive(Serialize, Debug, ToSchema)]
#[serde(untagged)]
pub enum ExtraInfo {
    ValidationIssues(ValidationErrorSchema),
    Message(String),
}

/// Stand-in OpenAPI schema for [ValidationErrors] which just provides an empty object
#[derive(Serialize, Debug)]
#[serde(transparent)]
pub struct ValidationErrorSchema(pub ValidationErrors);

impl<'schem> ToSchema<'schem> for ValidationErrorSchema {
    fn schema() -> (&'schem str, RefOr<Schema>) {
        (
            "ValidationErrorSchema",
            openapi::ObjectBuilder::new().into(),
        )
    }
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
