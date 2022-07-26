use crate::db;
use dotenv::dotenv;
use lazy_static::lazy_static;
use rand::{thread_rng, Rng};
use sqlx::{Connection, PgConnection, PgPool, Row, postgres::PgRow, PgExecutor};
use std::{env, future::Future, iter::Map};
use tokio::runtime::Runtime;

lazy_static! {
    static ref TOKIO_RT: Runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Tokio runtime failed to initialize");
}

struct TestDatabase {
    template_db_name: String,
}

impl TestDatabase {
    async fn clear_old_dbs(&self, mut conn: PgConnection) {
        let test_dbs = sqlx::query("SELECT datname FROM pg_catalog.pg_database WHERE datname LIKE 'test_db%'")
            .fetch_all(&mut conn)
            .await;
        let test_dbs = match test_dbs {
            Ok(results) => results.into_iter().map(|row| row.get::<String, _>(0)),
            Err(error) => {
                println!("Warning: failed to drop old test databases. You may );need to delete them manually. Error: {error}");
                conn.close();
                return;
            }
        };
            

        for db in test_dbs {
            let result = sqlx::query(format!("DROP DATABASE {}", db).as_str()).execute(&mut conn).await;
            if result.is_err() {
                println!("Warning: failed to drop old test database {}, you may need to do it manually.", db);
            }
        }
        conn.close();
    }

    async fn create(mut conn: PgConnection) -> Result<Self, sqlx::Error> {
        let mut rng = thread_rng();
        let schema_id: u32 = rng.gen_range(10_000..99_999);
        let template_db_name = format!("test_db_{}", schema_id);

        let result = sqlx::query("ALTER DATABASE postgres WITH is_template TRUE")
            .execute(&mut conn)
            .await;
        if let Err(error) = result {
            conn.close();
            return Err(error);
        }
        let result = sqlx::query(format!("CREATE DATABASE {} TEMPLATE postgres", template_db_name).as_str())
            .execute( &mut conn)
            .await;
        conn.close();
        result?;

        Ok(Self {
            template_db_name,
        })
    }

    fn template_db_name<'db>(&'db self) -> &'db str {
        self.template_db_name.as_str()
    }
}

/// Creates a temp database for a test by using the "postgres" default table's content as a template
/// when creating a new database.
///
/// Expects that the TEST_DB_URL environment variable is populated
pub fn prepare_db_and_test<F, R>(test_fn: F)
where
    R: Future<Output = ()>,
    F: FnOnce(PgPool) -> R,
{
    if dotenv().is_err() {
        println!("Test is running without .env file.");
    }

    TOKIO_RT.block_on(async move {
        let pg_connection_base_url = env::var("TEST_DB_URL")
            .expect("You must provide the TEST_DB_URL environment variable as the base postgres connection string");
        let test_db = {
            let initial_conn = PgConnection::connect(&pg_connection_base_url).await;
            if initial_conn.is_err() {
                panic!("Test failure - could not create initial connection to provision database.");
            }
            let test_db = TestDatabase::create(initial_conn).await;
            let test_db = match test_db {
                Ok(tdb) => tdb,
                Err(db_err) => panic!("Failed to start test database: {}", db_err),
            };
            test_db.clear_old_dbs(&initial_conn).await;

            test_db
        };
        
        let sqlx_pool = db::connect_sqlx(format!("{}/{}", pg_connection_base_url, test_db.template_db_name()).as_str()).await;
        test_fn(sqlx_pool.clone()).await;
    });
}
