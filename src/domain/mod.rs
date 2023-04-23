use derive_more::Display;
use thiserror::Error;
use validator::ValidationErrors;

pub mod todo;
pub mod user;

#[cfg(test)]
mod test_util;

#[derive(Error, Debug)]
pub enum Error {
    #[error("input was invalid: {0}")]
    Invalid(ValidationErrors),
    #[error("could not find dependent resource ({what}) while trying to {action}")]
    DependencyMissing { what: Resource, action: String },
    #[error("failed to {action} due to a communication failure: {cause}")]
    RetrieveFailure {
        action: String,
        #[source]
        cause: anyhow::Error,
    },
}

#[derive(Display, Debug)]
pub enum Resource {
    #[display(fmt = "user with ID of {id}")]
    User { id: u32 },
}

impl From<ValidationErrors> for Error {
    fn from(value: ValidationErrors) -> Self {
        Self::Invalid(value)
    }
}

#[derive(Error, Debug)]
pub enum DrivenPortError {
    #[error("a communication failure occurred: {0}")]
    CommsFailure(anyhow::Error),
}

impl DrivenPortError {
    /// Converts this DrivenPortError to a domain error with some extra info on the [action]
    /// being taken when communicating over the port
    fn into_error_trying_to(self, action: &str) -> Error {
        match self {
            Self::CommsFailure(err) => Error::RetrieveFailure {
                action: action.into(),
                cause: err,
            },
        }
    }
}
