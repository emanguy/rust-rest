mod db_user_driven_ports;

use std::fmt::{Debug, Display};
use crate::external_connections;
use crate::external_connections::ConnectionHandle;
use anyhow::{anyhow, Context};

use sqlx::pool::PoolConnection;
use sqlx::{Acquire, PgConnection, PgPool, Postgres, Transaction};

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


impl external_connections::ExternalConnectivity for ExternalConnectivity {
    type Handle<'cxn_borrow> = PoolConnectionHandle;
    type Error = anyhow::Error;

    async fn database_cxn(&mut self) -> Result<Self::Handle<'_>, Self::Error> {
        let handle = PoolConnectionHandle {
            active_connection: self.db.acquire().await?,
        };

        Ok(handle)
    }
}


impl<'tx> external_connections::Transactable<ExternalConnectionsInTransaction<'tx>>
    for ExternalConnectivity
{
    type Error = anyhow::Error;

    async fn start_transaction<'this>(&'this self) -> Result<ExternalConnectionsInTransaction<'tx>, Self::Error>
        where ExternalConnectionsInTransaction<'tx>: 'this {
        let transaction = self
            .db
            .begin()
            .await
            .context("Starting transaction from db pool")?;

        Ok(ExternalConnectionsInTransaction { txn: transaction })
    }
}

struct ExternalConnectionsInTransaction<'tx> {
    txn: Transaction<'tx, Postgres>,
}

struct TransactionHandle<'tx> {
    active_transaction: &'tx mut PgConnection,
}


impl<'tx> external_connections::ExternalConnectivity
    for ExternalConnectionsInTransaction<'tx>
{
    type Handle<'tx_borrow> = TransactionHandle<'tx_borrow> where Self: 'tx_borrow;
    type Error = anyhow::Error;

    async fn database_cxn(&mut self) -> Result<TransactionHandle<'_>, Self::Error> {
        let handle = self
            .txn
            .acquire()
            .await
            .context("acquiring connection from database transaction")?;

        Ok(TransactionHandle {
            active_transaction: handle,
        })
    }
}

impl<'tx> ConnectionHandle for TransactionHandle<'tx> {
    fn borrow_connection(&mut self) -> &mut PgConnection {
        &mut *self.active_transaction
    }
}


impl<'tx> external_connections::TransactionHandle for ExternalConnectionsInTransaction<'tx> {
    type Error = anyhow::Error;

    async fn commit(self) -> Result<(), Self::Error> {
        self.txn
            .commit()
            .await
            .context("Committing database transaction")?;

        Ok(())
    }
}

struct Count {
    count: Option<i64>,
}

impl Count {
    fn count(&self) -> i64 {
        self.count.expect("count() should always produce at least one row")
    }
}

struct NewId {
    id: i32,
}

fn anyhowify<T: Debug + Display>(errorish: T) -> anyhow::Error {
    anyhow!(format!("{}", errorish))
}
