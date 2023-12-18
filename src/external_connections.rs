use async_trait::async_trait;
use sqlx::{PgExecutor, Postgres};

#[async_trait]
pub trait ExternalConnectivity {
    fn database_cxn(&self) -> &dyn PgExecutor<Database=Postgres>;
    async fn with_transaction(&self) -> Self;
}

#[cfg(test)]
pub mod test_util {
    use std::sync::Arc;
    use async_trait::async_trait;
    use mockall::mock;
    use sqlx::{Executor, PgExecutor, Postgres};
    use crate::external_connections::ExternalConnectivity;

    mock! {
        #[derive(Debug)]
        DbConn {}

        impl<'c> Executor<'c, Database=Postgres> for DbConn {}
    }

    struct FakeExternalConnectivity {
        db_connection: Arc<MockDbConn>,
        is_transacting: bool,
    }

    impl FakeExternalConnectivity {
        fn new() -> Self {
            Self {
                db_connection: Arc::new(MockDbConn::new()),
                is_transacting: false,
            }
        }

        fn is_transacting(&self) -> bool {
            self.is_transacting
        }
    }

    #[async_trait]
    impl ExternalConnectivity for FakeExternalConnectivity {
        fn database_cxn(&self) -> &dyn PgExecutor<Database=Postgres> {
           &self.db_connection
        }

        async fn with_transaction(&self) -> Self {
            Self {
                db_connection: Arc::clone(&self.db_connection),
                is_transacting: true,
            }
        }
    }
}