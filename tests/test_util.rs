use rand::{thread_rng, Rng};
use sample_rest::db;
use sqlx::{PgPool, PgConnection, Connection, postgres::PgPoolOptions};
use tokio::runtime::Runtime;
use std::{env, future::Future, panic, pin::Pin};
use lazy_static::lazy_static;

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

        Ok(Self{ base_url: String::from(base_url), template_db_name })
    }

    fn template_db_name<'db>(&'db self) -> &'db str {
        self.template_db_name.as_str()
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        let db_to_drop = self.template_db_name.clone();
        let conn_str = self.base_url.clone();
        
        TOKIO_RT.block_on(async move {
            let conn = PgConnection::connect(conn_str.as_str()).await;
            let mut conn = match conn {
                Ok(cxn) => cxn,
                Err(conn_err) => {
                    println!("Failed to reconnect to database to drop test database {}, please remove it manually. Error: {}", db_to_drop, conn_err);
                    return;
                }
            };

            let drop_result = sqlx::query(format!("DROP DATABASE {}", db_to_drop).as_str()).execute(&mut conn).await;
            match drop_result {
                Err(db_err) => println!("Failed to drop test database {}, please remove it manually. Error: {}", db_to_drop, db_err),
                _ => {}
            };
        });
    }
}

/// Creates a temp database for a test by using the "postgres" default table's content as a template
/// when creating a new database.
/// 
/// Expects that the TEST_DB_URL environment variable is populated 
fn prepare_db_and_test<F>(test_fn: F)
where
    F: FnOnce(PgPool) -> Pin<Box<dyn Future<Output = ()>>>,
{
    TOKIO_RT.block_on(async move {
        let pg_connection_base_url = env::var("TEST_DB_URL")
            .expect("You must provide the TEST_DB_URL environment variable as the base postgres connection string");
        let test_db = TestDatabase::create(&pg_connection_base_url).await;
        let test_db = match test_db {
            Ok(tdb) => tdb,
            Err(db_err) => panic!("Failed to start test database: {}", db_err),
        };

        let sqlx_pool = db::connect_sqlx(format!("{}/{}", pg_connection_base_url, test_db.template_db_name()).as_str()).await;
        test_fn(sqlx_pool).await;
    });
}
