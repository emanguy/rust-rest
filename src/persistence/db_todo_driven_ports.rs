use crate::domain;
use crate::domain::todo::{NewTask, TodoTask, UpdateTask};
use crate::external_connections::{ConnectionHandle, ExternalConnectivity};
use anyhow::{Context, Error};
use sqlx::{query, query_as};

pub struct DbTaskReader;

struct TodoItemRow {
    id: i32,
    user_id: i32,
    item_desc: String,
}

impl From<TodoItemRow> for domain::todo::TodoTask {
    fn from(value: TodoItemRow) -> Self {
        TodoTask {
            id: value.id,
            owner_user_id: value.user_id,
            item_desc: value.item_desc,
        }
    }
}

impl domain::todo::driven_ports::TaskReader for DbTaskReader {
    async fn tasks_for_user(
        &self,
        user_id: i32,
        ext_cxn: &mut impl ExternalConnectivity,
    ) -> Result<Vec<TodoTask>, Error> {
        let mut cxn = ext_cxn.database_cxn().await.map_err(super::anyhowify)?;

        let todo_items: Vec<TodoTask> = query_as!(
            TodoItemRow,
            "SELECT ti.* FROM todo_item ti WHERE ti.user_id = $1",
            user_id
        )
        .fetch_all(cxn.borrow_connection())
        .await
        .context("trying to fetch todo items for a user")?
        .into_iter()
        .map(domain::todo::TodoTask::from)
        .collect();

        Ok(todo_items)
    }

    async fn user_task_by_id(
        &self,
        user_id: i32,
        task_id: i32,
        ext_cxn: &mut impl ExternalConnectivity,
    ) -> Result<Option<TodoTask>, Error> {
        let mut cxn = ext_cxn.database_cxn().await.map_err(super::anyhowify)?;

        let todo_item: Option<TodoTask> = query_as!(
            TodoItemRow,
            "SELECT ti.* FROM todo_item ti WHERE ti.user_id = $1 AND ti.id = $2",
            user_id,
            task_id
        )
        .fetch_optional(cxn.borrow_connection())
        .await
        .context("trying to fetch a todo item by ID")?
        .map(domain::todo::TodoTask::from);

        Ok(todo_item)
    }
}

pub struct DbTaskWriter;

impl domain::todo::driven_ports::TaskWriter for DbTaskWriter {
    async fn create_task_for_user(
        &self,
        user_id: i32,
        new_task: &NewTask,
        ext_cxn: &mut impl ExternalConnectivity,
    ) -> Result<i32, Error> {
        let mut cxn = ext_cxn.database_cxn().await.map_err(super::anyhowify)?;

        let new_id = query_as!(
            super::NewId,
            "INSERT INTO todo_item(user_id, item_desc) VALUES ($1, $2) RETURNING todo_item.id",
            user_id,
            new_task.description
        )
        .fetch_one(cxn.borrow_connection())
        .await
        .context("trying to insert a new task into the database")?;

        Ok(new_id.id)
    }

    async fn delete_task(
        &self,
        task_id: i32,
        ext_cxn: &mut impl ExternalConnectivity,
    ) -> Result<(), Error> {
        let mut cxn = ext_cxn.database_cxn().await.map_err(super::anyhowify)?;

        query!("DELETE FROM todo_item WHERE id = $1", task_id)
            .execute(cxn.borrow_connection())
            .await
            .context("trying to remove a task from the database")?;

        Ok(())
    }

    async fn update_task(
        &self,
        task_id: i32,
        update: &UpdateTask,
        ext_cxn: &mut impl ExternalConnectivity,
    ) -> Result<(), Error> {
        let mut cxn = ext_cxn.database_cxn().await.map_err(super::anyhowify)?;

        query!(
            "UPDATE todo_item SET item_desc = $1 WHERE id = $2",
            update.description,
            task_id
        )
        .execute(cxn.borrow_connection())
        .await
        .context("trying to update a task in the database")?;

        Ok(())
    }
}
