use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use derive_more::Display;
use serde::Serialize;

use crate::db::DbError;

#[derive(Serialize, Debug)]
pub struct BasicErrorResponse {
    error_code: String,
    error_description: String,
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
        match (db_error) {
            DbError::NoResults => BasicError {
                status: 404,
                full_error: BasicErrorResponse {
                    error_code: "not_found".to_owned(),
                    error_description: "The requested entity could not be found.".to_owned(),
                },
            },
            DbError::QueryFailure(..) => BasicError {
                status: 500,
                full_error: BasicErrorResponse {
                    error_code: "internal_error".to_owned(),
                    error_description: "Could not read data to complete your request.".to_owned(),
                },
            },
        }
    }
}
