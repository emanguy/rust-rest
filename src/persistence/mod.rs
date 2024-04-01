pub mod db_todo_driven_ports;
pub mod db_user_driven_ports;

use crate::external_connections;
use crate::external_connections::ConnectionHandle;
use anyhow::{anyhow, Context};
use std::fmt::{Debug, Display};

use sqlx::pool::PoolConnection;
use sqlx::{Acquire, PgConnection, PgPool, Postgres, Transaction};

/// Data structure which owns clients for connecting to external systems.
/// Allows business logic to be agnostic of the external systems it communicates with
/// so driven adapters can easily be swapped out for other implementations
#[derive(Clone)]
pub struct ExternalConnectivity {
    db: PgPool,
}

impl ExternalConnectivity {
    /// Accepts the set of clients used to connect to external systems and constructs
    /// an instance of ExternalConnectivity owning those clients
    pub fn new(db: PgPool) -> Self {
        ExternalConnectivity { db }
    }
}

/// A handle from ExternalConnectivity which can connect to a database
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

impl external_connections::Transactable for ExternalConnectivity {
    type Handle<'handle> = ExternalConnectionsInTransaction<'handle>;
    type Error = anyhow::Error;

    async fn start_transaction(&self) -> Result<Self::Handle<'_>, Self::Error> {
        let transaction = self
            .db
            .begin()
            .await
            .context("Starting transaction from db pool")?;

        Ok(ExternalConnectionsInTransaction { txn: transaction })
    }
}

/// A variant of ExternalConnectivity where the database client has an active database transaction
/// which can later be committed
pub struct ExternalConnectionsInTransaction<'tx> {
    txn: Transaction<'tx, Postgres>,
}

/// A handle from ExternalConnectionsInTransaction which can connect to a database
pub struct TransactionHandle<'tx> {
    active_transaction: &'tx mut PgConnection,
}

impl<'tx> external_connections::ExternalConnectivity for ExternalConnectionsInTransaction<'tx> {
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

/// Utility DTO for consuming the output of the PostgreSQL `count()` function
struct Count {
    count: Option<i64>,
}

impl Count {
    /// Retrieve the count value, as it's typechecked to be optional but should always be present
    fn count(&self) -> i64 {
        self.count
            .expect("count() should always produce at least one row")
    }
}

/// Utility DTO for retrieving the ID of a newly inserted record to PostgreSQL
struct NewId {
    id: i32,
}

/// Converts anything implementing Debug and Display into an [anyhow::Error]
fn anyhowify<T: Debug + Display>(errorish: T) -> anyhow::Error {
    anyhow!(format!("{}", errorish))
}
