use async_trait::async_trait;
use sqlx::{PgExecutor, Postgres};

#[async_trait]
pub trait ExternalConnectivity {
    fn database_cxn(&self) -> &dyn PgExecutor<Database=Postgres>;
    async fn with_transaction(&self) -> &Self;
}

#[cfg(test)]
pub mod test_util {

}