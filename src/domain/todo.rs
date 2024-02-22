use crate::domain;
use crate::domain::todo::driven_ports::{TaskReader, TaskWriter};
use crate::domain::todo::driving_ports::TaskError;
use crate::external_connections::ExternalConnectivity;
use log::error;

#[derive(PartialEq, Eq, Debug)]
#[cfg_attr(test, derive(Clone))]
pub struct TodoTask {
    pub id: i32,
    pub owner_user_id: i32,
    pub item_desc: String,
}

pub struct NewTask {
    pub description: String,
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
        domain::user::verify_user_exists(user_id, &mut *ext_cxn, u_detect).await?;
        let created_task_id = task_write
            .create_task_for_user(user_id, task, &mut *ext_cxn)
            .await?;
        Ok(created_task_id)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::RwLock;
    use speculoos::prelude::*;
    use super::test_util::*;
    use super::*;
    use crate::domain::todo::driving_ports::TaskPort;
    use crate::domain::user::CreateUser;
    use crate::domain::user::test_util::InMemoryUserPersistence;
    use crate::external_connections;

    mod tasks_for_user {
        use super::*;
        
        #[tokio::test]
        async fn tasks_for_user_happy_path() {
            let user_persist = RwLock::new(InMemoryUserPersistence::new_with_users(&[
                domain::user::test_util::user_create_default(),
                domain::user::test_util::user_create_default(),
            ]));
            let task_persist = RwLock::new(InMemoryUserTaskPersistence::new_with_tasks(&[
                NewTaskWithOwner{
                    owner: 1,
                    task: NewTask {
                        description: "Something to do".to_owned(),
                    },
                },
                NewTaskWithOwner{
                    owner: 2,
                    task: NewTask {
                        description: "Another thing to do".to_owned(),
                    }
                }
            ]));
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();
            
            let fetched_tasks = TaskService{}.tasks_for_user(1, &mut ext_cxn, &user_persist, &task_persist).await;
            assert_that!(fetched_tasks).is_ok().matches(|tasks| {
                matches!(tasks.as_slice(), [
                    TodoTask {
                        id: 1,
                        owner_user_id: 1,
                        item_desc,
                    }
                ] if item_desc == "Something to do")
            });
        }
    }
    
    
    mod create_task_for_user {
        use super::*;

        #[tokio::test]
        async fn happy_path() {
            let task_persist = InMemoryUserTaskPersistence::new_locked();
            let user_persist = RwLock::new(InMemoryUserPersistence::new_with_users(&[
                CreateUser {
                    first_name: "John".to_owned(),
                    last_name: "Doe".to_owned(),
                }
            ]));
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();
            let task = NewTask {
                description: "Something to do".to_owned(),
            };
            let service = TaskService{};

            let create_result = service.create_task_for_user(1, &task, &mut ext_cxn, &user_persist, &task_persist).await;
            assert_that!(create_result).is_ok_containing(1);
        }

        #[tokio::test]
        async fn does_not_allow_tasks_for_nonexistent_user() {
            let writer = InMemoryUserTaskPersistence::new_locked();
            let user_detector = InMemoryUserPersistence::new_locked();
            let task = NewTask {
                description: String::new(),
            };
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();
            let service = TaskService {};

            let create_result = service
                .create_task_for_user(1, &task, &mut ext_cxn, &user_detector, &writer)
                .await;
            let Err(TaskError::UserDoesNotExist) = create_result else {
                panic!("Did not get expected error, instead got this: {create_result:#?}");
            };
        }
    }
}

#[cfg(test)]
pub(super) mod test_util {
    use super::*;
    use crate::domain::test_util::Connectivity;
    use anyhow::Error;
    use std::sync::RwLock;

    pub struct InMemoryUserTaskPersistence {
        pub tasks: Vec<TodoTask>,
        pub connected: Connectivity,
        highest_task_id: i32,
    }
    
    pub struct NewTaskWithOwner {
        pub owner: i32,
        pub task: NewTask,
    }

    impl InMemoryUserTaskPersistence {
        pub fn new() -> InMemoryUserTaskPersistence {
            InMemoryUserTaskPersistence {
                tasks: Vec::new(),
                connected: Connectivity::Connected,
                highest_task_id: 0,
            }
        }
        
        pub fn new_with_tasks(tasks: &[NewTaskWithOwner]) -> InMemoryUserTaskPersistence {
            InMemoryUserTaskPersistence {
                tasks: tasks.iter().enumerate().map(|(index, task_with_owner)| TodoTask {
                    id: index as i32 + 1,
                    owner_user_id: task_with_owner.owner,
                    item_desc: task_with_owner.task.description.clone(),
                }).collect(),
                connected: Connectivity::Connected,
                highest_task_id: tasks.len() as i32,
            }
        }

        pub fn new_locked() -> RwLock<InMemoryUserTaskPersistence> {
            RwLock::new(Self::new())
        }
    }

    impl driven_ports::TaskReader for RwLock<InMemoryUserTaskPersistence> {
        async fn tasks_for_user(
            &self,
            user_id: i32,
            _ext_cxn: &mut impl ExternalConnectivity,
        ) -> Result<Vec<TodoTask>, Error> {
            let persistence = self.read().expect("task persist rw lock poisoned");
            persistence.connected.blow_up_if_disconnected()?;

            let matching_tasks: Vec<TodoTask> = persistence
                .tasks
                .iter()
                .filter_map(|task| {
                    if task.owner_user_id == user_id {
                        Some(task.clone())
                    } else {
                        None
                    }
                })
                .collect();

            Ok(matching_tasks)
        }

        async fn user_task_by_id(
            &self,
            user_id: i32,
            task_id: i32,
            _ext_cxn: &mut impl ExternalConnectivity,
        ) -> Result<Option<TodoTask>, Error> {
            let persistence = self.read().expect("task persist rw lock poisoned");
            persistence.connected.blow_up_if_disconnected()?;

            let task = persistence
                .tasks
                .iter()
                .find(|task| task.owner_user_id == user_id && task.id == task_id)
                .map(Clone::clone);

            Ok(task)
        }
    }
    
    impl driven_ports::TaskWriter for RwLock<InMemoryUserTaskPersistence> {
        async fn create_task_for_user(
            &self,
            user_id: i32,
            task: &NewTask,
            _ext_cxn: &mut impl ExternalConnectivity,
        ) -> Result<i32, anyhow::Error> {
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
