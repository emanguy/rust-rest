use crate::domain::user::{verify_user_exists, DetectUser};
use crate::domain::{DrivenPortError, Error};
use async_trait::async_trait;
use validator::Validate;

#[derive(PartialEq, Eq, Debug)]
pub struct TodoTask {
    id: i32,
    owner_user_id: i32,
    item_desc: String,
}

#[derive(Validate)]
pub struct NewTask {
    #[validate(length(min = 1))]
    description: String,
}


pub trait UserTaskReader {
    async fn tasks_for_user(&self, user_id: i32) -> Result<Vec<TodoTask>, DrivenPortError>;
    async fn user_task_by_id(
        &self,
        user_id: i32,
        task_id: i32,
    ) -> Result<Option<TodoTask>, DrivenPortError>;
}


pub trait UserTaskWriter {
    async fn create_task_for_user(
        &self,
        user_id: i32,
        task: &NewTask,
    ) -> Result<i32, DrivenPortError>;
}

pub async fn tasks_for_user<UDetect, TReader>(
    user_detect: &UDetect,
    task_reader: &TReader,
    user_id: i32,
) -> Result<Vec<TodoTask>, Error>
where
    UDetect: DetectUser,
    TReader: UserTaskReader,
{
    verify_user_exists(user_id, user_detect).await?;

    task_reader
        .tasks_for_user(user_id)
        .await
        .map_err(|err| err.into_error_trying_to("look up a user's tasks"))
}

pub async fn task_for_user<UDetect, TReader>(
    user_detect: &UDetect,
    task_reader: &TReader,
    user_id: i32,
    task_id: i32,
) -> Result<Option<TodoTask>, Error>
where
    UDetect: DetectUser,
    TReader: UserTaskReader,
{
    verify_user_exists(user_id, user_detect).await?;

    task_reader
        .user_task_by_id(user_id, task_id)
        .await
        .map_err(|err| err.into_error_trying_to("look up a user's task by id"))
}

pub async fn create_task<UDetect, UTWriter>(
    user_detect: &UDetect,
    task_writer: &UTWriter,
    user_id: i32,
    new_task: &NewTask,
) -> Result<i32, Error>
where
    UDetect: DetectUser,
    UTWriter: UserTaskWriter,
{
    new_task.validate().map_err(Error::Invalid)?;
    verify_user_exists(user_id, user_detect).await?;

    task_writer
        .create_task_for_user(user_id, new_task)
        .await
        .map_err(|err| err.into_error_trying_to("create a task for a user"))
}

#[cfg(test)]
mod tests {
    use super::test_util::*;
    use super::*;
    use crate::domain::user::test_util::InMemoryUserDetector;
    use crate::domain::Error;
    use std::sync::RwLock;

    #[tokio::test]
    async fn create_does_not_accept_invalid_tasks() {
        let writer = RwLock::new(InMemoryUserTaskWriter::new());
        let user_detector = RwLock::new(InMemoryUserDetector::with_users([1]));
        let bad_task = NewTask {
            description: String::new(),
        };

        let create_result = create_task(&user_detector, &writer, 1, &bad_task).await;
        let Err(Error::Invalid(validation_issues)) = create_result else {
            panic!("Did not get expected error, instead got this: {create_result:#?}");
        };

        let field_issues = validation_issues.field_errors();
        assert!(field_issues.contains_key("description"));
    }
}

#[cfg(test)]
pub(super) mod test_util {
    use super::*;
    use crate::domain::test_util::Connectivity;
    use std::sync::RwLock;

    pub struct InMemoryUserTaskWriter {
        pub tasks: Vec<TodoTask>,
        pub connected: Connectivity,
        highest_task_id: i32,
    }

    impl InMemoryUserTaskWriter {
        pub fn new() -> InMemoryUserTaskWriter {
            InMemoryUserTaskWriter {
                tasks: Vec::new(),
                connected: Connectivity::Connected,
                highest_task_id: 0,
            }
        }
    }


    impl UserTaskWriter for RwLock<InMemoryUserTaskWriter> {
        async fn create_task_for_user(
            &self,
            user_id: i32,
            task: &NewTask,
        ) -> Result<i32, DrivenPortError> {
            let mut persistence = self.write().expect("task persist rw lock poisoned");
            persistence.connected.blow_up_if_disconnected()?;

            persistence.highest_task_id += 1;
            let task_id = persistence.highest_task_id;
            persistence
                .tasks
                .push(task_from_create(user_id, task_id, task));
            Ok(task_id)
        }
    }

    pub fn task_from_create(user_id: i32, task_id: i32, new_task: &NewTask) -> TodoTask {
        TodoTask {
            id: task_id,
            owner_user_id: user_id,
            item_desc: new_task.description.clone(),
        }
    }
}
