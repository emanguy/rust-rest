use std::sync::Arc;
use crate::domain::{DrivenPortError, Error, Resource};
use async_trait::async_trait;
use crate::domain::user::driven_ports::{UserReader, UserWriter};
use crate::external_connections::ExternalConnectivity;

#[derive(PartialEq, Eq, Debug)]
pub struct TodoUser {
    pub id: u32,
    pub first_name: String,
    pub last_name: String,
}

#[derive(Debug, Error)]
#[error("User with id {id} does not exist")]
pub struct UserDoesNotExistError {
    pub id: u32,
}

pub mod driven_ports {
    use super::*;
    use async_trait::async_trait;
    use crate::external_connections::ExternalConnectivity;

    #[async_trait]
    pub trait UserReader {
        async fn get_all(&self, ext_cxn: &impl ExternalConnectivity) -> Result<Vec<TodoUser>, anyhow::Error>;
        async fn get_by_id(&self, id: u32, ext_cxn: &impl ExternalConnectivity) -> Result<Option<TodoUser>, anyhow::Error>;
    }

    #[async_trait]
    pub trait UserWriter {
        async fn create_user(&self, user: &CreateUser, ext_cxn: &impl ExternalConnectivity) -> Result<u32, anyhow::Error>;
    }

    #[async_trait]
    pub trait DetectUser {
        async fn user_exists(&self, user_id: u32, ext_cxn: &impl ExternalConnectivity) -> Result<bool, anyhow::Error>;
    }
}

pub struct CreateUser {
    pub first_name: String,
    pub last_name: String,
}

pub mod driving_ports {
    use super::*;
    use async_trait::async_trait;
    use crate::external_connections::ExternalConnectivity;

    #[async_trait]
    pub trait UserPort {
        async fn get_users(&self, ext_cxn: &impl ExternalConnectivity) -> Result<Vec<TodoUser>, ()>;
        async fn create_user(&self, new_user: &CreateUser, ext_cxn: &impl ExternalConnectivity) -> Result<u32, ()>;
    }
}


struct UserService<URead, UWrite>
    where URead: driven_ports::UserReader,
          UWrite: driven_ports::UserWriter {}

#[derive(Debug, Error)]
pub(super) enum UserExistsErr {
    #[error("user with ID {0} does not exist")]
    UserDoesNotExist(u32),

    #[error(transparent)]
    PortError(#[from] anyhow::Error)
}

pub(super) async fn verify_user_exists(
    user_detect: &impl driven_ports::DetectUser,
    id: u32,
) -> Result<(), UserExistsErr> {
    let does_user_exist = user_detect
        .user_exists(id)
        .await?;

    if does_user_exist {
        Ok(())
    } else {
        Err(UserExistsErr::UserDoesNotExist(id))
    }
}

#[async_trait]
impl <URead, UWrite> driving_ports::UserPort for UserService<URead, UWrite>
    where URead: UserReader,
          UWrite: UserWriter {
    async fn get_users(ext_cxn: &impl ExternalConnectivity) -> Result<Vec<TodoUser>, ()> {
        let all_users_result = URead::get_all(ext_cxn).await;
        if let Err(ref port_err) = all_users_result {
            log::error!("User fetch failure: {port_err}");
        }

        all_users_result.map_err(|err| err.into_error_trying_to("look up all users"))
    }

    async fn create_user(new_user: &CreateUser, ext_cxn: &impl ExternalConnectivity) -> Result<u32, ()> {
        UWrite::create_user(new_user, ext_cxn)
            .await
            .map_err(|err| err.into_error_trying_to("create a new user"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;


}

#[cfg(test)]
pub(super) mod test_util {
    use super::*;
    use crate::domain::test_util::Connectivity;
    use std::collections::HashSet;
    use std::sync::RwLock;
    use crate::domain::user::driven_ports::DetectUser;

    pub struct InMemoryUserPersistence {
        highest_user_id: u32,
        pub created_users: Vec<TodoUser>,
        pub connected: Connectivity,
    }

    impl InMemoryUserPersistence {
        pub fn new() -> InMemoryUserPersistence {
            InMemoryUserPersistence {
                highest_user_id: 0,
                created_users: Vec::new(),
                connected: Connectivity::Connected,
            }
        }
    }

    #[async_trait]
    impl UserWriter for RwLock<InMemoryUserPersistence> {
        async fn create_user(&self, user: &CreateUser, _: &impl ExternalConnectivity) -> Result<u32, anyhow::Error> {
            let mut persister = self.write().expect("user create mutex poisoned");
            persister.connected.blow_up_if_disconnected()?;

            persister.highest_user_id += 1;
            let id = persister.highest_user_id;
            persister.created_users.push(TodoUser {
                id,
                first_name: user.first_name.clone(),
                last_name: user.last_name.clone(),
            });

            Ok(persister.highest_user_id)
        }
    }

    #[async_trait]
    impl UserReader for RwLock<InMemoryUserPersistence> {
        async fn get_all(&self, _: &impl ExternalConnectivity) -> Result<Vec<TodoUser>, anyhow::Error> {
            let persister = self.read().expect("user read rwlock poisoned");
            persister.connected.blow_up_if_disconnected()?;

            Ok(persister
                .created_users
                .iter()
                .map(|user| TodoUser {
                    id: user.id,
                    first_name: user.first_name.clone(),
                    last_name: user.last_name.clone(),
                })
                .collect())
        }

        async fn get_by_id(&self, id: u32, _: &impl ExternalConnectivity) -> Result<Option<TodoUser>, anyhow::Error> {
            let persister = self.read().expect("user read rwlock poisoned");
            persister.connected.blow_up_if_disconnected()?;

            let user = persister.created_users.iter().find(|user| user.id == id);
            match user {
                Some(user) => Ok(Some(TodoUser {
                    id: user.id,
                    first_name: user.first_name.clone(),
                    last_name: user.last_name.clone(),
                })),
                None => Ok(None),
            }
        }
    }

    pub fn user_create_default() -> CreateUser {
        CreateUser {
            first_name: "First".into(),
            last_name: "Last".into(),
        }
    }

    pub fn user_from_create(create_request: &CreateUser, id: u32) -> TodoUser {
        TodoUser {
            id,
            first_name: create_request.first_name.clone(),
            last_name: create_request.last_name.clone(),
        }
    }

    pub struct InMemoryUserDetector {
        pub known_users: HashSet<u32>,
        pub connectivity: Connectivity,
    }

    impl InMemoryUserDetector {
        pub fn new() -> InMemoryUserDetector {
            InMemoryUserDetector {
                known_users: HashSet::new(),
                connectivity: Connectivity::Connected,
            }
        }

        pub fn with_users<const N: usize>(user_ids: [u32; N]) -> InMemoryUserDetector {
            InMemoryUserDetector {
                known_users: HashSet::from(user_ids),
                connectivity: Connectivity::Connected,
            }
        }
    }

    #[async_trait]
    impl DetectUser for RwLock<InMemoryUserDetector> {
        async fn user_exists(&self, user_id: u32, _: &impl ExternalConnectivity) -> Result<bool, anyhow::Error> {
            let detector = self.read().expect("user detect rwlock poisoned");
            detector.connectivity.blow_up_if_disconnected()?;

            Ok(detector.known_users.contains(&user_id))
        }
    }
}
