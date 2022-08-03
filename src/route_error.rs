use actix_web::{http::StatusCode, web::JsonConfig, HttpResponse, ResponseError};
use derive_more::Display;
use log::error;
use serde::Serialize;
use validator::ValidationErrors;

use crate::db::DbError;

/// Contains diagnostic information about an API failure
#[derive(Serialize, Debug)]
pub struct BasicErrorResponse {
    error_code: String,
    error_description: String,
    extra_info: Option<String>,
}

/// Basic error type for the application. Other lower level errors can be converted into this type using
/// transform functions defined on it in combination with [Result]'s `map_err()` function.
#[derive(Debug, Display)]
#[display(fmt = "Error {}: {}", "status", "full_error.error_description")]
pub struct BasicError {
    status: u16,
    full_error: BasicErrorResponse,
}

impl ResponseError for BasicError {
    fn status_code(&self) -> StatusCode {
        StatusCode::from_u16(self.status)
            .unwrap_or_else(|_| panic!("Tried to use illegal HTTP status: {}", self.status))
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code()).json(&self.full_error)
    }
}

impl BasicError {
    /// Converts a [DbError] into a [BasicError].
    pub fn from_db(db_error: DbError) -> Self {
        error!("Database error occurred: {}", db_error);
        match db_error {
            DbError::NoResults => BasicError {
                status: 404,
                full_error: BasicErrorResponse {
                    error_code: "not_found".to_owned(),
                    error_description: "The requested entity could not be found.".to_owned(),
                    extra_info: None,
                },
            },
            DbError::QueryFailure(..) => BasicError {
                status: 500,
                full_error: BasicErrorResponse {
                    error_code: "internal_error".to_owned(),
                    error_description: "Could not access data to complete your request.".to_owned(),
                    extra_info: None,
                },
            },
        }
    }

    /// Converts [ValidationErrors] into a [BasicError].
    pub fn from_validate(validation_err: ValidationErrors) -> Self {
        error!("Input was invalid: {}", validation_err);
        BasicError {
            status: 400,
            full_error: BasicErrorResponse {
                error_code: "invalid_input".to_owned(),
                error_description: "Submitted data was invalid.".to_owned(),
                extra_info: Some(format!(
                    "Input had the following validation errors: {}",
                    validation_err
                )),
            },
        }
    }
}

/// Factory for overriding the default actix JSON error handler to return a [BasicError]
/// to be consistent with the rest of the API.
pub fn default_json_error_handler() -> JsonConfig {
    JsonConfig::default().error_handler(|payload_err, _| {
        error!("Received invalid JSON: {}", payload_err);
        BasicError {
            status: 400,
            full_error: BasicErrorResponse {
                error_code: "incomplete_json".to_owned(),
                error_description: "You sent an incomplete request.".to_owned(),
                extra_info: Some(format!(
                    "The JSON was invalid for this reason: {}",
                    payload_err
                )),
            },
        }
        .into()
    })
}
