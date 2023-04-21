use crate::domain::{DrivenPortError, Error};
use async_trait::async_trait;
use validator::Validate;

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
}

#[async_trait]
pub trait UserWriter {
    async fn create_user(&self, user: &CreateUser) -> Result<u32, DrivenPortError>;
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
    use super::*;
    use crate::domain::test_util::Connectivity;
    use async_trait::async_trait;
    use std::sync::{Mutex, RwLock};

    struct InMemoryUserPersistence {
        highest_user_id: u32,
        created_users: Vec<TodoUser>,
        connected: Connectivity,
    }

    impl InMemoryUserPersistence {
        fn new() -> InMemoryUserPersistence {
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
    }

    #[tokio::test]
    async fn bad_user_data_gets_rejected() {
        let persister = RwLock::new(InMemoryUserPersistence::new());
        let bad_user = CreateUser {
            first_name: (0..35).map(|_| "A").collect(),
            last_name: (0..55).map(|_| "B").collect(),
        };

        let user_create_result = create_user(&persister, &bad_user).await;
        let Err(Error::Invalid(validation_errors)) = user_create_result else {
            panic!("did not get invalid response, got {user_create_result:?} instead");
        };

        let field_errors = validation_errors.field_errors();
        assert!(field_errors.contains_key("first_name"));
        assert!(field_errors.contains_key("last_name"));
    }
}
