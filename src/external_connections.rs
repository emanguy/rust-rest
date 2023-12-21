use async_trait::async_trait;
use sqlx::PgConnection;
use std::error::Error;
use std::future::Future;
use thiserror::Error;

#[async_trait]
pub trait ExternalConnectivity: Sync {
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
mod with_transaction_test {
    use super::*;
    use speculoos::prelude::*;
    use thiserror::Error;

    // I need this to help provide a size for the error in the async block used in the following test
    #[derive(Debug, Error)]
    #[error("Abcde")]
    struct SampleErr;

    #[tokio::test]
    async fn commits_on_success() {
        let ext_cxn = test_util::FakeExternalConnectivity::new();
        let tx_result = with_transaction(&ext_cxn, |_tx_cxn| async {
            println!("Woohoo!");
            Ok::<(), SampleErr>(())
        })
        .await;

        assert_that!(tx_result).is_ok();
        assert_that!(ext_cxn.did_transaction_commit()).is_true();
    }

    #[tokio::test]
    async fn does_not_commit_on_failure() {
        let ext_cxn = test_util::FakeExternalConnectivity::new();
        let tx_result = with_transaction(&ext_cxn, |_tx_cxn| async {
            println!("Whoopsie!");
            Err::<(), SampleErr>(SampleErr)
        })
        .await;

        assert_that!(tx_result)
            .is_err()
            .matches(|inner_err| matches!(inner_err, TxOrSourceError::Source(SampleErr)));
        assert_that!(ext_cxn.did_transaction_commit()).is_false();
    }
}

#[cfg(test)]
pub mod test_util {
    use crate::external_connections::{
        with_transaction, ExternalConnectivity, Transactable, TransactionHandle,
    };
    use async_trait::async_trait;
    use futures_core;
    use sqlx::{
        Acquire, ConnectOptions, Connection, Database, Error, Executor, PgConnection, PgExecutor,
        Postgres, Transaction,
    };
    use std::convert::Infallible;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use thiserror::Error;

    pub struct FakeExternalConnectivity {
        is_transacting: bool,
        downstream_transaction_committed: Arc<AtomicBool>,
    }

    impl FakeExternalConnectivity {
        pub fn new() -> Self {
            Self {
                is_transacting: false,
                downstream_transaction_committed: Arc::new(AtomicBool::new(false)),
            }
        }

        pub fn is_transacting(&self) -> bool {
            self.is_transacting
        }

        pub fn did_transaction_commit(&self) -> bool {
            self.downstream_transaction_committed.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl ExternalConnectivity for FakeExternalConnectivity {
        #[allow(clippy::diverging_sub_expression)]
        async fn database_cxn(&self) -> &mut PgConnection {
            panic!("You cannot actually connect to the database during a test.");
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
                is_transacting: true,
                downstream_transaction_committed: Arc::clone(
                    &self.downstream_transaction_committed,
                ),
            }
        }
    }
}
