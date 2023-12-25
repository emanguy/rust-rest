use crate::external_connections;
use crate::external_connections::ConnectionHandle;
use anyhow::Context;
use async_trait::async_trait;
use sqlx::pool::PoolConnection;
use sqlx::{Acquire, Database, PgConnection, PgPool, Postgres, Transaction};

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
impl <'tx> external_connections::ExternalConnectivity<'tx> for ExternalConnectivity {
    type Handle = PoolConnectionHandle;
    type Error = anyhow::Error;

    async fn database_cxn(&'tx mut self) -> Result<Self::Handle, Self::Error> {
        let handle = PoolConnectionHandle {
            active_connection: self.db.acquire().await?,
        };

        Ok(handle)
    }
}

struct ExternalConnectionsInTransaction<'tx> {
    txn: Transaction<'tx, Postgres>,
}

struct TransactionHandle<'tx> {
    active_transaction: &'tx mut PgConnection,
}

#[async_trait]
impl<'tx> external_connections::ExternalConnectivity<'tx> for ExternalConnectionsInTransaction<'tx> {
    type Handle = TransactionHandle<'tx>;
    type Error = anyhow::Error;

    async fn database_cxn(&'tx mut self) -> Result<TransactionHandle<'tx>, Self::Error> {
        let handle = self
            .txn
            .acquire()
            .await
            .context("acquiring connection from database transaction")?;

        return Ok(TransactionHandle {
            active_transaction: handle,
        })
    }
}

impl <'tx> ConnectionHandle for TransactionHandle<'tx> {
    fn borrow_connection(&mut self) -> &mut PgConnection {
        &mut *self.active_transaction
    }
}
