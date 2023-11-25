use anyhow::anyhow;

pub enum Connectivity {
    Connected,
    Disconnected,
}

impl Connectivity {
    pub fn blow_up_if_disconnected(&self) -> Result<(), anyhow::Error> {
        match self {
            Self::Connected => Ok(()),
            Self::Disconnected => Err(anyhow!(
                "could not connect to service!"
            )),
        }
    }
}
