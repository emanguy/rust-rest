use anyhow::{anyhow, Context, Error};
use sqlx::{query, query_as};
use crate::domain;
use crate::domain::user::driven_ports::UserDescription;
use crate::external_connections::{ConnectionHandle, ExternalConnectivity};
use super::Count;

struct DbDetectUser{}

impl domain::user::driven_ports::DetectUser for DbDetectUser {
    async fn user_exists(&self, user_id: i32, ext_cxn: &mut impl ExternalConnectivity) -> Result<bool, anyhow::Error> {
        let mut connection = ext_cxn.database_cxn()
            .await
            .map_err(|err| anyhow!(format!("{}", err)))?;

        let user_with_id_count = query_as!(Count, "SELECT count(*) FROM todo_user tu WHERE tu.id = $1", user_id)
            .fetch_one(connection.borrow_connection())
            .await
            .context("Detecting user with ID")?;

        Ok(user_with_id_count.count() > 0)
    }

    async fn user_with_name_exists<'strings>(&self, description: UserDescription<'strings>, ext_cxn: &mut impl ExternalConnectivity) -> Result<bool, Error> {
        let mut connection = ext_cxn.database_cxn()
            .await
            .map_err(|err| anyhow!(format!("{}", err)))?;

        let user_with_name_count = query_as!(Count, "SELECT count(*) from todo_user tu WHERE tu.first_name = $1 AND tu.last_name = $2", description.first_name, description.last_name)
            .fetch_one(connection.borrow_connection())
            .await
            .context("Detecting user via name")?;

        Ok(user_with_name_count.count() > 0)
    }
}