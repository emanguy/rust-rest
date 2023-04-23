use crate::domain::{DrivenPortError, Error, Resource};
use async_trait::async_trait;
use validator::Validate;

#[derive(PartialEq, Eq, Debug)]
pub struct TodoUser {
    pub id: u32,
    pub first_name: String,
    pub last_name: String,
}

#[derive(Validate)]
pub struct CreateUser {
    #[validate(length(max = 30))]
    pub first_name: String,
    #[validate(length(max = 50))]
    pub last_name: String,
}

#[async_trait]
pub trait UserReader {
    async fn get_all(&self) -> Result<Vec<TodoUser>, DrivenPortError>;
    async fn get_by_id(&self, id: u32) -> Result<Option<TodoUser>, DrivenPortError>;
}

#[async_trait]
pub trait UserWriter {
    async fn create_user(&self, user: &CreateUser) -> Result<u32, DrivenPortError>;
}

#[async_trait]
pub trait DetectUser {
    async fn user_exists(&self, user_id: u32) -> Result<bool, DrivenPortError>;
}

pub(super) async fn verify_user_exists<Det: DetectUser>(
    user_detect: &Det,
    id: u32,
    action: &str,
) -> Result<(), Error> {
    let does_user_exist = user_detect
        .user_exists(id)
        .await
        .map_err(|err| err.into_error_trying_to(action))?;

    if does_user_exist {
        Ok(())
    } else {
        Err(Error::DependencyMissing {
            what: Resource::User { id },
            action: action.into(),
        })
    }
}

pub async fn get_users<Reader: UserReader>(user_reader: &Reader) -> Result<Vec<TodoUser>, Error> {
    let all_users_result = user_reader.get_all().await;
    if let Err(ref port_err) = all_users_result {
        log::error!("User fetch failure: {port_err}");
    }

    all_users_result.map_err(|err| err.into_error_trying_to("look up all users"))
}

pub async fn create_user<Writer: UserWriter>(
    user_writer: &Writer,
    new_user: &CreateUser,
) -> Result<u32, Error> {
    new_user.validate()?;
    user_writer
        .create_user(new_user)
        .await
        .map_err(|err| err.into_error_trying_to("create a new user"))
}

#[cfg(test)]
mod tests {
    use super::{test_util::*, *};
    use crate::domain::test_util::Connectivity;
    use std::collections::HashSet;
    use std::sync::RwLock;

    #[tokio::test]
    async fn bad_user_data_gets_rejected() {
        let persister = RwLock::new(InMemoryUserPersistence::new());
        let bad_user = CreateUser {
            first_name: (0..35).map(|_| "A").collect(),
            last_name: (0..55).map(|_| "B").collect(),
        };

        let user_create_result = create_user(&persister, &bad_user).await;
        let Err(Error::Invalid(validation_errors)) = user_create_result else {
            panic!("did not get invalid response, got {user_create_result:#?} instead");
        };

        let field_errors = validation_errors.field_errors();
        assert!(field_errors.contains_key("first_name"));
        assert!(field_errors.contains_key("last_name"));
    }

    #[tokio::test]
    async fn correct_error_returned_on_connection_failure() {
        let persister_lock = RwLock::new(InMemoryUserPersistence::new());
        {
            let mut persister = persister_lock.write().expect("rwlock poisoned");
            persister.connected = Connectivity::Disconnected;
        }

        let new_user = user_create_default();
        let user_create_result = create_user(&persister_lock, &new_user).await;
        assert!(
            matches!(user_create_result, Err(Error::RetrieveFailure { .. })),
            "Got bad create result: {user_create_result:#?}"
        );
    }

    #[tokio::test]
    async fn create_user_happy_path() {
        let persister_lock = RwLock::new(InMemoryUserPersistence::new());
        let new_user = user_create_default();

        let user_create_result = create_user(&persister_lock, &new_user).await;
        let Ok(new_id) = user_create_result else {
            panic!("Did not get the expected response from our method: {user_create_result:#?}");
        };

        let expected_user = user_from_create(&new_user, new_id);
        let persister = persister_lock.read().expect("rwlock poisoned");
        assert_eq!(1, persister.created_users.len());
        assert_eq!(expected_user, persister.created_users[0]);
    }

    #[tokio::test]
    async fn user_detection_produces_correct_error() {
        let detector_lock = RwLock::new(InMemoryUserDetector::new());

        let result = verify_user_exists(&detector_lock, 10, "test").await;
        assert!(
            matches!(
                result,
                Err(Error::DependencyMissing {
                    what: Resource::User { id: 10 },
                    ..
                })
            ),
            "did not get correct error, got this instead: {result:#?}"
        );
    }
}

#[cfg(test)]
pub(super) mod test_util {
    use super::*;
    use crate::domain::test_util::Connectivity;
    use std::collections::HashSet;
    use std::sync::RwLock;

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
        async fn create_user(&self, user: &CreateUser) -> Result<u32, DrivenPortError> {
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
        async fn get_all(&self) -> Result<Vec<TodoUser>, DrivenPortError> {
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

        async fn get_by_id(&self, id: u32) -> Result<Option<TodoUser>, DrivenPortError> {
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
        async fn user_exists(&self, user_id: u32) -> Result<bool, DrivenPortError> {
            let detector = self.read().expect("user detect rwlock poisoned");
            detector.connectivity.blow_up_if_disconnected()?;

            Ok(detector.known_users.contains(&user_id))
        }
    }
}
