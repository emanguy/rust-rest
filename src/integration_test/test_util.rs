use rand::{thread_rng, Rng};
use crate::db;
use sqlx::{PgPool, PgConnection, Connection};
use tokio::runtime::Runtime;
use std::{env, future::Future};
use lazy_static::lazy_static;
use dotenv::dotenv;

lazy_static! {
    static ref TOKIO_RT: Runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Tokio runtime failed to initialize");
}

struct TestDatabase {
    base_url: String,
    template_db_name: String,
}

impl TestDatabase {
    async fn create(base_url: &str) -> Result<Self, sqlx::Error> {
        let mut rng = thread_rng();
        let schema_id: u32 = rng.gen_range(10_000..99_999);
        let template_db_name = format!("test_db_{}", schema_id);
        let mut conn = PgConnection::connect(base_url).await?;

        sqlx::query("ALTER DATABASE postgres WITH is_template TRUE").execute(&mut conn).await?;
        sqlx::query(format!("CREATE DATABASE {} TEMPLATE postgres", template_db_name).as_str()).execute(&mut conn).await?;

        Ok(Self{ base_url: String::from(base_url), template_db_name})
    }

    fn template_db_name<'db>(&'db self) -> &'db str {
        self.template_db_name.as_str()
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        TOKIO_RT.block_on(async move {
            let connection = PgConnection::connect(&self.base_url).await;
            let mut connection = match connection {
                Ok(conn) => conn,
                Err(error) => { 
                    println!("Failed to remove test database {}, please remove it by hand. Connect error: {}", self.template_db_name, error);
                    return;
                },
            };

            let drop_result = sqlx::query(format!("DROP DATABASE {}", self.template_db_name).as_str()).execute(&mut connection).await;
            if let Err(error) = drop_result {
                println!("Failed to remove test database {}, please remove it by hand. Schema drop error: {}", self.template_db_name, error);
                return;
            }
        });
    }
}

/// Creates a temp database for a test by using the "postgres" default table's content as a template
/// when creating a new database.
/// 
/// Expects that the TEST_DB_URL environment variable is populated 
pub fn prepare_db_and_test<F, R>(test_fn: F)
where
    R: Future<Output = ()>,
    F: FnOnce(PgPool) -> R
{
    if dotenv().is_err() {
        println!("Test is running without .env file.");
    }

    // The drop trait implemented by TestDatabase creates a tokio runtime. Nested runtimes cause a panic,
    // so pulling test_db out of the async runtime so it can be dropped allows the previous runtime to be closed down
    // preventing a panic.
    let _external_drop = TOKIO_RT.block_on(async move {
        let pg_connection_base_url = env::var("TEST_DB_URL")
            .expect("You must provide the TEST_DB_URL environment variable as the base postgres connection string");
        let test_db = TestDatabase::create(&pg_connection_base_url).await;
        let test_db = match test_db {
            Ok(tdb) => tdb,
            Err(db_err) => panic!("Failed to start test database: {}", db_err),
        };

        let sqlx_pool = db::connect_sqlx(format!("{}/{}", pg_connection_base_url, test_db.template_db_name()).as_str()).await;
        let _ = test_fn(sqlx_pool.clone()).await;
        // We need to make sure the pool's connections are closed so we can drop the temp DB
        sqlx_pool.close().await;

        test_db
    });
}
