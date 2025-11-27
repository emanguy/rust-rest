use crate::domain::Error;
use crate::domain::user::driving_ports::CreateUserError;
use crate::external_connections::ExternalConnectivity;
use anyhow::Context;
use tracing::*;

#[derive(PartialEq, Eq, Debug, Default)]
#[cfg_attr(test, derive(Clone))]
/// A user who can own to-do items
pub struct TodoUser {
    pub id: i32,
    pub first_name: String,
    pub last_name: String,
}

/// The set of driven ports that can be invoked by the business logic
pub mod driven_ports {
    use super::*;
    use crate::external_connections::ExternalConnectivity;

    /// An external system which can read user data
    pub trait UserReader: Sync {
        /// Retrieve all users in the system
        async fn all(
            &self,
            ext_cxn: &mut impl ExternalConnectivity,
        ) -> Result<Vec<TodoUser>, anyhow::Error>;

        #[expect(dead_code)]
        /// Retrieve a specific user in the system
        async fn by_id(
            &self,
            id: i32,
            ext_cxn: &mut impl ExternalConnectivity,
        ) -> Result<Option<TodoUser>, anyhow::Error>;
    }

    /// An external system which can accept new user data
    pub trait UserWriter: Sync {
        /// Create a new user
        async fn create_user(
            &self,
            user: &CreateUser,
            ext_cxn: &mut impl ExternalConnectivity,
        ) -> Result<i32, anyhow::Error>;
    }

    /// Contains a description of a user's unique personal information
    pub struct UserDescription<'names> {
        pub first_name: &'names str,
        pub last_name: &'names str,
    }

    /// An external system which can report the presence of a user with specific criteria
    pub trait DetectUser: Sync {
        /// Returns true if a user with the given ID already exists
        async fn user_exists(
            &self,
            user_id: i32,
            ext_cxn: &mut impl ExternalConnectivity,
        ) -> Result<bool, anyhow::Error>;

        #[allow(clippy::needless_lifetimes)]
        /// Returns true if a user with the given description already exists
        async fn user_with_name_exists<'strings>(
            &self,
            description: UserDescription<'strings>,
            ext_cxn: &mut impl ExternalConnectivity,
        ) -> Result<bool, anyhow::Error>;
    }
}

#[derive(Debug)]
#[cfg_attr(test, derive(Clone))]
/// Contains information necessary to create a new user
pub struct CreateUser {
    pub first_name: String,
    pub last_name: String,
}

/// Contains the set of driving ports for invoking business logic involving users
pub mod driving_ports {
    use super::*;
    use crate::external_connections::ExternalConnectivity;

    #[derive(Debug, Error)]
    /// Defines the set of reasons why a user would fail to be created
    pub enum CreateUserError {
        #[error("The provided user already exists.")]
        UserAlreadyExists,
        #[error(transparent)]
        PortError(#[from] anyhow::Error),
    }

    /// The driving port which exposes business logic involving users to driving adapters
    pub trait UserPort {
        /// Retrieve the set of users in the system
        async fn get_users(
            &self,
            ext_cxn: &mut impl ExternalConnectivity,
            u_reader: &impl driven_ports::UserReader,
        ) -> Result<Vec<TodoUser>, anyhow::Error>;

        /// Create a new user who can be responsible for to-do items
        async fn create_user(
            &self,
            new_user: &CreateUser,
            ext_cxn: &mut impl ExternalConnectivity,
            u_writer: &impl driven_ports::UserWriter,
            u_detect: &impl driven_ports::DetectUser,
        ) -> Result<i32, CreateUserError>;
    }

    #[cfg(test)]
    mod cue_clone {
        use crate::domain::user::driving_ports::CreateUserError;
        use anyhow::anyhow;

        // Implements clone for CreateUserInfo in tests so the error type can be used with mocks
        impl Clone for CreateUserError {
            fn clone(&self) -> Self {
                match self {
                    CreateUserError::UserAlreadyExists => CreateUserError::UserAlreadyExists,
                    CreateUserError::PortError(anyhow_err) => {
                        CreateUserError::PortError(anyhow!(format!("{}", anyhow_err)))
                    }
                }
            }
        }
    }
}

/// Implementation of the driving port which allows driving adapters to access user business logic
pub struct UserService;

#[derive(Debug, Error)]
/// Error which expresses problems that may occur when asserting a user exists
pub(super) enum UserExistsErr {
    #[error("user with ID {0} does not exist")]
    UserDoesNotExist(i32),

    #[error(transparent)]
    PortError(#[from] anyhow::Error),
}

#[instrument(skip(external_cxn, user_detect))]
/// Asserts that a user already exists in the system, returning an error if not
pub(super) async fn verify_user_exists(
    id: i32,
    external_cxn: &mut impl ExternalConnectivity,
    user_detect: &impl driven_ports::DetectUser,
) -> Result<(), UserExistsErr> {
    let does_user_exist = user_detect.user_exists(id, external_cxn).await?;

    if does_user_exist {
        Ok(())
    } else {
        Err(UserExistsErr::UserDoesNotExist(id))
    }
}

impl driving_ports::UserPort for UserService {
    #[instrument(skip(self, ext_cxn, u_reader))]
    async fn get_users(
        &self,
        ext_cxn: &mut impl ExternalConnectivity,
        u_reader: &impl driven_ports::UserReader,
    ) -> Result<Vec<TodoUser>, anyhow::Error> {
        let all_users_result = u_reader.all(ext_cxn).await;
        if let Err(ref port_err) = all_users_result {
            log::error!("User fetch failure: {port_err}");
        }

        all_users_result.context("Failed fetching users")
    }

    #[instrument(skip(self, ext_cxn, u_writer, u_detect))]
    async fn create_user(
        &self,
        new_user: &CreateUser,
        ext_cxn: &mut impl ExternalConnectivity,
        u_writer: &impl driven_ports::UserWriter,
        u_detect: &impl driven_ports::DetectUser,
    ) -> Result<i32, CreateUserError> {
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
mod tests {
    use super::*;
    use crate::domain::test_util::Connectivity;
    use crate::domain::user::driven_ports::UserWriter;
    use crate::domain::user::driving_ports::UserPort;
    use crate::external_connections;
    use speculoos::prelude::*;
    use std::sync::RwLock;

    mod verify_user_exists {
        use super::*;

        #[tokio::test]
        async fn detects_user() {
            let user_stuff = test_util::InMemoryUserPersistence::new_locked();
            let mut db_cxn = external_connections::test_util::FakeExternalConnectivity::new();

            // This is guaranteed to succeed because it's connected by default
            let create_result = user_stuff
                .create_user(&test_util::user_create_default(), &mut db_cxn)
                .await;
            let new_user_id = match create_result {
                Ok(info) => info,
                Err(_) => unreachable!(),
            };

            let exists_result = verify_user_exists(new_user_id, &mut db_cxn, &user_stuff).await;
            assert_that!(exists_result).is_ok();
        }

        #[tokio::test]
        async fn errors_when_user_doesnt_exist() {
            let user_stuff = test_util::InMemoryUserPersistence::new_locked();
            let mut db_cxn = external_connections::test_util::FakeExternalConnectivity::new();

            let exists_result = verify_user_exists(5, &mut db_cxn, &user_stuff).await;
            assert_that!(exists_result)
                .is_err()
                .matches(|inner_err| matches!(inner_err, UserExistsErr::UserDoesNotExist(5)));
        }

        #[tokio::test]
        async fn propagates_port_error() {
            let mut user_persistence = test_util::InMemoryUserPersistence::new();
            user_persistence.connectivity = Connectivity::Disconnected;

            let user_stuff = RwLock::new(user_persistence);
            let mut db_cxn = external_connections::test_util::FakeExternalConnectivity::new();

            let exists_result = verify_user_exists(5, &mut db_cxn, &user_stuff).await;
            assert_that!(exists_result)
                .is_err()
                .matches(|inner_err| matches!(inner_err, UserExistsErr::PortError(_)));
        }
    }

    mod user_service {
        use super::*;

        mod get_users {
            use super::*;

            #[tokio::test]
            async fn happy_path() {
                let mut db_cxn = external_connections::test_util::FakeExternalConnectivity::new();
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

                let users_result = user_service.get_users(&mut db_cxn, &locked_user_data).await;
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
            async fn propagates_error() {
                let mut db_cxn = external_connections::test_util::FakeExternalConnectivity::new();
                let mut user_data = test_util::InMemoryUserPersistence::new();
                user_data.connectivity = Connectivity::Disconnected;
                let locked_user_data = RwLock::new(user_data);
                let user_service = UserService {};

                let get_result = user_service.get_users(&mut db_cxn, &locked_user_data).await;
                assert_that!(get_result).is_err();
            }
        }

        mod create_user {
            use super::*;

            #[tokio::test]
            async fn happy_path() {
                let mut db_cxn = external_connections::test_util::FakeExternalConnectivity::new();
                let user_data = test_util::InMemoryUserPersistence::new_locked();
                let user_service = UserService {};
                let new_user = test_util::user_create_default();

                let create_result = user_service
                    .create_user(&new_user, &mut db_cxn, &user_data, &user_data)
                    .await;
                assert_that!(create_result).is_ok();
            }

            #[tokio::test]
            async fn fails_if_user_already_exists() {
                let mut db_cxn = external_connections::test_util::FakeExternalConnectivity::new();
                let user_persistence =
                    test_util::InMemoryUserPersistence::new_with_users(&[CreateUser {
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
                    .create_user(&new_user, &mut db_cxn, &locked_user_data, &locked_user_data)
                    .await;
                let returned_error = match create_result {
                    Err(error) => error,
                    Ok(num) => {
                        panic!(
                            "Creating user should not have succeeded, got this user ID back: {num}"
                        )
                    }
                };

                assert_that!(returned_error)
                    .matches(|err| matches!(err, CreateUserError::UserAlreadyExists));
            }

            #[tokio::test]
            async fn propagates_port_error() {
                let mut db_cxn = external_connections::test_util::FakeExternalConnectivity::new();
                let mut user_data = test_util::InMemoryUserPersistence::new();
                user_data.connectivity = Connectivity::Disconnected;
                let locked_user_data = RwLock::new(user_data);
                let user_service = UserService {};
                let new_user = test_util::user_create_default();

                let create_result = user_service
                    .create_user(&new_user, &mut db_cxn, &locked_user_data, &locked_user_data)
                    .await;
                assert_that!(create_result)
                    .is_err()
                    .matches(|err| matches!(err, CreateUserError::PortError(_)));
            }
        }
    }
}

#[cfg(test)]
pub mod test_util {
    use super::*;
    use crate::domain::test_util::{Connectivity, FakeImplementation};
    use crate::domain::user::driven_ports::{DetectUser, UserDescription, UserReader, UserWriter};
    use anyhow::Error;

    use crate::domain::user::driving_ports::UserPort;
    use std::sync::{Mutex, RwLock};

    /// A fake of driven ports for user data
    pub struct InMemoryUserPersistence {
        highest_user_id: i32,
        pub created_users: Vec<TodoUser>,
        pub connectivity: Connectivity,
    }

    impl InMemoryUserPersistence {
        /// Constructor for InMemoryUserPersistence
        pub fn new() -> InMemoryUserPersistence {
            InMemoryUserPersistence {
                highest_user_id: 0,
                created_users: Vec::new(),
                connectivity: Connectivity::Connected,
            }
        }

        /// Constructor for InMemoryUserPersistence that adds a set of already-existing users
        /// to the fake
        pub fn new_with_users(users: &[CreateUser]) -> InMemoryUserPersistence {
            InMemoryUserPersistence {
                highest_user_id: users.len() as i32,
                created_users: users
                    .iter()
                    .enumerate()
                    .map(|(index, user_info)| TodoUser {
                        id: (index + 1) as i32,
                        first_name: user_info.first_name.clone(),
                        last_name: user_info.last_name.clone(),
                    })
                    .collect(),
                connectivity: Connectivity::Connected,
            }
        }

        /// Constructor for InMemoryUserPersistence which wraps it in an RwLock so it
        /// can be immediately used as driven ports in domain logic tests
        pub fn new_locked() -> RwLock<InMemoryUserPersistence> {
            RwLock::new(InMemoryUserPersistence::new())
        }
    }

    impl driven_ports::UserWriter for RwLock<InMemoryUserPersistence> {
        async fn create_user(
            &self,
            user: &CreateUser,
            _: &mut impl ExternalConnectivity,
        ) -> Result<i32, anyhow::Error> {
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

    impl driven_ports::UserReader for RwLock<InMemoryUserPersistence> {
        async fn all(
            &self,
            _: &mut impl ExternalConnectivity,
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

        async fn by_id(
            &self,
            id: i32,
            _: &mut impl ExternalConnectivity,
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

    /// Creates a new CreateUser payload
    pub fn user_create_default() -> CreateUser {
        CreateUser {
            first_name: "First".into(),
            last_name: "Last".into(),
        }
    }

    impl DetectUser for RwLock<InMemoryUserPersistence> {
        async fn user_exists(
            &self,
            user_id: i32,
            _: &mut impl ExternalConnectivity,
        ) -> Result<bool, anyhow::Error> {
            let detector = self.read().expect("user detect rwlock poisoned");
            detector.connectivity.blow_up_if_disconnected()?;

            Ok(detector.created_users.iter().any(|user| user.id == user_id))
        }

        #[allow(clippy::needless_lifetimes)]
        async fn user_with_name_exists<'strings>(
            &self,
            description: UserDescription<'strings>,
            _: &mut impl ExternalConnectivity,
        ) -> Result<bool, Error> {
            let detector = self.read().expect("user detect rwlock poisoned");
            detector.connectivity.blow_up_if_disconnected()?;

            Ok(detector.created_users.iter().any(|user| {
                user.first_name == description.first_name && user.last_name == description.last_name
            }))
        }
    }

    /// A mock of UserService for use in API tests
    pub struct MockUserService {
        pub get_users_response: FakeImplementation<(), Result<Vec<TodoUser>, Error>>,
        pub create_user_response: FakeImplementation<CreateUser, Result<i32, CreateUserError>>,
    }

    impl MockUserService {
        /// Constructor for MockUserService
        pub fn new() -> MockUserService {
            MockUserService {
                get_users_response: FakeImplementation::new(),
                create_user_response: FakeImplementation::new(),
            }
        }

        /// Constructs a new MockUserService, allowing for configuration of mocks
        /// in the builder function before the mock is wrapped in a Mutex for use
        /// in API tests
        pub fn build_locked(builder: impl FnOnce(&mut Self)) -> Mutex<Self> {
            let mut new_svc = Self::new();
            builder(&mut new_svc);

            Mutex::new(new_svc)
        }
    }

    impl UserPort for Mutex<MockUserService> {
        async fn get_users(
            &self,
            _: &mut impl ExternalConnectivity,
            _: &impl UserReader,
        ) -> Result<Vec<TodoUser>, Error> {
            let locked_self = self.lock().expect("Lock is poisoned!");
            locked_self.get_users_response.return_value_anyhow()
        }

        async fn create_user(
            &self,
            new_user: &CreateUser,
            _: &mut impl ExternalConnectivity,
            _: &impl UserWriter,
            _: &impl DetectUser,
        ) -> Result<i32, CreateUserError> {
            let mut locked_self = self.lock().expect("Lock is poisoned!");
            locked_self
                .create_user_response
                .save_arguments(new_user.clone());
            locked_self.create_user_response.return_value_result()
        }
    }
}
