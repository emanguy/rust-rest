use async_trait::async_trait;
use sqlx::PgConnection;
use std::error::Error;
use std::future::Future;
use thiserror::Error;


#[async_trait]
pub trait ExternalConnectivity {
    async fn database_cxn(&self) -> &mut PgConnection;
}

#[async_trait]
pub trait Transactable<Handle: TransactionHandle> {
    async fn start_transaction(&self) -> Handle;
}

#[async_trait]
pub trait TransactionHandle {
    type Error: Error;

    async fn commit(self) -> Result<(), Self::Error>;
}

#[derive(Debug, Error)]
pub enum TxOrSourceError<SourceValue, SourceErr: Error, TxErr: Error> {
    #[error(transparent)]
    Source(SourceErr),
    #[error("Got a successful result, but the database transaction failed: {transaction_err}")]
    TxCommit {
        successful_result: SourceValue,
        transaction_err: TxErr,
    },
}

async fn with_transaction<Handle, TxAble, Fut, Fn, Ret, Err>(
    tx_origin: &TxAble,
    transaction_context: Fn,
) -> Result<Ret, TxOrSourceError<Ret, Err, Handle::Error>>
where
    Handle: TransactionHandle,
    TxAble: Transactable<Handle>,
    Err: Error,
    Fut: Future<Output = Result<Ret, Err>>,
    Fn: FnOnce(&Handle) -> Fut,
{
    let tx_handle = tx_origin.start_transaction().await;
    let ret_val = transaction_context(&tx_handle).await;
    if ret_val.is_ok() {
        let commit_result = tx_handle.commit().await;
        if let Err(commit_err) = commit_result {
            return Err(TxOrSourceError::TxCommit {
                successful_result: ret_val.unwrap(),
                transaction_err: commit_err,
            });
        }
    }

    match ret_val {
        Ok(value) => Ok(value),
        Err(error) => Err(TxOrSourceError::Source(error)),
    }
}

#[cfg(test)]
pub mod test_util {
    use std::convert::Infallible;
    use crate::external_connections::{ExternalConnectivity, Transactable, TransactionHandle, with_transaction};
    use async_trait::async_trait;
    use mockall::mock;
    use sqlx::{Acquire, Database, Error, Executor, PgConnection, PgExecutor, Postgres, Transaction};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use futures_core;

    mock!{
        DbConn {}

        impl <'c> Acquire<'c> for DbConn {
            type Database = Postgres;
            type Connection = &'c mut <Postgres as Database>::Connection;

            fn acquire(self) -> futures_core::future::BoxFuture<'c, Result<Self::Connection, Error>>;

            fn begin(self) -> futures_core::future::BoxFuture<'c, Result<Transaction<'c, Self::Database>, Error>>;
        }
    }

    struct FakeExternalConnectivity {
        db_connection: Arc<MockDbConn>,
        is_transacting: bool,
        downstream_transaction_committed: Arc<AtomicBool>,
    }

    impl FakeExternalConnectivity {
        fn new() -> Self {
            Self {
                db_connection: Arc::new(MockDbConn::new()),
                is_transacting: false,
                downstream_transaction_committed: Arc::new(AtomicBool::new(false)),
            }
        }

        fn is_transacting(&self) -> bool {
            self.is_transacting
        }

        fn did_transaction_commit(&self) -> bool {
            self.downstream_transaction_committed.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl ExternalConnectivity for FakeExternalConnectivity {
        async fn database_cxn(&self) -> &mut PgConnection {
            self.db_connection.acquire().await.unwrap()
        }
    }

    #[async_trait]
    impl TransactionHandle for FakeExternalConnectivity {
        type Error = Infallible;

        async fn commit(self) -> Result<(), Self::Error> {
            if !self.is_transacting {
                panic!("Tried to commit when we weren't in a transaction!")
            }

            self.downstream_transaction_committed
                .store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    #[async_trait]
    impl Transactable<FakeExternalConnectivity> for FakeExternalConnectivity {
        async fn start_transaction(&self) -> FakeExternalConnectivity {
            FakeExternalConnectivity {
                db_connection: Arc::clone(&self.db_connection),
                is_transacting: true,
                downstream_transaction_committed: Arc::clone(
                    &self.downstream_transaction_committed,
                ),
            }
        }
    }

    #[tokio::test]
    async fn with_transaction_commits() {
        let ext_cxn = FakeExternalConnectivity::new();
        let tx_result = with_transaction(&ext_cxn, |tx_cxn| {
            println!("Woohoo!");
            Ok(())
        }).await;

        assert!(tx_result.is_ok());
        assert!(ext_cxn.did_transaction_commit());
    }
}
