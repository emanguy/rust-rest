use crate::domain;
use crate::domain::todo::driven_ports::{TaskReader, TaskWriter};
use crate::domain::todo::driving_ports::TaskError;
use crate::external_connections::ExternalConnectivity;
use anyhow::{Context, Error};
use log::error;

#[derive(PartialEq, Eq, Debug)]
#[cfg_attr(test, derive(Clone))]
/// A task available for a user
pub struct TodoTask {
    pub id: i32,
    pub owner_user_id: i32,
    pub item_desc: String,
}

#[cfg_attr(test, derive(Clone))]
/// Contains information necessary to create a new task
pub struct NewTask {
    pub description: String,
}

#[cfg_attr(test, derive(Clone))]
/// Contains information which is allowed to be updated on a task
pub struct UpdateTask {
    pub description: String,
}

/// Contains the set of driven ports invoked by the business logic
pub mod driven_ports {
    use super::*;
    use crate::external_connections::ExternalConnectivity;

    /// An external system that can read a user's tasks
    pub trait TaskReader {
        /// Retrieve the set of tasks for a user
        async fn tasks_for_user(
            &self,
            user_id: i32,
            ext_cxn: &mut impl ExternalConnectivity,
        ) -> Result<Vec<TodoTask>, anyhow::Error>;

        /// Retrieve a single task belonging to a user
        async fn user_task_by_id(
            &self,
            user_id: i32,
            task_id: i32,
            ext_cxn: &mut impl ExternalConnectivity,
        ) -> Result<Option<TodoTask>, anyhow::Error>;
    }

    /// An external system that can edit the set of tasks for a user
    pub trait TaskWriter {
        /// Create a new task for a user
        async fn create_task_for_user(
            &self,
            user_id: i32,
            new_task: &NewTask,
            ext_cxn: &mut impl ExternalConnectivity,
        ) -> Result<i32, anyhow::Error>;

        /// Delete a task by its ID
        async fn delete_task(
            &self,
            task_id: i32,
            ext_cxn: &mut impl ExternalConnectivity,
        ) -> Result<(), anyhow::Error>;

        /// Update the content of an existing task
        async fn update_task(
            &self,
            task_id: i32,
            update: &UpdateTask,
            ext_cxn: &mut impl ExternalConnectivity,
        ) -> Result<(), anyhow::Error>;
    }
}

/// Contains the driving port interface that exposes business logic entrypoints to driving adapters
/// such as HTTP routers
pub mod driving_ports {
    use super::*;
    use crate::domain;
    use crate::external_connections::ExternalConnectivity;
    use thiserror::Error;

    #[derive(Debug, Error)]
    /// A set of things that can go wrong while dealing with tasks
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
                    TaskError::UserDoesNotExist
                }
                domain::user::UserExistsErr::PortError(err) => {
                    TaskError::from(err.context("Fetching user tasks"))
                }
            }
        }
    }

    #[cfg(test)]
    #[allow(clippy::items_after_test_module)]
    mod task_error_clone {
        use crate::domain::todo::driving_ports::TaskError;
        use anyhow::anyhow;

        // Implements clone for TaskError so it can be used in mocks during API tests
        impl Clone for TaskError {
            fn clone(&self) -> Self {
                match self {
                    Self::UserDoesNotExist => Self::UserDoesNotExist,
                    Self::PortError(err) => Self::PortError(anyhow!(format!("{}", err))),
                }
            }
        }
    }

    /// The driving port, or the set of business logic functions exposed to driving adapters
    pub trait TaskPort {
        /// Retrieve the set of tasks belonging to a user
        async fn tasks_for_user(
            &self,
            user_id: i32,
            ext_cxn: &mut impl ExternalConnectivity,
            u_detect: &impl domain::user::driven_ports::DetectUser,
            task_read: &impl driven_ports::TaskReader,
        ) -> Result<Vec<TodoTask>, TaskError>;

        /// Retrieve a single task belonging to a user
        async fn user_task_by_id(
            &self,
            user_id: i32,
            task_id: i32,
            ext_cxn: &mut impl ExternalConnectivity,
            u_detect: &impl domain::user::driven_ports::DetectUser,
            task_read: &impl driven_ports::TaskReader,
        ) -> Result<Option<TodoTask>, TaskError>;

        /// Create a new task for a user
        async fn create_task_for_user(
            &self,
            user_id: i32,
            task: &NewTask,
            ext_cxn: &mut impl ExternalConnectivity,
            u_detect: &impl domain::user::driven_ports::DetectUser,
            task_write: &impl driven_ports::TaskWriter,
        ) -> Result<i32, TaskError>;

        /// Delete a task by its ID
        async fn delete_task(
            &self,
            task_id: i32,
            ext_cxn: &mut impl ExternalConnectivity,
            task_write: &impl driven_ports::TaskWriter,
        ) -> Result<(), anyhow::Error>;

        /// Update the content of an existing task
        async fn update_task(
            &self,
            task_id: i32,
            update: &UpdateTask,
            ext_cxn: &mut impl ExternalConnectivity,
            task_write: &impl driven_ports::TaskWriter,
        ) -> Result<(), anyhow::Error>;
    }
}

/// TaskService implements the driving port for tasks so driving adapters can access task business
/// logic
pub struct TaskService;

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

    async fn delete_task(
        &self,
        task_id: i32,
        ext_cxn: &mut impl ExternalConnectivity,
        task_write: &impl TaskWriter,
    ) -> Result<(), Error> {
        task_write
            .delete_task(task_id, &mut *ext_cxn)
            .await
            .context("deleting a task")?;
        Ok(())
    }

    async fn update_task(
        &self,
        task_id: i32,
        update: &UpdateTask,
        ext_cxn: &mut impl ExternalConnectivity,
        task_write: &impl TaskWriter,
    ) -> Result<(), Error> {
        task_write
            .update_task(task_id, update, &mut *ext_cxn)
            .await
            .context("updating a task")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::*;
    use super::*;
    use crate::domain::todo::driving_ports::TaskPort;
    use crate::domain::user::test_util::InMemoryUserPersistence;
    use crate::domain::user::CreateUser;
    use crate::external_connections;
    use speculoos::prelude::*;
    use std::sync::RwLock;

    mod tasks_for_user {
        use super::*;

        #[tokio::test]
        async fn happy_path() {
            let user_persist = RwLock::new(InMemoryUserPersistence::new_with_users(&[
                domain::user::test_util::user_create_default(),
                domain::user::test_util::user_create_default(),
            ]));
            let task_persist = RwLock::new(InMemoryUserTaskPersistence::new_with_tasks(&[
                NewTaskWithOwner {
                    owner: 1,
                    task: NewTask {
                        description: "Something to do".to_owned(),
                    },
                },
                NewTaskWithOwner {
                    owner: 2,
                    task: NewTask {
                        description: "Another thing to do".to_owned(),
                    },
                },
            ]));
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();

            let fetched_tasks = TaskService {}
                .tasks_for_user(1, &mut ext_cxn, &user_persist, &task_persist)
                .await;
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

        #[tokio::test]
        async fn returns_error_on_nonexistent_user() {
            let user_persist = InMemoryUserPersistence::new_locked();
            let task_persist = InMemoryUserTaskPersistence::new_locked();
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();

            let fetched_task_result = TaskService {}
                .tasks_for_user(1, &mut ext_cxn, &user_persist, &task_persist)
                .await;
            let Err(TaskError::UserDoesNotExist) = fetched_task_result else {
                panic!(
                    "Got an unexpected result from task lookup: {:#?}",
                    fetched_task_result
                );
            };
        }
    }

    mod user_task_by_id {
        use super::*;

        #[tokio::test]
        async fn happy_path() {
            let user_persist = RwLock::new(InMemoryUserPersistence::new_with_users(&[
                domain::user::test_util::user_create_default(),
                domain::user::test_util::user_create_default(),
            ]));
            let task_persist = RwLock::new(InMemoryUserTaskPersistence::new_with_tasks(&[
                NewTaskWithOwner {
                    owner: 1,
                    task: NewTask {
                        description: "abcde".to_owned(),
                    },
                },
                NewTaskWithOwner {
                    owner: 1,
                    task: NewTask {
                        description: "fghijk".to_owned(),
                    },
                },
                NewTaskWithOwner {
                    owner: 2,
                    task: NewTask {
                        description: "lmnop".to_owned(),
                    },
                },
            ]));
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();

            let task_fetch_result = TaskService {}
                .user_task_by_id(1, 2, &mut ext_cxn, &user_persist, &task_persist)
                .await;
            assert_that!(task_fetch_result)
                .is_ok()
                .is_some()
                .matches(|task| {
                    matches!(task, TodoTask {
                       id: 2,
                       owner_user_id: 1,
                       item_desc
                    } if item_desc == "fghijk")
                });
        }

        #[tokio::test]
        async fn happy_path_not_found() {
            let user_persist = RwLock::new(InMemoryUserPersistence::new_with_users(&[
                domain::user::test_util::user_create_default(),
                domain::user::test_util::user_create_default(),
            ]));
            let task_persist = RwLock::new(InMemoryUserTaskPersistence::new_with_tasks(&[
                NewTaskWithOwner {
                    owner: 1,
                    task: NewTask {
                        description: "abcde".to_owned(),
                    },
                },
                NewTaskWithOwner {
                    owner: 1,
                    task: NewTask {
                        description: "fghijk".to_owned(),
                    },
                },
                NewTaskWithOwner {
                    owner: 2,
                    task: NewTask {
                        description: "lmnop".to_owned(),
                    },
                },
            ]));
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();

            let task_fetch_result = TaskService {}
                .user_task_by_id(1, 3, &mut ext_cxn, &user_persist, &task_persist)
                .await;
            assert_that!(task_fetch_result).is_ok().is_none();
        }

        #[tokio::test]
        async fn fails_if_user_doesnt_exist() {
            let user_persist = InMemoryUserPersistence::new_locked();
            let task_persist = InMemoryUserTaskPersistence::new_locked();
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();

            let task_fetch_result = TaskService {}
                .user_task_by_id(1, 5, &mut ext_cxn, &user_persist, &task_persist)
                .await;
            let Err(TaskError::UserDoesNotExist) = task_fetch_result else {
                panic!(
                    "Didn't get expected error for user not existing: {:#?}",
                    task_fetch_result
                );
            };
        }
    }

    mod create_task_for_user {
        use super::*;

        #[tokio::test]
        async fn happy_path() {
            let task_persist = InMemoryUserTaskPersistence::new_locked();
            let user_persist =
                RwLock::new(InMemoryUserPersistence::new_with_users(&[CreateUser {
                    first_name: "John".to_owned(),
                    last_name: "Doe".to_owned(),
                }]));
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();
            let task = NewTask {
                description: "Something to do".to_owned(),
            };
            let service = TaskService {};

            let create_result = service
                .create_task_for_user(1, &task, &mut ext_cxn, &user_persist, &task_persist)
                .await;
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

    mod delete_task {
        use super::*;
        use crate::domain::test_util::Connectivity;

        #[tokio::test]
        async fn happy_path() {
            let writer = RwLock::new(InMemoryUserTaskPersistence::new_with_tasks(&[
                NewTaskWithOwner {
                    owner: 1,
                    task: NewTask {
                        description: "abcde".to_owned(),
                    },
                },
                NewTaskWithOwner {
                    owner: 1,
                    task: NewTask {
                        description: "fghij".to_owned(),
                    },
                },
            ]));
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();

            let delete_result = TaskService {}.delete_task(2, &mut ext_cxn, &writer).await;
            assert_that!(delete_result).is_ok();

            let locked_writer = writer.read().expect("task writer rw lock poisoned");
            assert!(matches!(locked_writer.tasks.as_slice(), [
                    TodoTask {
                        id: 1,
                        owner_user_id: 1,
                        item_desc,
                    }
                ] if item_desc == "abcde"));
        }

        #[tokio::test]
        async fn happy_path_task_doesnt_exist() {
            let writer = InMemoryUserTaskPersistence::new_locked();
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();

            let delete_result = TaskService {}.delete_task(5, &mut ext_cxn, &writer).await;
            assert_that!(delete_result).is_ok();
        }

        #[tokio::test]
        async fn returns_port_err() {
            let writer = InMemoryUserTaskPersistence::new_locked();
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();
            {
                let mut locked_writer = writer.write().expect("writer rw lock poisoned");
                locked_writer.connected = Connectivity::Disconnected;
            }

            let delete_result = TaskService {}.delete_task(1, &mut ext_cxn, &writer).await;
            assert_that!(delete_result).is_err();
        }
    }

    mod update_task {
        use super::*;
        use crate::domain::test_util::Connectivity;

        #[tokio::test]
        async fn happy_path() {
            let writer = RwLock::new(InMemoryUserTaskPersistence::new_with_tasks(&[
                NewTaskWithOwner {
                    owner: 1,
                    task: NewTask {
                        description: "abcde".to_owned(),
                    },
                },
                NewTaskWithOwner {
                    owner: 1,
                    task: NewTask {
                        description: "fghij".to_owned(),
                    },
                },
            ]));
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();

            let update_result = TaskService {}
                .update_task(
                    2,
                    &UpdateTask {
                        description: "Something to do".to_owned(),
                    },
                    &mut ext_cxn,
                    &writer,
                )
                .await;

            assert_that!(update_result).is_ok();

            let locked_writer = writer.read().expect("rw lock poisoned");
            assert_eq!("Something to do", locked_writer.tasks[1].item_desc);
        }

        #[tokio::test]
        async fn happy_path_task_doesnt_exist() {
            let writer = InMemoryUserTaskPersistence::new_locked();
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();

            let update_result = TaskService {}
                .update_task(
                    5,
                    &UpdateTask {
                        description: "Something to do".to_owned(),
                    },
                    &mut ext_cxn,
                    &writer,
                )
                .await;
            assert_that!(update_result).is_ok();
        }

        #[tokio::test]
        async fn returns_port_err() {
            let mut raw_writer = InMemoryUserTaskPersistence::new();
            raw_writer.connected = Connectivity::Disconnected;
            let writer = RwLock::new(raw_writer);
            let mut ext_cxn = external_connections::test_util::FakeExternalConnectivity::new();

            let update_result = TaskService {}
                .update_task(
                    1,
                    &UpdateTask {
                        description: "Something to do".to_owned(),
                    },
                    &mut ext_cxn,
                    &writer,
                )
                .await;
            assert_that!(update_result).is_err();
        }
    }
}

#[cfg(test)]
pub mod test_util {
    use super::*;
    use crate::domain::test_util::{Connectivity, FakeImplementation};
    use crate::domain::user::driven_ports::DetectUser;
    use std::sync::{Mutex, RwLock};

    /// A fake providing task functionality for domain logic tests, as it implements
    /// the traits for all task driven ports
    pub struct InMemoryUserTaskPersistence {
        pub tasks: Vec<TodoTask>,
        pub connected: Connectivity,
        highest_task_id: i32,
    }

    /// Represents a task with a specific owner
    pub struct NewTaskWithOwner {
        pub owner: i32,
        pub task: NewTask,
    }

    impl InMemoryUserTaskPersistence {
        /// Constructor for InMemoryUserTaskPersistence
        pub fn new() -> InMemoryUserTaskPersistence {
            InMemoryUserTaskPersistence {
                tasks: Vec::new(),
                connected: Connectivity::Connected,
                highest_task_id: 0,
            }
        }

        /// Constructor for InMemoryUserTaskPersistence which adds a set of already-existing tasks
        pub fn new_with_tasks(tasks: &[NewTaskWithOwner]) -> InMemoryUserTaskPersistence {
            InMemoryUserTaskPersistence {
                tasks: tasks
                    .iter()
                    .enumerate()
                    .map(|(index, task_with_owner)| TodoTask {
                        id: index as i32 + 1,
                        owner_user_id: task_with_owner.owner,
                        item_desc: task_with_owner.task.description.clone(),
                    })
                    .collect(),
                connected: Connectivity::Connected,
                highest_task_id: tasks.len() as i32,
            }
        }

        /// Constructor for InMemoryUserTaskPersistence which wraps it in an RwLock right away
        /// for use as the set of task driven ports
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

        async fn delete_task(
            &self,
            task_id: i32,
            _ext_cxn: &mut impl ExternalConnectivity,
        ) -> Result<(), Error> {
            let mut persistence = self.write().expect("task persist rw lock poisoned");
            persistence.connected.blow_up_if_disconnected()?;

            let item_index = persistence
                .tasks
                .iter()
                .enumerate()
                .find(|(_, task)| task.id == task_id)
                .map(|(idx, _)| idx);
            if let Some(idx) = item_index {
                persistence.tasks.remove(idx);
            }

            Ok(())
        }

        async fn update_task(
            &self,
            task_id: i32,
            update: &UpdateTask,
            _ext_cxn: &mut impl ExternalConnectivity,
        ) -> Result<(), Error> {
            let mut persistence = self.write().expect("task persist rw lock poisoned");
            persistence.connected.blow_up_if_disconnected()?;

            let item_index = persistence
                .tasks
                .iter()
                .enumerate()
                .find(|(_, task)| task.id == task_id)
                .map(|(idx, _)| idx);
            if let Some(idx) = item_index {
                persistence.tasks[idx].item_desc = update.description.clone();
            }

            Ok(())
        }
    }

    /// Creates a new [TodoTask] from a create payload plus some supplemental information
    pub fn task_from_create(user_id: i32, task_id: i32, new_task: &NewTask) -> TodoTask {
        TodoTask {
            id: task_id,
            owner_user_id: user_id,
            item_desc: new_task.description.clone(),
        }
    }

    /// A mock of TaskService for use in API tests
    pub struct MockTaskService {
        pub tasks_for_user_result: FakeImplementation<i32, Result<Vec<TodoTask>, TaskError>>,
        pub user_task_by_id_result:
            FakeImplementation<(i32, i32), Result<Option<TodoTask>, TaskError>>,
        pub create_task_for_user_result: FakeImplementation<(i32, NewTask), Result<i32, TaskError>>,
        pub delete_task_result: FakeImplementation<i32, Result<(), anyhow::Error>>,
        pub update_task_result: FakeImplementation<(i32, UpdateTask), Result<(), anyhow::Error>>,
    }

    impl MockTaskService {
        /// Constructor for MockTaskService
        pub fn new() -> MockTaskService {
            MockTaskService {
                tasks_for_user_result: FakeImplementation::new(),
                user_task_by_id_result: FakeImplementation::new(),
                create_task_for_user_result: FakeImplementation::new(),
                delete_task_result: FakeImplementation::new(),
                update_task_result: FakeImplementation::new(),
            }
        }

        /// Constructor for MockTaskService which accepts a builder function to configure
        /// mock responses, wrapping the resulting mock in a mutex so it is ready for use
        /// in API tests
        pub fn build_locked(builder: impl FnOnce(&mut Self)) -> Mutex<MockTaskService> {
            let mut new_svc = Self::new();
            builder(&mut new_svc);

            Mutex::new(new_svc)
        }

        pub fn new_locked() -> Mutex<MockTaskService> {
            Mutex::new(Self::new())
        }
    }

    impl driving_ports::TaskPort for Mutex<MockTaskService> {
        async fn tasks_for_user(
            &self,
            user_id: i32,
            _ext_cxn: &mut impl ExternalConnectivity,
            _u_detect: &impl DetectUser,
            _task_read: &impl TaskReader,
        ) -> Result<Vec<TodoTask>, TaskError> {
            let mut locked_self = self.lock().expect("mock task service mutex poisoned");
            locked_self.tasks_for_user_result.save_arguments(user_id);

            locked_self.tasks_for_user_result.return_value_result()
        }

        async fn user_task_by_id(
            &self,
            user_id: i32,
            task_id: i32,
            _ext_cxn: &mut impl ExternalConnectivity,
            _u_detect: &impl DetectUser,
            _task_read: &impl TaskReader,
        ) -> Result<Option<TodoTask>, TaskError> {
            let mut locked_self = self.lock().expect("mock task service mutex poisoned");
            locked_self
                .user_task_by_id_result
                .save_arguments((user_id, task_id));

            locked_self.user_task_by_id_result.return_value_result()
        }

        async fn create_task_for_user(
            &self,
            user_id: i32,
            task: &NewTask,
            _ext_cxn: &mut impl ExternalConnectivity,
            _u_detect: &impl DetectUser,
            _task_write: &impl TaskWriter,
        ) -> Result<i32, TaskError> {
            let mut locked_self = self.lock().expect("mock task service mutex poisoned");
            locked_self
                .create_task_for_user_result
                .save_arguments((user_id, task.clone()));

            locked_self
                .create_task_for_user_result
                .return_value_result()
        }

        async fn delete_task(
            &self,
            task_id: i32,
            _ext_cxn: &mut impl ExternalConnectivity,
            _task_write: &impl TaskWriter,
        ) -> Result<(), anyhow::Error> {
            let mut locked_self = self.lock().expect("mock task service mutex poisoned");
            locked_self.delete_task_result.save_arguments(task_id);

            locked_self.delete_task_result.return_value_anyhow()
        }

        async fn update_task(
            &self,
            task_id: i32,
            update: &UpdateTask,
            _ext_cxn: &mut impl ExternalConnectivity,
            _task_write: &impl TaskWriter,
        ) -> Result<(), anyhow::Error> {
            let mut locked_self = self.lock().expect("mock task service mutex poisoned");
            locked_self
                .update_task_result
                .save_arguments((task_id, update.clone()));

            locked_self.update_task_result.return_value_anyhow()
        }
    }
}
