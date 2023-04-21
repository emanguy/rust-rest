use crate::domain::DrivenPortError;
use anyhow::anyhow;

pub enum Connectivity {
    Connected,
    Disconnected,
}

impl Connectivity {
    pub fn blow_up_if_disconnected(&self) -> Result<(), DrivenPortError> {
        match self {
            Self::Connected => Ok(()),
            Self::Disconnected => Err(DrivenPortError::CommsFailure(anyhow!(
                "could not connect to service!"
            ))),
        }
    }
}
