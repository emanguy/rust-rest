use async_trait::async_trait;
use sqlx::{PgConnection, PgPool, Postgres};
use sqlx::pool::PoolConnection;
use crate::external_connections;
use crate::external_connections::ConnectionHandle;

pub struct ExternalConnectivity {
    db: PgPool,
}

pub struct PoolConnectionHandle {
    active_connection: PoolConnection<Postgres>,
}

impl ConnectionHandle for PoolConnectionHandle {
    fn borrow_connection(&mut self) -> &mut PgConnection {
        &mut self.active_connection
    }
}

#[async_trait]
impl external_connections::ExternalConnectivity for ExternalConnectivity {
    type Handle = PoolConnectionHandle;
    type Error = anyhow::Error;

    async fn database_cxn(&self) -> Result<Self::Handle, Self::Error> {
        let handle = PoolConnectionHandle {
            active_connection: self.db.acquire().await?
        };

        Ok(handle)
    }
}