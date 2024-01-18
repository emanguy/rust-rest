use anyhow::{Context, Error};
use sqlx::{query_as};
use crate::domain;
use crate::domain::user::driven_ports::UserDescription;
use crate::external_connections::{ConnectionHandle, ExternalConnectivity};
use super::Count;
use crate::domain::user::{CreateUser, TodoUser};

struct DbDetectUser{}

impl domain::user::driven_ports::DetectUser for DbDetectUser {
    async fn user_exists(&self, user_id: i32, ext_cxn: &mut impl ExternalConnectivity) -> Result<bool, anyhow::Error> {
        let mut connection = ext_cxn.database_cxn()
            .await
            .map_err(super::anyhowify)?;

        let user_with_id_count = query_as!(Count, "SELECT count(*) FROM todo_user tu WHERE tu.id = $1", user_id)
            .fetch_one(connection.borrow_connection())
            .await
            .context("Detecting user with ID")?;

        Ok(user_with_id_count.count() > 0)
    }

    async fn user_with_name_exists<'strings>(&self, description: UserDescription<'strings>, ext_cxn: &mut impl ExternalConnectivity) -> Result<bool, Error> {
        let mut connection = ext_cxn.database_cxn()
            .await
            .map_err(super::anyhowify)?;

        let user_with_name_count = query_as!(Count, "SELECT count(*) from todo_user tu WHERE tu.first_name = $1 AND tu.last_name = $2", description.first_name, description.last_name)
            .fetch_one(connection.borrow_connection())
            .await
            .context("Detecting user via name")?;

        Ok(user_with_name_count.count() > 0)
    }
}

struct DbReadUsers{}

struct TodoUserRow {
    id: i32,
    first_name: String,
    last_name: String,
}

impl From<TodoUserRow> for TodoUser {
    fn from(value: TodoUserRow) -> Self {
       TodoUser {
           id: value.id,
           first_name: value.first_name,
           last_name: value.last_name,
       }
    }
}

impl domain::user::driven_ports::UserReader for DbReadUsers {
    async fn get_all(&self, ext_cxn: &mut impl ExternalConnectivity) -> Result<Vec<TodoUser>, Error> {
        let mut connection = ext_cxn.database_cxn().await.map_err(super::anyhowify)?;

        let users: Vec<TodoUser> = query_as!(TodoUserRow, "SELECT * FROM todo_user")
            .fetch_all(connection.borrow_connection())
            .await
            .context("Fetching all users")?
            .into_iter()
            .map(domain::user::TodoUser::from).collect();

        Ok(users)
    }

    async fn get_by_id(&self, id: i32, ext_cxn: &mut impl ExternalConnectivity) -> Result<Option<TodoUser>, Error> {
        let mut cxn_handle = ext_cxn.database_cxn().await.map_err(super::anyhowify)?;

        let user = query_as!(TodoUserRow, "SELECT * FROM todo_user tu WHERE tu.id = $1", id)
            .fetch_optional(cxn_handle.borrow_connection())
            .await
            .context("Fetching a user by id")?;

        Ok(user.map(TodoUser::from))
    }
}

struct DbWriteUsers{}

impl domain::user::driven_ports::UserWriter for DbWriteUsers {
    async fn create_user(&self, user: &CreateUser, ext_cxn: &mut impl ExternalConnectivity) -> Result<i32, Error> {
        let mut cxn_handle = ext_cxn.database_cxn().await.map_err(super::anyhowify)?;

        let user = query_as!(super::NewId, "INSERT INTO todo_user(first_name, last_name) VALUES ($1, $2) RETURNING todo_user.id",
            user.first_name,
            user.last_name,
        ).fetch_one(cxn_handle.borrow_connection())
            .await
            .context("Inserting new user")?;

        Ok(user.id)
    }
}