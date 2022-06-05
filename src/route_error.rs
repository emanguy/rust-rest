use actix_web::{http::StatusCode, HttpResponse, ResponseError, web::JsonConfig};
use derive_more::Display;
use log::error;
use serde::Serialize;
use validator::{ValidationError, ValidationErrors};

use crate::db::DbError;

#[derive(Serialize, Debug)]
pub struct BasicErrorResponse {
    error_code: String,
    error_description: String,
    extra_info: Option<String>,
}

#[derive(Debug, Display)]
#[display(fmt = "Error {}: {}", "status", "full_error.error_description")]
pub struct BasicError {
    status: u16,
    full_error: BasicErrorResponse,
}

impl ResponseError for BasicError {
    fn status_code(&self) -> StatusCode {
        StatusCode::from_u16(self.status)
            .expect(format!("Tried to use illegal HTTP status: {}", self.status).as_str())
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code()).json(&self.full_error)
    }
}

impl BasicError {
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

    pub fn from_validate(validation_err: ValidationErrors) -> Self {
        error!("Input was invalid: {}", validation_err);
        BasicError {
            status: 400,
            full_error: BasicErrorResponse {
                error_code: "invalid_input".to_owned(),
                error_description: "Submitted data was invalid.".to_owned(),
                extra_info: Some(format!("Input had the following validation errors: {}", validation_err)),
            }
        }
    }
}

pub fn default_json_error_handler() -> JsonConfig {
    JsonConfig::default().error_handler(|payload_err, _| {
        error!("Received invalid JSON: {}", payload_err);
        BasicError {
            status: 400,
            full_error: BasicErrorResponse { 
                error_code: "incomplete_json".to_owned(), 
                error_description: "You sent an incomplete request.".to_owned(), 
                extra_info: Some(format!("The JSON was invalid for this reason: {}", payload_err)),
            }
        }.into()
    })
}
