use sqlx::PgConnection;

use derive_more::{Display, Error};
use std::fmt::Debug;

#[expect(dead_code)]
/// TransactableExternalConnectivity represents an [ExternalConnectivity] that can initiate
/// a database transaction
pub trait TransactableExternalConnectivity: ExternalConnectivity + Transactable + Sync {}

impl<T: ExternalConnectivity + Transactable + Sync> TransactableExternalConnectivity for T {}

/// ExternalConnectivity owns clients that are able to communicate with the outside world,
/// such as database clients, HTTP clients, and more.
pub trait ExternalConnectivity: Sync {
    type DbHandle<'handle>: ConnectionHandle + 'handle
    where
        Self: 'handle;

    /// Acquire a handle which allows borrowing a connection from the database pool
    async fn database_cxn(&mut self) -> Result<Self::DbHandle<'_>, anyhow::Error>;

    /// Acquire an HTTP client for making HTTP requests
    fn http_client(&self) -> &reqwest_middleware::ClientWithMiddleware;
}

/// ConnectionHandle is a handle borrowed from [ExternalConnectivity] which can be
/// used to acquire a connection to the database
pub trait ConnectionHandle {
    /// Borrow a connection from the database pool to perform a query
    fn borrow_connection(&mut self) -> &mut PgConnection;
}

/// Anything that can initiate a database transaction
pub trait Transactable: Sync {
    type Handle: TransactionHandle + ExternalConnectivity;

    #[cfg_attr(not(test), expect(dead_code))]
    /// Retrieve a handle which contains a database connection in an active transaction
    async fn start_transaction(&self) -> Result<Self::Handle, anyhow::Error>;
}

/// TransactionHandle is a handle borrowed from [Transactable] which represents
/// an in-flight database transaction that can later be committed. It is expected
/// that dropping the handle without invoking `TransactionHandle::commit` will
/// roll back the transaction
pub trait TransactionHandle: Sync {
    #[cfg_attr(not(test), expect(dead_code))]
    /// Commit the changes to the database
    async fn commit(self) -> Result<(), anyhow::Error>;
}

#[allow(dead_code)]
#[derive(Debug, Display, Error)]
/// This error reports issues that occur during database transactions, allowing the
/// original result of a [with_transaction]'s lambda to be retrieved even if the transaction
/// commit fails.
pub enum TxOrSourceError<SourceValue, SourceErr>
where
    SourceErr: Debug + Display,
{
    /// Represents that the lambda failed, returning the error from the lambda
    Source(SourceErr),

    #[display("Failed to start the transaction: {_0}")]
    /// Represents that the database failed to start the transaction, and the lambda did not execute.
    TxBegin(anyhow::Error),

    #[display("Got a successful result, but the database transaction failed: {transaction_err}")]
    /// Represents that the lambda executed successfully, but the database transaction failed to commit.
    /// The original result of the lambda is provided in this error.
    TxCommit {
        /// The success value returned from the lambda
        successful_result: SourceValue,
        /// The database error that occurred when the commit failed
        transaction_err: anyhow::Error,
    },
}

// TxAble = "The thing that can begin a transaction"
// Handle = "The thing that can give you a database connection"
// Ret = "The success type returned from the passed async function"
// Err = "The error type returned from the passed async function"
#[tracing::instrument(name = "DB Transaction", skip(tx_origin, transaction_context))]
/// Accepts [tx_origin] which can start a database transaction. It then starts a transaction and
/// invokes [transaction_context] with the started transaction. When [transaction_context] completes,
/// the transaction handle passed to it is committed as long as [transaction_context] does not return
/// a [Result::Err].
pub async fn with_transaction<TxAble, Handle, Ret, Err>(
    tx_origin: &TxAble,
    transaction_context: impl AsyncFnOnce(&mut Handle) -> Result<Ret, Err>,
) -> Result<Ret, TxOrSourceError<Ret, Err>>
where
    TxAble: Transactable<Handle = Handle>,
    Handle: TransactionHandle + ExternalConnectivity,
    Err: Debug + Display,
{
    let mut tx_handle = tx_origin
        .start_transaction()
        .await
        .map_err(|err| TxOrSourceError::TxBegin(err))?;
    let ret_val = transaction_context(&mut tx_handle).await;
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

    // I need this to help provide a size for the error in the async block used in the following test
    #[derive(Debug, Display, Error)]
    #[display("Abcde")]
    struct SampleErr;

    #[tokio::test]
    async fn commits_on_success() {
        let ext_cxn = test_util::FakeExternalConnectivity::new();
        let tx_result = with_transaction(&ext_cxn, async |_tx_cxn| {
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
        let tx_result = with_transaction(&ext_cxn, async |_tx_cxn| {
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
        ConnectionHandle, ExternalConnectivity, Transactable, TransactionHandle,
    };

    use sqlx::PgConnection;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// A fake for ExternalConnectivity so unit tests don't actually have to connect to external systems.
    /// Also allows inspection in tests to verify a database transaction was committed
    pub struct FakeExternalConnectivity {
        is_transacting: bool,
        downstream_transaction_committed: Arc<AtomicBool>,
    }

    impl FakeExternalConnectivity {
        /// Constructor for FakeExternalConnectivity
        pub fn new() -> Self {
            Self {
                is_transacting: false,
                downstream_transaction_committed: Arc::new(AtomicBool::new(false)),
            }
        }

        /// Returns true if a database transaction is active
        #[allow(dead_code)]
        pub fn is_transacting(&self) -> bool {
            self.is_transacting
        }

        /// Returns true if there was a database transaction which successfully committed
        pub fn did_transaction_commit(&self) -> bool {
            self.downstream_transaction_committed.load(Ordering::SeqCst)
        }
    }

    /// A fake database connection handle which panics if code tries to acquire
    /// a real database connection
    pub struct MockHandle {}

    impl ConnectionHandle for MockHandle {
        fn borrow_connection(&mut self) -> &mut PgConnection {
            panic!("You cannot acquire a real database connection in a test.")
        }
    }

    impl ExternalConnectivity for FakeExternalConnectivity {
        type DbHandle<'cxn> = MockHandle;

        #[allow(clippy::diverging_sub_expression)]
        async fn database_cxn(&mut self) -> Result<Self::DbHandle<'_>, anyhow::Error> {
            Ok(MockHandle {})
        }

        fn http_client(&self) -> &reqwest_middleware::ClientWithMiddleware {
            panic!("You cannot acquire a real HTTP connection in a test.");
        }
    }

    impl TransactionHandle for FakeExternalConnectivity {
        async fn commit(self) -> Result<(), anyhow::Error> {
            if !self.is_transacting {
                panic!("Tried to commit when we weren't in a transaction!")
            }

            self.downstream_transaction_committed
                .store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    impl Transactable for FakeExternalConnectivity {
        type Handle = FakeExternalConnectivity;

        async fn start_transaction(&self) -> Result<FakeExternalConnectivity, anyhow::Error> {
            Ok(FakeExternalConnectivity {
                is_transacting: true,
                downstream_transaction_committed: Arc::clone(
                    &self.downstream_transaction_committed,
                ),
            })
        }
    }
}
