use crate::domain::user::driving_ports::CreateUserError;
use crate::domain::Error;
use crate::external_connections::ExternalConnectivity;
use anyhow::Context;
use async_trait::async_trait;

#[derive(PartialEq, Eq, Debug)]
pub struct TodoUser {
    pub id: u32,
    pub first_name: String,
    pub last_name: String,
}

pub mod driven_ports {
    use super::*;
    use crate::external_connections::ExternalConnectivity;
    use async_trait::async_trait;

    #[async_trait]
    pub trait UserReader: Sync {
        async fn get_all(
            &self,
            ext_cxn: &impl ExternalConnectivity<'_>,
        ) -> Result<Vec<TodoUser>, anyhow::Error>;
        async fn get_by_id(
            &self,
            id: u32,
            ext_cxn: &impl ExternalConnectivity<'_>,
        ) -> Result<Option<TodoUser>, anyhow::Error>;
    }

    #[async_trait]
    pub trait UserWriter: Sync {
        async fn create_user(
            &self,
            user: &CreateUser,
            ext_cxn: &impl ExternalConnectivity<'_>,
        ) -> Result<u32, anyhow::Error>;
    }

    pub struct UserDescription<'names> {
        pub first_name: &'names str,
        pub last_name: &'names str,
    }

    #[async_trait]
    pub trait DetectUser: Sync {
        async fn user_exists(
            &self,
            user_id: u32,
            ext_cxn: &impl ExternalConnectivity<'_>,
        ) -> Result<bool, anyhow::Error>;

        async fn user_with_name_exists<'strings>(
            &self,
            description: UserDescription<'strings>,
            ext_cxn: &impl ExternalConnectivity<'_>,
        ) -> Result<bool, anyhow::Error>;
    }
}

pub struct CreateUser {
    pub first_name: String,
    pub last_name: String,
}

pub mod driving_ports {
    use super::*;
    use crate::external_connections::ExternalConnectivity;
    use async_trait::async_trait;

    #[derive(Debug, Error)]
    pub enum CreateUserError {
        #[error("The provided user already exists.")]
        UserAlreadyExists,
        #[error(transparent)]
        PortError(#[from] anyhow::Error),
    }

    #[async_trait]
    pub trait UserPort {
        async fn get_users(
            &self,
            ext_cxn: &impl ExternalConnectivity<'_>,
            u_reader: &impl driven_ports::UserReader,
        ) -> Result<Vec<TodoUser>, anyhow::Error>;
        async fn create_user(
            &self,
            new_user: &CreateUser,
            ext_cxn: &impl ExternalConnectivity<'_>,
            u_writer: &impl driven_ports::UserWriter,
            u_detect: &impl driven_ports::DetectUser,
        ) -> Result<u32, CreateUserError>;
    }
}

pub struct UserService {}

#[derive(Debug, Error)]
pub(super) enum UserExistsErr {
    #[error("user with ID {0} does not exist")]
    UserDoesNotExist(u32),

    #[error(transparent)]
    PortError(#[from] anyhow::Error),
}

pub(super) async fn verify_user_exists(
    id: u32,
    external_cxn: &impl ExternalConnectivity<'_>,
    user_detect: &impl driven_ports::DetectUser,
) -> Result<(), UserExistsErr> {
    let does_user_exist = user_detect.user_exists(id, external_cxn).await?;

    if does_user_exist {
        Ok(())
    } else {
        Err(UserExistsErr::UserDoesNotExist(id))
    }
}

#[cfg(test)]
mod verify_user_exists_tests {
    use super::*;
    use crate::domain::test_util::Connectivity;
    use crate::domain::user::driven_ports::UserWriter;
    use crate::external_connections;
    use speculoos::prelude::*;
    use std::sync::RwLock;

    #[tokio::test]
    async fn detects_user() {
        let user_stuff = test_util::InMemoryUserPersistence::new_locked();
        let db_cxn = external_connections::test_util::FakeExternalConnectivity::new();

        // This is guaranteed to succeed because it's connected by default
        let create_result = user_stuff
            .create_user(&test_util::user_create_default(), &db_cxn)
            .await;
        let new_user_id = match create_result {
            Ok(info) => info,
            Err(_) => unreachable!(),
        };

        let exists_result = verify_user_exists(new_user_id, &db_cxn, &user_stuff).await;
        assert_that!(exists_result).is_ok();
    }

    #[tokio::test]
    async fn errors_when_user_doesnt_exist() {
        let user_stuff = test_util::InMemoryUserPersistence::new_locked();
        let db_cxn = external_connections::test_util::FakeExternalConnectivity::new();

        let exists_result = verify_user_exists(5, &db_cxn, &user_stuff).await;
        assert_that!(exists_result)
            .is_err()
            .matches(|inner_err| matches!(inner_err, UserExistsErr::UserDoesNotExist(5)));
    }

    #[tokio::test]
    async fn propagates_port_error() {
        let mut user_persistence = test_util::InMemoryUserPersistence::new();
        user_persistence.connectivity = Connectivity::Disconnected;

        let user_stuff = RwLock::new(user_persistence);
        let db_cxn = external_connections::test_util::FakeExternalConnectivity::new();

        let exists_result = verify_user_exists(5, &db_cxn, &user_stuff).await;
        assert_that!(exists_result)
            .is_err()
            .matches(|inner_err| matches!(inner_err, UserExistsErr::PortError(_)));
    }
}

#[async_trait]
impl driving_ports::UserPort for UserService {
    async fn get_users(
        &self,
        ext_cxn: &impl ExternalConnectivity<'_>,
        u_reader: &impl driven_ports::UserReader,
    ) -> Result<Vec<TodoUser>, anyhow::Error> {
        let all_users_result = u_reader.get_all(ext_cxn).await;
        if let Err(ref port_err) = all_users_result {
            log::error!("User fetch failure: {port_err}");
        }

        all_users_result.context("Failed fetching users")
    }

    async fn create_user(
        &self,
        new_user: &CreateUser,
        ext_cxn: &impl ExternalConnectivity<'_>,
        u_writer: &impl driven_ports::UserWriter,
        u_detect: &impl driven_ports::DetectUser,
    ) -> Result<u32, CreateUserError> {
        let description = driven_ports::UserDescription {
            first_name: &new_user.first_name,
            last_name: &new_user.last_name,
        };

        let user_exists = u_detect
            .user_with_name_exists(description, ext_cxn)
            .await
            .context("Looking up user during creation")?;
        if user_exists {
            return Err(CreateUserError::UserAlreadyExists);
        }

        Ok(u_writer
            .create_user(new_user, ext_cxn)
            .await
            .context("Trying to create user at service level")?)
    }
}

#[cfg(test)]
mod user_service_tests {
    use super::*;
    use crate::domain::test_util::Connectivity;
    use crate::domain::user::driving_ports::UserPort;
    use crate::external_connections;
    use speculoos::prelude::*;
    use std::sync::RwLock;

    #[tokio::test]
    async fn get_users_can_fetch_users() {
        let db_cxn = external_connections::test_util::FakeExternalConnectivity::new();
        let user_data = test_util::InMemoryUserPersistence::new_with_users(&[
            CreateUser {
                first_name: "John".to_owned(),
                last_name: "Doe".to_owned(),
            },
            CreateUser {
                first_name: "Jeff".to_owned(),
                last_name: "Doe".to_owned(),
            },
            CreateUser {
                first_name: "Jane".to_owned(),
                last_name: "Doe".to_owned(),
            },
        ]);
        let locked_user_data = RwLock::new(user_data);
        let user_service = UserService {};

        let users_result = user_service.get_users(&db_cxn, &locked_user_data).await;
        let fetched_users = match users_result {
            Ok(users) => users,
            Err(error) => panic!("Should have fetched users but failed: {}", error),
        };

        assert_that!(fetched_users).matches(|users| {
            matches!(users.as_slice(), [
                TodoUser {
                    id: 1,
                    first_name: fn1,
                    last_name: ln1,
                },
                TodoUser {
                    id: 2,
                    first_name: fn2,
                    last_name: ln2,
                },
                TodoUser {
                    id: 3,
                    first_name: fn3,
                    last_name: ln3
                }
            ] if fn1 == "John" &&
                ln1 == "Doe" &&
                fn2 == "Jeff" &&
                ln2 == "Doe" &&
                fn3 == "Jane" &&
                ln3 == "Doe"
            )
        });
    }

    #[tokio::test]
    async fn get_users_propagates_error() {
        let db_cxn = external_connections::test_util::FakeExternalConnectivity::new();
        let mut user_data = test_util::InMemoryUserPersistence::new();
        user_data.connectivity = Connectivity::Disconnected;
        let locked_user_data = RwLock::new(user_data);
        let user_service = UserService {};

        let get_result = user_service.get_users(&db_cxn, &locked_user_data).await;
        assert_that!(get_result).is_err();
    }

    #[tokio::test]
    async fn create_user_happy_path() {
        let db_cxn = external_connections::test_util::FakeExternalConnectivity::new();
        let user_data = test_util::InMemoryUserPersistence::new_locked();
        let user_service = UserService {};
        let new_user = test_util::user_create_default();

        let create_result = user_service
            .create_user(&new_user, &db_cxn, &user_data, &user_data)
            .await;
        assert_that!(create_result).is_ok();
    }

    #[tokio::test]
    async fn create_user_fails_if_user_already_exists() {
        let db_cxn = external_connections::test_util::FakeExternalConnectivity::new();
        let user_persistence = test_util::InMemoryUserPersistence::new_with_users(&[CreateUser {
            first_name: "Evan".to_owned(),
            last_name: "Rittenhouse".to_owned(),
        }]);
        let locked_user_data = RwLock::new(user_persistence);
        let user_service = UserService {};
        let new_user = CreateUser {
            first_name: "Evan".to_owned(),
            last_name: "Rittenhouse".to_owned(),
        };

        let create_result = user_service
            .create_user(&new_user, &db_cxn, &locked_user_data, &locked_user_data)
            .await;
        let returned_error = match create_result {
            Err(error) => error,
            Ok(num) => {
                panic!("Creating user should not have succeeded, got this user ID back: {num}")
            }
        };

        assert_that!(returned_error)
            .matches(|err| matches!(err, CreateUserError::UserAlreadyExists));
    }

    #[tokio::test]
    async fn create_user_propagates_port_error() {
        let db_cxn = external_connections::test_util::FakeExternalConnectivity::new();
        let mut user_data = test_util::InMemoryUserPersistence::new();
        user_data.connectivity = Connectivity::Disconnected;
        let locked_user_data = RwLock::new(user_data);
        let user_service = UserService {};
        let new_user = test_util::user_create_default();

        let create_result = user_service
            .create_user(&new_user, &db_cxn, &locked_user_data, &locked_user_data)
            .await;
        assert_that!(create_result)
            .is_err()
            .matches(|err| matches!(err, CreateUserError::PortError(_)));
    }
}

#[cfg(test)]
pub(super) mod test_util {
    use super::*;
    use crate::domain::test_util::Connectivity;
    use crate::domain::user::driven_ports::{DetectUser, UserDescription};
    use anyhow::Error;

    use std::sync::RwLock;

    pub struct InMemoryUserPersistence {
        highest_user_id: u32,
        pub created_users: Vec<TodoUser>,
        pub connectivity: Connectivity,
    }

    impl InMemoryUserPersistence {
        pub fn new() -> InMemoryUserPersistence {
            InMemoryUserPersistence {
                highest_user_id: 0,
                created_users: Vec::new(),
                connectivity: Connectivity::Connected,
            }
        }

        pub fn new_with_users(users: &[CreateUser]) -> InMemoryUserPersistence {
            InMemoryUserPersistence {
                highest_user_id: users.len() as u32,
                created_users: users
                    .iter()
                    .enumerate()
                    .map(|(index, user_info)| TodoUser {
                        id: (index + 1) as u32,
                        first_name: user_info.first_name.clone(),
                        last_name: user_info.last_name.clone(),
                    })
                    .collect(),
                connectivity: Connectivity::Connected,
            }
        }

        pub fn new_locked() -> RwLock<InMemoryUserPersistence> {
            RwLock::new(InMemoryUserPersistence::new())
        }
    }

    #[async_trait]
    impl driven_ports::UserWriter for RwLock<InMemoryUserPersistence> {
        async fn create_user(
            &self,
            user: &CreateUser,
            _: &impl ExternalConnectivity<'_>,
        ) -> Result<u32, anyhow::Error> {
            let mut persister = self.write().expect("user create mutex poisoned");
            persister.connectivity.blow_up_if_disconnected()?;

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
    impl driven_ports::UserReader for RwLock<InMemoryUserPersistence> {
        async fn get_all(
            &self,
            _: &impl ExternalConnectivity<'_>,
        ) -> Result<Vec<TodoUser>, anyhow::Error> {
            let persister = self.read().expect("user read rwlock poisoned");
            persister.connectivity.blow_up_if_disconnected()?;

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

        async fn get_by_id(
            &self,
            id: u32,
            _: &impl ExternalConnectivity<'_>,
        ) -> Result<Option<TodoUser>, anyhow::Error> {
            let persister = self.read().expect("user read rwlock poisoned");
            persister.connectivity.blow_up_if_disconnected()?;

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

    #[derive(Eq, PartialEq, Hash)]
    struct UserEntry {
        first_name: String,
        last_name: String,
    }

    #[async_trait]
    impl DetectUser for RwLock<InMemoryUserPersistence> {
        async fn user_exists(
            &self,
            user_id: u32,
            _: &impl ExternalConnectivity<'_>,
        ) -> Result<bool, anyhow::Error> {
            let detector = self.read().expect("user detect rwlock poisoned");
            detector.connectivity.blow_up_if_disconnected()?;

            Ok(detector.created_users.iter().any(|user| user.id == user_id))
        }

        async fn user_with_name_exists<'strings>(
            &self,
            description: UserDescription<'strings>,
            _: &impl ExternalConnectivity<'_>,
        ) -> Result<bool, Error> {
            let detector = self.read().expect("user detect rwlock poisoned");
            detector.connectivity.blow_up_if_disconnected()?;

            Ok(detector.created_users.iter().any(|user| {
                user.first_name == description.first_name && user.last_name == description.last_name
            }))
        }
    }
}
