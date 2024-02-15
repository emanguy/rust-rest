use crate::domain;
use crate::domain::todo::driven_ports::{TaskReader, TaskWriter};
use crate::domain::todo::driving_ports::TaskError;
use crate::external_connections::ExternalConnectivity;
use log::error;

#[derive(PartialEq, Eq, Debug)]
pub struct TodoTask {
    id: i32,
    owner_user_id: i32,
    item_desc: String,
}

pub struct NewTask {
    description: String,
}

pub mod driven_ports {
    use super::*;
    use crate::external_connections::ExternalConnectivity;

    pub trait TaskReader {
        async fn tasks_for_user(
            &self,
            user_id: i32,
            ext_cxn: &mut impl ExternalConnectivity,
        ) -> Result<Vec<TodoTask>, anyhow::Error>;
        async fn user_task_by_id(
            &self,
            user_id: i32,
            task_id: i32,
            ext_cxn: &mut impl ExternalConnectivity,
        ) -> Result<Option<TodoTask>, anyhow::Error>;
    }

    pub trait TaskWriter {
        async fn create_task_for_user(
            &self,
            user_id: i32,
            new_task: &NewTask,
            ext_cxn: &mut impl ExternalConnectivity,
        ) -> Result<i32, anyhow::Error>;
    }
}

pub mod driving_ports {
    use super::*;
    use crate::domain;
    use crate::external_connections::ExternalConnectivity;
    use thiserror::Error;

    #[derive(Debug, Error)]
    pub enum TaskError {
        #[error("The specified user did not exist.")]
        UserDoesNotExist,
        #[error(transparent)]
        PortError(#[from] anyhow::Error),
    }

    impl From<domain::user::UserExistsErr> for TaskError {
        fn from(value: domain::user::UserExistsErr) -> Self {
            match value {
                domain::user::UserExistsErr::UserDoesNotExist(user_id) => {
                    error!("User {} didn't exist when fetching tasks.", user_id);
                    return TaskError::UserDoesNotExist;
                }
                domain::user::UserExistsErr::PortError(err) => {
                    return TaskError::from(err.context("Fetching user tasks"))
                }
            }
        }
    }

    pub trait TaskPort {
        async fn tasks_for_user(
            &self,
            user_id: i32,
            ext_cxn: &mut impl ExternalConnectivity,
            u_detect: &impl domain::user::driven_ports::DetectUser,
            task_read: &impl driven_ports::TaskReader,
        ) -> Result<Vec<TodoTask>, TaskError>;
        async fn user_task_by_id(
            &self,
            user_id: i32,
            task_id: i32,
            ext_cxn: &mut impl ExternalConnectivity,
            u_detect: &impl domain::user::driven_ports::DetectUser,
            task_read: &impl driven_ports::TaskReader,
        ) -> Result<Option<TodoTask>, TaskError>;
        async fn create_task_for_user(
            &self,
            user_id: i32,
            task: &NewTask,
            ext_cxn: &mut impl ExternalConnectivity,
            u_detect: &impl domain::user::driven_ports::DetectUser,
            task_read: &impl driven_ports::TaskWriter,
        ) -> Result<i32, TaskError>;
    }
}

pub struct TaskService {}

impl driving_ports::TaskPort for TaskService {
    async fn tasks_for_user(
        &self,
        user_id: i32,
        ext_cxn: &mut impl ExternalConnectivity,
        u_detect: &impl domain::user::driven_ports::DetectUser,
        task_read: &impl TaskReader,
    ) -> Result<Vec<TodoTask>, TaskError> {
        domain::user::verify_user_exists(user_id, &mut *ext_cxn, u_detect).await?;
        let tasks_result = task_read.tasks_for_user(user_id, &mut *ext_cxn).await?;

        Ok(tasks_result)
    }

    async fn user_task_by_id(
        &self,
        user_id: i32,
        task_id: i32,
        ext_cxn: &mut impl ExternalConnectivity,
        u_detect: &impl domain::user::driven_ports::DetectUser,
        task_read: &impl TaskReader,
    ) -> Result<Option<TodoTask>, TaskError> {
        domain::user::verify_user_exists(user_id, &mut *ext_cxn, u_detect).await?;
        let tasks_result = task_read
            .user_task_by_id(user_id, task_id, &mut *ext_cxn)
            .await?;

        Ok(tasks_result)
    }

    async fn create_task_for_user(
        &self,
        user_id: i32,
        task: &NewTask,
        ext_cxn: &mut impl ExternalConnectivity,
        u_detect: &impl domain::user::driven_ports::DetectUser,
        task_write: &impl TaskWriter,
    ) -> Result<i32, TaskError> {
        domain::user::verify_user_exists(user_id, &mut *ext_cxn, u_detect)?;
        let created_task_id = task_write.create_task_for_user(user_id, task, &mut *ext_cxn).await?;
        Ok(created_task_id)
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::*;
    use super::*;
    use crate::domain::user::test_util::InMemoryUserPersistence;
    use crate::domain::Error;
    use std::sync::RwLock;

    #[tokio::test]
    async fn create_does_not_accept_invalid_tasks() {
        let writer = RwLock::new(InMemoryUserTaskWriter::new());
        let user_detector = RwLock::new(InMemoryUserPersistence::with_users([1]));
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
